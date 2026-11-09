//! Tauri commands: the only interface between the frontend and the backend.
//!
//! Commands are thin: they validate input, delegate to domain modules, and
//! map errors to [`CommandError`]. Each command must also be listed in
//! `build.rs` and granted in `capabilities/` before the frontend can call it.

use serde::Serialize;
use tauri::{AppHandle, Runtime, State};

use crate::{
    core::error::CommandError,
    database::Database,
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
