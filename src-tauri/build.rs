fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .plugin(
                "web",
                tauri_build::InlinedPlugin::new()
                    .commands(&["open_browser", "pubsub_broadcast", "config"])
                    .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands)
            )
    )
    .expect("failed to run tauri-build");
}
