//! What a study reminder is, and what makes one valid.

use std::fmt;

use serde::{Deserialize, Serialize, Serializer};

use super::ReminderError;
use crate::platform::notifications::MAX_PENDING;

/// Longest label, in characters.
pub const MAX_LABEL_CHARS: usize = 60;

/// A day of the week. Serialised as `"mon"` … `"sun"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Weekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

impl Weekday {
    pub const ALL: [Self; 7] = [
        Self::Mon,
        Self::Tue,
        Self::Wed,
        Self::Thu,
        Self::Fri,
        Self::Sat,
        Self::Sun,
    ];

    const fn bit(self) -> u8 {
        1 << self as u8
    }

    /// `NSCalendar` numbering: 1 = Sunday … 7 = Saturday.
    pub const fn calendar_number(self) -> u8 {
        match self {
            Self::Sun => 1,
            Self::Mon => 2,
            Self::Tue => 3,
            Self::Wed => 4,
            Self::Thu => 5,
            Self::Fri => 6,
            Self::Sat => 7,
        }
    }

    pub const fn short_name(self) -> &'static str {
        match self {
            Self::Mon => "Mon",
            Self::Tue => "Tue",
            Self::Wed => "Wed",
            Self::Thu => "Thu",
            Self::Fri => "Fri",
            Self::Sat => "Sat",
            Self::Sun => "Sun",
        }
    }
}

/// A non-empty set of weekdays. Stored as a 7-bit mask (bit 0 = Monday);
/// serialised as a list of [`Weekday`]s from Monday to Sunday.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weekdays(u8);

impl Weekdays {
    /// `None` for an empty set or bits beyond Sunday.
    pub const fn from_bits(bits: u8) -> Option<Self> {
        if bits == 0 || bits >= 1 << 7 {
            None
        } else {
            Some(Self(bits))
        }
    }

    /// `None` if `days` is empty. Repeated days count once.
    pub fn from_days(days: &[Weekday]) -> Option<Self> {
        Self::from_bits(days.iter().fold(0, |bits, day| bits | day.bit()))
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub fn iter(self) -> impl Iterator<Item = Weekday> {
        Weekday::ALL
            .into_iter()
            .filter(move |day| self.0 & day.bit() != 0)
    }

    /// Number of days; at least 1.
    pub const fn count(self) -> usize {
        self.0.count_ones() as usize
    }

    pub const fn overlaps(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl Serialize for Weekdays {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

/// A local time of day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeOfDay {
    hour: u8,
    minute: u8,
}

impl TimeOfDay {
    pub const fn new(hour: u8, minute: u8) -> Option<Self> {
        if hour < 24 && minute < 60 {
            Some(Self { hour, minute })
        } else {
            None
        }
    }

    pub const fn from_minute_of_day(minutes: u16) -> Option<Self> {
        if minutes < 24 * 60 {
            Some(Self {
                hour: (minutes / 60) as u8,
                minute: (minutes % 60) as u8,
            })
        } else {
            None
        }
    }

    pub const fn hour(self) -> u8 {
        self.hour
    }

    pub const fn minute(self) -> u8 {
        self.minute
    }

    pub const fn minute_of_day(self) -> u16 {
        self.hour as u16 * 60 + self.minute as u16
    }
}

impl fmt::Display for TimeOfDay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hour, self.minute)
    }
}

/// A reminder as the frontend submits it. Validated by [`validate`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewReminder {
    pub hour: u8,
    pub minute: u8,
    pub weekdays: Vec<Weekday>,
    #[serde(default)]
    pub label: Option<String>,
}

/// A stored reminder, as sent to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub id: i64,
    pub hour: u8,
    pub minute: u8,
    pub weekdays: Weekdays,
    /// Shown instead of the default title.
    pub label: Option<String>,
}

impl Reminder {
    pub fn time(&self) -> TimeOfDay {
        // Constructed only from validated values.
        TimeOfDay::new(self.hour, self.minute).unwrap_or(TimeOfDay { hour: 0, minute: 0 })
    }
}

