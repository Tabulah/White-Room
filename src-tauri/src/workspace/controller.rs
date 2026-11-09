//! Entering and leaving the workspace.
//!
//! [`Workspace`] is managed Tauri state. It applies window changes through
//! [`WorkspaceWindow`], hides the Dock and the menu bar through [`SystemUi`]
//! while the workspace is open, and records the result in [`WorkspaceState`].
//!
//! The Dock and menu bar follow the state in [`Workspace::sync_system_ui`]:
//! hidden while the workspace is open and macOS has made the window
//! fullscreen, shown otherwise. Leaving through WHITE ROOM shows them first,
//! while still fullscreen; leaving through the system's own controls clears
//! them once fullscreen has ended. Hiding them is never essential: if they
//! cannot be changed, that is logged and entering or leaving carries on.
//!
//! Locking rule: the state lock is never held while calling the window. Window
//! getters called off the main thread block until the main thread answers, and
//! the main thread may itself be waiting for this lock (the emergency shortcut
//! and window events run there), so holding both would deadlock.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, MutexGuard, PoisonError,
};

use serde::Serialize;

use super::{
    display::{self, Display, Placement},
    emergency_exit::EmergencyExitInfo,
    state::{ExitReason, Status, WorkspaceDisplay, WorkspaceState},
    window::WorkspaceWindow,
};
use crate::platform::presentation::SystemUi;

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("the main window is not available")]
    WindowNotFound,

    #[error("the workspace has not been initialised yet")]
    NotReady,

    #[error("could not change the window's fullscreen state: {0}")]
    Fullscreen(#[source] tauri::Error),
}

/// Workspace state as sent to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    /// Increases with every change; the frontend ignores older snapshots.
    pub revision: u64,
    pub status: Status,
    pub display: Option<WorkspaceDisplay>,
    pub last_exit: Option<ExitReason>,
    pub emergency_exit: EmergencyExitInfo,
}

/// Result of an operation: the new snapshot and whether anything changed
/// (in which case the frontend should be notified).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub snapshot: WorkspaceSnapshot,
    pub changed: bool,
}

/// The fullscreen workspace for the main window.
#[derive(Debug)]
pub struct Workspace {
    state: Mutex<WorkspaceState>,
    emergency_exit: EmergencyExitInfo,
    /// Whether WHITE ROOM has hidden the Dock and menu bar and not yet undone
    /// that. Only changed on the main thread.
    ui_hidden: AtomicBool,
}

impl Workspace {
    pub fn new(emergency_exit: EmergencyExitInfo) -> Self {
        Self {
            state: Mutex::default(),
            emergency_exit,
            ui_hidden: AtomicBool::new(false),
        }
    }

    pub fn snapshot(&self) -> WorkspaceSnapshot {
        self.snapshot_of(&self.lock())
    }

    /// Moves the window to the primary display and makes it fullscreen.
    ///
    /// Display placement is best effort: if it fails, the workspace opens on
    /// whichever display holds the window. Only a failure to go fullscreen is
    /// an error, and in that case the state is left unchanged.
    ///
    /// The Dock and menu bar are hidden once macOS has put the window into
    /// fullscreen, which usually happens later, in
    /// [`sync_system_ui`](Self::sync_system_ui).
    pub fn enter(
        &self,
        window: &impl WorkspaceWindow,
        ui: &impl SystemUi,
    ) -> Result<Outcome, WorkspaceError> {
        let display = place_on_primary_display(window);
        window
            .set_fullscreen(true)
            .map_err(WorkspaceError::Fullscreen)?;
        let fullscreen_now = window.is_fullscreen().unwrap_or_else(|err| {
            log::warn!("could not read fullscreen state after entering: {err}");
            false
        });
        if let Err(err) = window.focus() {
            log::debug!("could not focus the workspace window: {err}");
        }

        let outcome = {
            let mut state = self.lock();
            let changed = state.enter(display, fullscreen_now);
            if changed {
                log::info!("entered workspace on {}", describe(state.display()));
            }
            self.outcome(&state, changed)
        };
        // Takes effect now only if the window was already fullscreen.
        self.sync_system_ui(ui);
        Ok(outcome)
    }

