//! Application startup.

use std::error::Error;

use tauri::{App, Manager};

use super::{config, logging};
use crate::{
    database::{Database, DATABASE_FILE_NAME},
    workspace,
};

/// Loads configuration, opens and migrates the database, registers the
/// emergency-exit shortcut, and registers managed state. Runs once from the
/// Tauri `setup` hook.
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

    #[cfg(debug_assertions)]
    super::smoke_test::start_if_requested(app.handle());

    Ok(())
}
