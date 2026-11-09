# WHITE ROOM — Roadmap

This roadmap turns `PROJECT-BRIEF.md` and `AGENTS.md` into ordered, verifiable stages.

It follows the development priorities in `AGENTS.md` §12 and the deployment priorities in §13. Personal Edition is the primary target throughout; Managed Edition work does not begin until the Personal Edition shell is stable.

## Status Legend

| Marker | Meaning |
|---|---|
| `[ ]` | Planned — not started |
| `[~]` | In progress |
| `[x]` | Implemented and verified |

**Current state:** Phase 1 (macOS fullscreen workspace) is implemented and verified on a Mac by an automated smoke test: the app enters and leaves a fullscreen workspace repeatedly and restores its window exactly each time. Still open: green CI runs with the smoke test on macOS and Windows (the first run found two problems, both now fixed; see Phase 1 notes), and a short check by hand and eye on a Mac. Phase 2 should not begin in earnest until those are done.

## Guiding Rules

- Each phase must be stable and tested before the next phase begins in earnest.
- Rust owns authoritative state and business rules; Svelte handles presentation.
- Safe entry, exit, and recovery come before any feature that depends on them.
- Never represent an unenforced restriction as enforced.
- Never introduce mechanics that punish rest, missed sessions, or changed goals.
- Roadmap features are not implemented ahead of their phase unless explicitly requested.

---

## Phase 0 — Project Foundation

**Goal:** A buildable, testable Tauri 2 application skeleton with the agreed module layout.

- [x] Scaffold Tauri 2 app with Svelte, TypeScript, and Tailwind CSS.
- [x] Establish Rust module layout: `core`, `platform`, `workspace`, `sessions`, `learning`, `gamification`, `economy`, `database`.
- [x] Establish frontend layout separating UI components from API/command bindings.
- [x] Configure Tauri capabilities with a minimal, deny-by-default permission set.
- [x] Add SQLite with a versioned migration runner and an initial empty schema.
- [x] Add application configuration loading and validation (`core`).
- [x] Add structured logging to a local file (no telemetry).
- [x] Set up formatting, linting, and test commands for Rust and TypeScript.
- [x] Add continuous integration for build, lint, and test (macOS runner).
- [x] Document build and run instructions in a `README.md`.

**Notes:**

- Verified on Linux: frontend format, type check, tests, and build; Rust fmt, clippy, and tests (19); the app launches, migrates a new database, writes its log file, and shows data fetched from Rust. Removing the capability grant makes the same call fail, which confirms commands are deny-by-default.
- Verified on macOS CI (Apple Silicon runner, first run green): frontend format, type check, and tests; Rust fmt, clippy, and tests (19 of 19); an unsigned debug `WHITE ROOM.app` bundle is built.
- Launched on a Mac (2026-11-09) through the Phase 1 smoke test: the database and log are under `~/Library`, and the frontend loads and reaches the backend. CI now launches the app too. Still to confirm by eye: the entry window shows the version and schema.
- The initial schema contains only migration bookkeeping and an `app_metadata` table that records when the database was created.

**Exit criteria:** App builds and launches on macOS; migrations run on a fresh database; CI is green.

---

## Phase 1 — macOS Fullscreen Workspace

**Goal:** A calm fullscreen workspace that can be entered and left reliably.

- [x] Fullscreen workspace window with minimal dashboard placeholder.
- [x] Monochromatic design tokens (spacing, typography, greyscale palette) in Tailwind.
- [x] Always-visible, always-functional exit control.
- [x] Global emergency-exit keyboard shortcut.
- [x] Multi-display behaviour defined and documented (primary display first).

**Notes:**

- Behaviour, exits, and display policy are documented in [`docs/workspace.md`](docs/workspace.md).
- Verified on macOS (Apple Silicon, 2026-11-09): Rust fmt, clippy, and tests (57); frontend format, type check, and tests (42); production frontend build; debug `.app` bundle. The smoke test passed. It entered and left the workspace 3 times, alternating the exit-control and emergency-shortcut code paths, and the window returned to its exact original frame each time. The shortcut was registered. A deliberately broken run exits with code 1, so CI will notice failures.
- First CI run (commit `7b46a49`) failed on both platforms. Both causes are fixed:
  - macOS: the smoke test failed on the runner's 1024-point-wide display. The 1200 × 800 window was centred partly off-screen, and macOS moved it back on screen after fullscreen, so its frame changed. The same failure was reproduced locally with an oversized window. Fixed with `preventOverflow`: the window is shrunk to fit the display at launch, and the oversized case then passes all three cycles.
  - Windows: everything compiled and passed clippy. One Phase 0 test, `unreadable_path_is_an_error`, assumed every OS reports reading a directory the same way, and failed on Windows. It now uses a realistic path. Windows' build and smoke-test steps have not run yet.
