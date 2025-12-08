use core::str;
use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};
use std::str::from_utf8;
use std::time::Duration;

use anyhow::{Context, bail};
use futures::TryFutureExt;
use rodio::Decoder;
use serde::{Deserialize, Serialize};
use tauri::{Manager, Runtime, State, plugin};
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::ChildStdout;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tracing::{debug, instrument, trace};

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

    /// directory to put the generated audio
    out_path: Option<PathBuf>,

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
    stdin
        .write_all(bytes)
        .await
        .context("writing to piper stdin")?;
    Ok(())
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
    // reading files from stdout directly works but causes trouble with properly
    // separating individual "files" (tried to use stderr piper output but that
    // works poorly since it's not properly synchronized with stdout)
    let mut lock = state.process.lock().await;
    let Some(child) = lock.as_mut() else {
        bail!("piper instance not running");
    };

    write_to_stdin(&mut child.process, format!("{}\n", text).as_bytes()).await?;
    trace!("text was input to piper");
    let mut path = String::new();
    child.stdout.read_line(&mut path).await?;
    let path = path.trim();
    trace!("piper produced output at '{path}'");
    let bytes = fs::read(path)?;
    #[cfg(windows)]
    std::thread::yield_now();
    match &child.out_dir {
        PiperOutput::Temp(_) => {
            fs::remove_file(path)?;
            trace!("deleted tmpfile '{path}'");
        }
        PiperOutput::Keep(dir) => {
            let mut new_path = PathBuf::from(dir);
            new_path.push(format!(
                "{}{}-{}",
                text.trim().chars().take(20).collect::<String>(),
                if text.trim().chars().count() > 20 {
                    "[...]"
                } else {
                    ""
                },
                PathBuf::from(path)
                    .file_name()
                    .expect("if not its not valid piper output")
                    .to_str()
                    .expect("path was built from utf-8"),
            ));
            fs::rename(path, &new_path)?;
            trace!(
                "renamed output file '{path}' -> '{}",
                new_path.to_string_lossy()
            );
        }
    }

    Ok(bytes)
}

enum PiperOutput {
    Temp(TempDir),
    Keep(PathBuf),
}
impl PiperOutput {
    fn path(&self) -> &Path {
        match self {
            PiperOutput::Temp(temp_dir) => temp_dir.path(),
            PiperOutput::Keep(path_buf) => path_buf.as_path(),
        }
    }
}

struct PiperProcess {
    process: tokio::process::Child,
    stdout: BufReader<ChildStdout>,
    out_dir: PiperOutput,
}

#[derive(Default)]
struct PiperInstance {
    process: Mutex<Option<PiperProcess>>,
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
    if args.out_path.as_ref().is_some_and(|path| !path.is_dir()) {
        return Err(format!(
            "output path '{:?}' is not a directory",
            args.out_path.expect("was checked above")
        ));
    }
    let out_dir = match args.out_path {
        None => PiperOutput::Temp(
            tempfile::tempdir()
                .context("creating temp piper output dir")
                .map_err(|e| e.to_string())?,
        ),
        Some(path) => {
            fs::create_dir_all(&path)
                .context("creating persistent piper output dir")
                .map_err(|e| e.to_string())?;
            PiperOutput::Keep(path)
        }
    };
    *sink_lock = Some(
        get_independent_sink(&args.device)
            .context("creating piper-tts sink")
            .map_err(|err| err.to_string())?,
    );
    if lock.is_some() {
        return Err("already running".to_string());
    }

    probe_piper_version(&args.exe_path)
        .await
        .map_err(|err| err.to_string())?;

    let mut command = tokio::process::Command::new(&args.exe_path);
    command.kill_on_drop(true);
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    command.arg("-m");
    command.arg(args.voice_path);
    command.arg("-d");
    command.arg(out_dir.path());

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

    trace!("starting piper process, '{command:?}'");
    let mut process = command
        .spawn()
        .with_context(|| format!("Failed to start '{}'", args.exe_path.display()))
        .map_err(|err| err.to_string())?;

    let mut buffered_error = BufReader::new(
        process
            .stderr
            .as_mut()
            .expect("stderr should have been captured"),
    );
    let buffered_out = BufReader::new(
        process
            .stdout
            .take()
            .expect("stdout should have been captured"),
    );
    // expected piper output
    // ```
    // [yyyy-mm-dd hh:mm:ss.nnn] [piper] [info] Loaded voice in <time> second(s)
    // [yyyy-mm-dd hh:mm:ss.nnn] [piper] [info] Initialized piper
    // [yyyy-mm-dd hh:mm:ss.nnn] [piper] [info] Output directory: <tempdir>
    // ```
    let mut buf = String::new();
    loop {
        buf.clear();
        buffered_error
            .read_line(&mut buf)
            .await
            .context("reading piper output")
            .map_err(|err| err.to_string())?;

        if buf.contains("Loaded voice") || buf.contains("Initialized piper") {
            continue;
        } else if buf.contains("Output directory") {
            debug!(
                "piper output dir: '{}'",
                buf.split(':').next_back().expect("invalid piper output")
            );
            break;
        } else {
            process
                .kill()
                .await
                .context("stopping piper")
                .map_err(|err| err.to_string())?;
            return Err(format!("piper exec error: '{buf}'"));
        }
    }
    let process = PiperProcess {
        process,
        stdout: buffered_out,
        out_dir,
    };
    *lock = Some(process);
    Ok(())
}

#[instrument()]
async fn probe_piper_version(exe: &PathBuf) -> anyhow::Result<()> {
    trace!("probing for piper version");
    let mut version_command = tokio::process::Command::new(exe);
    version_command.arg("--version");
    version_command.kill_on_drop(true);
    let output = timeout(Duration::from_secs(5), version_command.output())
        .await
        .context("executing piper version probe command")? // context for timeout
        .context("executing piper version probe command")?; // context for cmd failure
    match from_utf8(&output.stdout) {
        Ok(tag) => {
            if tag.is_empty() {
                if output.status.code().is_some() {
                    bail!(
                        "you are probably using piper version 1.3.0 from OHF-Voice, which has a very broken command line interface, try to use v1.2.0 from rhasspy"
                    )
                } else {
                    bail!("invalid piper output: '{tag}', unable to detect piper runtime version")
                }
            }
            let version: Vec<_> = tag.split('.').collect();
            if version.len() == 3 && version[0] == "1" {
                trace!("piper version 1.x from rhasspy detected '{tag}'");
                return Ok(());
            } else {
                bail!("invalid piper output: '{tag}', unable to detect piper runtime version")
            }
        }
        Err(_) => {
            bail!("invalid piper output: not utf-8, unable to detect piper runtime version")
        }
    };
}

#[tauri::command]
async fn stop(state: State<'_, PiperInstance>) -> Result<(), String> {
    if let Some(sink) = state.sink.lock().await.take() {
        drop(sink);
    }
    if let Some(mut child) = state.process.lock().await.take() {
        child.process.kill().map_err(|err| err.to_string()).await?
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
