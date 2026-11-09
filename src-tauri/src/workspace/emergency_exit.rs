//! The global emergency-exit shortcut.
//!
//! The shortcut is registered with the operating system for the lifetime of
//! the app, so it works even when WHITE ROOM is not the focused app or its
//! interface has stopped responding: the handler runs in Rust and never goes
//! through the webview.
//!
//! Registration can fail, for example when another app already owns the key
//! combination, or on Linux under Wayland. That is reported to the user rather
//! than hidden; the on-screen exit control and the operating system's own
//! fullscreen controls keep working either way.

use serde::Serialize;
use tauri::{plugin::TauriPlugin, AppHandle, Runtime};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState,
};

/// macOS: Shift-Command-Escape. Unused by macOS itself; the nearby system
/// shortcuts (Option-Command-Escape for Force Quit, Control-Command-Q to lock
/// the screen) are deliberately avoided.
#[cfg(target_os = "macos")]
mod keys {
    use super::Modifiers;
    pub const MODIFIERS: Modifiers = Modifiers::SHIFT.union(Modifiers::SUPER);
    pub const LABEL: &str = "⇧⌘⎋";
    pub const DESCRIPTION: &str = "Shift-Command-Escape";
}

/// Windows and Linux: Control+Alt+Shift+Escape. Control+Shift+Escape (the
/// macOS equivalent) opens Task Manager on Windows and cannot be registered.
#[cfg(not(target_os = "macos"))]
mod keys {
    use super::Modifiers;
    pub const MODIFIERS: Modifiers = Modifiers::CONTROL
        .union(Modifiers::ALT)
        .union(Modifiers::SHIFT);
    pub const LABEL: &str = "Ctrl+Alt+Shift+Esc";
    pub const DESCRIPTION: &str = "Control+Alt+Shift+Escape";
}

/// The emergency-exit key combination for this platform.
pub fn shortcut() -> Shortcut {
    Shortcut::new(Some(keys::MODIFIERS), Code::Escape)
}

/// Whether the emergency-exit shortcut is working, as shown to the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmergencyExitInfo {
    /// Compact label, e.g. `⇧⌘⎋`.
    pub label: String,
    /// Spelled-out form for screen readers and plain text.
    pub description: String,
    /// `true` only if the operating system accepted the registration.
    pub available: bool,
    /// Why the shortcut is unavailable, when it is.
    pub unavailable_reason: Option<String>,
}

impl EmergencyExitInfo {
    pub fn available() -> Self {
        Self {
            label: keys::LABEL.into(),
            description: keys::DESCRIPTION.into(),
            available: true,
            unavailable_reason: None,
        }
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            available: false,
            unavailable_reason: Some(reason.into()),
            ..Self::available()
        }
    }
}

/// The global-shortcut plugin. Shortcuts are registered only from Rust; the
/// frontend is not granted any of the plugin's permissions.
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new().build()
}

/// Registers the shortcut so that pressing it calls `on_press`. Never fails:
/// the returned info says whether the shortcut is actually available.
pub fn register<R, F>(app: &AppHandle<R>, on_press: F) -> EmergencyExitInfo
where
    R: Runtime,
    F: Fn(&AppHandle<R>) + Send + Sync + 'static,
{
    let handler = move |app: &AppHandle<R>, _: &Shortcut, event: ShortcutEvent| {
        if event.state() == ShortcutState::Pressed {
            on_press(app);
        }
    };
    match app.global_shortcut().on_shortcut(shortcut(), handler) {
        Ok(()) => {
            log::info!("emergency exit shortcut {} registered", keys::DESCRIPTION);
            EmergencyExitInfo::available()
        }
        Err(err) => {
            log::warn!(
                "emergency exit shortcut {} could not be registered: {err}",
                keys::DESCRIPTION
            );
            EmergencyExitInfo::unavailable(format!(
                "The system did not accept {}: {err}",
                keys::DESCRIPTION
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_escape_with_modifiers() {
        let shortcut = shortcut();
        assert_eq!(shortcut.key, Code::Escape);
        assert!(
            shortcut.mods.contains(Modifiers::SHIFT),
            "a bare or single-modifier Escape is too easy to press by accident"
        );
    }

    #[test]
    fn avoids_reserved_system_shortcuts() {
        let reserved = [
            // macOS: Force Quit, lock screen, log out.
            Shortcut::new(Some(Modifiers::ALT | Modifiers::SUPER), Code::Escape),
            Shortcut::new(
                Some(Modifiers::ALT | Modifiers::SHIFT | Modifiers::SUPER),
                Code::Escape,
            ),
            Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SUPER), Code::KeyQ),
            Shortcut::new(Some(Modifiers::SHIFT | Modifiers::SUPER), Code::KeyQ),
            // Windows: Task Manager, Start menu, switch windows.
            Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Escape),
            Shortcut::new(Some(Modifiers::CONTROL), Code::Escape),
            Shortcut::new(Some(Modifiers::ALT), Code::Escape),
        ];
        let ours = shortcut();
        for shortcut in reserved {
            assert_ne!(ours, shortcut, "{shortcut} is reserved by the OS");
        }
    }

    #[test]
    fn unavailable_info_keeps_the_label_and_explains_why() {
        let info = EmergencyExitInfo::unavailable("taken");
        assert!(!info.available);
        assert_eq!(info.unavailable_reason.as_deref(), Some("taken"));
        assert_eq!(info.label, EmergencyExitInfo::available().label);
    }

    #[test]
    fn serializes_for_the_frontend() {
        let json = serde_json::to_value(EmergencyExitInfo::unavailable("taken")).unwrap();
        assert_eq!(json["available"], false);
        assert_eq!(json["unavailableReason"], "taken");
        assert!(json["label"].is_string());
        assert!(json["description"].is_string());
    }
}
