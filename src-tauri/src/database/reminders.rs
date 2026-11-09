//! Storage for study reminders. The rules about what a valid reminder is live
//! in `sessions::reminders`; the table's CHECK constraints are a second line
//! of defence.

use rusqlite::{params, OptionalExtension};

use super::{Database, DatabaseError};

/// A stored reminder, as plain values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderRow {
    pub id: i64,
    /// Minutes after midnight, local time.
    pub minute_of_day: u16,
    /// Bit 0 = Monday … bit 6 = Sunday.
    pub weekdays: u8,
    pub label: Option<String>,
}

impl Database {
    pub fn reminders_enabled(&self) -> Result<bool, DatabaseError> {
        let conn = self.lock()?;
        let enabled = conn
            .query_row(
                "SELECT enabled FROM study_reminder_settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(enabled.unwrap_or(false))
    }

    pub fn set_reminders_enabled(&self, enabled: bool) -> Result<(), DatabaseError> {
        self.lock()?.execute(
            "INSERT INTO study_reminder_settings (id, enabled) VALUES (1, ?1)
             ON CONFLICT (id) DO UPDATE SET enabled = excluded.enabled",
            [enabled],
        )?;
        Ok(())
    }

    /// All reminders, earliest time of day first.
    pub fn reminders(&self) -> Result<Vec<ReminderRow>, DatabaseError> {
        let conn = self.lock()?;
        let mut statement = conn.prepare(
            "SELECT id, minute_of_day, weekdays, label FROM study_reminder
             ORDER BY minute_of_day, id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(ReminderRow {
                id: row.get(0)?,
                minute_of_day: row.get(1)?,
                weekdays: row.get(2)?,
                label: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Stores a new reminder and returns its id.
    pub fn insert_reminder(
        &self,
        minute_of_day: u16,
        weekdays: u8,
        label: Option<&str>,
    ) -> Result<i64, DatabaseError> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO study_reminder (minute_of_day, weekdays, label) VALUES (?1, ?2, ?3)",
            params![minute_of_day, weekdays, label],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Deletes a reminder. Returns `false` if there was none with that id.
    pub fn delete_reminder(&self, id: i64) -> Result<bool, DatabaseError> {
        let deleted = self
            .lock()?
            .execute("DELETE FROM study_reminder WHERE id = ?1", [id])?;
        Ok(deleted > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn reminders_start_switched_off_and_empty() {
        let db = db();
        assert!(!db.reminders_enabled().unwrap());
        assert!(db.reminders().unwrap().is_empty());
    }

    #[test]
    fn switching_on_and_off_is_stored() {
        let db = db();
        db.set_reminders_enabled(true).unwrap();
        assert!(db.reminders_enabled().unwrap());
        db.set_reminders_enabled(false).unwrap();
        assert!(!db.reminders_enabled().unwrap());
    }

    #[test]
    fn inserts_lists_by_time_and_deletes() {
        let db = db();
        let evening = db.insert_reminder(18 * 60, 0b001_1111, None).unwrap();
        let morning = db
            .insert_reminder(9 * 60 + 30, 0b110_0000, Some("Chemistry"))
            .unwrap();

        let rows = db.reminders().unwrap();
        assert_eq!(
            rows,
            [
                ReminderRow {
                    id: morning,
                    minute_of_day: 570,
                    weekdays: 0b110_0000,
                    label: Some("Chemistry".into()),
                },
                ReminderRow {
                    id: evening,
                    minute_of_day: 1080,
                    weekdays: 0b001_1111,
                    label: None,
                },
            ]
        );

        assert!(db.delete_reminder(evening).unwrap());
        assert!(!db.delete_reminder(evening).unwrap(), "already gone");
        assert_eq!(db.reminders().unwrap().len(), 1);
    }

    #[test]
    fn the_table_rejects_invalid_rows_itself() {
        let db = db();
        assert!(
            db.insert_reminder(1440, 1, None).is_err(),
            "time past midnight"
        );
        assert!(db.insert_reminder(0, 0, None).is_err(), "no days");
        assert!(db.insert_reminder(0, 128, None).is_err(), "an eighth day");
        assert!(db.insert_reminder(0, 1, Some("")).is_err(), "empty label");
        assert!(
            db.insert_reminder(0, 1, Some(&"x".repeat(61))).is_err(),
            "long label"
        );
        assert!(db.reminders().unwrap().is_empty());
    }
}