    /// Takes the window out of fullscreen.
    ///
    /// The window is always asked to leave fullscreen, whatever the recorded
    /// state says, so that exiting works even if that state is wrong. If the
    /// window refuses, the state is left unchanged and the error returned.
    ///
    /// The Dock and menu bar are shown again first, so that macOS leaves
    /// fullscreen from the options it set up itself.
    pub fn exit(
        &self,
        window: &impl WorkspaceWindow,
        ui: &impl SystemUi,
        reason: ExitReason,
    ) -> Result<Outcome, WorkspaceError> {
        self.show_system_ui(ui);
        if let Err(err) = window.set_fullscreen(false) {
            self.sync_system_ui(ui); // Still in the workspace: hide it again.
            return Err(WorkspaceError::Fullscreen(err));
        }

        let mut state = self.lock();
        let changed = state.exit(reason);
        if changed {
            log::info!("left workspace ({reason:?})");
        }
        Ok(self.outcome(&state, changed))
    }

    /// Brings the state in line with the window after a resize, for example
    /// when the user leaves fullscreen with the operating system's own
    /// controls. Returns an outcome only if the state changed.
    ///
    /// Window events arrive inside AppKit's fullscreen transition callbacks,
    /// where changes to the Dock and menu bar are ignored, so this leaves them
    /// alone; call [`sync_system_ui`](Self::sync_system_ui) afterwards, on a
    /// later main-loop turn.
    pub fn reconcile(&self, window: &impl WorkspaceWindow) -> Option<Outcome> {
        if self.lock().status() == Status::Inactive {
            return None;
        }
        let fullscreen = match window.is_fullscreen() {
            Ok(fullscreen) => fullscreen,
            Err(err) => {
                log::warn!("could not read fullscreen state: {err}");
                return None;
            }
        };

        let mut state = self.lock();
        let changed = state.observe_fullscreen(fullscreen);
        if changed {
            log::info!("left workspace through the system fullscreen control");
        }
        changed.then(|| self.outcome(&state, true))
    }

    /// Whether [`sync_system_ui`](Self::sync_system_ui) has anything to do. False
    /// outside the workspace once everything is undone, so ordinary window
    /// resizes cost nothing.
    pub fn needs_system_ui_sync(&self) -> bool {
        self.lock().status() == Status::Active || self.ui_hidden.load(Ordering::SeqCst)
    }

    /// Brings the Dock and menu bar in line with the state: hidden while the
    /// workspace is open, otherwise undone once fullscreen has ended.
    /// Idempotent. Must run on the main thread, outside AppKit's fullscreen
    /// transition callbacks.
    pub fn sync_system_ui(&self, ui: &impl SystemUi) {
        if self.lock().status() == Status::Active {
            match ui.hide() {
                Ok(true) => {
                    if !self.ui_hidden.swap(true, Ordering::SeqCst) {
                        log::info!("Dock and menu bar hidden for the workspace");
                    }
                }
                Ok(false) => {} // macOS has not entered fullscreen yet.
                Err(err) => log::warn!("could not hide the Dock and menu bar: {err}"),
            }
        } else if self.ui_hidden.load(Ordering::SeqCst) {
            // Fullscreen was ended by the system's own controls before they
            // could be shown.
            match ui.clear_after_fullscreen() {
                Ok(true) => self.ui_hidden.store(false, Ordering::SeqCst),
                Ok(false) => {} // Still leaving fullscreen; try again later.
                Err(err) => log::error!("could not show the Dock and menu bar again: {err}"),
            }
        }
    }

