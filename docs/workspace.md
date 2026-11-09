# The Workspace

How WHITE ROOM's fullscreen workspace is entered and left, what it changes on your computer, and how it behaves with more than one display. This describes Phase 1 of [`ROADMAP.md`](../ROADMAP.md).

## Entering

WHITE ROOM opens in an ordinary window. **Enter workspace** makes that window fullscreen on the primary display (see [Displays](#displays)) and shows the dashboard. Before you go in, the window tells you how to get back out, including whether the emergency shortcut is working.

On macOS the workspace uses **native fullscreen**: the same mode as the green window button. macOS gives the window its own Space and hides the Dock and menu bar for it. Both reappear when you move the pointer to the bottom or top edge of the screen.

## Leaving

There are four independent ways out. None of them depend on each other, so no single failure can trap you.

| Way out | How | Notes |
| --- | --- | --- |
| Exit control | The **EXIT** button, bottom right of the workspace | Always shown, never disabled. |
| Emergency shortcut | macOS: **⇧⌘⎋** (Shift-Command-Escape)<br>Windows/Linux: **Ctrl+Alt+Shift+Esc** | Works from any app, even if WHITE ROOM's interface stops responding. |
| System fullscreen controls | macOS: green window button, View › Exit Full Screen, or ⌃⌘F | WHITE ROOM never disables these, and notices when you use them. |
| Quit | ⌘Q, or Force Quit | Ending the app always ends fullscreen. |

**Why bottom right?** In macOS fullscreen, moving the pointer to the top edge slides the menu bar and title bar down over the window. An exit control at the top would be covered exactly when you reach for it. Nothing covers the bottom corners.

### The emergency shortcut

The shortcut is registered with the operating system when WHITE ROOM starts and is handled entirely in Rust, without going through the interface. Pressing it while the workspace is open leaves the workspace. Pressing it at any other time takes the WHITE ROOM window out of fullscreen if it is fullscreen, and otherwise does nothing.

The key combinations avoid the operating system's own shortcuts: Option-Command-Escape (Force Quit) and Control-Command-Q (Lock Screen) on macOS, and Control+Shift+Escape (Task Manager) on Windows.

Registration can fail, for example when another app already uses the combination, or on Linux under Wayland. WHITE ROOM then says **Unavailable** on the entry screen and in the workspace status bar, and stops showing the shortcut on the exit control. It never claims a shortcut that the system did not accept.

## What changes on your computer

Nothing persistent. Fullscreen is managed by the operating system and ends with the window, even if WHITE ROOM crashes. WHITE ROOM does not change Dock, menu bar, notification, or display settings in this phase.

After leaving, the window returns to the exact position and size it had before entering. The automated smoke test checks this after every exit (see the README).

Phase 2 adds explicit control over the Dock and menu bar. Those changes do outlast a crash, so that phase also adds the snapshot, restore, and recovery mechanisms that make them safe.

## Displays

**Rule: the workspace always opens on the primary display. Other displays are never touched.**

- **Primary display.** On macOS this is the display with the menu bar (System Settings › Displays › Arrange). On Windows it is the display marked "Make this my main display".
- **If the window is on another display**, WHITE ROOM first moves it to the centre of the primary display, then makes it fullscreen. After you leave, the window stays on the primary display.
- **If the primary display cannot be determined**, which is rare and mostly happens on Linux, the workspace opens on whichever display holds the window. The status bar then says **Current display**.
- **Other displays** stay as they are, with their own windows and Spaces. On macOS, if *Displays have separate Spaces* is turned off (System Settings › Desktop & Dock), macOS itself blanks the other displays while any app is fullscreen. That is macOS behaviour, not WHITE ROOM's.
- **If a display is disconnected while the workspace is open**, the operating system moves the window to a remaining display. The exit control and the emergency shortcut keep working.

Choosing a different display, and spanning several, are not planned yet. Multi-display behaviour in Strict Mode is an open question in the roadmap.

## Known limitations

- Moving the pointer to the top edge in macOS fullscreen reveals the menu bar over the workspace. This is intended macOS behaviour; it also offers View › Exit Full Screen.
- On macOS, a fullscreen request made while another fullscreen animation is running is postponed by macOS until the animation ends. WHITE ROOM accounts for this rather than treating the delay as you leaving.
- Windows and Linux are not supported targets. CI checks that Windows builds, passes the tests, and can enter and leave the workspace, but nothing beyond that is promised there.

## For developers

- `src-tauri/src/workspace.rs` is the entry point. `workspace/state.rs` is the pure state machine, `workspace/controller.rs` applies it to a window, `workspace/display.rs` holds the display policy, `workspace/window.rs` is the window abstraction, and `workspace/emergency_exit.rs` is the shortcut.
- Rust owns the state. The frontend reads it with `get_workspace_state`, changes it with `enter_workspace` and `exit_workspace`, and listens for `workspace://changed`. Every change has a higher `revision`, so late or out-of-order snapshots are ignored.
- The workspace commands are synchronous on purpose. Tauri runs them on the main thread, where window events and the emergency shortcut are also handled, so they cannot interleave.
