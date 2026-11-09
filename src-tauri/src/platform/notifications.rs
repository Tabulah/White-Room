//! Scheduled notifications, delivered by the operating system.
//!
//! On macOS these go through `UNUserNotificationCenter` as repeating calendar
//! notifications. macOS keeps and delivers them itself, even while WHITE ROOM
//! is not running, and asks the user for permission the first time. The user
//! stays in control in System Settings › Notifications.
//!
//! macOS keeps at most [`MAX_PENDING`] pending notifications per app.

use std::time::Duration;

/// macOS keeps at most this many pending notifications per app; the rest are
/// silently dropped.
pub const MAX_PENDING: usize = 64;

/// A notification that repeats every week at a fixed local time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeeklyNotification {
    /// Unique among the app's notifications.
    pub identifier: String,
    /// 1 = Sunday … 7 = Saturday, as in `NSCalendar`.
    pub weekday: u8,
    pub hour: u8,
    pub minute: u8,
    pub title: String,
    pub body: String,
}

/// Whether this build, in its current form, can schedule notifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Available,
    /// With a reason a user can read.
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotificationError {
    #[error("{0}")]
    Unavailable(String),

    #[error("macOS did not answer within {} seconds", .0.as_secs())]
    NoAnswer(Duration),

    #[error("macOS refused {refused} of {total} notifications")]
    Refused { refused: usize, total: usize },

    #[error("macOS keeps at most {MAX_PENDING} notifications per app; asked for {0}")]
    TooMany(usize),
}

/// Schedules notifications with the operating system.
pub trait NotificationScheduler: Send + Sync {
    fn availability(&self) -> Availability;

    /// Asks the user for permission to show notifications, the first time
    /// only. After the user has answered, macOS returns that answer without
    /// asking again; it can be changed in System Settings › Notifications.
    /// Blocks until there is an answer.
    fn request_permission(&self) -> Result<bool, NotificationError>;

    /// Replaces every pending notification of this app with `notifications`.
    /// Blocks until the operating system has accepted or refused them.
    fn replace_all(&self, notifications: &[WeeklyNotification]) -> Result<(), NotificationError>;

    /// Removes every pending notification of this app. Needs no permission.
    fn cancel_all(&self) -> Result<(), NotificationError>;
}

#[cfg(target_os = "macos")]
pub use macos::UserNotifications as NativeScheduler;
#[cfg(not(target_os = "macos"))]
pub use unsupported::NoScheduler as NativeScheduler;

#[cfg(target_os = "macos")]
mod macos {
    use std::{sync::mpsc, time::Duration};

    use block2::RcBlock;
    use objc2::{rc::Retained, runtime::Bool};
    use objc2_foundation::{ns_string, NSBundle, NSDateComponents, NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNCalendarNotificationTrigger, UNMutableNotificationContent,
        UNNotificationRequest, UNNotificationSound, UNUserNotificationCenter,
    };

    use super::{
        Availability, NotificationError, NotificationScheduler, WeeklyNotification, MAX_PENDING,
    };

    /// How long to wait for the user to answer the permission prompt.
    const PERMISSION_TIMEOUT: Duration = Duration::from_secs(300);

    /// How long to wait for macOS to accept the scheduled notifications.
    const SCHEDULE_TIMEOUT: Duration = Duration::from_secs(10);

    /// `UNUserNotificationCenter`, for the running app bundle.
    ///
    /// WHITE ROOM schedules no other notifications, so "every pending
    /// notification of this app" means every study reminder. If another kind
    /// is added, removal must switch to identifiers.
    #[derive(Debug, Default, Clone, Copy)]
    pub struct UserNotifications;

    impl UserNotifications {
        /// The notification center. macOS raises an exception (aborting the
        /// app) if a process outside an app bundle asks for it, as
        /// `tauri dev` runs do, so this is checked first.
        fn center(&self) -> Result<Retained<UNUserNotificationCenter>, NotificationError> {
            match self.availability() {
                Availability::Available => {
                    Ok(UNUserNotificationCenter::currentNotificationCenter())
                }
                Availability::Unavailable(reason) => Err(NotificationError::Unavailable(reason)),
            }
        }
    }