- Still to check by hand on a Mac:
  - look at the entry screen and the workspace;
  - press ⇧⌘⎋ for real (the smoke test calls the shortcut's handler function, not the keys);
  - leave with the green window button and check that the entry screen comes back;
  - with a second display, enter from it and check that the workspace opens on the primary display.
- Decisions:
  - Native macOS fullscreen. The OS hides and restores the Dock and menu bar and nothing persists after a crash. Explicit presentation options wait for Phase 2's restore machinery.
  - The app opens in a window with an **Enter workspace** button, so entering and leaving can be repeated without restarting.
  - The exit control is bottom-right. In macOS fullscreen the menu bar slides over the top edge.
  - The emergency shortcut leaves the workspace; it does not quit the app. It is registered for the app's lifetime.
- New dependency: `tauri-plugin-global-shortcut` (official Tauri plugin), used from Rust only. The frontend is granted none of its permissions. Writing our own OS hotkey code would need `unsafe`, which the crate forbids.
- Windows compatibility check: added at the user's request. Windows is still not a supported target (see Long-Term Vision); the workflow only shows whether the code builds and runs there. This needed `icons/icon.ico` and a `.gitattributes` that keeps LF line endings.

**Exit criteria:** User can enter and exit the workspace repeatedly without leaving the desktop in an altered state.

---

## Phase 2 — Safe Presentation-Mode Lifecycle

**Goal:** WHITE ROOM never leaves macOS in a broken or hidden state.

- [ ] `platform` interface for presentation options (Dock, menu bar, notifications) isolated behind a trait.
- [ ] Snapshot the user's existing presentation configuration before any change.
- [ ] Persist the snapshot to disk before applying changes.
- [ ] Restore on normal exit, window close, and app quit.
- [ ] Restore on panic and termination signals where possible.
- [ ] On launch, detect an unclean previous shutdown and restore from the persisted snapshot.
- [ ] Independent recovery mechanism outside the main workspace UI (e.g. menu bar item or standalone recovery command).
- [ ] Tests for snapshot, persist, restore, and crash-recovery paths.
- [ ] Document what is and is not modified on macOS.

**Exit criteria:** Forced quits and simulated crashes always result in restored Dock/menu bar state on next launch or via the recovery mechanism.

---

## Phase 3 — Standard and Strict Modes

**Goal:** User-selected focus modes with honestly reported capabilities.

- [ ] Focus mode policy model in Rust (Standard, Strict) with user-configurable settings.
- [ ] Restriction provider abstraction that reports actual enforcement capabilities.
- [ ] Personal (ordinary macOS) restriction provider.
- [ ] Standard Mode: fullscreen workspace, launcher, optional notification suppression via supported features, easy exit.
- [ ] Strict Mode: reduced navigation, deliberate exit flow, configurable external link access — best effort only.
- [ ] UI clearly communicates which restrictions are enforced vs. best effort.
- [ ] Emergency exit remains available in every mode.
- [ ] Tests for focus mode policy logic and provider capability reporting.

**Exit criteria:** Both modes work end to end; no UI text overstates what is enforced.

---

## Phase 4 — Application Discovery and Launcher

**Goal:** Launch approved external applications safely.

- [ ] Discover installed applications (bundle identifier, name, icon).
- [ ] User-managed allowlist of approved applications.
- [ ] Launch only by validated bundle identifier — never by arbitrary path or shell command.
- [ ] Launcher UI in the workspace.
- [ ] Persist launcher configuration in SQLite.
- [ ] Tests for identifier validation and allowlist rules.

**Exit criteria:** Approved apps launch from the workspace; unvalidated input cannot launch anything.

---

## Phase 5 — Session Manager

**Goal:** Focus sessions with a clear, recoverable lifecycle.

- [ ] Session state machine (planned → active → paused → completed / abandoned) in Rust.
- [ ] Sessions bound to a focus mode.
- [ ] Session persistence across restarts and crash recovery.
- [ ] Session history view.
- [ ] No penalties for ending early, pausing, or skipping days.
- [ ] Tests for all lifecycle transitions.

**Exit criteria:** Sessions survive restarts and crashes; lifecycle is fully tested.

---

## Phase 6 — Tasks and XP Progression

**Goal:** Auditable XP and deterministic levels.

- [ ] Task model (create, complete, archive).
- [ ] Central, configurable XP award table.
- [ ] Append-only XP transaction ledger with idempotency keys to prevent duplicate rewards.
- [ ] Deterministic level curve computed in Rust.
- [ ] Anti-exploit rules for repetitive or trivial actions.
- [ ] Profile view showing XP, level, and recent ledger entries.
- [ ] Tests for XP awards, duplicate prevention, and level calculation.

**Exit criteria:** XP and levels are reproducible from the ledger alone.

---

## Phase 7 — Curriculum and Course Management

**Goal:** A flexible learning structure for academic and independent study.

- [ ] Hierarchy: Curriculum → Subject → Course → Unit → Lesson → Activity, with optional levels.
- [ ] Academic and Independent curriculum categories on shared infrastructure.
- [ ] User-created courses and custom learning objectives.
- [ ] Prerequisite recommendations (advisory, not blocking).
- [ ] External learning resources attached to lessons.
- [ ] Curriculum pack format (local import) with schema validation.
- [ ] Course progress tracking and XP integration.
- [ ] Tests for curriculum progression and pack validation.

**Exit criteria:** Users can build, import, and progress through courses entirely offline.

---

## Phase 8 — Assessments and Mastery

**Goal:** Mastery that reflects demonstrated understanding, separate from XP.

- [ ] Exercise and assessment models (e.g. multiple choice, short answer, self-graded, project rubric).
- [ ] Deterministic assessment scoring in Rust.
- [ ] Practical project submissions with rubric-based evaluation.
- [ ] Mastery model per skill/subject driven by assessments and projects, not time spent.
- [ ] Mastery views that explain how mastery was established.
- [ ] Tests for scoring and mastery calculation.

**Exit criteria:** Mastery can only increase through assessments, exercises, or projects.

---

## Phase 9 — Skill Trees and Achievements

**Goal:** Graph-based skill progression and meaningful milestones.

- [ ] Skill graph with prerequisites and cross-subject relationships.
- [ ] Skill graph visualisation consistent with the clinical aesthetic.
- [ ] Achievement definitions and evaluation engine.
- [ ] Initial achievements (first course, first project, first experiment, subject completion, multidisciplinary exploration).
- [ ] Tests for skill unlock rules and achievement triggers.

**Exit criteria:** Achievements trigger exactly once and are recorded auditably.

---

## Phase 10 — Focus Credits and Cosmetic Shop

**Goal:** A fair, cosmetic-only economy.

- [ ] Focus Credit ledger, strictly separate from XP.
- [ ] Configurable FC earning rules.
- [ ] Cosmetic catalogue: themes, avatars, frames, decorations, backgrounds, effects, typography.
- [ ] Atomic purchase transactions with validation.
- [ ] Inventory and equip/unequip.
- [ ] No loot boxes, gambling, paid multipliers, or manipulative pricing.
- [ ] Tests for balances, purchase validation, and atomicity.

**Exit criteria:** Balances are reproducible from the ledger; failed purchases never alter state.

---

## Phase 11 — Embedded Learning Tools

**Goal:** In-workspace tools that support study.

- [ ] Notes / scratchpad.
- [ ] Controlled terminal with explicit process management (no generic shell command exposure).
- [ ] Additional tools scoped individually as they are proposed.

**Exit criteria:** Each tool passes a capability and security review before release.

---

## Phase 12 — Data Ownership

**Goal:** Users can take their data with them.

- [ ] Export of profile, curricula, progress, and ledgers to an open format.
- [ ] Import / restore from export.
- [ ] Local backup guidance.

---

## Phase 13 — Managed Edition (Research and Prototype)

**Goal:** Optional institutional deployment, never required for core features.

- [ ] Research Apple device-management and Endpoint Security options.
- [ ] Managed restriction provider prototype reporting real enforcement capabilities.
- [ ] Managed configuration policies.
- [ ] Administrative recovery procedures.
- [ ] Institutional deployment documentation.

---

## Long-Term Vision (Unscheduled)

From `PROJECT-BRIEF.md`. These are not scheduled and will not be built without an explicit decision:

- Community curriculum packs.
- Extensible application integrations.
- Optional AI-assisted curriculum creation.
- Advanced (local) analytics.
- More sophisticated desktop customisation.
- Additional operating systems.

## Open Questions

- Bundle identifier. Phase 0 uses the placeholder `com.whiteroom.desktop`. Changing it later moves the data directory, so it should be settled before any release.
- Minimum supported macOS version.
- Exact mechanism for the independent recovery tool (menu bar agent vs. separate helper binary).
- Level curve formula and initial XP/FC award values.
- Curriculum pack file format and versioning.
- Multi-display behaviour in Strict Mode.
- Emergency shortcut: confirm ⇧⌘⎋ (Shift-Command-Escape), and whether it should only leave the workspace (current behaviour) or also quit the app.
- Whether to let users choose the workspace display, or span several.
