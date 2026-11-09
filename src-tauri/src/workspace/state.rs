//! The workspace state machine.
//!
//! Pure and synchronous: it records what the controller has done and what the
//! window has been observed to do, and never touches a window itself.
//!
//! ```text
//!              enter                       fullscreen observed
//! Inactive ───────────▶ Active (unconfirmed) ─────────────────▶ Active (confirmed)
//!    ▲                        │                                       │
//!    └──── exit (any reason) ─┴───── exit, or window left fullscreen ─┘
//! ```
//!
//! "Unconfirmed" covers the window still on its way into fullscreen. On macOS
//! a fullscreen request made during another fullscreen animation is deferred
//! until that animation ends, so a non-fullscreen window right after entering
//! does not mean the user left. Only a window that *was* seen fullscreen and
//! then is not counts as having left via the operating system.

use serde::Serialize;

/// Whether the fullscreen workspace is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Inactive,
    Active,
}

/// How the workspace was last left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ExitReason {
    /// The on-screen exit control.
    ExitControl,
    /// The global emergency-exit shortcut.
    EmergencyShortcut,
    /// The window left fullscreen through the operating system, for example
    /// the green window button or View › Exit Full Screen on macOS.
    System,
}

/// The display the workspace is open on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDisplay {
    pub name: Option<String>,
    /// `false` when the primary display could not be determined.
    pub primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Phase {
    #[default]
    Inactive,
    Active {
        confirmed: bool,
    },
}

/// Current workspace state. Every visible change increments `revision`, so
/// the frontend can discard snapshots that arrive out of order.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceState {
    revision: u64,
    phase: Phase,
    display: Option<WorkspaceDisplay>,
    last_exit: Option<ExitReason>,
}

impl WorkspaceState {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn status(&self) -> Status {
        match self.phase {
            Phase::Inactive => Status::Inactive,
            Phase::Active { .. } => Status::Active,
        }
    }

    pub fn display(&self) -> Option<&WorkspaceDisplay> {
        self.display.as_ref()
    }

    pub fn last_exit(&self) -> Option<ExitReason> {
        self.last_exit
    }

    /// Records that the window was asked to go fullscreen. `fullscreen_now`
    /// is whether the window already reports fullscreen. Returns whether the
    /// visible state changed.
    pub fn enter(&mut self, display: Option<WorkspaceDisplay>, fullscreen_now: bool) -> bool {
        match self.phase {
            Phase::Active { confirmed } => {
                // Already open. Entering again only firms up the confirmation.
                self.phase = Phase::Active {
                    confirmed: confirmed || fullscreen_now,
                };
                false
            }
            Phase::Inactive => {
                self.phase = Phase::Active {
                    confirmed: fullscreen_now,
                };
                self.display = display;
                self.revision += 1;
                true
            }
        }
    }

    /// Records that the window was asked to leave fullscreen. Returns whether
    /// the visible state changed.
    pub fn exit(&mut self, reason: ExitReason) -> bool {
        match self.phase {
            Phase::Inactive => false,
            Phase::Active { .. } => {
                self.close(reason);
                true
            }
        }
    }

    /// Records the window's observed fullscreen state. Returns whether the
    /// visible state changed (only when the window left fullscreen by itself).
    pub fn observe_fullscreen(&mut self, fullscreen: bool) -> bool {
        match (self.phase, fullscreen) {
            (Phase::Active { confirmed: false }, true) => {
                self.phase = Phase::Active { confirmed: true };
                false
            }
            (Phase::Active { confirmed: true }, false) => {
                self.close(ExitReason::System);
                true
            }
            // Still entering, already confirmed, or not open: nothing to do.
            _ => false,
        }
    }

