//! User configuration loaded from `config.json` in the app config directory.
//!
//! The file is optional: when it is missing, defaults are used and nothing is
//! written to disk. When it is present it is validated strictly (unknown keys
//! and unsupported versions are rejected) so typos are reported rather than
//! silently ignored.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// File name of the configuration file inside the app config directory.
pub const CONFIG_FILE_NAME: &str = "config.json";

/// Configuration format version understood by this build.
pub const CURRENT_CONFIG_VERSION: u32 = 1;

/// Validated application configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    /// Format version of the configuration file.
    #[serde(default = "current_config_version")]
    pub config_version: u32,

    /// Minimum level written to the log.
    #[serde(default)]
    pub log_level: LogLevel,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config_version: CURRENT_CONFIG_VERSION,
            log_level: LogLevel::default(),
        }
    }
}

fn current_config_version() -> u32 {
    CURRENT_CONFIG_VERSION
}

/// Log verbosity accepted in the configuration file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for log::LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => Self::Error,
            LogLevel::Warn => Self::Warn,
            LogLevel::Info => Self::Info,
            LogLevel::Debug => Self::Debug,
            LogLevel::Trace => Self::Trace,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not read config file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("invalid config: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("unsupported config_version {found} (this build supports {supported})")]
    UnsupportedVersion { found: u32, supported: u32 },
}

/// Parses and validates configuration from JSON text.
pub fn parse(json: &str) -> Result<AppConfig, ConfigError> {
    let config: AppConfig = serde_json::from_str(json)?;
    validate(&config)?;
    Ok(config)
}

/// Loads configuration from `path`, returning defaults if the file does not exist.
pub fn load(path: &Path) -> Result<AppConfig, ConfigError> {
    match fs::read_to_string(path) {
        Ok(json) => parse(&json),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(AppConfig::default()),
        Err(source) => Err(ConfigError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn validate(config: &AppConfig) -> Result<(), ConfigError> {
    if config.config_version != CURRENT_CONFIG_VERSION {
        return Err(ConfigError::UnsupportedVersion {
            found: config.config_version,
            supported: CURRENT_CONFIG_VERSION,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_object_yields_defaults() {
        assert_eq!(parse("{}").unwrap(), AppConfig::default());
    }

    #[test]
    fn parses_all_fields() {
        let config = parse(r#"{ "config_version": 1, "log_level": "debug" }"#).unwrap();
        assert_eq!(config.log_level, LogLevel::Debug);
        assert_eq!(config.config_version, 1);
    }

    #[test]
    fn rejects_unknown_fields() {
        let err = parse(r#"{ "log_levl": "debug" }"#).unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)), "{err}");
    }

    #[test]
    fn rejects_invalid_log_level() {
        let err = parse(r#"{ "log_level": "verbose" }"#).unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)), "{err}");
    }

    #[test]
    fn rejects_unsupported_version() {
        let err = parse(r#"{ "config_version": 2 }"#).unwrap_err();
        assert!(
            matches!(
                err,
                ConfigError::UnsupportedVersion {
                    found: 2,
                    supported: 1
                }
            ),
            "{err}"
        );
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(matches!(parse("{"), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn missing_file_yields_defaults() {
        let path = std::env::temp_dir().join("white-room-test-config-that-does-not-exist.json");
        assert_eq!(load(&path).unwrap(), AppConfig::default());
    }

    #[test]
    fn unreadable_path_is_an_error() {
        // A directory cannot be read as a file.
        let err = load(&std::env::temp_dir()).unwrap_err();
        assert!(matches!(err, ConfigError::Read { .. }), "{err}");
    }

    #[test]
    fn log_level_maps_to_level_filter() {
        assert_eq!(
            log::LevelFilter::from(LogLevel::Warn),
            log::LevelFilter::Warn
        );
        assert_eq!(
            log::LevelFilter::from(LogLevel::Trace),
            log::LevelFilter::Trace
        );
    }
}
