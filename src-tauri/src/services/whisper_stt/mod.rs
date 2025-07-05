use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use futures::StreamExt;
use futures::channel::mpsc::{self};
use futures::channel::oneshot::{self, Receiver};
use ringbuf::HeapRb;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use rodio::DeviceTrait;
use rodio::cpal::Stream;
use rodio::cpal::traits::StreamTrait;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, plugin};
use thiserror::Error;
use tokio::select;
use tracing::{error, trace_span, warn};
use vad::{
    InputDeviceError,
    ResamplingVad,
    ResamplingVadSetupError,
    VadActivity,
    audio_loop,
    get_microphone_by_name,
};
use whisper::{MAX_WHISPER_FRAME, Whisper, WhisperOptions, WhisperSetupError};

mod vad;
mod whisper;

#[derive(Error, Debug)]
pub enum WhisperError {
    #[error("there is already a running whisper instance")]
    AlreadyRunning,
    #[error("error setting up the audio device: '{0}'")]
    AudioSetupError(#[from] InputDeviceError),
    #[error("error in audio stream: '{0}'")]
    AudioStreamError(String),
    #[error("error setting up whisper instance: '{0}'")]
    WhisperSetupError(#[from] WhisperSetupError),
    #[error("error setting up resampling+vad pipeline: '{0}'")]
    ResamplingVadSetupError(#[from] ResamplingVadSetupError),
}

impl Serialize for WhisperError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Default)]
pub struct WhisperState {
    stop: Mutex<Option<Receiver<()>>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperArgs {
    model_path: String,
    input_device: String,
    lang: String,
    translate_to_english: bool,
    silence_interval: u64,
}

pub fn init<R: Runtime>() -> plugin::TauriPlugin<R> {
    plugin::Builder::new("whisper-stt")
        .invoke_handler(tauri::generate_handler![start, stop])
        .setup(|app, _api| {
            app.manage(WhisperState::default());
            Ok(())
        })
        .build()
}

#[tauri::command]
pub async fn start<R: Runtime>(app: AppHandle<R>, args: WhisperArgs) -> Result<(), WhisperError> {
    let whisper_opt = WhisperOptions {
        translate_en: args.translate_to_english,
        language: args.lang,
    };

    let state = app.state::<WhisperState>();
    let mut stop = {
        let mut stop = state.stop.lock().expect("should be able to lock mutex");
        if stop.is_some() {
            Err(WhisperError::AlreadyRunning)?;
        }
        let (tx, rx) = oneshot::channel::<()>();
        *stop = Some(rx);
        tx
    };
    let ring = HeapRb::<i16>::try_new(MAX_WHISPER_FRAME * 2).expect("cannot allocate audio ring");
    let mut whisper = Whisper::with_options(args.model_path, whisper_opt)?;
    let (mut producer, mut consumer) = ring.split();
    let (mut activity_tx, mut activity_rx) = mpsc::unbounded::<VadActivity>();

    let (mut err_tx, mut err_rx) = mpsc::channel(1);
    // cancellation using the condvar pattern https://doc.rust-lang.org/std/sync/struct.Condvar.html
    let cancel_pair = Arc::new((Mutex::new(false), Condvar::new()));
    let cancellation_pair = cancel_pair.clone();

    let (device, config) =
        get_microphone_by_name(&args.input_device).map_err(WhisperError::AudioSetupError)?;

    let (mut audio_prod, mut vad) = ResamplingVad::with_silence_interval(
        &config,
        Some(Duration::from_millis(args.silence_interval)),
    )?;

    // audio processing thread
    thread::spawn(move || {
        let requested_frames_pair = Arc::new((Mutex::new(vad.missing_frames()), Condvar::new()));
        let stream_requested_frames_pair = requested_frames_pair.clone();

        // audio fetching thread
        // start this thread after setup was successful to reduce cleanup work
        let audio_fetcher = thread::spawn(move || {
            let mut start_err = err_tx.clone();
            let audio_loop = trace_span!("audio_loop", samplerate = config.sample_rate.0);
            let error_span = audio_loop.clone();
            let result = || -> Result<Stream, WhisperError> {
                let stream = device
                    .build_input_stream(
                        &config,
                        move |data: &[f32], _info| {
                            let _span = audio_loop.enter();
                            let written = audio_prod.push_slice(data);
                            let diff = data.len() - written;

                            if diff != 0 {
                                warn!("cannot keep up, dropping {diff} audio frames",);
                            }

                            // track how many frames we have written and notify audio thread if it
                            // has what it wanted
                            let (lock, cvar) = &*stream_requested_frames_pair;
                            let requested = lock.lock().unwrap();
                            if *requested <= audio_prod.occupied_len() {
                                cvar.notify_all(); // tell audio processor it can proceed
                            }
                        },
                        move |err| {
                            // only try send because we might have had an earlier error
                            let _span = error_span.enter();
                            error!("fatal: '{err}'");
                            let _ =
                                err_tx.try_send(WhisperError::AudioStreamError(err.to_string()));
                        },
                        None,
                    )
                    .map_err(|err| WhisperError::AudioStreamError(err.to_string()))?;
                stream
                    .play()
                    .map_err(|err| WhisperError::AudioStreamError(err.to_string()))?;
                Ok(stream)
            };
            let stream = match result() {
                Ok(v) => v,
                Err(err) => {
                    // only try send because we might have had an earlier error
                    let _ = start_err.try_send(err);
                    return;
                }
            };
            let (lock, cvar) = &*cancellation_pair;
            let mut cancelled = lock.lock().expect("should be able to lock cancellation");
            while !*cancelled {
                // wait until we get cancelled
                cancelled = cvar
                    .wait(cancelled)
                    .expect("should be able to wait for cancellation");
            }
            if let Err(err) = stream.pause() {
                // only try send because we might have had an earlier error
                let _ = start_err.try_send(WhisperError::AudioStreamError(err.to_string()));
            }
        });

        let (lock, cvar) = &*requested_frames_pair;
        let mut request = lock.lock().unwrap();
        while !audio_fetcher.is_finished() {
            // exit if we don't get more audio
            request = cvar.wait(request).unwrap();
            // handles spurious wake ups well so we don't need to check anything on the
            // condvar
            audio_loop(&mut producer, &mut vad, &mut activity_tx);
            *request = vad.input_frames_next();
        }
    });

    let handle_stream = async {
        loop {
            select! {
                event = activity_rx.next() => {
                    match event {
                        Some(VadActivity::SpeechStart) => {
                            if app.emit("whisper_stt_interim", "[speaking]").is_err() {
                                eprintln!("wasn't able to emit to frontend {}:{}", file!(), line!());
                            }
                        },
                        Some(VadActivity::SpeechEnd(samples)) => {
                            if consumer.pop_slice(whisper.audio_buf(samples)) != samples {
                                return Err(WhisperError::AudioStreamError("logic error: not enough samples could be fetched".into()));
                            }
                            if let Some(final_text) = whisper.transcribe() {
                                if app.emit("whisper_stt_final", final_text).is_err() {
                                    eprintln!("wasn't able to emit to frontend {}:{}", file!(), line!());
                                }
                            }
                        },
                        None => return Err(WhisperError::AudioStreamError("closed unexpectedly".into())),
                    }
                },
                _ = stop.cancellation() => {
                    return Ok(());
                },
                err = err_rx.next() => {
                    return Err(err.unwrap_or_else(|| WhisperError::AudioStreamError("closed unexpectedly".into())));
                }
            }
        }
    };
    let result = handle_stream.await;
    {
        // put in a child scope so lock gets dropped immediately
        let (lock, cvar) = &*cancel_pair;
        let mut cancel = lock.lock().expect("should be able to lock");
        *cancel = true;
        cvar.notify_all();
    }
    result?;
    if let Some(err) = err_rx.next().await {
        return Err(err);
    }
    Ok(())
}

#[tauri::command]
pub fn stop(state: State<'_, WhisperState>) {
    state
        .stop
        .lock()
        .expect("should be able to obtain a lock")
        .take();
}
