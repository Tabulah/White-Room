//! Desktop presentation: the fullscreen workspace and its exits.
//! (The approved-application launcher arrives in Phase 4.)
//!
//! Entering the workspace makes the main window fullscreen on the primary
//! display using the operating system's own fullscreen mode. On macOS that is
//! native fullscreen in its own Space: macOS hides and restores the Dock and
//! menu bar itself, and nothing about the desktop is changed persistently, so
//! even a crash cannot leave it altered. Explicit presentation options come in
//! Phase 2, together with their snapshot and restore machinery.
//!
//! There are four independent ways out, so no single failure traps the user:
//!
//! 1. the on-screen exit control (`exit_workspace` command);
//! 2. the global emergency shortcut ([`emergency_exit`]), handled entirely in
//!    Rust, so it works even if the interface has stopped responding;
//! 3. the operating system's own fullscreen controls, which are never
//!    disabled; the state follows along via [`on_window_event`];
//! 4. quitting the app, which ends fullscreen along with the window.
//!
//! See `docs/workspace.md` for the user-facing behaviour.

mod controller;
mod display;
pub mod emergency_exit;
mod state;
mod window;

use tauri::{AppHandle, Emitter, Manager, Runtime, WebviewWindow, Window, WindowEvent};

pub use controller::{Outcome, Workspace, WorkspaceError, WorkspaceSnapshot};
pub use emergency_exit::EmergencyExitInfo;
pub use state::{ExitReason, Status, WorkspaceDisplay};
pub use window::WorkspaceWindow;

/// Label of the window that hosts the workspace (see `tauri.conf.json`).
pub const MAIN_WINDOW_LABEL: &str = "main";

/// Event sent to the main window whenever the workspace state changes.
/// The payload is a [`WorkspaceSnapshot`].
pub const CHANGED_EVENT: &str = "workspace://changed";

/// Registers the emergency shortcut and returns the workspace state to manage.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> Workspace {
    let emergency_exit = emergency_exit::register(app, |app| {
        // Runs on the main thread, outside the webview.
        if let Err(err) = exit(app, ExitReason::EmergencyShortcut) {
            log::error!("emergency exit failed: {err}");
        }
    });
    Workspace::new(emergency_exit)
}

/// Enters the workspace and notifies the frontend.
pub fn enter<R: Runtime>(app: &AppHandle<R>) -> Result<WorkspaceSnapshot, WorkspaceError> {
    let window = main_window(app)?;
    let workspace = app
        .try_state::<Workspace>()
        .ok_or(WorkspaceError::NotReady)?;
    let outcome = workspace.enter(&window)?;
    publish(app, &outcome);
    Ok(outcome.snapshot)
}

/// Leaves the workspace and notifies the frontend.
///
/// If the workspace state is not registered yet, the window is still taken
/// out of fullscreen before reporting [`WorkspaceError::NotReady`]: leaving
/// must never depend on bookkeeping.
pub fn exit<R: Runtime>(
    app: &AppHandle<R>,
    reason: ExitReason,
) -> Result<WorkspaceSnapshot, WorkspaceError> {
    let window = main_window(app)?;
    let Some(workspace) = app.try_state::<Workspace>() else {
        window
            .set_fullscreen(false)
            .map_err(WorkspaceError::Fullscreen)?;
        return Err(WorkspaceError::NotReady);
    };
    let outcome = workspace.exit(&window, reason)?;
    publish(app, &outcome);
    Ok(outcome.snapshot)
}

/// Keeps the workspace state in step with the window, e.g. when the user
/// leaves fullscreen with the operating system's controls.
pub fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if window.label() != MAIN_WINDOW_LABEL || !matches!(event, WindowEvent::Resized(_)) {
        return;
    }
    let app = window.app_handle();
    // Resize events can arrive before setup has registered the state.
    let Some(workspace) = app.try_state::<Workspace>() else {
        return;
    };
    let Ok(window) = main_window(app) else {
        return;
    };
    if let Some(outcome) = workspace.reconcile(&window) {
        publish(app, &outcome);
    }
}

fn main_window<R: Runtime>(app: &AppHandle<R>) -> Result<WebviewWindow<R>, WorkspaceError> {
    app.get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or(WorkspaceError::WindowNotFound)
}

fn publish<R: Runtime>(app: &AppHandle<R>, outcome: &Outcome) {
    if !outcome.changed {
        return;
    }
    if let Err(err) = app.emit_to(MAIN_WINDOW_LABEL, CHANGED_EVENT, &outcome.snapshot) {
        // The frontend also receives the snapshot as the command's return
        // value, and re-reads it on load, so this is not fatal.
        log::warn!("could not notify the frontend of a workspace change: {err}");
    }
}
