# Study Reminders

WHITE ROOM can remind you to study at times you choose. Open **Study reminders…** on the entry screen.

## How they work

- **Off until you turn them on.** The first time you turn them on, macOS asks whether WHITE ROOM may show notifications. It asks only once; afterwards you can change your answer in System Settings › Notifications › WHITE ROOM.
- **Days and times you choose.** Each reminder has a time and one or more days, for example weekdays at 18:00 and Saturdays at 10:00. Times are local and follow your Mac when it changes time zone.
- **Delivered by macOS.** Reminders are handed to macOS as repeating notifications, so they arrive even while WHITE ROOM is closed.
- **Quiet while you are studying.** macOS keeps an app's own notifications silent while that app is the one in front, so a reminder never interrupts the workspace.
- **Calm wording.** A reminder says "Time to study" and "WHITE ROOM is ready when you are", or the label you gave it instead of "Time to study". Reminders never mention streaks, missed days, or counts.

The screen always says whether reminders will actually arrive: on and scheduled, off, not allowed by macOS (with where to change that), or not available (for example in a development build). It never claims they are scheduled unless macOS accepted them.

## Limits

- At most 64 reminder times a week in total, because macOS keeps at most 64 pending notifications per app. A reminder on weekdays counts as 5.
- Two reminders cannot share a time on the same day.
- Labels are up to 60 characters, on one line.

## What is stored

Reminders and the on/off switch are stored in WHITE ROOM's local database (see the README) and nowhere else. The database is the source of truth: every change, and every launch, replaces what macOS has scheduled with what the database says. Turning reminders off removes them from macOS but keeps them in the list.

## For developers

- `src-tauri/src/sessions/reminders.rs`: the service, its errors, and what the screen is told about delivery. `reminders/model.rs` validates; `reminders/plan.rs` turns reminders into notifications.
- `src-tauri/src/platform/notifications.rs`: the `NotificationScheduler` trait and its `UNUserNotificationCenter` implementation. macOS aborts a process that asks for the notification center from outside an app bundle, so the implementation checks for one first; `tauri dev` runs report reminders as unavailable.
- Commands: `get_reminders`, `set_reminders_enabled`, `add_reminder`, `remove_reminder`. They run on a blocking thread because they can wait for the permission prompt.
- No notification action buttons (such as snooze): those need an Objective-C delegate class, which needs `unsafe` code.
