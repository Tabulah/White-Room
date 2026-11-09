//! Entering and leaving the workspace.
//!
//! [`Workspace`] is managed Tauri state. It applies window changes through
//! [`WorkspaceWindow`] and records them in [`WorkspaceState`].
//!
//! Locking rule: the state lock is never held while calling the window. Window
//! getters called off the main thread block until the main thread answers, and
//! the main thread may itself be waiting for this lock (the emergency shortcut
//! and window events run there), so holding both would deadlock.

use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;

use super::{
    display::{self, Display, Placement},
    emergency_exit::EmergencyExitInfo,
    state::{ExitReason, Status, WorkspaceDisplay, WorkspaceState},
    window::WorkspaceWindow,
};

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
}

impl Workspace {
    pub fn new(emergency_exit: EmergencyExitInfo) -> Self {
        Self {
            state: Mutex::default(),
            emergency_exit,
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
    pub fn enter(&self, window: &impl WorkspaceWindow) -> Result<Outcome, WorkspaceError> {
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

        let mut state = self.lock();
        let changed = state.enter(display, fullscreen_now);
        if changed {
            log::info!("entered workspace on {}", describe(state.display()));
        }
        Ok(self.outcome(&state, changed))
    }

    /// Takes the window out of fullscreen.
    ///
    /// The window is always asked to leave fullscreen, whatever the recorded
    /// state says, so that exiting works even if that state is wrong. If the
    /// window refuses, the state is left unchanged and the error returned.
    pub fn exit(
        &self,
        window: &impl WorkspaceWindow,
        reason: ExitReason,
    ) -> Result<Outcome, WorkspaceError> {
        window
            .set_fullscreen(false)
            .map_err(WorkspaceError::Fullscreen)?;

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
    use crate::workspace::display::{Rect, Size};

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

    fn workspace() -> Workspace {
        Workspace::new(EmergencyExitInfo::available())
    }

    #[test]
    fn enter_makes_the_window_fullscreen_on_the_primary_display() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();

        let outcome = workspace.enter(&window).unwrap();

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

        let outcome = workspace.enter(&window).unwrap();

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

        let outcome = workspace.enter(&window).unwrap();

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

        let outcome = workspace.enter(&window).unwrap();

        assert!(window.fullscreen.get());
        assert_eq!(outcome.snapshot.status, Status::Active);
        assert_eq!(outcome.snapshot.display, None);
    }

    #[test]
    fn failed_enter_leaves_the_state_unchanged() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        window.fail_fullscreen.set(true);

        let err = workspace.enter(&window).unwrap_err();

        assert!(matches!(err, WorkspaceError::Fullscreen(_)), "{err}");
        assert_eq!(workspace.snapshot().status, Status::Inactive);
        assert_eq!(workspace.snapshot().revision, 0);
    }

    #[test]
    fn exit_leaves_fullscreen_and_records_the_reason() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        workspace.enter(&window).unwrap();

        let outcome = workspace
            .exit(&window, ExitReason::EmergencyShortcut)
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

        let outcome = workspace.exit(&window, ExitReason::ExitControl).unwrap();

        assert!(!outcome.changed);
        assert_eq!(*window.set_fullscreen_calls.borrow(), vec![false]);
        assert!(!window.fullscreen.get());
    }

    #[test]
    fn failed_exit_keeps_the_workspace_open() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        workspace.enter(&window).unwrap();
        window.fail_fullscreen.set(true);

        assert!(workspace.exit(&window, ExitReason::ExitControl).is_err());
        assert_eq!(workspace.snapshot().status, Status::Active);

        // The window is still fullscreen, so a later attempt can succeed.
        window.fail_fullscreen.set(false);
        assert!(
            workspace
                .exit(&window, ExitReason::ExitControl)
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
            assert!(workspace.enter(&window).unwrap().changed);
            assert!(window.fullscreen.get());
            assert!(workspace.exit(&window, *reason).unwrap().changed);
            assert!(!window.fullscreen.get());
            assert_eq!(workspace.snapshot().revision, (cycle as u64 + 1) * 2);
        }
        assert_eq!(workspace.snapshot().status, Status::Inactive);
        assert_eq!(window.moves.borrow().len(), 1, "moved once, then stayed");
    }

    #[test]
    fn reconcile_detects_leaving_through_the_system() {
        let workspace = workspace();
        let window = FakeWindow::on_primary();
        workspace.enter(&window).unwrap();

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
        workspace.enter(&window).unwrap();

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
        workspace.enter(&window).unwrap();

        let poisoner = Arc::clone(&workspace);
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.state.lock().unwrap();
            panic!("simulated panic while holding the workspace state");
        })
        .join();
        assert!(workspace.state.is_poisoned());

        let outcome = workspace.exit(&window, ExitReason::ExitControl).unwrap();
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
