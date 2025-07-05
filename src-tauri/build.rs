fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .plugin(
                "audio",
                tauri_build::InlinedPlugin::new()
                    .commands(&["play_async", "get_output_devices", "get_input_devices"])
                    .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
            )
            .plugin(
                "custom-tts",
                tauri_build::InlinedPlugin::new()
                    .commands(&["speak"])
                    .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
            )
            .plugin(
                "keyboard",
                tauri_build::InlinedPlugin::new()
                    .commands(&["start_tracking", "stop_tracking"])
                    .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
            )
            .plugin(
                "osc",
                tauri_build::InlinedPlugin::new()
                    .commands(&["send"])
                    .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
            )
            .plugin(
                "piper-tts",
                tauri_build::InlinedPlugin::new()
                    .commands(&["get_voices", "start", "speak", "stop"])
                    .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
            )
            .plugin(
                "translate",
                tauri_build::InlinedPlugin::new()
                    .commands(&["translate"])
                    .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
            )
            .plugin(
                "uberduck-tts",
                tauri_build::InlinedPlugin::new()
                    .commands(&["speak", "get_voices"])
                    .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
            )
            .plugin(
                "uwu",
                tauri_build::InlinedPlugin::new()
                    .commands(&["translate"])
                    .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
            )
            .plugin(
                "web",
                tauri_build::InlinedPlugin::new()
                    .commands(&["open_browser", "pubsub_broadcast", "config"])
                    .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
            )
            .plugin(
                "whisper-stt",
                tauri_build::InlinedPlugin::new()
                    .commands(&["start", "stop"])
                    .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
            )
            .plugin(
                "windows-tts",
                tauri_build::InlinedPlugin::new()
                    .commands(&["speak", "get_voices"])
                    .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
            ),
    )
    .expect("failed to run tauri-build");
}
