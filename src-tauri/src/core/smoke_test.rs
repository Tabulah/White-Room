//! Debug-build smoke test of the running app, used by CI on macOS and Windows.
//!
//! Start a debug build with `WHITE_ROOM_SMOKE_TEST=1` and, instead of waiting
//! for a user, the app checks itself:
//!
//! 1. the database was created and migrated to the latest schema, and the log
//!    file exists;
//! 2. the frontend loaded and reached the backend over IPC;
//! 3. the emergency-exit shortcut was accepted by the operating system;
//! 4. on macOS, the app can schedule notifications (it runs as an app bundle);
//! 5. it enters and leaves the workspace several times, alternating the exit
//!    control and emergency-shortcut code paths. Inside, it checks that the
//!    window is fullscreen and, on macOS, that the Dock and menu bar are
//!    hidden. After every exit it checks that they are back, that macOS
//!    records no changed presentation mode for the app any more (see
//!    [`launch_services_options`]), and that the window is out of fullscreen
//!    at exactly its original position and size;
//! 6. on macOS, one more cycle leaves through the system's own fullscreen
//!    toggle (what the green window button does), bypassing WHITE ROOM's exit
//!    path, and checks the same.
//!
//! It then quits with exit code 0 (pass) or 1 (fail), printing progress to
//! stdout with a `[smoke]` prefix. Release builds do not contain this module.
//!
//! With `WHITE_ROOM_SMOKE_TEST=hold` it runs the same checks, enters the
//! workspace once, prints `[smoke] HOLDING`, and waits to be killed. The
//! crash test (`scripts/crash-test.sh`) uses this to check that a crash
//! inside the workspace cannot leave the Dock or menu bar hidden.
//!
//! Not covered: physically pressing the shortcut (synthesising global key
//! presses needs accessibility permissions, so the handler's code path is
//! called directly), and how the interface looks.

