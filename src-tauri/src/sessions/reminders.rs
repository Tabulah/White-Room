//! Study reminders: notifications at times the user chooses, on the days they
//! choose, that say it is time to study.
//!
//! - Off until the user turns them on. macOS asks for notification permission
//!   at that point, once.
//! - Scheduled with macOS as repeating notifications, so they arrive even
//!   while WHITE ROOM is closed. macOS does not show an app's notifications
//!   while that app is the active one, so they never interrupt the workspace.
//! - The wording is calm and never mentions streaks, missed days, or counts
//!   (see [`plan`]).
//!
//! The database is the source of truth. Every change, and every launch,
//! replaces what macOS has scheduled with what the database says
//! ([`Reminders::sync`]), so the two cannot drift apart for long.

mod model;
mod plan;

use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;

pub use model::{NewReminder, Reminder, TimeOfDay, Weekday, Weekdays, MAX_LABEL_CHARS};
pub use plan::{plan, BODY, DEFAULT_TITLE};

use crate::{
    database::{reminders::ReminderRow, Database, DatabaseError},
    platform::notifications::{Availability, NativeScheduler, NotificationScheduler, MAX_PENDING},
};

#[derive(Debug, thiserror::Error)]
pub enum ReminderError {
    #[error("Choose a time between 00:00 and 23:59.")]
    InvalidTime,

    #[error("Choose at least one day.")]
    NoDays,

    #[error("Keep the label to {MAX_LABEL_CHARS} characters or fewer.")]
    LabelTooLong,

    #[error("The label can't contain line breaks or other control characters.")]
    LabelInvalid,

    #[error("There is already a reminder at {time} on {day}.")]
    Duplicate { time: String, day: String },

    #[error(
        "macOS keeps at most {MAX_PENDING} reminder times a week for an app; this would make {0}."
    )]
    TooMany(usize),

    #[error("That reminder no longer exists.")]
    NotFound(i64),

    #[error("A stored reminder is invalid (id {0}).")]
    Corrupt(i64),

    #[error("Reminders could not be loaded or saved.")]
    Database(#[from] DatabaseError),
}

/// Whether the reminders will actually arrive. Never claims more than macOS
/// confirmed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Delivery {
    /// Switched off; nothing is scheduled.
    Off,
    /// macOS accepted every reminder.
    Scheduled,
    /// Switched on, but the user has not allowed WHITE ROOM to show
    /// notifications, so nothing is scheduled.
    NotAllowed,
    /// This platform or build cannot schedule notifications.
    Unavailable { reason: String },
    /// Scheduling failed.
    Failed { reason: String },
}

/// Everything the reminders screen shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderState {
    /// The user's choice, kept even when delivery is not possible.
    pub enabled: bool,
    pub reminders: Vec<Reminder>,
    pub delivery: Delivery,
    /// Title of reminders without a label, so the screen can show it.
    pub default_title: &'static str,
}

/// Managed Tauri state. Every method talks to the operating system and may
/// wait for the user to answer the permission prompt, so call them off the
/// main thread and off the async runtime.
pub struct Reminders {
    scheduler: Box<dyn NotificationScheduler>,
    /// Serialises changes, so that checking a new reminder against the
    /// existing ones, storing it, and scheduling happen as one step.
    busy: Mutex<()>,
}

impl Reminders {
    pub fn new(scheduler: impl NotificationScheduler + 'static) -> Self {
        Self {
            scheduler: Box::new(scheduler),
            busy: Mutex::new(()),
        }
    }

    /// Uses the operating system's notifications.
    pub fn native() -> Self {
        Self::new(NativeScheduler)
    }

    /// Brings macOS in line with the stored reminders and returns them.
    pub fn sync(&self, db: &Database) -> Result<ReminderState, ReminderError> {
        let _busy = self.lock();
        self.sync_locked(db)
    }

    /// Switches reminders on or off. Switching on may show macOS's
    /// permission prompt.
    pub fn set_enabled(
        &self,
        db: &Database,
        enabled: bool,
    ) -> Result<ReminderState, ReminderError> {
        let _busy = self.lock();
        db.set_reminders_enabled(enabled)?;
        log::info!(
            "study reminders switched {}",
            if enabled { "on" } else { "off" }
        );
        self.sync_locked(db)
    }

    pub fn add(&self, db: &Database, new: NewReminder) -> Result<ReminderState, ReminderError> {
        let _busy = self.lock();
        let valid = model::validate(new, &load(db)?)?;
        db.insert_reminder(
            valid.time.minute_of_day(),
            valid.weekdays.bits(),
            valid.label.as_deref(),
        )?;
        self.sync_locked(db)
    }

    pub fn remove(&self, db: &Database, id: i64) -> Result<ReminderState, ReminderError> {
        let _busy = self.lock();
        if !db.delete_reminder(id)? {
            return Err(ReminderError::NotFound(id));
        }
        self.sync_locked(db)
    }

    fn sync_locked(&self, db: &Database) -> Result<ReminderState, ReminderError> {
        let enabled = db.reminders_enabled()?;
        let reminders = load(db)?;
        let delivery = self.deliver(enabled, &reminders);
        Ok(ReminderState {
            enabled,
            reminders,
            delivery,
            default_title: DEFAULT_TITLE,
        })
    }

