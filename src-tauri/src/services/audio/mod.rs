use std::hint::black_box;
use std::io::Cursor;
use std::sync::mpsc::Receiver;
use std::thread;

use anyhow::{anyhow, bail};
use rodio::cpal::traits::HostTrait;
use rodio::cpal::{self};
use rodio::{Decoder, DeviceTrait, OutputStream, OutputStreamHandle, Sink};
use serde::{Deserialize, Serialize};
use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Runtime, command};
use tracing::trace;

fn get_output_stream(device_name: &str) -> Option<(OutputStream, OutputStreamHandle)> {
    let host = cpal::default_host();
    let mut devices = host.output_devices().unwrap();
    if let Some(device) = devices.find(|device| device.name().unwrap() == device_name) {
        OutputStream::try_from_device(&device).ok()
    } else {
        None
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RpcAudioPlayAsync {
    pub device_name: String,
    pub data: Vec<u8>,
    pub volume: f32, // 1 - base
    pub rate: f32,   // 1 - base
}

pub struct IndependentSink {
    pub inner: Sink,
    _drop_guard: Receiver<anyhow::Result<Sink>>,
}

pub fn get_independent_sink(device_name: &str) -> anyhow::Result<IndependentSink> {
    let device_name = device_name.to_string();
    let (tx, rx) = std::sync::mpsc::sync_channel(0);
    thread::Builder::new()
        .name("audio".to_string())
        .spawn(move || {
            let Some((_stream, stream_handle)) = get_output_stream(&device_name) else {
                _ = tx.send(Err(anyhow!(
                    "could not create audio stream for device: '{device_name}'"
                )));
                return;
            };

            let sink = Sink::try_new(&stream_handle).unwrap();
            sink.set_volume(1.);
            sink.set_speed(1.);
            _ = tx.send(Ok(sink));

            // try to send, since this is a rendezvous channel this will not complete and
            // error once the receiver is dropped
            if !tx.send(Err(anyhow!("will never be read"))).is_err() {
                panic!("logic error");
            }
            // 'use' stream down here so that it is not ever dropped prior
            black_box(_stream);
            trace!("independent audio sink closed");
        })?;

    let sink = match rx.recv() {
        Ok(Ok(sink)) => sink,
        Ok(Err(err)) => return Err(err),
        Err(_err) => bail!("unknown error during audio stream allocation"),
    };
    Ok(IndependentSink {
        inner: sink,
        _drop_guard: rx,
    })
}

#[command]
pub async fn play_async(data: RpcAudioPlayAsync) -> Result<(), String> {
    if let Some((_stream, stream_handle)) = get_output_stream(data.device_name.as_str()) {
        // let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();
        sink.set_volume(data.volume);
        sink.set_speed(data.rate);
        match Decoder::new(Cursor::new(data.data)) {
            Ok(source) => {
                sink.append(source);
                sink.sleep_until_end();
                Ok(())
            }
            Err(err) => Err(format!("Unable to play file: '{err}'")),
        }
    } else {
        Err("Invalid device".into())
    }
}

#[command]
fn get_output_devices() -> Vec<String> {
    let host = cpal::default_host();
    let devices = host.output_devices().unwrap();
    devices.map(|device| device.name().unwrap()).collect()
}

#[command]
fn get_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    let devices = host.input_devices().unwrap();
    devices.map(|device| device.name().unwrap()).collect()
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("audio")
        .invoke_handler(tauri::generate_handler![
            play_async,
            get_output_devices,
            get_input_devices
        ])
        .build()
}
