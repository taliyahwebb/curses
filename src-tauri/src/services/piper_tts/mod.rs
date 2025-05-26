use core::str;
use std::io::{self, Cursor, Read};
use std::path::PathBuf;
use std::time::Duration;
use std::{future, thread};

use anyhow::{Context, bail};
use futures::{FutureExt, TryFutureExt};
use rodio::Decoder;
use serde::{Deserialize, Serialize};
use tauri::{Manager, Runtime, State, plugin};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::ChildStderr;
use tokio::sync::Mutex;
use tracing::{debug, trace};

use super::audio::{IndependentSink, get_independent_sink};

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
#[serde(rename_all = "camelCase")]
struct PiperArgs {
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
async fn write_to_stdin(process: &mut tokio::process::Child, bytes: &[u8]) -> anyhow::Result<()> {
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

/// sends the text to piper instance to generate a WAV file and returns it as a
/// byte vector
async fn get_wav_bytes(text: &str, state: &State<'_, PiperInstance>) -> anyhow::Result<Vec<u8>> {
    let mut lock = state.process.lock().await;
    let Some(child) = lock.as_mut() else {
        bail!("piper instance not running");
    };

    write_to_stdin(&mut child.0, format!("{}\n", text).as_bytes()).await?;
    debug!("text was input to piper");
    let mut str_buf = Vec::new();
    let mut wav_file = Vec::new();
    let mut buf = [0u8; 1024]; // common os buffer size
    let make_dummy_future = || {
        thread::sleep(Duration::from_micros(100));
        future::pending()
    };
    loop {
        tokio::select! {
            biased;
            Ok(read) = child.0.stdout.as_mut().expect("stdout").read(&mut buf) => {
                wav_file.extend_from_slice(&buf[..read]);
            }
            _dummy = make_dummy_future() => {}, // introduce a small delay because there is a race condition between stdout and stderr here
            _ = child.1.read_until(b"\n"[0], &mut str_buf) => {break;},
        }
    }
    let str_buf = str::from_utf8(&str_buf).expect("stderr should only output utf8 valid");
    if str_buf.contains("Real-time factor") {
        debug!("piper created output");
        // noop, we matched on success
    } else {
        let mut child = lock.take().expect("there should have been a child process");
        child.0.kill().await?;
        bail!("piper exec error: '{str_buf}'");
    }

    Ok(wav_file)
}

#[derive(Default)]
struct PiperInstance {
    process: Mutex<Option<(tokio::process::Child, BufReader<ChildStderr>)>>,
    sink: Mutex<Option<IndependentSink>>,
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
async fn speak(
    args: PiperArgs,
    text: String,
    state: State<'_, PiperInstance>,
) -> Result<(), String> {
    use crate::services::audio::RpcAudioPlayAsync;

    // current piper impl breaks if input contains newlines
    for line in text.lines() {
        // fast path for empty string
        if line.is_empty() {
            continue;
        }
        let bytes = get_wav_bytes(line, &state)
            .await
            .map_err(|e| e.to_string())?;

        let sink_lock = state.sink.lock().await;
        let Some(sink) = sink_lock.as_ref() else {
            return Err("piper instance not running (missing sink)".to_string());
        };

        let play_async_args = RpcAudioPlayAsync {
            device_name: args.device.clone(),
            data: bytes,
            volume: 1.0,
            rate: 1.0,
        };

        match Decoder::new(Cursor::new(play_async_args.data)) {
            Ok(source) => {
                sink.inner.append(source);
                sink.inner.sleep_until_end();
                continue;
            }
            Err(err) => return Err(format!("Unable to play file: '{err}'")),
        }
    }
    Ok(())
}

#[tauri::command]
async fn start(state: State<'_, PiperInstance>, args: PiperArgs) -> Result<(), String> {
    let mut lock = state.process.lock().await;
    let mut sink_lock = state.sink.lock().await;
    if sink_lock.is_some() {
        return Err("already running".to_string());
    }
    *sink_lock = Some(
        get_independent_sink(&args.device)
            .context("creating piper-tts sink")
            .map_err(|err| err.to_string())?,
    );
    if lock.is_some() {
        return Err("already running".to_string());
    }
    let mut command = tokio::process::Command::new(&args.exe_path);
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    command.arg("-m");
    command.arg(args.voice_path);
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
        .with_context(|| format!("Failed to start '{}'", args.exe_path.display()))
        .map_err(|err| err.to_string())?;

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
        buffered_error
            .read_line(&mut buf)
            .await
            .context("reading piper output")
            .map_err(|err| err.to_string())?;

        if buf.contains("Initialized piper") {
            break;
        } else if buf.contains("Loaded voice") {
            continue;
        } else {
            process
                .kill()
                .await
                .context("stopping piper")
                .map_err(|err| err.to_string())?;
            return Err(format!("piper exec error: '{buf}'"));
        }
    }
    *lock = Some((process, buffered_error));
    Ok(())
}

#[tauri::command]
async fn stop(state: State<'_, PiperInstance>) -> Result<(), String> {
    if let Some(sink) = state.sink.lock().await.take() {
        drop(sink);
    }
    if let Some(mut child) = state.process.lock().await.take() {
        child.0.kill().map_err(|err| err.to_string()).await?
    }
    Ok(())
}

pub fn init<R: Runtime>() -> plugin::TauriPlugin<R> {
    plugin::Builder::new("piper-tts")
        .invoke_handler(tauri::generate_handler![get_voices, start, speak, stop])
        .setup(|app, _api| {
            app.manage(PiperInstance::default());
            Ok(())
        })
        .build()
}
