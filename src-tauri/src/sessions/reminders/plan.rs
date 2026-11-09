//! Turns stored reminders into the notifications macOS should deliver.

use super::model::Reminder;
use crate::platform::notifications::WeeklyNotification;

/// Title of a reminder without a label of its own.
pub const DEFAULT_TITLE: &str = "Time to study";

/// Body of every reminder. Calm on purpose: reminders never mention streaks,
/// missed days, or counts.
pub const BODY: &str = "WHITE ROOM is ready when you are.";

/// One repeating notification per reminder and weekday, in a stable order.
pub fn plan(reminders: &[Reminder]) -> Vec<WeeklyNotification> {
    reminders
        .iter()
        .flat_map(|reminder| {
            reminder.weekdays.iter().map(move |day| WeeklyNotification {
                identifier: format!(
                    "study-reminder-{}-{}",
                    reminder.id,
                    day.short_name().to_lowercase()
                ),
                weekday: day.calendar_number(),
                hour: reminder.hour,
                minute: reminder.minute,
                title: reminder
                    .label
                    .clone()
                    .unwrap_or_else(|| DEFAULT_TITLE.into()),
                body: BODY.into(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::sessions::reminders::model::{Weekday, Weekdays};

    fn reminder(id: i64, days: &[Weekday], label: Option<&str>) -> Reminder {
        Reminder {
            id,
            hour: 18,
            minute: 30,
            weekdays: Weekdays::from_days(days).unwrap(),
            label: label.map(Into::into),
        }
    }

    #[test]
    fn one_notification_per_reminder_and_day() {
        let planned = plan(&[
            reminder(1, &[Weekday::Mon, Weekday::Sun], None),
            reminder(2, &[Weekday::Sat], Some("Calculus revision")),
        ]);

        let summary: Vec<_> = planned
            .iter()
            .map(|n| (n.identifier.as_str(), n.weekday, n.title.as_str()))
            .collect();
        assert_eq!(
            summary,
            [
                ("study-reminder-1-mon", 2, DEFAULT_TITLE),
                ("study-reminder-1-sun", 1, DEFAULT_TITLE),
                ("study-reminder-2-sat", 7, "Calculus revision"),
            ]
        );
        assert!(planned
            .iter()
            .all(|n| n.hour == 18 && n.minute == 30 && n.body == BODY));
    }

    #[test]
    fn identifiers_are_unique() {
        let planned = plan(&[
            reminder(1, &Weekday::ALL, None),
            reminder(2, &Weekday::ALL, None),
        ]);
        let ids: HashSet<_> = planned.iter().map(|n| &n.identifier).collect();
        assert_eq!(ids.len(), 14);
    }

    #[test]
    fn the_wording_stays_calm() {
        for text in [DEFAULT_TITLE, BODY] {
            let lower = text.to_lowercase();
            for word in ["streak", "missed", "behind", "don't", "!"] {
                assert!(!lower.contains(word), "{text:?} contains {word:?}");
            }
        }
    }

    #[test]
    fn nothing_to_plan_without_reminders() {
        assert!(plan(&[]).is_empty());
    }
}
