//! The Dock and menu bar while the workspace is open.
//!
//! WHITE ROOM changes them only through its own *presentation options*
//! (`NSApplication.presentationOptions`). macOS applies an app's options only
//! while that app is the active one, and drops them the moment the app quits,
//! crashes, or is force-quit. So nothing here can outlast WHITE ROOM or affect
//! other apps, and no system setting is read or written.
//!
//! In native fullscreen macOS lets the Dock and the menu bar slide in at the
//! screen edges. While the workspace is open, WHITE ROOM hides both
//! completely, so nothing slides in. The system's fullscreen menu items are
//! then out of mouse reach; WHITE ROOM's own exits and the system's keyboard
//! shortcuts are not affected.
//!
//! Observed AppKit behaviour this module works around (macOS 15, checked with
//! a standalone AppKit probe app as well as WHITE ROOM):
//!
//! - Changes made while AppKit is inside a fullscreen transition callback
//!   (such as `windowDidEnterFullScreen`) are silently ignored. Callers make
//!   changes on a later turn of the main run loop.
//! - If the app leaves fullscreen while they are hidden, AppKit resets the
//!   app's options to the default, but LaunchServices goes on recording a
//!   changed presentation mode for the app (`lsappinfo`'s
//!   `UIPresentationOptions`; with only the Dock hidden it also shows in
//!   `currentSystemPresentationOptions`). Showing them *before* leaving
//!   fullscreen avoids this. When the system's own controls end fullscreen
//!   first, [`SystemUi::clear_after_fullscreen`] clears it by changing the
//!   options once more.

use std::{fmt, ops};

/// The subset of `NSApplicationPresentationOptions` WHITE ROOM reasons about,
/// with the same bit values as AppKit. Other bits are carried through
/// unchanged.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct PresentationOptions(usize);

impl PresentationOptions {
    pub const DEFAULT: Self = Self(0);
    pub const AUTO_HIDE_DOCK: Self = Self(1 << 0);
    pub const HIDE_DOCK: Self = Self(1 << 1);
    pub const AUTO_HIDE_MENU_BAR: Self = Self(1 << 2);
    pub const HIDE_MENU_BAR: Self = Self(1 << 3);
    pub const DISABLE_PROCESS_SWITCHING: Self = Self(1 << 5);
    pub const DISABLE_FORCE_QUIT: Self = Self(1 << 6);
    pub const DISABLE_SESSION_TERMINATION: Self = Self(1 << 7);
    pub const FULL_SCREEN: Self = Self(1 << 10);
    pub const AUTO_HIDE_TOOLBAR: Self = Self(1 << 11);