/// A new reminder that passed validation, ready to store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidReminder {
    pub time: TimeOfDay,
    pub weekdays: Weekdays,
    pub label: Option<String>,
}

/// Checks a new reminder on its own and against the existing ones.
pub fn validate(new: NewReminder, existing: &[Reminder]) -> Result<ValidReminder, ReminderError> {
    let time = TimeOfDay::new(new.hour, new.minute).ok_or(ReminderError::InvalidTime)?;
    let weekdays = Weekdays::from_days(&new.weekdays).ok_or(ReminderError::NoDays)?;
    let label = validate_label(new.label)?;

    if let Some(clash) = existing
        .iter()
        .find(|reminder| reminder.time() == time && reminder.weekdays.overlaps(weekdays))
    {
        let day = weekdays
            .iter()
            .find(|day| clash.weekdays.iter().any(|other| other == *day))
            .map_or("", Weekday::short_name);
        return Err(ReminderError::Duplicate {
            time: time.to_string(),
            day: day.into(),
        });
    }

    let total = existing
        .iter()
        .map(|reminder| reminder.weekdays.count())
        .sum::<usize>()
        + weekdays.count();
    if total > MAX_PENDING {
        return Err(ReminderError::TooMany(total));
    }

    Ok(ValidReminder {
        time,
        weekdays,
        label,
    })
}