    /// Shows the Dock and menu bar while the window is still fullscreen, if
    /// WHITE ROOM hid them.
    fn show_system_ui(&self, ui: &impl SystemUi) {
        if !self.ui_hidden.load(Ordering::SeqCst) {
            return;
        }
        match ui.show() {
            Ok(()) => {
                self.ui_hidden.store(false, Ordering::SeqCst);
                log::info!("Dock and menu bar shown again");
            }
            // Left set, so `sync_system_ui` clears it after fullscreen ends.
            Err(err) => log::error!("could not show the Dock and menu bar again: {err}"),
        }
    }

    fn outcome(&self, state: &WorkspaceState, changed: bool) -> Outcome {
        Outcome {
            snapshot: self.snapshot_of(state),
            changed,
        }
    }

    fn snapshot_of(&self, state: &WorkspaceState) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            revision: state.revision(),
            status: state.status(),
            display: state.display().cloned(),
            last_exit: state.last_exit(),
            emergency_exit: self.emergency_exit.clone(),
        }
    }

    /// The state is a few plain values that are always internally consistent,
    /// so a panic elsewhere while it was locked cannot have corrupted it.
    /// Recovering from poisoning keeps the exit path working after a panic.
    fn lock(&self) -> MutexGuard<'_, WorkspaceState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Applies the display policy in [`display`] and returns the display the
/// workspace will open on, if known.
fn place_on_primary_display(window: &impl WorkspaceWindow) -> Option<WorkspaceDisplay> {
    let primary = window.primary_display().unwrap_or_else(|err| {
        log::warn!("could not determine the primary display: {err}");
        None
    });
    let current = window.current_display().unwrap_or_else(|err| {
        log::warn!("could not determine the window's display: {err}");
        None
    });
    let size = window
        .outer_size()
        .inspect_err(|err| log::warn!("could not read the window size: {err}"))
        .ok();

    let on = |display: Option<Display>, primary: bool| {
        display.map(|display| WorkspaceDisplay {
            name: display.name,
            primary,
        })
    };

    match display::plan(primary.as_ref(), current.as_ref(), size) {
        Placement::StayOnPrimary => on(primary, true),
        Placement::MoveToPrimary { x, y } => match window.move_to(x, y) {
            Ok(()) => on(primary, true),
            Err(err) => {
                log::warn!("could not move the window to the primary display: {err}");
                on(current, false)
            }
        },
        Placement::StayOnCurrent => {
            log::warn!("primary display unknown; opening on the window's current display");
            on(current, false)
        }
    }
}

