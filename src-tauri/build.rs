fn main() {
    // Declaring the app's commands in a manifest makes them deny-by-default:
    // each command is only callable from a window whose capability grants its
    // generated `allow-<command>` permission (see `capabilities/`).
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_app_info",
            "get_workspace_state",
            "enter_workspace",
            "exit_workspace",
            "get_reminders",
            "set_reminders_enabled",
            "add_reminder",
            "remove_reminder",
        ]),
    ))
    .expect("failed to run tauri-build");
}