    fn close(&mut self, reason: ExitReason) {
        self.phase = Phase::Inactive;
        self.display = None;
        self.last_exit = Some(reason);
        self.revision += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn primary() -> Option<WorkspaceDisplay> {
        Some(WorkspaceDisplay {
            name: Some("Built-in".into()),
            primary: true,
        })
    }

    #[test]
    fn starts_inactive() {
        let state = WorkspaceState::default();
        assert_eq!(state.status(), Status::Inactive);
        assert_eq!(state.revision(), 0);
        assert_eq!(state.last_exit(), None);
        assert_eq!(state.display(), None);
    }

    #[test]
    fn enter_then_exit() {
        let mut state = WorkspaceState::default();

        assert!(state.enter(primary(), true));
        assert_eq!(state.status(), Status::Active);
        assert_eq!(state.display(), primary().as_ref());
        assert_eq!(state.revision(), 1);

        assert!(state.exit(ExitReason::ExitControl));
        assert_eq!(state.status(), Status::Inactive);
        assert_eq!(state.display(), None);
        assert_eq!(state.last_exit(), Some(ExitReason::ExitControl));
        assert_eq!(state.revision(), 2);
    }

    #[test]
    fn repeated_cycles_always_return_to_inactive() {
        let mut state = WorkspaceState::default();
        for cycle in 1..=50 {
            assert!(state.enter(primary(), true));
            assert!(state.exit(ExitReason::EmergencyShortcut));
            assert_eq!(state.status(), Status::Inactive);
            assert_eq!(state.revision(), cycle * 2);
        }
    }

    #[test]
    fn entering_twice_is_not_a_change() {
        let mut state = WorkspaceState::default();
        assert!(state.enter(primary(), true));
        assert!(!state.enter(None, true));
        assert_eq!(state.revision(), 1);
        assert_eq!(state.display(), primary().as_ref(), "display is kept");
    }

    #[test]
    fn exiting_while_inactive_is_not_a_change() {
        let mut state = WorkspaceState::default();
        assert!(!state.exit(ExitReason::ExitControl));
        assert_eq!(state.revision(), 0);
        assert_eq!(state.last_exit(), None);
    }

    #[test]
    fn leaving_fullscreen_through_the_system_closes_the_workspace() {
        let mut state = WorkspaceState::default();
        state.enter(primary(), true);

        assert!(!state.observe_fullscreen(true));
        assert!(state.observe_fullscreen(false));
        assert_eq!(state.status(), Status::Inactive);
        assert_eq!(state.last_exit(), Some(ExitReason::System));
    }

    #[test]
    fn not_yet_fullscreen_while_entering_is_ignored() {
        // macOS defers a fullscreen request made during another animation.
        let mut state = WorkspaceState::default();
        state.enter(primary(), false);

        assert!(!state.observe_fullscreen(false));
        assert_eq!(state.status(), Status::Active);

        // Once fullscreen has been seen, leaving it does close the workspace.
        assert!(!state.observe_fullscreen(true));
        assert!(state.observe_fullscreen(false));
        assert_eq!(state.status(), Status::Inactive);
    }

    #[test]
    fn re_entering_confirms_a_pending_entry() {
        let mut state = WorkspaceState::default();
        state.enter(primary(), false);
        state.enter(primary(), true);
        assert!(state.observe_fullscreen(false));
    }

    #[test]
    fn observations_while_inactive_are_ignored() {
        let mut state = WorkspaceState::default();
        assert!(!state.observe_fullscreen(true));
        assert!(!state.observe_fullscreen(false));
        assert_eq!(state.status(), Status::Inactive);
        assert_eq!(state.revision(), 0);
    }

    #[test]
    fn explicit_exit_works_before_fullscreen_is_confirmed() {
        let mut state = WorkspaceState::default();
        state.enter(primary(), false);
        assert!(state.exit(ExitReason::ExitControl));
        assert_eq!(state.status(), Status::Inactive);
    }

    #[test]
    fn serializes_for_the_frontend() {
        assert_eq!(serde_json::to_value(Status::Active).unwrap(), "active");
        assert_eq!(
            serde_json::to_value(ExitReason::EmergencyShortcut).unwrap(),
            "emergencyShortcut"
        );
    }
}