    pub const fn from_bits(bits: usize) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> usize {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// Whether AppKit accepts this combination. Setting an invalid one raises
    /// an Objective-C exception, which would abort the app, so every change
    /// is checked first. These are the rules from Apple's documentation for
    /// the options WHITE ROOM can produce.
    pub const fn is_valid(self) -> bool {
        let any_dock = self.intersects(Self(Self::AUTO_HIDE_DOCK.0 | Self::HIDE_DOCK.0));
        let both_dock = self.contains(Self(Self::AUTO_HIDE_DOCK.0 | Self::HIDE_DOCK.0));
        let both_menu_bar = self.contains(Self(Self::AUTO_HIDE_MENU_BAR.0 | Self::HIDE_MENU_BAR.0));
        let needs_dock_option = Self(
            Self::AUTO_HIDE_MENU_BAR.0
                | Self::DISABLE_PROCESS_SWITCHING.0
                | Self::DISABLE_FORCE_QUIT.0
                | Self::DISABLE_SESSION_TERMINATION.0,
        );

        !both_dock
            && !both_menu_bar
            && (!self.contains(Self::HIDE_MENU_BAR) || self.contains(Self::HIDE_DOCK))
            && (!self.intersects(needs_dock_option) || any_dock)
            && (!self.contains(Self::AUTO_HIDE_TOOLBAR) || self.contains(Self::FULL_SCREEN))
    }
}

impl ops::BitOr for PresentationOptions {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl fmt::Debug for PresentationOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const NAMES: [(PresentationOptions, &str); 9] = [
            (PresentationOptions::AUTO_HIDE_DOCK, "AutoHideDock"),
            (PresentationOptions::HIDE_DOCK, "HideDock"),
            (PresentationOptions::AUTO_HIDE_MENU_BAR, "AutoHideMenuBar"),
            (PresentationOptions::HIDE_MENU_BAR, "HideMenuBar"),
            (
                PresentationOptions::DISABLE_PROCESS_SWITCHING,
                "DisableProcessSwitching",
            ),
            (PresentationOptions::DISABLE_FORCE_QUIT, "DisableForceQuit"),
            (
                PresentationOptions::DISABLE_SESSION_TERMINATION,
                "DisableSessionTermination",
            ),
            (PresentationOptions::FULL_SCREEN, "FullScreen"),
            (PresentationOptions::AUTO_HIDE_TOOLBAR, "AutoHideToolbar"),
        ];
        let mut names: Vec<&str> = NAMES
            .iter()
            .filter(|(flag, _)| self.contains(*flag))
            .map(|(_, name)| *name)
            .collect();
        if names.is_empty() {
            names.push("Default");
        }
        write!(f, "{} ({:#x})", names.join("|"), self.0)
    }
}

/// Both hidden completely.
const HIDDEN: PresentationOptions =
    PresentationOptions(PresentationOptions::HIDE_DOCK.0 | PresentationOptions::HIDE_MENU_BAR.0);

/// The options that hide the Dock and the menu bar, given the app's current
/// options, or `None` if there is nothing to change.
///
/// Only acts once macOS has put the app into fullscreen (`FULL_SCREEN` is
/// set): macOS replaces the app's options when the fullscreen transition
/// starts, so anything set earlier would be overwritten.
pub fn with_hidden(current: PresentationOptions) -> Option<PresentationOptions> {
    if !current.contains(PresentationOptions::FULL_SCREEN) || current.contains(HIDDEN) {
        return None;
    }
    let next = current
        .without(PresentationOptions::AUTO_HIDE_DOCK)
        .without(PresentationOptions::AUTO_HIDE_MENU_BAR)
        | HIDDEN;
    next.is_valid().then_some(next)
}

/// The options that undo [`with_hidden`] while still in fullscreen, or
/// `None` if there is nothing to undo there. They return to what macOS itself
/// uses in fullscreen: a Dock and menu bar that slide in at the edges.
pub fn with_shown(current: PresentationOptions) -> Option<PresentationOptions> {
    if !current.intersects(HIDDEN) || !current.contains(PresentationOptions::FULL_SCREEN) {
        return None;
    }
    let mut next = current;
    if next.contains(PresentationOptions::HIDE_DOCK) {
        next = next.without(PresentationOptions::HIDE_DOCK) | PresentationOptions::AUTO_HIDE_DOCK;
    }
    if next.contains(PresentationOptions::HIDE_MENU_BAR) {
        next = next.without(PresentationOptions::HIDE_MENU_BAR)
            | PresentationOptions::AUTO_HIDE_MENU_BAR;
    }
    Some(if next.is_valid() {
        next
    } else {
        PresentationOptions::FULL_SCREEN
            | PresentationOptions::AUTO_HIDE_DOCK
            | PresentationOptions::AUTO_HIDE_MENU_BAR
    })
}

/// Options to set, in order, to clear the system mode macOS keeps applying
/// after the app left fullscreen with the Dock hidden. Setting the options
/// to what they already are is not enough; they must actually change. Outside
/// fullscreen WHITE ROOM uses the default options, so that is where it ends.
pub const CLEAR_AFTER_FULLSCREEN: [PresentationOptions; 2] = [
    PresentationOptions::AUTO_HIDE_DOCK,
    PresentationOptions::DEFAULT,
];

