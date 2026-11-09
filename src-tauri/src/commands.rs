//! Tauri commands: the only interface between the frontend and the backend.
//!
//! Commands are thin: they validate input, delegate to domain modules, and
//! map errors to [`CommandError`]. Each command must also be listed in
//! `build.rs` and granted in `capabilities/` before the frontend can call it.

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};

use crate::{
    core::error::CommandError,
    database::Database,
    sessions::reminders::{NewReminder, ReminderError, ReminderState, Reminders},
    workspace::{self, ExitReason, Workspace, WorkspaceSnapshot},
};

/// Basic information about the running application.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub schema_version: u32,
}

#[tauri::command]
pub async fn get_app_info<R: Runtime>(
    app: AppHandle<R>,
    database: State<'_, Database>,
) -> Result<AppInfo, CommandError> {
    let package = app.package_info();
    Ok(AppInfo {
        name: package.name.clone(),
        version: package.version.to_string(),
        schema_version: database.schema_version()?,
    })
}

// Workspace commands are deliberately synchronous: Tauri runs non-async
// commands on the main thread, which serialises them with window events and
// the emergency shortcut (both also handled on the main thread).

#[tauri::command]
pub fn get_workspace_state(workspace: State<'_, Workspace>) -> WorkspaceSnapshot {
    #[cfg(debug_assertions)]
    crate::core::smoke_test::note_frontend_connected();
    workspace.snapshot()
}

#[tauri::command]
pub fn enter_workspace<R: Runtime>(app: AppHandle<R>) -> Result<WorkspaceSnapshot, CommandError> {
    Ok(workspace::enter(&app)?)
}

/// Called by the on-screen exit control.
#[tauri::command]
pub fn exit_workspace<R: Runtime>(app: AppHandle<R>) -> Result<WorkspaceSnapshot, CommandError> {
    Ok(workspace::exit(&app, ExitReason::ExitControl)?)
}

// Reminder commands talk to macOS and may wait for the user to answer the
// notification permission prompt, so they run on a blocking thread.

/// The stored reminders, after bringing macOS in line with them.
#[tauri::command]
pub async fn get_reminders<R: Runtime>(app: AppHandle<R>) -> Result<ReminderState, CommandError> {
    with_reminders(app, |reminders, db| reminders.sync(db)).await
}

#[tauri::command]
pub async fn set_reminders_enabled<R: Runtime>(
    app: AppHandle<R>,
    enabled: bool,
) -> Result<ReminderState, CommandError> {
    with_reminders(app, move |reminders, db| reminders.set_enabled(db, enabled)).await
}

/// `reminder` is validated in Rust; see `sessions::reminders`.
#[tauri::command]
pub async fn add_reminder<R: Runtime>(
    app: AppHandle<R>,
    reminder: NewReminder,
) -> Result<ReminderState, CommandError> {
    with_reminders(app, move |reminders, db| reminders.add(db, reminder)).await
}

#[tauri::command]
pub async fn remove_reminder<R: Runtime>(
    app: AppHandle<R>,
    id: i64,
) -> Result<ReminderState, CommandError> {
    with_reminders(app, move |reminders, db| reminders.remove(db, id)).await
}

async fn with_reminders<R: Runtime, T: Send + 'static>(
    app: AppHandle<R>,
    f: impl FnOnce(&Reminders, &Database) -> Result<T, ReminderError> + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        f(&app.state::<Reminders>(), &app.state::<Database>())
    })
    .await
    .map_err(|err| CommandError::Interrupted(err.to_string()))?
    .map_err(CommandError::from)
}
