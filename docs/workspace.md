# The Workspace

How WHITE ROOM's fullscreen workspace is entered and left, what it changes on your computer, and how it behaves with more than one display. This describes Phases 1 and 2 of [`ROADMAP.md`](../ROADMAP.md).

## Entering

WHITE ROOM opens in an ordinary window, centred and always fully on screen: on a display smaller than its default 1200 × 800, it is shrunk to fit. **Enter workspace** makes that window fullscreen on the primary display (see [Displays](#displays)) and shows the dashboard. Before you go in, the window tells you how to get back out, including whether the emergency shortcut is working.

On macOS the workspace uses **native fullscreen**, the same mode as the green window button, so the window gets its own Space. On top of that, WHITE ROOM hides the **Dock and the menu bar completely**: nothing slides in when the pointer reaches a screen edge.

## Leaving

There are four independent ways out. None of them depend on each other, so no single failure can trap you.

| Way out | How | Notes |
| --- | --- | --- |
| Exit control | The **EXIT** button, bottom right of the workspace | Always shown, never disabled. |
| Emergency shortcut | macOS: **⇧⌘⎋** (Shift-Command-Escape)<br>Windows/Linux: **Ctrl+Alt+Shift+Esc** | Works from any app, even if WHITE ROOM's interface stops responding. |
| System fullscreen shortcut | macOS: **⌃⌘F** | WHITE ROOM never disables it, and notices when you use it. The green window button and View › Exit Full Screen are hidden along with the menu bar. |
| Quit | **⌘Q**, or Force Quit (**⌥⌘⎋**) | Ending the app always ends fullscreen and brings the Dock and menu bar back. |

The exit control sits in the bottom-right corner, away from the content and in the same place every time.

### The emergency shortcut

The shortcut is registered with the operating system when WHITE ROOM starts and is handled entirely in Rust, without going through the interface. Pressing it while the workspace is open leaves the workspace. Pressing it at any other time takes the WHITE ROOM window out of fullscreen if it is fullscreen, and otherwise does nothing.

The key combinations avoid the operating system's own shortcuts: Option-Command-Escape (Force Quit) and Control-Command-Q (Lock Screen) on macOS, and Control+Shift+Escape (Task Manager) on Windows.

Registration can fail, for example when another app already uses the combination, or on Linux under Wayland. WHITE ROOM then says **Unavailable** on the entry screen and in the workspace status bar, and stops showing the shortcut on the exit control. It never claims a shortcut that the system did not accept.

## What changes on your computer

**Nothing persistent.** No system setting is read or written: not the Dock's or menu bar's "automatically hide" settings, not notifications, not displays.

The Dock and menu bar are hidden through WHITE ROOM's own *presentation options*, which belong to the WHITE ROOM app, not to the system:

- macOS applies them only while WHITE ROOM is the app in front. Switch to another app (⌘Tab) and the Dock and menu bar are there for that app; come back and they are hidden again.
- macOS drops them the moment WHITE ROOM quits, crashes, or is force-quit. Nothing needs restoring afterwards, and there is nothing a recovery tool would have to repair.
- Leaving the workspace puts them back before the window leaves fullscreen.

After leaving, the window also returns to the exact position and size it had before entering. This relies on the window starting fully on screen: macOS moves a window that hangs off the display edge back on screen when it leaves fullscreen.

All of this is tested automatically on every CI run (see the README): the smoke test leaves the workspace through every path, including the system's own fullscreen toggle, and checks that macOS records no changed presentation mode for WHITE ROOM afterwards; the crash test kills WHITE ROOM with `SIGKILL` inside the workspace and checks that the frontmost app is back to normal.

A later phase may add notification silencing (Phase 3). That change does outlast a crash, so it will come with a saved record of what was changed and a way to restore it.

## Displays

**Rule: the workspace always opens on the primary display. Other displays are never touched.**

- **Primary display.** On macOS this is the display with the menu bar (System Settings › Displays › Arrange). On Windows it is the display marked "Make this my main display".
- **If the window is on another display**, WHITE ROOM first moves it to the centre of the primary display, then makes it fullscreen. After you leave, the window stays on the primary display.
- **If the primary display cannot be determined**, which is rare and mostly happens on Linux, the workspace opens on whichever display holds the window. The status bar then says **Current display**.
- **Other displays** stay as they are, with their own windows and Spaces. On macOS, if *Displays have separate Spaces* is turned off (System Settings › Desktop & Dock), macOS itself blanks the other displays while any app is fullscreen. That is macOS behaviour, not WHITE ROOM's.
- **If a display is disconnected while the workspace is open**, the operating system moves the window to a remaining display. The exit control and the emergency shortcut keep working.

Choosing a different display, and spanning several, are not planned yet. Multi-display behaviour in Strict Mode is an open question in the roadmap.

## Known limitations

The workspace is a calm place to work, not a lock. On an ordinary Mac, WHITE ROOM cannot and does not try to prevent:

- switching apps with ⌘Tab, Mission Control, or trackpad gestures (the Dock and menu bar then appear for the other app);
- notifications from other apps (silencing them is planned for Phase 3);
- Spotlight, hot corners, and other system features.

Also:

- On macOS, a fullscreen request made while another fullscreen animation is running is postponed by macOS until the animation ends. WHITE ROOM accounts for this rather than treating the delay as you leaving.
- Windows and Linux are not supported targets. CI checks that Windows builds, passes the tests, and can enter and leave the workspace, but the Dock and menu bar handling is macOS only.

## For developers

- `src-tauri/src/workspace.rs` is the entry point. `workspace/state.rs` is the pure state machine, `workspace/controller.rs` applies it to a window and to the Dock and menu bar, `workspace/display.rs` holds the display policy, `workspace/window.rs` is the window abstraction, and `workspace/emergency_exit.rs` is the shortcut.
- `src-tauri/src/platform/presentation.rs` holds the presentation options: pure, tested functions that compute the next options, and the `SystemUi` trait with its AppKit implementation. Every combination is checked against AppKit's rules before it is set, because an invalid one raises an Objective-C exception.
- Rust owns the state. The frontend reads it with `get_workspace_state`, changes it with `enter_workspace` and `exit_workspace`, and listens for `workspace://changed`. Every change has a higher `revision`, so late or out-of-order snapshots are ignored.
- The workspace commands are synchronous on purpose. Tauri runs them on the main thread, where window events and the emergency shortcut are also handled, so they cannot interleave.

AppKit behaviour the code works around (macOS 15; found with a standalone AppKit probe app and confirmed in WHITE ROOM):

- Presentation options set inside a fullscreen transition callback (`windowDidEnterFullScreen`, `windowWillExitFullScreen`) are silently ignored. Window events reach Rust inside those callbacks, so `Workspace::sync_system_ui` runs on a later main-loop turn, queued from a background task (`run_on_main_thread` runs inline when already on the main thread).
- Leaving fullscreen with the Dock and menu bar still hidden makes AppKit reset the app's options, but LaunchServices goes on recording a changed presentation mode for the app. Showing them before leaving avoids it; after a system-initiated exit, `clear_after_fullscreen` changes the options once more, which clears it. `currentSystemPresentationOptions` does not show this leftover when both are hidden, so the smoke test reads LaunchServices' record (`lsappinfo`, `UIPresentationOptions`) instead. That record is undocumented, so only tests use it.