#[derive(Debug, thiserror::Error)]
pub enum SystemUiError {
    #[error("the Dock and menu bar can only be changed from the main thread")]
    NotMainThread,

    #[error("refused to apply presentation options {0:?}: AppKit would reject them")]
    Invalid(PresentationOptions),
}

/// Hides and shows the Dock and the menu bar for the workspace.
///
/// Every method must be called on the main thread.
pub trait SystemUi {
    /// Hides the Dock and the menu bar for as long as WHITE ROOM is the
    /// active app. Does nothing until macOS has put the window into
    /// fullscreen, so it is safe to call on every resize. Returns whether
    /// both are now hidden.
    fn hide(&self) -> Result<bool, SystemUiError>;

    /// Undoes [`hide`](Self::hide) while the window is still fullscreen, so
    /// that macOS leaves fullscreen cleanly. Does nothing if nothing is
    /// hidden.
    fn show(&self) -> Result<(), SystemUiError>;

    /// Call once fullscreen has ended while they were hidden, i.e. the
    /// system's own controls ended it before [`show`](Self::show) could run.
    /// Clears what macOS still applies to the app. Returns `Ok(false)`, doing
    /// nothing, while macOS is still in fullscreen.
    fn clear_after_fullscreen(&self) -> Result<bool, SystemUiError>;

    /// Whether WHITE ROOM's options currently hide both.
    fn is_hidden(&self) -> Result<bool, SystemUiError>;
}

#[cfg(target_os = "macos")]
pub use macos::AppKitSystemUi as NativeSystemUi;
#[cfg(not(target_os = "macos"))]
pub use unsupported::NoSystemUi as NativeSystemUi;

#[cfg(target_os = "macos")]
mod macos {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationPresentationOptions};

    use super::{
        with_hidden, with_shown, PresentationOptions, SystemUi, SystemUiError,
        CLEAR_AFTER_FULLSCREEN, HIDDEN,
    };

    /// Reads and writes `NSApp.presentationOptions`.
    #[derive(Debug, Default, Clone, Copy)]
    pub struct AppKitSystemUi;

    fn app() -> Result<objc2::rc::Retained<NSApplication>, SystemUiError> {
        let mtm = MainThreadMarker::new().ok_or(SystemUiError::NotMainThread)?;
        Ok(NSApplication::sharedApplication(mtm))
    }

    fn current(app: &NSApplication) -> PresentationOptions {
        PresentationOptions::from_bits(app.presentationOptions().0)
    }

    fn set(app: &NSApplication, options: PresentationOptions) -> Result<(), SystemUiError> {
        if !options.is_valid() {
            return Err(SystemUiError::Invalid(options));
        }
        log::debug!("presentation options: {:?} -> {options:?}", current(app));
        app.setPresentationOptions(NSApplicationPresentationOptions(options.bits()));
        Ok(())
    }

    impl SystemUi for AppKitSystemUi {
        fn hide(&self) -> Result<bool, SystemUiError> {
            let app = app()?;
            if let Some(next) = with_hidden(current(&app)) {
                set(&app, next)?;
            }
            Ok(current(&app).contains(HIDDEN))
        }

        fn show(&self) -> Result<(), SystemUiError> {
            let app = app()?;
            match with_shown(current(&app)) {
                Some(next) => set(&app, next),
                None => Ok(()),
            }
        }

        fn clear_after_fullscreen(&self) -> Result<bool, SystemUiError> {
            let app = app()?;
            if current(&app).contains(PresentationOptions::FULL_SCREEN) {
                return Ok(false);
            }
            for options in CLEAR_AFTER_FULLSCREEN {
                set(&app, options)?;
            }
            log::info!("cleared the hidden Dock and menu bar after fullscreen ended");
            Ok(true)
        }

        fn is_hidden(&self) -> Result<bool, SystemUiError> {
            let app = app()?;
            Ok(current(&app).contains(HIDDEN))
        }
    }

    /// Leaves fullscreen the way the green window button, View › Exit Full
    /// Screen, and ⌃⌘F do, bypassing WHITE ROOM's own exit path. Debug
    /// builds only; the smoke test uses it. Acts on this app's fullscreen
    /// window, whether or not it has keyboard focus; returns `false` if there
    /// is none.
    #[cfg(debug_assertions)]
    pub fn toggle_fullscreen_like_the_system() -> Result<bool, SystemUiError> {
        use objc2_app_kit::NSWindowStyleMask;

        let app = app()?;
        let fullscreen = app.windows().iter().find(|window| {
            window.isVisible() && window.styleMask().contains(NSWindowStyleMask::FullScreen)
        });
        Ok(match fullscreen {
            Some(window) => {
                window.toggleFullScreen(None);
                true
            }
            None => false,
        })
    }

    /// The options macOS applies system-wide for the active app, as seen
    /// from this process.
    pub fn system_options() -> Option<PresentationOptions> {
        let mtm = MainThreadMarker::new()?;
        let app = NSApplication::sharedApplication(mtm);
        Some(PresentationOptions::from_bits(
            app.currentSystemPresentationOptions().0,
        ))
    }
}

