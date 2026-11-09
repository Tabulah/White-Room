//! Debug-build smoke test of the running app, used by CI on macOS and Windows.
//!
//! Start a debug build with `WHITE_ROOM_SMOKE_TEST=1` and, instead of waiting
//! for a user, the app checks itself:
//!
//! 1. the database was created and migrated to the latest schema, and the log
//!    file exists;
//! 2. the frontend loaded and reached the backend over IPC;
//! 3. the emergency-exit shortcut was accepted by the operating system;
//! 4. it enters and leaves the workspace several times, alternating the exit
//!    control and emergency-shortcut code paths, and after every exit checks
//!    that the window is out of fullscreen and back at exactly its original
//!    position and size.
//!
//! It then quits with exit code 0 (pass) or 1 (fail), printing progress to
//! stdout with a `[smoke]` prefix. Release builds do not contain this module.
//!
//! Not covered: physically pressing the shortcut (synthesising global key
//! presses needs accessibility permissions, so the handler's code path is
//! called directly), and how the interface looks.

use std::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

use tauri::{AppHandle, Manager, Runtime, WebviewWindow};

use super::logging::LOG_FILE_NAME;
use crate::{
    database::{self, Database, DATABASE_FILE_NAME},
    workspace::{self, ExitReason, Status, Workspace, MAIN_WINDOW_LABEL},
};

/// Environment variable that starts the smoke test when set to `1`.
pub const ENV_VAR: &str = "WHITE_ROOM_SMOKE_TEST";

const CYCLES: u32 = 3;
const FRONTEND_TIMEOUT: Duration = Duration::from_secs(60);
const TRANSITION_TIMEOUT: Duration = Duration::from_secs(20);
const POLL_INTERVAL: Duration = Duration::from_millis(100);
/// A condition must hold for this many consecutive polls, so that a window
/// still animating is not mistaken for one that has settled.
const STABLE_POLLS: u32 = 5;
/// Allowed difference, in physical pixels, between the window frame before
/// entering and after leaving.
const FRAME_TOLERANCE: i64 = 2;

static FRONTEND_CONNECTED: AtomicBool = AtomicBool::new(false);

/// Called by the `get_workspace_state` command, which the frontend invokes on
/// load. Proves the webview, IPC, and capability chain work end to end.
pub fn note_frontend_connected() {
    FRONTEND_CONNECTED.store(true, Ordering::SeqCst);
}

/// Starts the smoke test on a background thread if [`ENV_VAR`] is `1`.
pub fn start_if_requested<R: Runtime>(app: &AppHandle<R>) {
    if std::env::var(ENV_VAR).as_deref() != Ok("1") {
        return;
    }
    println!("[smoke] starting on {}", std::env::consts::OS);
    let app = app.clone();
    thread::spawn(move || {
        let code = match run(&app) {
            Ok(()) => {
                println!("[smoke] PASS");
                0
            }
            Err(err) => {
                println!("[smoke] FAIL: {err}");
                1
            }
        };
        log::info!("smoke test finished with exit code {code}");
        app.exit(code);
    });
}

fn run<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    check_storage(app)?;
    wait_for_frontend()?;
    check_emergency_shortcut(app)?;

    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or("main window not found")?;
    if window.is_fullscreen().map_err(text)? {
        return Err("window was already fullscreen before the test".into());
    }
    let initial = Frame::of(&window)?;
    println!("[smoke] initial window frame: {initial}");

    for cycle in 1..=CYCLES {
        let reason = if cycle % 2 == 1 {
            ExitReason::ExitControl
        } else {
            ExitReason::EmergencyShortcut
        };

        let entered = workspace::enter(app).map_err(text)?;
        if entered.status != Status::Active {
            return Err(format!("cycle {cycle}: enter returned {entered:?}"));
        }
        wait_until(&format!("cycle {cycle}: window fullscreen"), || {
            is_fullscreen_workspace(app, &window)
        })?;

        // `ExitReason::EmergencyShortcut` goes through the same function the
        // shortcut handler calls.
        let left = workspace::exit(app, reason).map_err(text)?;
        if left.status != Status::Inactive {
            return Err(format!("cycle {cycle}: exit returned {left:?}"));
        }
        wait_until(&format!("cycle {cycle}: window restored"), || {
            is_restored(app, &window, &initial)
        })?;

        println!("[smoke] cycle {cycle}/{CYCLES}: entered, left via {reason:?}, window restored");
    }
    Ok(())
}

