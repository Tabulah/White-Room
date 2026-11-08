//! Local file logging.
//!
//! Logs are written only to the local log directory
//! (`~/Library/Logs/<identifier>/` on macOS). Nothing is sent off-device.

use tauri::{plugin::TauriPlugin, Runtime};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

use super::config::LogLevel;

const LOG_FILE_NAME: &str = "white-room";
const MAX_LOG_FILE_BYTES: u128 = 5 * 1024 * 1024;
const KEPT_LOG_FILES: usize = 5;

/// Builds the logging plugin.
///
/// The plugin accepts every level; the effective level is applied from the
/// user's configuration by [`apply_level`] once the config has been loaded,
/// because the config directory is only known after the app has started.
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    let mut builder = tauri_plugin_log::Builder::new()
        .clear_targets()
        .target(Target::new(TargetKind::LogDir {
            file_name: Some(LOG_FILE_NAME.into()),
        }))
        .level(log::LevelFilter::Trace)
        .max_file_size(MAX_LOG_FILE_BYTES)
        .rotation_strategy(RotationStrategy::KeepSome(KEPT_LOG_FILES))
        .timezone_strategy(TimezoneStrategy::UseUtc);

    if cfg!(debug_assertions) {
        builder = builder.target(Target::new(TargetKind::Stdout));
    }

    builder.build()
}

/// Applies the configured log level process-wide.
pub fn apply_level(level: LogLevel) {
    log::set_max_level(level.into());
}