fn describe(display: Option<&WorkspaceDisplay>) -> String {
    match display {
        Some(WorkspaceDisplay {
            name,
            primary: true,
        }) => format!("primary display ({})", name.as_deref().unwrap_or("unnamed")),
        Some(WorkspaceDisplay { name, .. }) => {
            format!("current display ({})", name.as_deref().unwrap_or("unnamed"))
        }
        None => "unknown display".into(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        sync::Arc,
    };

    use super::*;
    use crate::{
        platform::presentation::SystemUiError,
        workspace::display::{Rect, Size},
    };

    fn display(name: &str, x: f64) -> Display {
        let bounds = Rect {
            x,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        Display {
            name: Some(name.into()),
            bounds,
            work_area: bounds,
        }
    }

    /// In-memory window. By default it behaves like Windows: fullscreen
    /// changes take effect immediately.
    ///
    /// It also stands in for the Dock and menu bar ([`SystemUi`]), like
    /// macOS: they can only be hidden while the window is fullscreen. Sharing
    /// one fake lets tests check the order of window and Dock changes in
    /// `journal`.
    struct FakeWindow {
        fullscreen: Cell<bool>,
        /// Simulates macOS deferring a request made mid-animation.
        defer_fullscreen: Cell<bool>,
        fail_fullscreen: Cell<bool>,
        fail_queries: bool,
        fail_move: bool,
        primary: Option<Display>,
        current: RefCell<Option<Display>>,
        set_fullscreen_calls: RefCell<Vec<bool>>,
        moves: RefCell<Vec<(f64, f64)>>,
        ui_hidden: Cell<bool>,
        fail_ui: Cell<bool>,
        journal: RefCell<Vec<&'static str>>,
    }

    impl FakeWindow {
        fn on_primary() -> Self {
            let primary = display("Primary", 0.0);
            Self {
                fullscreen: Cell::new(false),
                defer_fullscreen: Cell::new(false),
                fail_fullscreen: Cell::new(false),
                fail_queries: false,
                fail_move: false,
                current: RefCell::new(Some(primary.clone())),
                primary: Some(primary),
                set_fullscreen_calls: RefCell::default(),
                moves: RefCell::default(),
                ui_hidden: Cell::new(false),
                fail_ui: Cell::new(false),
                journal: RefCell::default(),
            }
        }

        fn on_secondary() -> Self {
            let window = Self::on_primary();
            *window.current.borrow_mut() = Some(display("Secondary", 1920.0));
            window
        }

        fn query<T>(&self, value: T) -> tauri::Result<T> {
            if self.fail_queries {
                Err(tauri::Error::FailedToReceiveMessage)
            } else {
                Ok(value)
            }
        }
    }

    impl WorkspaceWindow for FakeWindow {
        fn is_fullscreen(&self) -> tauri::Result<bool> {
            self.query(self.fullscreen.get())
        }

        fn set_fullscreen(&self, fullscreen: bool) -> tauri::Result<()> {
            self.set_fullscreen_calls.borrow_mut().push(fullscreen);
            if self.fail_fullscreen.get() {
                return Err(tauri::Error::FailedToReceiveMessage);
            }
            if !self.defer_fullscreen.get() {
                self.fullscreen.set(fullscreen);
                self.journal.borrow_mut().push(if fullscreen {
                    "enter fullscreen"
                } else {
                    "leave fullscreen"
                });
            }
            Ok(())
        }

        fn primary_display(&self) -> tauri::Result<Option<Display>> {
            self.query(self.primary.clone())
        }

        fn current_display(&self) -> tauri::Result<Option<Display>> {
            self.query(self.current.borrow().clone())
        }

        fn outer_size(&self) -> tauri::Result<Size> {
            self.query(Size {
                width: 1200.0,
                height: 800.0,
            })
        }

        fn move_to(&self, x: f64, y: f64) -> tauri::Result<()> {
            if self.fail_move {
                return Err(tauri::Error::FailedToReceiveMessage);
            }
            self.moves.borrow_mut().push((x, y));
            *self.current.borrow_mut() = self.primary.clone();
            Ok(())
        }

        fn focus(&self) -> tauri::Result<()> {
            self.query(())
        }
    }

    impl SystemUi for FakeWindow {
        fn hide(&self) -> Result<bool, SystemUiError> {
            if self.fail_ui.get() {
                return Err(SystemUiError::NotMainThread);
            }
            if self.fullscreen.get() && !self.ui_hidden.get() {
                self.ui_hidden.set(true);
                self.journal.borrow_mut().push("hide");
            }
            Ok(self.ui_hidden.get())
        }

        fn show(&self) -> Result<(), SystemUiError> {
            if self.fail_ui.get() {
                return Err(SystemUiError::NotMainThread);
            }
            if self.ui_hidden.get() && self.fullscreen.get() {
                self.ui_hidden.set(false);
                self.journal.borrow_mut().push("show");
            }
            Ok(())
        }

        fn clear_after_fullscreen(&self) -> Result<bool, SystemUiError> {
            if self.fail_ui.get() {
                return Err(SystemUiError::NotMainThread);
            }
            if self.fullscreen.get() {
                return Ok(false);
            }
            self.ui_hidden.set(false);
            self.journal.borrow_mut().push("clear after fullscreen");
            Ok(true)
        }

        fn is_hidden(&self) -> Result<bool, SystemUiError> {
            Ok(self.ui_hidden.get())
        }
    }

    fn workspace() -> Workspace {
        Workspace::new(EmergencyExitInfo::available())
    }

    #[test]
    fn enter_makes_the_window_fullscreen_on_the_primary_display() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();

        let outcome = workspace.enter(&window, &window).unwrap();

        assert!(outcome.changed);
        assert!(window.fullscreen.get());
        assert!(window.moves.borrow().is_empty(), "already on primary");
        assert_eq!(outcome.snapshot.status, Status::Active);
        assert_eq!(
            outcome.snapshot.display,
            Some(WorkspaceDisplay {
                name: Some("Primary".into()),
                primary: true
            })
        );
    }

    #[test]
    fn enter_moves_the_window_from_a_secondary_display_first() {
        let workspace = workspace();
        let window = FakeWindow::on_secondary();

        let outcome = workspace.enter(&window, &window).unwrap();

        assert_eq!(*window.moves.borrow(), vec![(360.0, 140.0)]);
        assert!(window.fullscreen.get());
        assert!(outcome.snapshot.display.unwrap().primary);
    }

    #[test]
    fn enter_still_works_when_the_window_cannot_be_moved() {
        let workspace = workspace();
        let window = FakeWindow {
            fail_move: true,
            ..FakeWindow::on_secondary()
        };

        let outcome = workspace.enter(&window, &window).unwrap();

        assert!(window.fullscreen.get());
        assert_eq!(
            outcome.snapshot.display,
            Some(WorkspaceDisplay {
                name: Some("Secondary".into()),
                primary: false
            }),
            "reports where it actually opened"
        );
    }

    #[test]
    fn enter_still_works_when_displays_cannot_be_queried() {
        let workspace = workspace();
        let window = FakeWindow {
            fail_queries: true,
            ..FakeWindow::on_primary()
        };

        let outcome = workspace.enter(&window, &window).unwrap();

        assert!(window.fullscreen.get());
        assert_eq!(outcome.snapshot.status, Status::Active);
        assert_eq!(outcome.snapshot.display, None);
    }

    #[test]
    fn failed_enter_leaves_the_state_unchanged() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        window.fail_fullscreen.set(true);

        let err = workspace.enter(&window, &window).unwrap_err();

        assert!(matches!(err, WorkspaceError::Fullscreen(_)), "{err}");
        assert_eq!(workspace.snapshot().status, Status::Inactive);
        assert_eq!(workspace.snapshot().revision, 0);
    }

    #[test]
    fn exit_leaves_fullscreen_and_records_the_reason() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        workspace.enter(&window, &window).unwrap();

        let outcome = workspace
            .exit(&window, &window, ExitReason::EmergencyShortcut)
            .unwrap();

        assert!(outcome.changed);
        assert!(!window.fullscreen.get());
        assert_eq!(outcome.snapshot.status, Status::Inactive);
        assert_eq!(
            outcome.snapshot.last_exit,
            Some(ExitReason::EmergencyShortcut)
        );
        assert_eq!(outcome.snapshot.display, None);
    }

    #[test]
    fn exit_always_asks_the_window_to_leave_fullscreen() {
        // Even when the state thinks the workspace is closed, e.g. the user
        // made the window fullscreen with the green button.
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        window.fullscreen.set(true);

        let outcome = workspace
            .exit(&window, &window, ExitReason::ExitControl)
            .unwrap();

        assert!(!outcome.changed);
        assert_eq!(*window.set_fullscreen_calls.borrow(), vec![false]);
        assert!(!window.fullscreen.get());
    }

    #[test]
    fn failed_exit_keeps_the_workspace_open() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        workspace.enter(&window, &window).unwrap();
        window.fail_fullscreen.set(true);

        assert!(workspace
            .exit(&window, &window, ExitReason::ExitControl)
            .is_err());
        assert_eq!(workspace.snapshot().status, Status::Active);

        // The window is still fullscreen, so a later attempt can succeed.
        window.fail_fullscreen.set(false);
        assert!(
            workspace
                .exit(&window, &window, ExitReason::ExitControl)
                .unwrap()
                .changed
        );
    }

    #[test]
    fn repeated_cycles_leave_the_window_as_it_started() {
        let workspace = workspace();
        let window = FakeWindow::on_secondary();
        let reasons = [ExitReason::ExitControl, ExitReason::EmergencyShortcut];

        for (cycle, reason) in reasons.iter().cycle().take(20).enumerate() {
            assert!(workspace.enter(&window, &window).unwrap().changed);
            assert!(window.fullscreen.get());
            assert!(window.ui_hidden.get());
            assert!(workspace.exit(&window, &window, *reason).unwrap().changed);
            assert!(!window.fullscreen.get());
            assert!(!window.ui_hidden.get());
            assert_eq!(workspace.snapshot().revision, (cycle as u64 + 1) * 2);
        }
        assert_eq!(workspace.snapshot().status, Status::Inactive);
        assert!(!window.ui_hidden.get());
        assert_eq!(window.moves.borrow().len(), 1, "moved once, then stayed");
    }

    #[test]
    fn reconcile_detects_leaving_through_the_system() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        workspace.enter(&window, &window).unwrap();

        assert_eq!(workspace.reconcile(&window), None, "still fullscreen");

        window.fullscreen.set(false); // e.g. the green window button
        let outcome = workspace.reconcile(&window).unwrap();

        assert!(outcome.changed);
        assert_eq!(outcome.snapshot.status, Status::Inactive);
        assert_eq!(outcome.snapshot.last_exit, Some(ExitReason::System));
    }

    #[test]
    fn reconcile_waits_for_a_deferred_fullscreen_request() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        window.defer_fullscreen.set(true);
        workspace.enter(&window, &window).unwrap();

        // Resizes during the previous animation: not fullscreen yet.
        assert_eq!(workspace.reconcile(&window), None);
        assert_eq!(workspace.snapshot().status, Status::Active);

        // The deferred request is applied, then the user leaves.
        window.fullscreen.set(true);
        assert_eq!(workspace.reconcile(&window), None);
        window.fullscreen.set(false);
        assert!(workspace.reconcile(&window).is_some());
    }

    #[test]
    fn reconcile_does_nothing_while_inactive() {
        let workspace = workspace();
        let window = FakeWindow {
            fail_queries: true,
            ..FakeWindow::on_primary()
        };
        window.fullscreen.set(true);

        assert_eq!(workspace.reconcile(&window), None);
        assert_eq!(workspace.snapshot().status, Status::Inactive);
    }

    #[test]
    fn exit_still_works_after_a_panic_poisoned_the_state() {
        let workspace = Arc::new(workspace());
        let window = FakeWindow::on_primary();
        workspace.enter(&window, &window).unwrap();

        let poisoner = Arc::clone(&workspace);
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.state.lock().unwrap();
            panic!("simulated panic while holding the workspace state");
        })
        .join();
        assert!(workspace.state.is_poisoned());

        let outcome = workspace
            .exit(&window, &window, ExitReason::ExitControl)
            .unwrap();
        assert_eq!(outcome.snapshot.status, Status::Inactive);
        assert!(!window.fullscreen.get());
    }

    #[test]
    fn dock_is_hidden_once_the_window_is_fullscreen() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        window.defer_fullscreen.set(true);
        workspace.enter(&window, &window).unwrap();
        assert!(!window.ui_hidden.get(), "not fullscreen yet");

        window.fullscreen.set(true); // macOS finished entering
        workspace.reconcile(&window);
        assert!(!window.ui_hidden.get(), "reconcile leaves them alone");
        workspace.sync_system_ui(&window); // the deferred call
        assert!(window.ui_hidden.get());
    }

    #[test]
    fn dock_is_hidden_at_once_if_the_window_was_already_fullscreen() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        window.fullscreen.set(true); // e.g. the green button, before entering

        workspace.enter(&window, &window).unwrap();
        assert!(window.ui_hidden.get());
    }

    #[test]
    fn exit_shows_the_dock_before_leaving_fullscreen() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        workspace.enter(&window, &window).unwrap();
        window.journal.borrow_mut().clear();

        workspace
            .exit(&window, &window, ExitReason::ExitControl)
            .unwrap();

        assert_eq!(*window.journal.borrow(), ["show", "leave fullscreen"]);
    }

    #[test]
    fn leaving_through_the_system_clears_the_dock_afterwards() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        workspace.enter(&window, &window).unwrap();
        assert!(window.ui_hidden.get());
        window.journal.borrow_mut().clear();

        window.fullscreen.set(false); // the green window button
        workspace.reconcile(&window).unwrap();
        assert!(workspace.needs_system_ui_sync());
        workspace.sync_system_ui(&window);

        assert!(!window.ui_hidden.get());
        assert_eq!(*window.journal.borrow(), ["clear after fullscreen"]);
        assert!(!workspace.needs_system_ui_sync());
    }

    #[test]
    fn clearing_waits_until_fullscreen_has_ended() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        workspace.enter(&window, &window).unwrap();
        // The state closed while macOS is still leaving fullscreen.
        workspace.lock().observe_fullscreen(false);

        workspace.sync_system_ui(&window);
        assert!(window.ui_hidden.get(), "too early");
        assert!(workspace.needs_system_ui_sync());

        window.fullscreen.set(false);
        workspace.sync_system_ui(&window);
        assert!(!window.ui_hidden.get());
    }

    #[test]
    fn exit_does_not_touch_a_dock_it_did_not_hide() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        window.defer_fullscreen.set(true);
        workspace.enter(&window, &window).unwrap();
        window.ui_hidden.set(true); // hidden by someone else

        workspace
            .exit(&window, &window, ExitReason::ExitControl)
            .unwrap();
        workspace.sync_system_ui(&window);

        assert!(window.ui_hidden.get());
        assert!(window.journal.borrow().is_empty());
    }

    #[test]
    fn ordinary_window_resizes_need_no_dock_sync() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        assert!(!workspace.needs_system_ui_sync());

        workspace.enter(&window, &window).unwrap();
        assert!(workspace.needs_system_ui_sync());
        workspace
            .exit(&window, &window, ExitReason::ExitControl)
            .unwrap();
        assert!(!workspace.needs_system_ui_sync());
    }

    #[test]
    fn failed_exit_keeps_the_ui_hidden() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        workspace.enter(&window, &window).unwrap();
        window.fail_fullscreen.set(true);

        assert!(workspace
            .exit(&window, &window, ExitReason::ExitControl)
            .is_err());

        assert!(window.ui_hidden.get(), "still in the workspace");
    }

    #[test]
    fn dock_failures_never_block_entering_or_leaving() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        window.fail_ui.set(true);

        assert!(workspace.enter(&window, &window).unwrap().changed);
        assert_eq!(workspace.reconcile(&window), None);
        workspace.sync_system_ui(&window);
        let outcome = workspace
            .exit(&window, &window, ExitReason::EmergencyShortcut)
            .unwrap();

        assert_eq!(outcome.snapshot.status, Status::Inactive);
        assert!(!window.fullscreen.get());
    }

    #[test]
    fn snapshot_reports_emergency_exit_availability() {
        let workspace = Workspace::new(EmergencyExitInfo::unavailable("taken"));
        let snapshot = workspace.snapshot();
        assert!(!snapshot.emergency_exit.available);

        let json = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(json["status"], "inactive");
        assert_eq!(json["revision"], 0);
        assert!(json["lastExit"].is_null());
        assert_eq!(json["emergencyExit"]["available"], false);
    }
}