fn check_storage<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let schema = app.state::<Database>().schema_version().map_err(text)?;
    let latest = database::latest_schema_version();
    if schema != latest {
        return Err(format!("database schema is {schema}, expected {latest}"));
    }
    let db_path = app
        .path()
        .app_data_dir()
        .map_err(text)?
        .join(DATABASE_FILE_NAME);
    if !db_path.is_file() {
        return Err(format!("database file missing at {}", db_path.display()));
    }
    println!("[smoke] database: schema {schema} at {}", db_path.display());

    let log_path = app
        .path()
        .app_log_dir()
        .map_err(text)?
        .join(format!("{LOG_FILE_NAME}.log"));
    if !log_path.is_file() {
        return Err(format!("log file missing at {}", log_path.display()));
    }
    println!("[smoke] log file: {}", log_path.display());
    Ok(())
}

fn wait_for_frontend() -> Result<(), String> {
    let start = Instant::now();
    while !FRONTEND_CONNECTED.load(Ordering::SeqCst) {
        if start.elapsed() > FRONTEND_TIMEOUT {
            return Err(format!(
                "frontend did not call the backend within {}s",
                FRONTEND_TIMEOUT.as_secs()
            ));
        }
        thread::sleep(POLL_INTERVAL);
    }
    println!(
        "[smoke] frontend connected after {:.1}s",
        start.elapsed().as_secs_f64()
    );
    Ok(())
}

fn check_emergency_shortcut<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let info = app.state::<Workspace>().snapshot().emergency_exit;
    if !info.available {
        return Err(format!(
            "emergency exit shortcut unavailable: {}",
            info.unavailable_reason.unwrap_or_default()
        ));
    }
    println!(
        "[smoke] emergency exit shortcut registered: {}",
        info.description
    );
    Ok(())
}

fn is_fullscreen_workspace<R: Runtime>(
    app: &AppHandle<R>,
    window: &WebviewWindow<R>,
) -> Result<Observation, String> {
    let status = app.state::<Workspace>().snapshot().status;
    let fullscreen = window.is_fullscreen().map_err(text)?;
    let frame = Frame::of(window)?;
    let monitor_width = window
        .current_monitor()
        .map_err(text)?
        .map(|monitor| i64::from(monitor.size().width));
    // `is_fullscreen` can report the requested state before the animation
    // has finished, so also require the window to span the display.
    let spans_display =
        monitor_width.is_some_and(|width| (frame.width - width).abs() <= FRAME_TOLERANCE);
    Ok(Observation {
        ok: status == Status::Active && fullscreen && spans_display,
        detail: format!(
            "status {status:?}, fullscreen {fullscreen}, frame {frame}, display width {monitor_width:?}"
        ),
    })
}

fn is_restored<R: Runtime>(
    app: &AppHandle<R>,
    window: &WebviewWindow<R>,
    initial: &Frame,
) -> Result<Observation, String> {
    let status = app.state::<Workspace>().snapshot().status;
    let fullscreen = window.is_fullscreen().map_err(text)?;
    let frame = Frame::of(window)?;
    Ok(Observation {
        ok: status == Status::Inactive && !fullscreen && frame.matches(initial),
        detail: format!(
            "status {status:?}, fullscreen {fullscreen}, frame {frame} (expected {initial})"
        ),
    })
}

struct Observation {
    ok: bool,
    detail: String,
}

/// Polls `observe` until it has been `ok` for [`STABLE_POLLS`] polls in a row.
fn wait_until(
    what: &str,
    mut observe: impl FnMut() -> Result<Observation, String>,
) -> Result<(), String> {
    let start = Instant::now();
    let mut streak = 0;
    loop {
        let observation = observe()?;
        streak = if observation.ok { streak + 1 } else { 0 };
        if streak >= STABLE_POLLS {
            return Ok(());
        }
        if start.elapsed() > TRANSITION_TIMEOUT {
            return Err(format!(
                "{what}: not reached within {}s; last seen: {}",
                TRANSITION_TIMEOUT.as_secs(),
                observation.detail
            ));
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// Window position and size in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Frame {
    x: i64,
    y: i64,
    width: i64,
    height: i64,
}

impl Frame {
    fn of<R: Runtime>(window: &WebviewWindow<R>) -> Result<Self, String> {
        let position = window.outer_position().map_err(text)?;
        let size = window.outer_size().map_err(text)?;
        Ok(Self {
            x: i64::from(position.x),
            y: i64::from(position.y),
            width: i64::from(size.width),
            height: i64::from(size.height),
        })
    }

    fn matches(&self, other: &Frame) -> bool {
        (self.x - other.x).abs() <= FRAME_TOLERANCE
            && (self.y - other.y).abs() <= FRAME_TOLERANCE
            && (self.width - other.width).abs() <= FRAME_TOLERANCE
            && (self.height - other.height).abs() <= FRAME_TOLERANCE
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}x{} at ({}, {})",
            self.width, self.height, self.x, self.y
        )
    }
}

fn text(err: impl fmt::Display) -> String {
    err.to_string()
}