/// Trims the label; an empty one means "use the default".
fn validate_label(label: Option<String>) -> Result<Option<String>, ReminderError> {
    let Some(label) = label else {
        return Ok(None);
    };
    let label = label.trim();
    if label.is_empty() {
        return Ok(None);
    }
    if label.chars().any(char::is_control) {
        return Err(ReminderError::LabelInvalid);
    }
    if label.chars().count() > MAX_LABEL_CHARS {
        return Err(ReminderError::LabelTooLong);
    }
    Ok(Some(label.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::Weekday::*;
    use super::*;

    fn new(hour: u8, minute: u8, weekdays: &[Weekday]) -> NewReminder {
        NewReminder {
            hour,
            minute,
            weekdays: weekdays.to_vec(),
            label: None,
        }
    }

    fn stored(id: i64, hour: u8, minute: u8, weekdays: &[Weekday]) -> Reminder {
        Reminder {
            id,
            hour,
            minute,
            weekdays: Weekdays::from_days(weekdays).unwrap(),
            label: None,
        }
    }

    #[test]
    fn accepts_a_reminder_and_normalises_its_days() {
        let valid = validate(new(18, 0, &[Fri, Mon, Mon]), &[]).unwrap();
        assert_eq!(valid.time.to_string(), "18:00");
        assert_eq!(valid.weekdays.iter().collect::<Vec<_>>(), [Mon, Fri]);
        assert_eq!(valid.label, None);
    }

    #[test]
    fn rejects_impossible_times() {
        for (hour, minute) in [(24, 0), (23, 60), (255, 255)] {
            assert!(matches!(
                validate(new(hour, minute, &[Mon]), &[]),
                Err(ReminderError::InvalidTime)
            ));
        }
        assert!(validate(new(23, 59, &[Mon]), &[]).is_ok());
        assert!(validate(new(0, 0, &[Mon]), &[]).is_ok());
    }

    #[test]
    fn needs_at_least_one_day() {
        assert!(matches!(
            validate(new(9, 0, &[]), &[]),
            Err(ReminderError::NoDays)
        ));
    }

    #[test]
    fn trims_labels_and_treats_blank_as_default() {
        let mut reminder = new(9, 0, &[Mon]);
        reminder.label = Some("  Calculus revision  ".into());
        assert_eq!(
            validate(reminder.clone(), &[]).unwrap().label.as_deref(),
            Some("Calculus revision")
        );
        reminder.label = Some("   ".into());
        assert_eq!(validate(reminder, &[]).unwrap().label, None);
    }

    #[test]
    fn limits_labels_by_characters_not_bytes() {
        let mut reminder = new(9, 0, &[Mon]);
        reminder.label = Some("é".repeat(MAX_LABEL_CHARS));
        assert!(validate(reminder.clone(), &[]).is_ok());
        reminder.label = Some("é".repeat(MAX_LABEL_CHARS + 1));
        assert!(matches!(
            validate(reminder, &[]),
            Err(ReminderError::LabelTooLong)
        ));
    }

    #[test]
    fn rejects_control_characters_in_labels() {
        let mut reminder = new(9, 0, &[Mon]);
        reminder.label = Some("Line one\nLine two".into());
        assert!(matches!(
            validate(reminder, &[]),
            Err(ReminderError::LabelInvalid)
        ));
    }

    #[test]
    fn rejects_a_second_reminder_at_the_same_time_and_day() {
        let existing = [stored(1, 18, 0, &[Mon, Tue, Wed])];

        let err = validate(new(18, 0, &[Sat, Wed]), &existing).unwrap_err();
        assert_eq!(
            err.to_string(),
            "There is already a reminder at 18:00 on Wed."
        );
        assert!(validate(new(18, 0, &[Sat]), &existing).is_ok(), "other day");
        assert!(
            validate(new(18, 1, &[Mon]), &existing).is_ok(),
            "other time"
        );
    }

    #[test]
    fn keeps_within_the_macos_limit() {
        // 9 reminders every day: 63 weekly notifications.
        let existing: Vec<_> = (0..9)
            .map(|hour| stored(hour.into(), hour, 0, &Weekday::ALL))
            .collect();

        assert!(validate(new(12, 0, &[Mon]), &existing).is_ok(), "64th");
        assert!(matches!(
            validate(new(12, 0, &[Mon, Tue]), &existing),
            Err(ReminderError::TooMany(65))
        ));
    }

    #[test]
    fn weekday_sets_reject_empty_and_out_of_range_masks() {
        assert_eq!(Weekdays::from_bits(0), None);
        assert_eq!(Weekdays::from_bits(0x80), None);
        assert_eq!(Weekdays::from_bits(0x7f).unwrap().count(), 7);
    }

    #[test]
    fn calendar_numbers_start_on_sunday() {
        let numbers: Vec<_> = Weekday::ALL.iter().map(|d| d.calendar_number()).collect();
        assert_eq!(numbers, [2, 3, 4, 5, 6, 7, 1]);
    }

    #[test]
    fn times_convert_to_and_from_minutes() {
        let time = TimeOfDay::new(9, 30).unwrap();
        assert_eq!(time.minute_of_day(), 570);
        assert_eq!(TimeOfDay::from_minute_of_day(570), Some(time));
        assert_eq!(TimeOfDay::from_minute_of_day(1440), None);
    }

    #[test]
    fn serialises_for_the_frontend() {
        let json = serde_json::to_value(stored(3, 7, 5, &[Sat, Sun])).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "id": 3, "hour": 7, "minute": 5, "weekdays": ["sat", "sun"], "label": null
            })
        );
    }

    #[test]
    fn parses_what_the_frontend_sends_and_nothing_else() {
        let parsed: NewReminder = serde_json::from_value(serde_json::json!({
            "hour": 18, "minute": 0, "weekdays": ["mon", "fri"], "label": "Physics"
        }))
        .unwrap();
        assert_eq!(parsed.weekdays, [Mon, Fri]);

        let unknown_day = serde_json::from_value::<NewReminder>(serde_json::json!({
            "hour": 18, "minute": 0, "weekdays": ["funday"]
        }));
        assert!(unknown_day.is_err());

        let extra_field = serde_json::from_value::<NewReminder>(serde_json::json!({
            "hour": 18, "minute": 0, "weekdays": ["mon"], "sound": "loud"
        }));
        assert!(extra_field.is_err());
    }
}
