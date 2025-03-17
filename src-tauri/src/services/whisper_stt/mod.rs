use std::{
    sync::{Arc, Condvar, Mutex},
    thread,
};

use futures::{
    channel::{
        mpsc::{self},
        oneshot::{self, Receiver},
    },
    StreamExt,
};
use ringbuf::{
    traits::{Consumer, Split},
    HeapRb,
};
use rodio::{
    cpal::{traits::StreamTrait, Stream},
    DeviceTrait,
};
use serde::{Deserialize, Serialize};
use tauri::{plugin, AppHandle, Emitter, Manager, Runtime, State};
use tokio::select;
use vad::{audio_loop, get_microphone_by_name, get_resampler, AudioError, Vad, VadActivity};
use whisper::{Whisper, WhisperOptions, WhisperSetupError, MAX_WHISPER_FRAME};

mod vad;
mod whisper;

#[derive(Debug, Serialize)]
pub enum WhisperError {
    AlreadyRunning,
    AudioSetupError(AudioError),
    AudioStreamError(String),
    WhisperSetupError(WhisperSetupError),
}
impl From<WhisperSetupError> for WhisperError {
    fn from(value: WhisperSetupError) -> Self {
        WhisperError::WhisperSetupError(value)
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
            return Err(WhisperError::AlreadyRunning);
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

    let (device, config) = get_microphone_by_name(&args.input_device).map_err(WhisperError::AudioSetupError)?;
    let mut vad = Vad::new(&config);
    let (audio_tx, audio_rx) = std::sync::mpsc::sync_channel(10);
    // audio processing thread
    thread::spawn(move || {
        let mut start_err = err_tx.clone();
        let channels = match config.channels {
            n @ 1..=2 => {
                eprintln!("running with {n} channels");
                n
            }
            invalid => {
                panic!("configs with {invalid} channels are not supported");
            }
        };
        // we need to build the resampler in here because it cannot be send across threads
        let resample_with = match get_resampler(config.sample_rate.0) {
            Ok(resampler) => resampler,
            Err(err) => {
                let _ = start_err.try_send(WhisperError::AudioSetupError(err));
                return;
            }
        };

        // audio fetching thread
        // start this thread after setup was successfull to reduce cleanup work
        thread::spawn(move || {
            let mut start_err = err_tx.clone();
            let result = || -> Result<Stream, WhisperError> {
                let stream = device
                    .build_input_stream(
                        &config,
                        move |data: &[f32], _info| {
                            if audio_tx.try_send(data.to_vec()).is_err() {
                                eprintln!("audio is being dropped");
                            }
                        },
                        move |err| {
                            // only try send because we might have had an earlier error
                            let _ = err_tx.try_send(WhisperError::AudioStreamError(err.to_string()));
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

        while let Ok(data) = audio_rx.recv() {
            audio_loop(&data, channels, &resample_with, &mut producer, &mut vad, &mut activity_tx);
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
        // put in a child scope so lock gets dropped immediatly
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