use std::{
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use tauri::{AppHandle, Manager, Runtime, WebviewWindow};

use super::logging::LOG_FILE_NAME;
use crate::{
    database::{self, Database, DATABASE_FILE_NAME},
    platform::{
        notifications::{Availability, NativeScheduler, NotificationScheduler},
        presentation::{self, NativeSystemUi, PresentationOptions, SystemUi},
    },
    workspace::{self, ExitReason, Status, Workspace, WorkspaceSnapshot, MAIN_WINDOW_LABEL},
};

/// Environment variable that starts the smoke test: `1` for the full test,
/// `hold` to enter the workspace and wait to be killed.
pub const ENV_VAR: &str = "WHITE_ROOM_SMOKE_TEST";

/// Whether the workspace hides the Dock and menu bar on this platform.
const HIDES_SYSTEM_UI: bool = cfg!(target_os = "macos");

const CYCLES: u32 = 3;
const FRONTEND_TIMEOUT: Duration = Duration::from_secs(60);
const TRANSITION_TIMEOUT: Duration = Duration::from_secs(20);
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const MAIN_THREAD_TIMEOUT: Duration = Duration::from_secs(10);
/// In `hold` mode, give up if nobody has killed the app by then.
const HOLD_TIMEOUT: Duration = Duration::from_secs(120);
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

#[derive(Debug, Clone, Copy)]
enum Mode {
    Cycles,
    Hold,
}

/// Starts the smoke test on a background thread if [`ENV_VAR`] is set.
pub fn start_if_requested<R: Runtime>(app: &AppHandle<R>) {
    let mode = match std::env::var(ENV_VAR).as_deref() {
        Ok("1") => Mode::Cycles,
        Ok("hold") => Mode::Hold,
        _ => return,
    };
    println!("[smoke] starting on {} ({mode:?})", std::env::consts::OS);
    let app = app.clone();
    thread::spawn(move || {
        let code = match run(&app, mode) {
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

fn run<R: Runtime>(app: &AppHandle<R>, mode: Mode) -> Result<(), String> {
    check_storage(app)?;
    wait_for_frontend()?;
    check_emergency_shortcut(app)?;
    check_notifications()?;

    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or("main window not found")?;
    if window.is_fullscreen().map_err(text)? {
        return Err("window was already fullscreen before the test".into());
    }
    let initial = Frame::of(&window)?;
    println!("[smoke] initial window frame: {initial}");

    if let Mode::Hold = mode {
        return hold(app, &window);
    }

    for cycle in 1..=CYCLES {
        let reason = if cycle % 2 == 1 {
            ExitReason::ExitControl
        } else {
            ExitReason::EmergencyShortcut
        };

        let entered = enter(app)?;
        if entered.status != Status::Active {
            return Err(format!("cycle {cycle}: enter returned {entered:?}"));
        }
        wait_until(&format!("cycle {cycle}: window fullscreen"), || {
            is_fullscreen_workspace(app, &window)
        })?;

        // `ExitReason::EmergencyShortcut` goes through the same function the
        // shortcut handler calls.
        let left = on_main(app, move |app| workspace::exit(app, reason))?.map_err(text)?;
        if left.status != Status::Inactive {
            return Err(format!("cycle {cycle}: exit returned {left:?}"));
        }
        wait_until(&format!("cycle {cycle}: window restored"), || {
            is_restored(app, &window, &initial)
        })?;

        println!("[smoke] cycle {cycle}/{CYCLES}: entered, left via {reason:?}, window restored");
    }

    #[cfg(target_os = "macos")]
    system_exit_cycle(app, &window, &initial)?;
    Ok(())
}

/// Leaves the workspace the way the green window button does, so that
/// WHITE ROOM only finds out afterwards.
#[cfg(target_os = "macos")]
fn system_exit_cycle<R: Runtime>(
    app: &AppHandle<R>,
    window: &WebviewWindow<R>,
    initial: &Frame,
) -> Result<(), String> {
    enter(app)?;
    wait_until("system cycle: window fullscreen", || {
        is_fullscreen_workspace(app, window)
    })?;
    if !on_main(app, |_| presentation::toggle_fullscreen_like_the_system())?.map_err(text)? {
        return Err("system cycle: WHITE ROOM has no key window".into());
    }
    wait_until("system cycle: window restored", || {
        let last_exit = app.state::<Workspace>().snapshot().last_exit;
        let mut observation = is_restored(app, window, initial)?;
        observation.ok &= last_exit == Some(ExitReason::System);
        observation.detail += &format!(", last exit {last_exit:?}");
        Ok(observation)
    })?;
    println!(
        "[smoke] system cycle: entered, left via the system fullscreen toggle, window restored"
    );
    Ok(())
}

/// Enters the workspace, waits until it is fully open, then waits to be
/// killed.
fn hold<R: Runtime>(app: &AppHandle<R>, window: &WebviewWindow<R>) -> Result<(), String> {
    enter(app)?;
    wait_until("workspace open", || is_fullscreen_workspace(app, window))?;
    println!(
        "[smoke] HOLDING (pid {}): the workspace is open; kill this process to test crash recovery",
        std::process::id()
    );
    thread::sleep(HOLD_TIMEOUT);
    Err(format!(
        "nothing killed the app within {}s",
        HOLD_TIMEOUT.as_secs()
    ))
}

fn enter<R: Runtime>(app: &AppHandle<R>) -> Result<WorkspaceSnapshot, String> {
    on_main(app, |app| workspace::enter(app))?.map_err(text)
}

/// Runs `f` on the main thread, as the Tauri commands, window events, and
/// shortcut handler that normally call the workspace do, and waits for it.
fn on_main<R: Runtime, T: Send + 'static>(
    app: &AppHandle<R>,
    f: impl FnOnce(&AppHandle<R>) -> T + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(f(&handle));
    })
    .map_err(text)?;
    rx.recv_timeout(MAIN_THREAD_TIMEOUT)
        .map_err(|_| "the main thread did not respond".to_string())
}

fn is_ui_hidden<R: Runtime>(app: &AppHandle<R>) -> Result<bool, String> {
    on_main(app, |_| NativeSystemUi.is_hidden())?.map_err(text)
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

/// On macOS the smoke test runs from the app bundle, which is what
/// scheduling notifications needs. No permission is asked for.
fn check_notifications() -> Result<(), String> {
    match NativeScheduler.availability() {
        Availability::Available => println!("[smoke] notifications: can be scheduled"),
        Availability::Unavailable(reason) if cfg!(target_os = "macos") => {
            return Err(format!(
                "notifications unavailable: {reason} (run the smoke test from the .app bundle)"
            ));
        }
        Availability::Unavailable(reason) => {
            println!("[smoke] notifications: unavailable here, as expected ({reason})");
        }
    }
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
    let ui_hidden = is_ui_hidden(app)?;
    // Non-zero while hidden: shows the restore check below can see it.
    let recorded = launch_services_options()?;
    let recorded_ok = recorded.is_none_or(|options| (options != 0) == HIDES_SYSTEM_UI);
    Ok(Observation {
        ok: status == Status::Active
            && fullscreen
            && spans_display
            && ui_hidden == HIDES_SYSTEM_UI
            && recorded_ok,
        detail: format!(
            "status {status:?}, fullscreen {fullscreen}, frame {frame}, \
             display width {monitor_width:?}, Dock and menu bar hidden {ui_hidden}, \
             recorded mode {recorded:?}"
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
    let ui_hidden = is_ui_hidden(app)?;
    // `None` on platforms without presentation options.
    let system = on_main(app, |_| presentation::system_options())?;
    let system_clean = system.is_none_or(|options| options == PresentationOptions::DEFAULT);
    let recorded = launch_services_options()?;
    Ok(Observation {
        ok: status == Status::Inactive
            && !fullscreen
            && !ui_hidden
            && system_clean
            && recorded.is_none_or(|options| options == 0)
            && frame.matches(initial),
        detail: format!(
            "status {status:?}, fullscreen {fullscreen}, Dock and menu bar hidden {ui_hidden}, \
             system options {system:?}, recorded mode {recorded:?}, \
             frame {frame} (expected {initial})"
        ),
    })
}

/// The presentation mode LaunchServices records for this app, from
/// `lsappinfo` (`UIPresentationOptions`); `None` on other platforms.
///
/// Undocumented, so used only here and in `scripts/crash-test.sh`, but it is
/// the one place a hidden Dock and menu bar left behind after fullscreen
/// shows up: `currentSystemPresentationOptions` reports the default.
fn launch_services_options() -> Result<Option<u64>, String> {
    if !cfg!(target_os = "macos") {
        return Ok(None);
    }
    let output = std::process::Command::new("/usr/bin/lsappinfo")
        .args(["info", "-only", "UIPresentationOptions"])
        .arg(std::process::id().to_string())
        .output()
        .map_err(|err| format!("could not run lsappinfo: {err}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.trim()
        .rsplit('=')
        .next()
        .and_then(|value| value.parse().ok())
        .map(Some)
        .ok_or_else(|| format!("unexpected lsappinfo output: {text:?}"))
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
