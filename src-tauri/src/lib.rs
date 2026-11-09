//! WHITE ROOM backend.
//!
//! Rust owns authoritative application state and business rules. The Svelte
//! frontend talks to it exclusively through the Tauri commands in [`commands`].
//!
//! Domain modules (see `AGENTS.md` §3):
//!
//! | Module           | Responsibility                                            |
//! |------------------|-----------------------------------------------------------|
//! | [`core`]         | Lifecycle, configuration, logging, shared errors          |
//! | [`platform`]     | macOS-specific integration                                |
//! | [`workspace`]    | Fullscreen workspace, its exits, and (later) the launcher |
//! | [`sessions`]     | Focus sessions and their lifecycle; study reminders       |
//! | [`learning`]     | Curricula, courses, lessons, assessments, mastery         |
//! | [`gamification`] | XP, levels, achievements, skill trees                     |
//! | [`economy`]      | Focus Credits, transactions, inventory, cosmetics         |
//! | [`database`]     | SQLite persistence and migrations                         |

pub mod commands;
pub mod core;
pub mod database;
pub mod economy;
pub mod gamification;
pub mod learning;
pub mod platform;
pub mod sessions;
pub mod workspace;

/// Builds and runs the Tauri application.
pub fn run() {
    tauri::Builder::default()
        .plugin(core::logging::plugin())
        .plugin(workspace::emergency_exit::plugin())
        .setup(|app| {
            core::lifecycle::initialize(app)?;
            Ok(())
        })
        .on_window_event(workspace::on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_workspace_state,
            commands::enter_workspace,
            commands::exit_workspace,
            commands::get_reminders,
            commands::set_reminders_enabled,
            commands::add_reminder,
            commands::remove_reminder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running WHITE ROOM");
}
