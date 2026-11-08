//! Tauri commands: the only interface between the frontend and the backend.
//!
//! Commands are thin: they validate input, delegate to domain modules, and
//! map errors to [`CommandError`]. Each command must also be listed in
//! `build.rs` and granted in `capabilities/` before the frontend can call it.

use serde::Serialize;
use tauri::{AppHandle, Runtime, State};

use crate::{core::error::CommandError, database::Database};

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