    fn deliver(&self, enabled: bool, reminders: &[Reminder]) -> Delivery {
        if let Availability::Unavailable(reason) = self.scheduler.availability() {
            return Delivery::Unavailable { reason };
        }
        if !enabled {
            return match self.scheduler.cancel_all() {
                Ok(()) => Delivery::Off,
                Err(err) => failed("could not remove scheduled reminders", err),
            };
        }
        match self.scheduler.request_permission() {
            Ok(true) => {}
            Ok(false) => {
                if let Err(err) = self.scheduler.cancel_all() {
                    log::warn!("could not remove scheduled reminders: {err}");
                }
                return Delivery::NotAllowed;
            }
            Err(err) => return failed("could not ask for notification permission", err),
        }
        match self.scheduler.replace_all(&plan(reminders)) {
            Ok(()) => Delivery::Scheduled,
            Err(err) => failed("could not schedule reminders", err),
        }
    }

    /// Only serialises; the guarded data is `()`, so a panic elsewhere cannot
    /// have left anything inconsistent.
    fn lock(&self) -> MutexGuard<'_, ()> {
        self.busy.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn failed(what: &str, err: impl std::fmt::Display) -> Delivery {
    log::warn!("{what}: {err}");
    Delivery::Failed {
        reason: err.to_string(),
    }
}

fn load(db: &Database) -> Result<Vec<Reminder>, ReminderError> {
    db.reminders()?.into_iter().map(from_row).collect()
}

fn from_row(row: ReminderRow) -> Result<Reminder, ReminderError> {
    let time =
        TimeOfDay::from_minute_of_day(row.minute_of_day).ok_or(ReminderError::Corrupt(row.id))?;
    let weekdays = Weekdays::from_bits(row.weekdays).ok_or(ReminderError::Corrupt(row.id))?;
    Ok(Reminder {
        id: row.id,
        hour: time.hour(),
        minute: time.minute(),
        weekdays,
        label: row.label,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::platform::notifications::{NotificationError, WeeklyNotification};

    /// What the fake operating system currently has scheduled, and how it
    /// answers.
    #[derive(Default)]
    struct FakeOs {
        unavailable: bool,
        permission: Option<bool>,
        refuse: bool,
        prompts: usize,
        scheduled: Vec<WeeklyNotification>,
    }

    #[derive(Clone, Default)]
    struct FakeScheduler(Arc<Mutex<FakeOs>>);

    impl FakeScheduler {
        fn allowing(allow: bool) -> Self {
            let fake = Self::default();
            fake.0.lock().unwrap().permission = Some(allow);
            fake
        }

        fn os(&self) -> std::sync::MutexGuard<'_, FakeOs> {
            self.0.lock().unwrap()
        }
    }

    impl NotificationScheduler for FakeScheduler {
        fn availability(&self) -> Availability {
            if self.os().unavailable {
                Availability::Unavailable("not here".into())
            } else {
                Availability::Available
            }
        }

        fn request_permission(&self) -> Result<bool, NotificationError> {
            let mut os = self.os();
            os.prompts += 1;
            os.permission
                .ok_or(NotificationError::NoAnswer(std::time::Duration::ZERO))
        }

        fn replace_all(
            &self,
            notifications: &[WeeklyNotification],
        ) -> Result<(), NotificationError> {
            let mut os = self.os();
            os.scheduled.clear();
            if os.refuse {
                return Err(NotificationError::Refused {
                    refused: notifications.len(),
                    total: notifications.len(),
                });
            }
            os.scheduled = notifications.to_vec();
            Ok(())
        }

        fn cancel_all(&self) -> Result<(), NotificationError> {
            self.os().scheduled.clear();
            Ok(())
        }
    }

    fn setup(scheduler: &FakeScheduler) -> (Reminders, Database) {
        (
            Reminders::new(scheduler.clone()),
            Database::open_in_memory().unwrap(),
        )
    }

    fn weekdays_at(hour: u8, label: Option<&str>) -> NewReminder {
        NewReminder {
            hour,
            minute: 0,
            weekdays: vec![
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri,
            ],
            label: label.map(Into::into),
        }
    }

    #[test]
    fn starts_off_with_nothing_scheduled_and_no_prompt() {
        let os = FakeScheduler::allowing(true);
        let (reminders, db) = setup(&os);

        let state = reminders.sync(&db).unwrap();

        assert!(!state.enabled);
        assert!(state.reminders.is_empty());
        assert_eq!(state.delivery, Delivery::Off);
        assert_eq!(os.os().prompts, 0, "never asks before being switched on");
    }

    #[test]
    fn reminders_added_while_off_are_stored_but_not_scheduled() {
        let os = FakeScheduler::allowing(true);
        let (reminders, db) = setup(&os);

        let state = reminders.add(&db, weekdays_at(18, None)).unwrap();

        assert_eq!(state.reminders.len(), 1);
        assert_eq!(state.delivery, Delivery::Off);
        assert!(os.os().scheduled.is_empty());
    }

    #[test]
    fn switching_on_schedules_every_reminder() {
        let os = FakeScheduler::allowing(true);
        let (reminders, db) = setup(&os);
        reminders.add(&db, weekdays_at(18, None)).unwrap();
        reminders
            .add(&db, weekdays_at(9, Some("Chemistry")))
            .unwrap();

        let state = reminders.set_enabled(&db, true).unwrap();

        assert!(state.enabled);
        assert_eq!(state.delivery, Delivery::Scheduled);
        assert_eq!(os.os().scheduled, plan(&state.reminders));
        assert_eq!(os.os().scheduled.len(), 10);
        assert_eq!(
            state.reminders[0].label.as_deref(),
            Some("Chemistry"),
            "9:00 first"
        );
    }

    #[test]
    fn changes_while_on_are_scheduled_at_once() {
        let os = FakeScheduler::allowing(true);
        let (reminders, db) = setup(&os);
        reminders.set_enabled(&db, true).unwrap();

        let state = reminders.add(&db, weekdays_at(18, None)).unwrap();
        assert_eq!(os.os().scheduled.len(), 5);

        let state = reminders.remove(&db, state.reminders[0].id).unwrap();
        assert!(state.reminders.is_empty());
        assert!(os.os().scheduled.is_empty());
        assert_eq!(state.delivery, Delivery::Scheduled);
    }

    #[test]
    fn switching_off_removes_everything_from_macos_but_keeps_the_reminders() {
        let os = FakeScheduler::allowing(true);
        let (reminders, db) = setup(&os);
        reminders.add(&db, weekdays_at(18, None)).unwrap();
        reminders.set_enabled(&db, true).unwrap();

        let state = reminders.set_enabled(&db, false).unwrap();

        assert_eq!(state.delivery, Delivery::Off);
        assert_eq!(state.reminders.len(), 1);
        assert!(os.os().scheduled.is_empty());
    }

    #[test]
    fn says_so_when_notifications_are_not_allowed() {
        let os = FakeScheduler::allowing(false);
        let (reminders, db) = setup(&os);
        reminders.add(&db, weekdays_at(18, None)).unwrap();

        let state = reminders.set_enabled(&db, true).unwrap();

        assert!(state.enabled, "the user's choice is kept");
        assert_eq!(state.delivery, Delivery::NotAllowed);
        assert!(os.os().scheduled.is_empty());

        // Allowed later in System Settings: the next sync schedules them.
        os.os().permission = Some(true);
        assert_eq!(reminders.sync(&db).unwrap().delivery, Delivery::Scheduled);
        assert_eq!(os.os().scheduled.len(), 5);
    }

    #[test]
    fn says_so_when_notifications_are_unavailable() {
        let os = FakeScheduler::allowing(true);
        os.os().unavailable = true;
        let (reminders, db) = setup(&os);

        let state = reminders.set_enabled(&db, true).unwrap();

        assert!(state.enabled);
        assert_eq!(
            state.delivery,
            Delivery::Unavailable {
                reason: "not here".into()
            }
        );
        assert_eq!(os.os().prompts, 0);
    }

    #[test]
    fn reports_a_refusal_instead_of_claiming_success() {
        let os = FakeScheduler::allowing(true);
        os.os().refuse = true;
        let (reminders, db) = setup(&os);
        reminders.add(&db, weekdays_at(18, None)).unwrap();

        let state = reminders.set_enabled(&db, true).unwrap();

        assert!(matches!(state.delivery, Delivery::Failed { .. }));
    }

    #[test]
    fn invalid_reminders_change_nothing() {
        let os = FakeScheduler::allowing(true);
        let (reminders, db) = setup(&os);
        reminders.set_enabled(&db, true).unwrap();
        reminders.add(&db, weekdays_at(18, None)).unwrap();

        let duplicate = reminders.add(&db, weekdays_at(18, Some("Again")));
        assert!(matches!(duplicate, Err(ReminderError::Duplicate { .. })));
        let no_days = reminders.add(
            &db,
            NewReminder {
                weekdays: vec![],
                ..weekdays_at(7, None)
            },
        );
        assert!(matches!(no_days, Err(ReminderError::NoDays)));

        assert_eq!(db.reminders().unwrap().len(), 1);
        assert_eq!(os.os().scheduled.len(), 5);
    }

    #[test]
    fn removing_an_unknown_reminder_is_an_error() {
        let os = FakeScheduler::allowing(true);
        let (reminders, db) = setup(&os);
        assert!(matches!(
            reminders.remove(&db, 42),
            Err(ReminderError::NotFound(42))
        ));
    }

    #[test]
    fn serialises_delivery_for_the_frontend() {
        let json = |delivery: Delivery| serde_json::to_value(delivery).unwrap();
        assert_eq!(json(Delivery::Off), serde_json::json!({ "status": "off" }));
        assert_eq!(
            json(Delivery::NotAllowed),
            serde_json::json!({ "status": "notAllowed" })
        );
        assert_eq!(
            json(Delivery::Unavailable {
                reason: "why".into()
            }),
            serde_json::json!({ "status": "unavailable", "reason": "why" })
        );
    }
}
