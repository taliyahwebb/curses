use core::str;
use std::io;
use std::path::PathBuf;

use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use tauri::{Manager, Runtime, State, plugin};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::ChildStderr;
use tokio::sync::Mutex;

use crate::utils::*;

#[derive(Serialize, Deserialize, Debug)]
struct Voice {
    /// human-readable name to display to the user
    name: String,

    /// full path to the .onnx file
    path: PathBuf,
}

/// arguments to the `speak` function. most of these get passed straight to
/// piper.exe
#[derive(Serialize, Deserialize, Debug)]
struct SpeakArgs {
    /// audio output device
    device: String,

    /// path to piper.exe
    exe_path: PathBuf,

    /// path to the voice model
    voice_path: PathBuf,

    /// id of speaker when using multi-speaker models
    speaker_id: Option<u32>,

    /// generator noise
    noise_scale: Option<f32>,

    /// phoneme width noise
    noise_width: Option<f32>,

    /// phoneme length
    length_scale: Option<f32>,

    /// seconds of silence after each sentence
    sentence_silence: Option<f32>,

    /// the actual text to speak
    value: String,
}

/// Scans the given directory for Piper voice files.
/// A valid piper voice file must end in `.onnx` and have an accompanying
/// `.onnx.json`
fn scan_voice_directory(path: PathBuf) -> io::Result<Vec<Voice>> {
    fn try_into_voice(entry: std::fs::DirEntry) -> Option<Voice> {
        let path = entry.path();
        if path.is_file()
            && path.extension() == Some("onnx".as_ref())
            && path.with_extension("onnx.json").exists()
        {
            // we only need the name for display purposes, so the lossy conversion here is
            // fine.
            let name = path.file_stem()?.to_string_lossy().into();
            Some(Voice { name, path })
        } else {
            None
        }
    }

    Ok(std::fs::read_dir(path)?
        .filter_map(Result::ok)
        .filter_map(try_into_voice)
        .collect())
}

/// Writes a byte slice to the standard input of a child process
async fn write_to_stdin(process: &mut tokio::process::Child, bytes: &[u8]) -> io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let stdin = process.stdin.as_mut().expect("Failed to open stdin");
    let result = stdin.write_all(bytes).await;
    result.with_context(|| "Could not write to piper's standard input")
}

/// Handles adding optional command line parameters to a Command object
fn add_arg_if_some(
    command: &mut tokio::process::Command,
    name: &str,
    value: Option<impl ToString>,
) {
    if let Some(value) = value {
        command.arg(name);
        command.arg(value.to_string());
    }
}

/// Invokes piper to generate a WAV file and returns it as a byte vector
async fn get_wav_bytes(args: &SpeakArgs, state: State<'_, PiperInstance>) -> io::Result<Vec<u8>> {
    // at first i tried to read the wav data from stdout, but that didn't work,
    // so now i just use a regular file here.
    let model_path = &args.voice_path;
    let piper_path = &args.exe_path;

    let mut lock = state.process.lock().await;
    let child = if let Some(child) = lock.as_mut() {
        child
    } else {
        let mut command = tokio::process::Command::new(piper_path);
        command.stdin(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());
        command.arg("-m");
        command.arg(model_path);
        command.arg("-f");
        command.arg("-");

        add_arg_if_some(&mut command, "--speaker", args.speaker_id);
        add_arg_if_some(&mut command, "--noise_scale", args.noise_scale);
        add_arg_if_some(&mut command, "--noise_w", args.noise_width);
        add_arg_if_some(&mut command, "--length_scale", args.length_scale);
        add_arg_if_some(&mut command, "--sentence_silence", args.sentence_silence);

        #[cfg(windows)]
        {
            // console applications on windows have the annoying habit of spawning a
            // terminal window. we need to explicitly tell CreateProcess not to
            // do that.
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        let mut process = command
            .spawn()
            .with_context(|| format!("Failed to start '{}'", piper_path.display()))?;

        let mut buffered_error = BufReader::new(
            process
                .stderr
                .take()
                .expect("stderr should have been captured"),
        );
        // expected piper output
        // ```
        // [yyyy-mm-dd hh:mm:ss.nnn] [piper] [info] Loaded voice in <time> second(s)
        // [yyyy-mm-dd hh:mm:ss.nnn] [piper] [info] Initialized piper
        // ```
        let mut buf = String::new();
        loop {
            buf.clear();
            buffered_error.read_line(&mut buf).await?;

            if buf.contains("Initialized piper") {
                break;
            } else if buf.contains("Loaded voice") {
                continue;
            } else {
                process.kill().await?;
                return Err(io::Error::other("piper exec error: '{buf}'"));
            }
        }
        lock.insert((process, buffered_error))
    };

    write_to_stdin(&mut child.0, format!("{}\n", args.value).as_bytes()).await?;
    let mut str_buf = Vec::new();
    let mut wav_file = Vec::new();
    let mut buf = [0u8; 1024]; // common os buffer size
    loop {
        tokio::select! {
            biased;
            Ok(read) = child.0.stdout.as_mut().expect("stdout").read(&mut buf) => {
                wav_file.extend_from_slice(&buf[..read]);
            }
            _ = child.1.read_until(b"\n"[0], &mut str_buf) => {break;},
        }
    }
    let str_buf = str::from_utf8(&str_buf).expect("stderr should only output utf8 valid");
    if str_buf.contains("Real-time factor") {
        // noop, we matched on success
    } else {
        let mut child = lock.take().expect("there should have been a child process");
        child.0.kill().await?;
        return Err(io::Error::other("piper exec error: '{buf}'"));
    }

    Ok(wav_file)
}

#[derive(Default)]
struct PiperInstance {
    process: Mutex<Option<(tokio::process::Child, BufReader<ChildStderr>)>>,
}

#[tauri::command]
fn get_voices(path: PathBuf) -> Result<Vec<Voice>, String> {
    if path.to_string_lossy().is_empty() {
        return Ok(Vec::new());
    }
    match scan_voice_directory(path) {
        Ok(vec) if vec.is_empty() => Err("No voices found. Voice files must come in pairs named '<file>.onnx' and '<file>.onnx.json'".into()),
        Ok(vec) => Ok(vec),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
async fn speak(args: SpeakArgs, state: State<'_, PiperInstance>) -> Result<(), String> {
    use crate::services::audio::{RpcAudioPlayAsync, play_async};

    // fast path for empty string
    if args.value.is_empty() {
        return Ok(());
    }

    let bytes = get_wav_bytes(&args, state)
        .await
        .map_err(|e| e.to_string())?;

    let play_async_args = RpcAudioPlayAsync {
        device_name: args.device,
        data: bytes,
        volume: 1.0,
        rate: 1.0,
    };

    play_async(play_async_args).await
}

#[tauri::command]
async fn stop(state: State<'_, PiperInstance>) -> Result<(), String> {
    if let Some(mut child) = state.process.lock().await.take() {
        child.0.kill().map_err(|err| err.to_string()).await?
    }
    Ok(())
}

pub fn init<R: Runtime>() -> plugin::TauriPlugin<R> {
    plugin::Builder::new("piper-tts")
        .invoke_handler(tauri::generate_handler![speak, get_voices, stop])
        .setup(|app, _api| {
            app.manage(PiperInstance::default());
            Ok(())
        })
        .build()
}
