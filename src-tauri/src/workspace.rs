//! Desktop presentation: the fullscreen workspace and its exits.
//! (The approved-application launcher arrives in Phase 4.)
//!
//! Entering the workspace makes the main window fullscreen on the primary
//! display using the operating system's own fullscreen mode. On macOS that is
//! native fullscreen in its own Space, and WHITE ROOM also hides the Dock and
//! the menu bar completely while the workspace is open (see
//! [`crate::platform::presentation`]).
//! Both are scoped to WHITE ROOM: macOS undoes them when the app quits or
//! crashes, so nothing about the desktop is changed persistently.
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

use crate::platform::presentation::NativeSystemUi;

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

/// Enters the workspace and notifies the frontend. Must run on the main
/// thread, like everything that changes the Dock and menu bar.
pub fn enter<R: Runtime>(app: &AppHandle<R>) -> Result<WorkspaceSnapshot, WorkspaceError> {
    let window = main_window(app)?;
    let workspace = app
        .try_state::<Workspace>()
        .ok_or(WorkspaceError::NotReady)?;
    let outcome = workspace.enter(&window, &NativeSystemUi)?;
    publish(app, &outcome);
    Ok(outcome.snapshot)
}

/// Leaves the workspace and notifies the frontend. Must run on the main
/// thread.
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
    let outcome = workspace.exit(&window, &NativeSystemUi, reason)?;
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
    if workspace.needs_system_ui_sync() {
        sync_system_ui_later(app.clone());
    }
}

/// Runs [`Workspace::sync_system_ui`] on a later turn of the main loop.
///
/// Window events arrive inside AppKit's fullscreen transition callbacks,
/// where changes to the Dock and menu bar are silently ignored.
/// `run_on_main_thread` would run the task inline when called from the main
/// thread, so it is called from a background task instead, which queues it
/// behind the current event.
fn sync_system_ui_later<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let handle = app.clone();
        let queued = app.run_on_main_thread(move || {
            if let Some(workspace) = handle.try_state::<Workspace>() {
                workspace.sync_system_ui(&NativeSystemUi);
            }
        });
        if let Err(err) = queued {
            log::warn!("could not schedule a Dock and menu bar update: {err}");
        }
    });
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
