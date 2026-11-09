//! Operating-system integration, kept behind small interfaces so the rest of
//! the backend does not call macOS APIs directly.
//!
//! | Module             | Interface                    | macOS implementation          |
//! |--------------------|------------------------------|-------------------------------|
//! | [`presentation`]   | [`presentation::SystemUi`]   | the app's presentation options |
//! | [`notifications`]  | [`notifications::NotificationScheduler`] | `UNUserNotificationCenter` |
//!
//! On other platforms each interface has an implementation that does nothing
//! and says so, so callers never assume a change was made when it was not.
//!
//! Application discovery arrives in Phase 4.

pub mod notifications;
pub mod presentation;
