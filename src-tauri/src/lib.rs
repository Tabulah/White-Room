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
//! | [`workspace`]    | Desktop presentation and application launcher             |
//! | [`sessions`]     | Focus sessions and their lifecycle                        |
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
        .setup(|app| {
            core::lifecycle::initialize(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::get_app_info])
        .run(tauri::generate_context!())
        .expect("error while running WHITE ROOM");
}