/// The options macOS applies system-wide for the active app, or `None` off
/// the main thread or on other platforms. Used by the smoke test to check that
/// leaving the workspace leaves nothing behind.
#[cfg(target_os = "macos")]
pub use macos::system_options;
#[cfg(all(target_os = "macos", debug_assertions))]
pub use macos::toggle_fullscreen_like_the_system;

#[cfg(not(target_os = "macos"))]
pub fn system_options() -> Option<PresentationOptions> {
    None
}

#[cfg(not(target_os = "macos"))]
mod unsupported {
    use super::{SystemUi, SystemUiError};

    /// Other platforms: the operating system's fullscreen mode is used as is.
    #[derive(Debug, Default, Clone, Copy)]
    pub struct NoSystemUi;

    impl SystemUi for NoSystemUi {
        fn hide(&self) -> Result<bool, SystemUiError> {
            Ok(false)
        }

        fn show(&self) -> Result<(), SystemUiError> {
            Ok(())
        }

        fn clear_after_fullscreen(&self) -> Result<bool, SystemUiError> {
            Ok(true)
        }

        fn is_hidden(&self) -> Result<bool, SystemUiError> {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PresentationOptions as O;
    use super::*;

    /// What macOS applies by default in native fullscreen.
    const NATIVE_FULLSCREEN: O = O::from_bits(
        O::FULL_SCREEN.bits() | O::AUTO_HIDE_DOCK.bits() | O::AUTO_HIDE_MENU_BAR.bits(),
    );

    #[test]
    fn hides_the_dock_and_the_menu_bar_completely() {
        let hidden = with_hidden(NATIVE_FULLSCREEN).unwrap();
        assert_eq!(hidden, O::FULL_SCREEN | O::HIDE_DOCK | O::HIDE_MENU_BAR);
        assert!(hidden.is_valid());
    }

    #[test]
    fn completes_a_partly_hidden_state() {
        let partly = O::FULL_SCREEN | O::HIDE_DOCK | O::AUTO_HIDE_MENU_BAR;
        assert_eq!(
            with_hidden(partly),
            Some(O::FULL_SCREEN | O::HIDE_DOCK | O::HIDE_MENU_BAR)
        );
    }

    #[test]
    fn showing_undoes_a_dock_hidden_on_its_own() {
        let partly = O::FULL_SCREEN | O::HIDE_DOCK | O::AUTO_HIDE_MENU_BAR;
        assert_eq!(with_shown(partly), Some(NATIVE_FULLSCREEN));
    }

    #[test]
    fn keeps_options_it_does_not_manage() {
        let current = NATIVE_FULLSCREEN | O::AUTO_HIDE_TOOLBAR;
        let hidden = with_hidden(current).unwrap();
        assert!(hidden.contains(O::AUTO_HIDE_TOOLBAR));
        assert_eq!(with_shown(hidden), Some(current));
    }

    #[test]
    fn waits_for_macos_to_enter_fullscreen() {
        assert_eq!(with_hidden(O::DEFAULT), None);
    }

    #[test]
    fn hiding_twice_changes_nothing() {
        let hidden = with_hidden(NATIVE_FULLSCREEN).unwrap();
        assert_eq!(with_hidden(hidden), None);
    }

    #[test]
    fn showing_restores_the_native_fullscreen_options() {
        let hidden = with_hidden(NATIVE_FULLSCREEN).unwrap();
        assert_eq!(with_shown(hidden), Some(NATIVE_FULLSCREEN));
    }

    #[test]
    fn showing_does_nothing_when_the_dock_is_not_hidden() {
        assert_eq!(with_shown(O::DEFAULT), None);
        assert_eq!(with_shown(NATIVE_FULLSCREEN), None);
    }

    #[test]
    fn showing_outside_fullscreen_is_left_to_clear_after_fullscreen() {
        assert_eq!(with_shown(O::HIDE_DOCK | O::HIDE_MENU_BAR), None);
    }

    #[test]
    fn clearing_changes_the_options_and_ends_at_the_default() {
        let [interim, last] = CLEAR_AFTER_FULLSCREEN;
        assert_ne!(interim, last, "an unchanged value would not clear anything");
        assert!(interim.is_valid());
        assert_eq!(last, O::DEFAULT);
    }

    #[test]
    fn every_result_is_valid_for_every_valid_input() {
        // All combinations of the flags this module knows about.
        let flags = [
            O::AUTO_HIDE_DOCK,
            O::HIDE_DOCK,
            O::AUTO_HIDE_MENU_BAR,
            O::HIDE_MENU_BAR,
            O::DISABLE_PROCESS_SWITCHING,
            O::DISABLE_FORCE_QUIT,
            O::DISABLE_SESSION_TERMINATION,
            O::FULL_SCREEN,
            O::AUTO_HIDE_TOOLBAR,
        ];
        let mut checked = 0;
        for mask in 0..(1usize << flags.len()) {
            let options = flags
                .iter()
                .enumerate()
                .filter(|(bit, _)| mask & (1 << bit) != 0)
                .fold(O::DEFAULT, |acc, (_, flag)| acc | *flag);
            if !options.is_valid() {
                continue;
            }
            checked += 1;
            for next in [with_hidden(options), with_shown(options)]
                .into_iter()
                .flatten()
            {
                assert!(next.is_valid(), "{options:?} -> {next:?}");
            }
        }
        assert!(checked > 20, "only {checked} valid combinations");
    }

    #[test]
    fn rejects_combinations_appkit_rejects() {
        assert!(!(O::HIDE_DOCK | O::AUTO_HIDE_DOCK).is_valid());
        assert!(!(O::HIDE_MENU_BAR | O::AUTO_HIDE_MENU_BAR | O::HIDE_DOCK).is_valid());
        assert!(!(O::AUTO_HIDE_DOCK | O::HIDE_MENU_BAR).is_valid());
        assert!(!O::AUTO_HIDE_MENU_BAR.is_valid());
        assert!(!O::DISABLE_PROCESS_SWITCHING.is_valid());
        assert!(!(O::AUTO_HIDE_TOOLBAR | O::HIDE_DOCK).is_valid());
        assert!(O::DEFAULT.is_valid());
        assert!(NATIVE_FULLSCREEN.is_valid());
    }

    #[test]
    fn debug_output_names_the_flags() {
        assert_eq!(format!("{:?}", O::DEFAULT), "Default (0x0)");
        assert_eq!(
            format!("{NATIVE_FULLSCREEN:?}"),
            "AutoHideDock|AutoHideMenuBar|FullScreen (0x405)"
        );
    }
}
