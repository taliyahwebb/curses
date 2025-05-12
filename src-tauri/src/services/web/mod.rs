use super::AppConfiguration;
use local_ip_address::local_ip;
use serde::{Deserialize, Serialize};
use std::{process::{Command, Stdio}, sync::Arc};
use tauri::{Emitter, Manager, Runtime, State, async_runtime::Mutex, command, plugin::{Builder, TauriPlugin}};
use tokio::sync::mpsc;
use warp::Filter;

mod assets;
mod peer;
mod pubsub;

struct PubSubInput {
    tx: Mutex<mpsc::Sender<String>>,
}

#[command]
async fn pubsub_broadcast(value: String, input: State<'_, PubSubInput>) -> Result<(), String> {
    let tx = input.tx.lock().await;
    tx.send(value).await.map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct WebConfig {
    pub local_ip: String,
    pub port: String,
    pub peer_path: String,
    pub pubsub_path: String,
}

#[command]
async fn config(config: State<'_, AppConfiguration>) -> Result<WebConfig, String> {
    let Ok(ip) = local_ip() else {
        return Err("Error retrieving local IP".to_string());
    };
    Ok(WebConfig {
        local_ip: ip.to_string(),
        port: config.port.to_string(),
        peer_path: "peer".to_string(),
        pubsub_path: "pubsub".to_string(),
    })
}

#[cfg(windows)]
fn try_open_browser(browser: &String, url: &String) -> Result<bool, String> {
    Ok(Command::new("cmd")
        .stderr(Stdio::null()) // errors are expected, don't print to terminal
        .args(["/C", format!("start {} {}", browser, url).as_str()])
        .status()
        .expect("failed to execute process `cmd`")
        .success())
}

#[cfg(target_os = "linux")]
fn try_open_browser(browser: &String, url: &String) -> Result<bool, String> {
    Ok(Command::new(browser)
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .arg(url)
        .spawn()
        .is_ok())
}

#[cfg(not(any(windows, target_os = "linux")))]
fn try_open_browser(_browser: &String, _url: &String) -> Result<bool, String> {
    Err("Action not implemented for your OS".to_string())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenBrowserCommand {
    browser_names: Vec<String>,
    url: String,
}
#[command]
fn open_browser(data: OpenBrowserCommand) -> Result<(), String> {
    for browser in &data.browser_names {
        match try_open_browser(browser, &data.url) {
            Ok(success) => {
                if success {
                    return Ok(());
                }
            }
            Err(err) => return Err(err),
        };
    }
    Err("Could not find browser executable".to_string())
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    let (pubsub_input_tx, pubsub_input_rx) = mpsc::channel::<String>(1); // to pubsub
    let (pubsub_output_tx, mut pubsub_output_rx) = mpsc::channel::<String>(1); // to js
    Builder::new("web")
        .invoke_handler(tauri::generate_handler![open_browser, pubsub_broadcast, config])
        .setup(|app, _api| {
            app.manage(PubSubInput {
                tx: Mutex::new(pubsub_input_tx),
            });

            let app_port = app.state::<AppConfiguration>().port;

            let a = Arc::new(app.asset_resolver());
            tauri::async_runtime::spawn(async move {
                let routes = warp::path!("ping")
                    .map(|| "pong".to_string())
                    .or(peer::path())
                    .or(pubsub::path(pubsub_input_rx, pubsub_output_tx))
                    .or(assets::path(a));

                loop {
                    warp::serve(routes.clone())
                        .run(([0, 0, 0, 0], app_port))
                        .await
                }
            });
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    if let Some(output) = pubsub_output_rx.recv().await {
                        handle.emit("pubsub", output).unwrap();
                    }
                }
            });

            Ok(())
        })
        .build()
}