    impl NotificationScheduler for UserNotifications {
        fn availability(&self) -> Availability {
            let bundle = NSBundle::mainBundle();
            let in_app_bundle = bundle.bundlePath().to_string().ends_with(".app");
            if in_app_bundle && bundle.bundleIdentifier().is_some() {
                Availability::Available
            } else {
                Availability::Unavailable(
                    "Reminders can only be scheduled by the WHITE ROOM app itself, not by a \
                     development run."
                        .into(),
                )
            }
        }

        fn request_permission(&self) -> Result<bool, NotificationError> {
            let center = self.center()?;
            let (tx, rx) = mpsc::channel();
            // Called once, on a background queue. Only whether an error was
            // reported is needed; macOS also answers `false` in that case.
            let done = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
                let _ = tx.send(granted.as_bool());
            });
            center.requestAuthorizationWithOptions_completionHandler(
                UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
                &done,
            );
            rx.recv_timeout(PERMISSION_TIMEOUT)
                .map_err(|_| NotificationError::NoAnswer(PERMISSION_TIMEOUT))
        }

        fn replace_all(
            &self,
            notifications: &[WeeklyNotification],
        ) -> Result<(), NotificationError> {
            if notifications.len() > MAX_PENDING {
                return Err(NotificationError::TooMany(notifications.len()));
            }
            let center = self.center()?;
            center.removeAllPendingNotificationRequests();

            let (tx, rx) = mpsc::channel();
            for notification in notifications {
                let tx = tx.clone();
                let done = RcBlock::new(move |error: *mut NSError| {
                    let _ = tx.send(error.is_null());
                });
                center.addNotificationRequest_withCompletionHandler(
                    &request(notification),
                    Some(&done),
                );
            }
            drop(tx);

            let mut refused = 0;
            for _ in notifications {
                match rx.recv_timeout(SCHEDULE_TIMEOUT) {
                    Ok(true) => {}
                    Ok(false) => refused += 1,
                    Err(_) => return Err(NotificationError::NoAnswer(SCHEDULE_TIMEOUT)),
                }
            }
            if refused > 0 {
                return Err(NotificationError::Refused {
                    refused,
                    total: notifications.len(),
                });
            }
            Ok(())
        }

        fn cancel_all(&self) -> Result<(), NotificationError> {
            self.center()?.removeAllPendingNotificationRequests();
            Ok(())
        }
    }

    fn request(notification: &WeeklyNotification) -> Retained<UNNotificationRequest> {
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(&notification.title));
        content.setBody(&NSString::from_str(&notification.body));
        content.setSound(Some(&UNNotificationSound::defaultSound()));
        // Groups all reminders together in Notification Center.
        content.setThreadIdentifier(ns_string!("study-reminders"));

        // No calendar or time zone is set, so the time is local and follows
        // the Mac when it changes time zone.
        let when = NSDateComponents::new();
        when.setWeekday(notification.weekday.into());
        when.setHour(notification.hour.into());
        when.setMinute(notification.minute.into());
        let trigger =
            UNCalendarNotificationTrigger::triggerWithDateMatchingComponents_repeats(&when, true);

        UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(&notification.identifier),
            &content,
            Some(&trigger),
        )
    }
}

#[cfg(not(target_os = "macos"))]
mod unsupported {
    use super::{Availability, NotificationError, NotificationScheduler, WeeklyNotification};

    const REASON: &str = "Reminders are only available on macOS.";

    /// Other platforms: nothing is scheduled, and this says so.
    #[derive(Debug, Default, Clone, Copy)]
    pub struct NoScheduler;

    impl NotificationScheduler for NoScheduler {
        fn availability(&self) -> Availability {
            Availability::Unavailable(REASON.into())
        }

        fn request_permission(&self) -> Result<bool, NotificationError> {
            Err(NotificationError::Unavailable(REASON.into()))
        }

        fn replace_all(&self, _: &[WeeklyNotification]) -> Result<(), NotificationError> {
            Err(NotificationError::Unavailable(REASON.into()))
        }

        fn cancel_all(&self) -> Result<(), NotificationError> {
            Ok(())
        }
    }
}
