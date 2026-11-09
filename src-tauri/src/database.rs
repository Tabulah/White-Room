//! SQLite persistence.
//!
//! The database lives at `<app data dir>/white-room.sqlite3`. Business logic
//! never touches SQL directly; domain modules go through the repository
//! functions in this module's submodules.

mod migrations;
pub mod reminders;

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use rusqlite::Connection;

pub use migrations::MigrationError;

/// File name of the database inside the app data directory.
pub const DATABASE_FILE_NAME: &str = "white-room.sqlite3";

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// The schema version this build migrates databases to.
pub fn latest_schema_version() -> u32 {
    migrations::MIGRATIONS.len() as u32
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("could not create database directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),

    #[error(transparent)]
    Migration(#[from] MigrationError),

    #[error("database connection lock was poisoned by a panic")]
    LockPoisoned,
}

/// The application's single SQLite connection.
///
/// Managed as Tauri state; access is serialized through a mutex.
#[derive(Debug)]
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Opens (creating if necessary) the database at `path`, applies connection
    /// settings, and runs pending migrations.
    pub fn open(path: &Path) -> Result<Self, DatabaseError> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|source| DatabaseError::CreateDir {
                path: dir.to_path_buf(),
                source,
            })?;
        }
        Self::init(Connection::open(path)?)
    }

    /// Opens a fresh in-memory database. Intended for tests.
    pub fn open_in_memory() -> Result<Self, DatabaseError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self, DatabaseError> {
        configure(&conn)?;
        migrations::migrate(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Returns the highest applied migration version.
    pub fn schema_version(&self) -> Result<u32, DatabaseError> {
        let conn = self.lock()?;
        Ok(migrations::current_version(&conn)?)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, DatabaseError> {
        self.conn.lock().map_err(|_| DatabaseError::LockPoisoned)
    }
}

/// Per-connection settings. These are not persisted by SQLite, so they must be
/// applied every time a connection is opened.
fn configure(conn: &Connection) -> rusqlite::Result<()> {
    conn.busy_timeout(BUSY_TIMEOUT)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    // WAL keeps committed data durable across crashes and lets readers proceed
    // during writes. It is a no-op for in-memory databases.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_database_is_migrated_to_latest() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(db.schema_version().unwrap(), latest_schema_version());
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let db = Database::open_in_memory().unwrap();
        let enabled: bool = db
            .lock()
            .unwrap()
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert!(enabled);
    }

    #[test]
    fn file_database_persists_across_reopen() {
        let dir = std::env::temp_dir().join(format!(
            "white-room-db-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("nested").join(DATABASE_FILE_NAME);

        let first = Database::open(&path).unwrap();
        let version = first.schema_version().unwrap();
        drop(first);

        let reopened = Database::open(&path).unwrap();
        assert_eq!(reopened.schema_version().unwrap(), version);
        let journal_mode: String = reopened
            .lock()
            .unwrap()
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(journal_mode, "wal");

        drop(reopened);
        fs::remove_dir_all(&dir).unwrap();
    }
}
