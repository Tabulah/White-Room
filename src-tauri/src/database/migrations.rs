//! Versioned schema migrations.
//!
//! Rules:
//! - Migrations are append-only. Never edit or reorder a migration that has
//!   shipped; add a new one instead.
//! - Versions start at 1 and increase by exactly 1.
//! - Each migration runs in its own transaction together with its bookkeeping
//!   row, so it is applied completely or not at all. Migration SQL must
//!   therefore not contain `BEGIN`/`COMMIT`.
//! - A database whose schema is newer than this build understands is refused
//!   rather than modified.

use rusqlite::{params, Connection};

/// A single schema migration.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

/// All migrations known to this build, in order.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "app_metadata",
        sql: include_str!("migrations/0001_app_metadata.sql"),
    },
    Migration {
        version: 2,
        name: "study_reminders",
        sql: include_str!("migrations/0002_study_reminders.sql"),
    },
];

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error(
        "database schema version {found} is newer than this build supports ({supported}); \
         refusing to open it to avoid data loss"
    )]
    NewerSchema { found: u32, supported: u32 },

    #[error("migration list is invalid: expected version {expected}, found {found}")]
    InvalidSequence { expected: u32, found: u32 },

    #[error("migration {version} ({name}) failed: {source}")]
    Failed {
        version: u32,
        name: &'static str,
        #[source]
        source: rusqlite::Error,
    },

    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

/// Applies all pending migrations and returns the resulting schema version.
pub fn migrate(conn: &mut Connection) -> Result<u32, MigrationError> {
    apply(conn, MIGRATIONS)
}

/// Returns the highest applied migration version (0 for a new database).
pub fn current_version(conn: &Connection) -> rusqlite::Result<u32> {
    ensure_bookkeeping_table(conn)?;
    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )
}

fn ensure_bookkeeping_table(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY NOT NULL,
            name       TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        ) STRICT;",
    )
}

fn validate_sequence(migrations: &[Migration]) -> Result<(), MigrationError> {
    for (index, migration) in migrations.iter().enumerate() {
        let expected = index as u32 + 1;
        if migration.version != expected {
            return Err(MigrationError::InvalidSequence {
                expected,
                found: migration.version,
            });
        }
    }
    Ok(())
}

fn apply(conn: &mut Connection, migrations: &[Migration]) -> Result<u32, MigrationError> {
    validate_sequence(migrations)?;

    let current = current_version(conn)?;
    let supported = migrations.len() as u32;
    if current > supported {
        return Err(MigrationError::NewerSchema {
            found: current,
            supported,
        });
    }

    for migration in migrations.iter().filter(|m| m.version > current) {
        let failed = |source| MigrationError::Failed {
            version: migration.version,
            name: migration.name,
            source,
        };

        let tx = conn.transaction()?;
        tx.execute_batch(migration.sql).map_err(failed)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
            params![migration.version, migration.name],
        )
        .map_err(failed)?;
        tx.commit()?;

        log::info!(
            "applied database migration {} ({})",
            migration.version,
            migration.name
        );
    }

    Ok(current_version(conn)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Schema version produced by applying every migration in [`MIGRATIONS`].
    const LATEST_VERSION: u32 = MIGRATIONS.len() as u32;

    fn table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [name],
            |row| row.get(0),
        )
        .unwrap()
    }

    const TEST_MIGRATIONS: &[Migration] = &[
        Migration {
            version: 1,
            name: "first",
            sql: "CREATE TABLE first (id INTEGER PRIMARY KEY);",
        },
        Migration {
            version: 2,
            name: "second",
            sql: "CREATE TABLE second (id INTEGER PRIMARY KEY);",
        },
    ];

    #[test]
    fn shipped_migrations_are_sequential() {
        validate_sequence(MIGRATIONS).unwrap();
        assert_eq!(LATEST_VERSION, MIGRATIONS.last().map_or(0, |m| m.version));
    }

    #[test]
    fn migrates_fresh_database_to_latest() {
        let mut conn = Connection::open_in_memory().unwrap();
        assert_eq!(migrate(&mut conn).unwrap(), LATEST_VERSION);
        assert!(table_exists(&conn, "app_metadata"));
        assert!(table_exists(&conn, "study_reminder"));
        assert!(table_exists(&conn, "study_reminder_settings"));

        let created_at: String = conn
            .query_row(
                "SELECT value FROM app_metadata WHERE key = 'created_at'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!created_at.is_empty());
    }

    #[test]
    fn migrating_twice_is_a_no_op() {
        let mut conn = Connection::open_in_memory().unwrap();
        let first = migrate(&mut conn).unwrap();
        let second = migrate(&mut conn).unwrap();
        assert_eq!(first, second);

        let rows: u32 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(rows, LATEST_VERSION);
    }

    #[test]
    fn applies_only_pending_migrations() {
        let mut conn = Connection::open_in_memory().unwrap();
        assert_eq!(apply(&mut conn, &TEST_MIGRATIONS[..1]).unwrap(), 1);
        assert!(!table_exists(&conn, "second"));

        assert_eq!(apply(&mut conn, TEST_MIGRATIONS).unwrap(), 2);
        assert!(table_exists(&conn, "first"));
        assert!(table_exists(&conn, "second"));
    }

    #[test]
    fn refuses_newer_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply(&mut conn, TEST_MIGRATIONS).unwrap();

        let err = apply(&mut conn, &TEST_MIGRATIONS[..1]).unwrap_err();
        assert!(
            matches!(
                err,
                MigrationError::NewerSchema {
                    found: 2,
                    supported: 1
                }
            ),
            "{err}"
        );
    }

    #[test]
    fn failed_migration_is_rolled_back() {
        let broken = [
            TEST_MIGRATIONS[0],
            Migration {
                version: 2,
                name: "broken",
                // The first statement succeeds; the second fails. Neither may persist.
                sql: "CREATE TABLE partial (id INTEGER); THIS IS NOT SQL;",
            },
        ];
        let mut conn = Connection::open_in_memory().unwrap();

        let err = apply(&mut conn, &broken).unwrap_err();
        assert!(
            matches!(err, MigrationError::Failed { version: 2, .. }),
            "{err}"
        );
        assert_eq!(current_version(&conn).unwrap(), 1);
        assert!(table_exists(&conn, "first"));
        assert!(!table_exists(&conn, "partial"));
    }

    #[test]
    fn rejects_gaps_in_migration_versions() {
        let gapped = [
            TEST_MIGRATIONS[0],
            Migration {
                version: 3,
                ..TEST_MIGRATIONS[1]
            },
        ];
        let mut conn = Connection::open_in_memory().unwrap();

        let err = apply(&mut conn, &gapped).unwrap_err();
        assert!(
            matches!(
                err,
                MigrationError::InvalidSequence {
                    expected: 2,
                    found: 3
                }
            ),
            "{err}"
        );
        assert_eq!(current_version(&conn).unwrap(), 0);
    }
}
