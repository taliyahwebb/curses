#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use serde::{Deserialize, Serialize};
use tauri::{Manager, State, command};
use tauri_plugin_window_state::{AppHandleExt, StateFlags};
#[cfg(windows)]
use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_PERMISSION_KIND_MICROPHONE,
    COREWEBVIEW2_PERMISSION_STATE_ALLOW,
    ICoreWebView2_13,
    ICoreWebView2Profile4,
};
#[cfg(windows)]
use windows::core::{Interface, PCWSTR};

use crate::services::AppConfiguration;

mod services;
mod utils;

#[derive(Parser, Debug)]
struct InitArguments {
    #[arg(short, long, default_value_t = 3030)]
    port: u16,
}

#[derive(Serialize, Deserialize)]
struct NativeFeatures {
    background_input: bool,
}

#[command]
fn get_native_features() -> NativeFeatures {
    NativeFeatures {
        background_input: cfg!(feature = "background_input"),
    }
}

#[command]
fn get_port(state: State<'_, InitArguments>) -> u16 {
    state.port
}

#[command]
fn grant_mic_access(_origin: &str, _webview_window: tauri::WebviewWindow) {
    #[cfg(windows)]
    {
        let mut origin = _origin.to_string();
        origin.push('\0');
        let origin = origin.encode_utf16().collect::<Vec<u16>>();

        _webview_window
            .with_webview(move |webview| unsafe {
                let origin = PCWSTR::from_raw(origin.as_ptr());

                let core = Interface::cast::<ICoreWebView2_13>(
                    &webview.controller().CoreWebView2().unwrap(),
                )
                .unwrap();
                let profile =
                    Interface::cast::<ICoreWebView2Profile4>(&core.Profile().unwrap()).unwrap();

                profile
                    .SetPermissionState(
                        COREWEBVIEW2_PERMISSION_KIND_MICROPHONE,
                        origin,
                        COREWEBVIEW2_PERMISSION_STATE_ALLOW,
                        None,
                    )
                    .unwrap();
            })
            .unwrap();
    }
}

#[command]
fn app_close(app_handle: tauri::AppHandle) {
    let Some(window) = app_handle.get_webview_window("main") else {
        return app_handle.exit(0);
    };
    app_handle.save_window_state(StateFlags::all()).ok(); // don't really care if it saves it

    if window.close().is_err() {
        app_handle.exit(0)
    }
}

fn app_setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let window = app.get_webview_window("main").unwrap();
    window.set_shadow(true).ok(); // ignore failure
    Ok(())
}

fn main() {
    let args = InitArguments::parse();

    // crash if port is not available
    let port_availability = std::net::TcpListener::bind(format!("0.0.0.0:{}", args.port));
    match port_availability {
        Ok(l) => l.set_nonblocking(true).unwrap(),
        Err(_err) => {
            #[cfg(windows)]
            {
                use windows::Win32::UI::WindowsAndMessaging::{MB_ICONWARNING, MB_OK, MessageBoxA};
                use windows::core::*;
                let message = format!("Port {} is not available!", args.port);
                unsafe {
                    MessageBoxA(
                        None,
                        windows::core::PCSTR(message.as_ptr()),
                        s!("Curses error"),
                        MB_OK | MB_ICONWARNING,
                    );
                }
            }
            return;
        }
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_shell::init())
        .setup(app_setup)
        .manage(AppConfiguration { port: args.port })
        .invoke_handler(tauri::generate_handler![
            get_port,
            get_native_features,
            app_close,
            grant_mic_access,
        ])
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(services::osc::init())
        .plugin(services::web::init())
        .plugin(services::audio::init())
        .plugin(services::windows_tts::init())
        .plugin(services::uberduck_tts::init())
        .plugin(services::piper_tts::init())
        .plugin(services::custom_tts::init())
        .plugin(services::whisper_stt::init())
        .plugin(services::keyboard::init())
        .plugin(services::uwu::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
