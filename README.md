# WHITE ROOM

A local-first, RPG-inspired learning and focus environment for macOS.

See [`PROJECT-BRIEF.md`](PROJECT-BRIEF.md) for the product vision, [`AGENTS.md`](AGENTS.md) for engineering rules, and [`ROADMAP.md`](ROADMAP.md) for what is built and what is planned.

> **Status:** Phase 1 (fullscreen workspace). You can enter a calm, monochrome fullscreen workspace and leave it again: with the on-screen exit control, a global emergency shortcut (⇧⌘⎋ on macOS), or the system's own fullscreen controls. The dashboard is a placeholder. See [`docs/workspace.md`](docs/workspace.md).

## Prerequisites

- macOS (primary target)
- Xcode Command Line Tools: `xcode-select --install`
- Rust (stable, 1.90 or newer): <https://rustup.rs>
- Node.js 22 and npm

On Linux the backend also builds and its tests run, provided the [Tauri Linux dependencies](https://v2.tauri.app/start/prerequisites/#linux) are installed. Windows is checked in CI (see below). Neither is a supported target.

## Commands

Run from the repository root unless noted.

| Task                          | Command                                          |
| ----------------------------- | ------------------------------------------------ |
| Install dependencies          | `npm install`                                    |
| Run the app (with hot reload) | `npm run tauri dev`                              |
| Build a release `.app`/`.dmg` | `npm run tauri build`                            |
| Frontend type check + lint    | `npm run check`                                  |
| Frontend format / check       | `npm run format` / `npm run format:check`        |
| Frontend tests                | `npm test`                                       |
| Rust format check             | `cargo fmt --check` (in `src-tauri/`)            |
| Rust lint                     | `cargo clippy --all-targets -- -D warnings` (in `src-tauri/`) |
| Rust tests                    | `cargo test` (in `src-tauri/`)                   |
| App smoke test                | see [Smoke test](#smoke-test)                    |

### Continuous integration

- **CI** (`.github/workflows/ci.yml`) runs all of the checks above on a macOS runner, builds an unsigned debug `.app`, and runs the smoke test against it.
- **Windows compatibility** (`.github/workflows/windows.yml`) runs the type check, frontend and Rust tests, clippy, a debug build, and the smoke test on a Windows runner. It is a separate workflow, so a Windows failure never blocks macOS work. Windows is not a supported platform; this only shows whether the code works there. It can also be started by hand from the Actions tab.

### Smoke test

A debug build started with `WHITE_ROOM_SMOKE_TEST=1` tests itself instead of waiting for input. It checks that the database was migrated, the log file exists, the frontend reached the backend, and the emergency shortcut was registered. It then enters and leaves the workspace three times, alternating the exit-control and emergency-shortcut code paths, and checks that the window comes back to exactly its original position and size. It prints `[smoke]` lines and exits `0` on success, `1` on failure. Release builds do not contain it.

```sh
npm run tauri -- build --debug --bundles app
WHITE_ROOM_SMOKE_TEST=1 "src-tauri/target/debug/bundle/macos/WHITE ROOM.app/Contents/MacOS/white-room"
```

On macOS the screen switches to the workspace and back three times. The whole run takes about ten seconds. It uses the same data directory as the normal app.

It does not physically press the shortcut keys, which would need accessibility permissions; it calls the same Rust function the shortcut triggers. It also does not check how the interface looks.

## Project layout

```
src/                      Svelte + TypeScript frontend (presentation only)
  lib/api/                Typed bindings to Tauri commands, one file per area
  lib/components/         UI components (entry screen, workspace, exit control)
  lib/design/tokens.css   Design tokens: greyscale palette, type scale, spacing
src-tauri/                Rust backend (authoritative state and business rules)
  src/
    commands.rs           Tauri commands — the only frontend/backend interface
    core/                 Lifecycle, config, logging, shared errors
    database/             SQLite connection and versioned migrations
    platform.rs           macOS integration            (planned)
    workspace/            Fullscreen workspace, exits, display policy
    sessions.rs           Focus sessions               (planned)
    learning.rs           Curricula and mastery        (planned)
    gamification.rs       XP, levels, achievements     (planned)
    economy.rs            Focus Credits and cosmetics  (planned)
  capabilities/           Tauri permission grants per window
  tauri.conf.json         App, window, security, and bundle configuration
docs/workspace.md         Workspace behaviour: exits, displays, what is changed
```

## Local data

Everything stays on the device. Paths below are for macOS; `<id>` is the bundle identifier `com.whiteroom.desktop`.

| What          | Where                                                     |
| ------------- | --------------------------------------------------------- |
| Database      | `~/Library/Application Support/<id>/white-room.sqlite3`   |
| Configuration | `~/Library/Application Support/<id>/config.json` (optional) |
| Logs          | `~/Library/Logs/<id>/white-room.log` (rotated, 5 × 5 MB)  |

### Configuration

`config.json` is optional and is never created or overwritten by the app. If it is missing, defaults are used. If it is invalid, a warning is logged and defaults are used.

```json
{
  "config_version": 1,
  "log_level": "info"
}
```

`log_level` is one of `error`, `warn`, `info`, `debug`, `trace`. Unknown keys are rejected so typos are reported rather than ignored.

## Development conventions

### Adding a Tauri command

Commands are deny-by-default. A new command needs all of:

1. The handler in `src-tauri/src/commands.rs` (thin: validate, delegate to a domain module, map errors).
2. Registration in `tauri::generate_handler![...]` in `src-tauri/src/lib.rs`.
3. Its name in the `AppManifest` command list in `src-tauri/build.rs`, which generates an `allow-<command>` permission.
4. That permission granted in `src-tauri/capabilities/default.json`.
5. A typed binding in `src/lib/api/`.

Skipping step 3 or 4 means the frontend's call is refused at runtime.

### Database migrations

Migrations live in `src-tauri/src/database/migrations/` and are listed in `src-tauri/src/database/migrations.rs`.

- Append only: never edit or reorder a migration that has shipped.
- Versions are sequential starting at 1.
- Each migration runs in its own transaction; do not put `BEGIN`/`COMMIT` in migration SQL.
- The app refuses to open a database whose schema is newer than it understands.
- Prefer `STRICT` tables and explicit foreign keys (foreign-key enforcement is on).

### Design tokens

All styling uses the tokens in `src/lib/design/tokens.css`. Tailwind's default colour palette is removed, so only greys exist. Use the semantic names (`bg-surface`, `text-ink-muted`, `border-line`, and so on) rather than raw greys, and the named type sizes (`text-micro`, `text-caption`, `text-body`, `text-lead`, `text-title`, `text-display`). Pair any animation with `motion-safe:`. `tokens.test.ts` fails if a colour has a hue or if a text colour drops below WCAG AA contrast on any surface.

### Frontend linting

`npm run check` runs `svelte-check` with `--fail-on-warnings` (TypeScript errors, Svelte compiler and accessibility warnings) and type-checks the Vite config. ESLint is intentionally not included yet to keep dependencies minimal; add it if a concrete need appears.

Component tests render with `svelte/server` and check the HTML, so they need no DOM library.
