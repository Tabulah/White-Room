-- Study reminders (Phase 2).
--
-- One row per reminder: a local time of day on a set of weekdays. Each
-- weekday becomes one repeating notification scheduled with macOS.
CREATE TABLE study_reminder (
    id            INTEGER PRIMARY KEY,
    -- Minutes after midnight, local time.
    minute_of_day INTEGER NOT NULL CHECK (minute_of_day BETWEEN 0 AND 1439),
    -- Bit 0 = Monday … bit 6 = Sunday; at least one day.
    weekdays      INTEGER NOT NULL CHECK (weekdays BETWEEN 1 AND 127),
    -- Shown instead of the default title; NULL for the default.
    label         TEXT CHECK (label IS NULL OR length(label) BETWEEN 1 AND 60),
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

-- Whether reminders are switched on. Exactly one row; off until the user
-- turns them on.
CREATE TABLE study_reminder_settings (
    id      INTEGER PRIMARY KEY CHECK (id = 1),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1))
) STRICT;

INSERT INTO study_reminder_settings (id, enabled) VALUES (1, 0);
