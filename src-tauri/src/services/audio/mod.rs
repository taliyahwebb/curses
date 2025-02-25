use std::io::Cursor;

use rodio::{
    cpal::{self, traits::HostTrait},
    Decoder, DeviceTrait, OutputStream, OutputStreamHandle, Sink,
};
use serde::{Serialize, Deserialize};
use tauri::{
    command,
    plugin::{Builder, TauriPlugin},
    Runtime,
};

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
    pub rate: f32, // 1 - base
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
    }
    else {
        Err("Invalid device".into())
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("audio")
        .invoke_handler(tauri::generate_handler![play_async])
        .build()
}
