use std::{
    sync::{Arc, Condvar, Mutex},
    thread,
};

use futures::{
    channel::{
        mpsc::{self, UnboundedSender},
        oneshot::{self, Receiver},
    },
    StreamExt,
};
use ringbuf::{
    traits::{Consumer, Producer, Split},
    HeapRb,
};
use rodio::{
    cpal::{
        self,
        traits::{HostTrait, StreamTrait},
        BufferSize, SampleRate, Stream, StreamConfig,
    },
    Device, DeviceTrait,
};
use serde::{Deserialize, Serialize};
use tauri::{plugin, AppHandle, Manager, Runtime, State};
use tokio::select;
use vad::{NSamples, Vad, VadStatus, VAD_FRAME};
use whisper::{Whisper, WhisperSetupError, MAX_WHISPER_FRAME, SAMPLE_RATE};

mod vad;
mod whisper;

#[derive(Debug, Serialize)]
pub enum WhisperError {
    AlreadyRunning,
    InputDeviceUnavailable(String),
    AudioStreamError(String),
    /// a config without fixed buffer size has been used
    VadSetupError,
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
}

enum VadActivity {
    SpeechStart,
    SpeechEnd(NSamples),
}

pub fn init<R: Runtime>() -> plugin::TauriPlugin<R> {
    plugin::Builder::new("whisper_stt")
        .invoke_handler(tauri::generate_handler![start, stop])
        .setup(|app| {
            app.manage(WhisperState::default());
            Ok(())
        })
        .build()
}

#[tauri::command]
pub async fn start<R: Runtime>(app: AppHandle<R>, args: WhisperArgs) -> Result<(), WhisperError> {
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
    let mut whisper = Whisper::new(args.model_path)?;
    let (mut producer, mut consumer) = ring.split();
    let (mut activity_tx, mut activity_rx) = mpsc::unbounded::<VadActivity>();

    let (mut err_tx, mut err_rx) = mpsc::channel(1);
    // cancellation using the condvar pattern https://doc.rust-lang.org/std/sync/struct.Condvar.html
    let cancel_pair = Arc::new((Mutex::new(false), Condvar::new()));
    let cancellation_pair = cancel_pair.clone();
    let (device, config) = get_microphone_by_name(&args.input_device)?;
    thread::spawn(move || {
        let mut start_err = err_tx.clone();
        let mut vad = match Vad::try_new(&config) {
            Ok(vad) => vad,
            Err(_) => {
                let _ = start_err.try_send(WhisperError::VadSetupError);
                return;
            }
        };
        let result = || -> Result<Stream, WhisperError> {
            let stream = device
                .build_input_stream(
                    &config,
                    move |data: &[i16], _info| {
                        audio_loop(data, &mut producer, &mut vad, &mut activity_tx);
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

    let handle_stream = async {
        loop {
            select! {
                event = activity_rx.next() => {
                    match event {
                        Some(VadActivity::SpeechStart) => {
                            if app.emit_all("whisper_stt_interim", "[speaking]").is_err() {
                                eprintln!("wasn't able to emit to frontend {}:{}", file!(), line!());
                            }
                        },
                        Some(VadActivity::SpeechEnd(samples)) => {
                            if consumer.pop_slice(whisper.audio_buf(samples)) != samples {
                                return Err(WhisperError::AudioStreamError("logic error: not enough samples could be fetched".into()));
                            }
                            if let Some(final_text) = whisper.transcribe() {
                                if app.emit_all("whisper_stt_final", final_text).is_err() {
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
    match err_rx.next().await {
        Some(err) => return Err(err),
        None => (),
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

fn get_microphone_by_name(name: &str) -> Result<(Device, StreamConfig), WhisperError> {
    let host = cpal::default_host();
    let mut devices = host.input_devices().unwrap();
    if let Some(device) = devices.find(|device| device.name().unwrap() == name) {
        let config = device
            .supported_input_configs()
            .map_err(|err| WhisperError::InputDeviceUnavailable(format!("{name}: '{err}'")))?
            .next()
            .ok_or_else(|| WhisperError::InputDeviceUnavailable(format!("{name}: 'does not have any valid input configurations'")))?
            .try_with_sample_rate(SampleRate(SAMPLE_RATE as u32))
            .ok_or_else(|| WhisperError::InputDeviceUnavailable("16kHz sample rate unavailable".into()))?;
        let buffer_size = match config.buffer_size() {
            cpal::SupportedBufferSize::Range { min, max } => (VAD_FRAME as u32).max(*min).min(*max),
            cpal::SupportedBufferSize::Unknown => VAD_FRAME as u32,
        };
        let buffer_size = BufferSize::Fixed(buffer_size);
        let sample_rate = config.sample_rate();
        let channels = config.channels();
        let config = StreamConfig {
            channels,
            sample_rate,
            buffer_size,
        };
        Ok((device, config))
    } else {
        Err(WhisperError::InputDeviceUnavailable(name.into()))
    }
}

fn audio_loop(data: &[i16], ring_buffer: &mut impl Producer<Item = i16>, vad: &mut Vad, activity: &mut UnboundedSender<VadActivity>) {
    vad.input(data);
    loop {
        let status = vad.output_to(ring_buffer);
        match status {
            VadStatus::Silence => (),
            VadStatus::Speech => (),
            VadStatus::SpeechEnd(samples) => {
                // can safely drop the error case here as it only happens when the receiver has hung up (which means the stream is bound to stop soon too)
                let _ = activity.unbounded_send(VadActivity::SpeechEnd(samples));
                continue; // make sure we run this input to completion
            }
            VadStatus::SpeechStart => {
                // can safely drop the error case here as it only happens when the receiver has hung up (which means the stream is bound to stop soon too)
                let _ = activity.unbounded_send(VadActivity::SpeechStart);
                continue; // make sure we run this input to completion
            }
        }
        break;
    }
}
