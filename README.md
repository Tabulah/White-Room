# WHITE ROOM

A local-first, RPG-inspired learning and focus environment for macOS.

See [`PROJECT-BRIEF.md`](PROJECT-BRIEF.md) for the product vision, [`AGENTS.md`](AGENTS.md) for engineering rules, and [`ROADMAP.md`](ROADMAP.md) for what is built and what is planned.

> **Status:** Phase 0 (project foundation). The app opens a window, loads configuration, and opens and migrates its local database. There are no user-facing features yet.

## Prerequisites

- macOS (primary target)
- Xcode Command Line Tools: `xcode-select --install`
- Rust (stable, 1.90 or newer): <https://rustup.rs>
- Node.js 22 and npm

On Linux the backend also builds and its tests run, provided the [Tauri Linux dependencies](https://v2.tauri.app/start/prerequisites/#linux) are installed. Linux is not a supported target.

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

CI (`.github/workflows/ci.yml`) runs all of the checks above on a macOS runner and builds an unsigned debug `.app`.

## Project layout

```
src/                      Svelte + TypeScript frontend (presentation only)
  lib/api/                Typed bindings to Tauri commands, one file per area
src-tauri/                Rust backend (authoritative state and business rules)
  src/
    commands.rs           Tauri commands — the only frontend/backend interface
    core/                 Lifecycle, config, logging, shared errors
    database/             SQLite connection and versioned migrations
    platform.rs           macOS integration            (planned)
    workspace.rs          Workspace and launcher       (planned)
    sessions.rs           Focus sessions               (planned)
    learning.rs           Curricula and mastery        (planned)
    gamification.rs       XP, levels, achievements     (planned)
    economy.rs            Focus Credits and cosmetics  (planned)
  capabilities/           Tauri permission grants per window
  tauri.conf.json         App, window, security, and bundle configuration
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

### Frontend linting

`npm run check` runs `svelte-check` with `--fail-on-warnings` (TypeScript errors, Svelte compiler and accessibility warnings) and type-checks the Vite config. ESLint is intentionally not included yet to keep dependencies minimal; add it if a concrete need appears.
