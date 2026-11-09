//! Application startup.

use std::error::Error;

use tauri::{App, AppHandle, Manager, Runtime};

use super::{config, logging};
use crate::{
    database::{Database, DATABASE_FILE_NAME},
    sessions::reminders::{Delivery, Reminders},
    workspace,
};

/// Loads configuration, opens and migrates the database, registers the
/// emergency-exit shortcut, registers managed state, and brings scheduled
/// study reminders in line with the database. Runs once from the Tauri
/// `setup` hook.
pub fn initialize(app: &mut App) -> Result<(), Box<dyn Error>> {
    let paths = app.path();

    let config_path = paths.app_config_dir()?.join(config::CONFIG_FILE_NAME);
    let config = config::load(&config_path).unwrap_or_else(|err| {
        // Never overwrite the user's file; fall back to defaults and say so.
        log::warn!("{err}; using default configuration");
        config::AppConfig::default()
    });
    logging::apply_level(config.log_level);

    log::info!(
        "starting {} {}",
        app.package_info().name,
        app.package_info().version
    );

    let db_path = paths.app_data_dir()?.join(DATABASE_FILE_NAME);
    let database = Database::open(&db_path).map_err(|err| {
        log::error!("failed to open database at {}: {err}", db_path.display());
        err
    })?;
    log::info!(
        "database ready at {} (schema version {})",
        db_path.display(),
        database.schema_version()?
    );

    app.manage(database);

    let workspace = workspace::init(app.handle());
    app.manage(workspace);

    app.manage(Reminders::native());
    sync_reminders_in_background(app.handle());

    #[cfg(debug_assertions)]
    super::smoke_test::start_if_requested(app.handle());

    Ok(())
}

/// Repairs any drift between the stored reminders and what macOS has
/// scheduled (for example after the app was reinstalled). Asks for permission
/// only if reminders are switched on and the user has never answered.
fn sync_reminders_in_background<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        match app.state::<Reminders>().sync(&app.state::<Database>()) {
            Ok(state) => match state.delivery {
                Delivery::Off => log::debug!("study reminders are off"),
                delivery => log::info!(
                    "study reminders: {} stored, delivery {delivery:?}",
                    state.reminders.len()
                ),
            },
            Err(err) => log::warn!("could not sync study reminders: {err}"),
        }
    });
}
