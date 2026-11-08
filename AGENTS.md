# AGENTS.md — WHITE ROOM

## 1. Project Overview

WHITE ROOM is a local-first, RPG-inspired learning and productivity environment for macOS.

It provides a distraction-controlled desktop workspace where users can:
- Create personalized learning curricula.
- Study academic and independent subjects.
- Complete lessons, exercises, assessments, and projects.
- Track knowledge mastery and skill progression.
- Earn XP and spendable cosmetic currency.
- Unlock achievements, levels, skill trees, and cosmetics.
- Launch approved external applications.
- Work in configurable distraction-control modes.

WHITE ROOM is not intended to replace macOS. It provides an alternative desktop experience built on top of macOS.

### Product Philosophy

- User autonomy comes first.
- Learning should be self-directed.
- Progress should reflect meaningful accomplishments.
- Productivity should not come at the expense of wellbeing.
- The interface should be calm, clinical, and distraction-free.
- User data belongs to the user.
- Core functionality must work offline.

Never introduce mechanics that punish users for resting, missing sessions, or changing their learning goals.

---

## 2. Technology Stack

Primary technologies:

| Component | Technology |
|---|---|
| Desktop framework | Tauri 2 |
| Backend | Rust |
| Frontend | Svelte |
| Language | TypeScript |
| Styling | Tailwind CSS |
| Database | SQLite |
| Native platform integration | macOS APIs |
| Packaging | Tauri bundler |

macOS is the primary and initial target platform.

Use Swift or Objective-C native helpers only when macOS functionality cannot reasonably be implemented through existing Rust or Tauri APIs.

Keep platform-specific functionality isolated behind well-defined interfaces.

Do not introduce additional frameworks, dependencies, or architectural layers without a clear justification.

---

## 3. Architecture Principles

WHITE ROOM should use modular architecture.

Separate the application into these logical domains:

- `core`: Application lifecycle, configuration, and shared infrastructure.
- `platform`: macOS-specific functionality.
- `workspace`: Desktop presentation and application launcher.
- `sessions`: Focus sessions and session lifecycle.
- `learning`: Curricula, courses, lessons, assessments, and mastery.
- `gamification`: XP, levels, achievements, skill trees, and progression.
- `economy`: Focus Credits, transactions, inventory, and cosmetics.
- `database`: SQLite persistence and migrations.

### Architectural rules

1. Keep business logic separate from UI components.
2. Keep database access separate from business logic.
3. Avoid giant files and monolithic modules.
4. Prefer explicit types and well-defined interfaces.
5. Use asynchronous operations where appropriate.
6. Avoid unnecessary global state.
7. Do not duplicate business logic between Rust and TypeScript.
8. Validate data at system boundaries.
9. Keep dependencies minimal.
10. Avoid premature abstraction.

Rust should own authoritative application state and business rules.

The frontend should primarily handle presentation, interaction, and client-side UI state.

Use Tauri commands for communication between frontend and backend.

---

## 4. macOS Integration

WHITE ROOM must integrate with macOS without pretending to replace its operating system.

### Standard Mode

- Fullscreen workspace.
- Configurable application launcher.
- Approved external application access.
- Optional notification suppression through supported macOS features.
- Easy exit and desktop restoration.

### Strict Mode

- More restrictive workspace navigation.
- Approved application launcher.
- Reduced opportunities for accidental application switching.
- Configurable external link access.
- Deliberate session exit.

### Important limitations

Standard macOS applications cannot guarantee system-wide application lockdown.

Do not claim that WHITE ROOM can reliably prevent:
- Command-Tab.
- Finder access.
- Dock access in every circumstance.
- Access to other applications.
- User access to system-level escape mechanisms.

Strict Mode is a best-effort distraction-control environment, not a security boundary.

Never disable or remove the emergency exit.

### Recovery requirements

- Preserve the user's previous macOS presentation configuration.
- Restore the previous configuration when leaving WHITE ROOM.
- Handle application shutdown and unexpected failures safely.
- Avoid leaving the Dock or menu bar unintentionally hidden.
- Provide a recovery mechanism independent of the main workspace UI.

Do not silently modify unrelated macOS settings.

---

## 5. Learning System

The learning system is a core product feature.

Users must be able to choose what they want to learn.

Support two curriculum categories:

### Academic Curriculum

Examples:
- Mathematics.
- Physics.
- Chemistry.
- Biology.
- History.
- Literature.
- Languages.
- Geography.

### Independent Curriculum

Examples:
- Quantum computing.
- Programming.
- Assembly.
- Robotics.
- Cybersecurity.
- Artificial intelligence.
- Philosophy.
- Astronomy.
- Game development.

Both categories must use the same underlying course and progression infrastructure.

### Curriculum hierarchy

Use a flexible hierarchy:

`Curriculum → Subject → Course → Unit → Lesson → Activity`

Support:
- User-created courses.
- Predefined curriculum packs.
- Custom learning objectives.
- Prerequisite recommendations.
- External learning resources.
- Exercises and assessments.
- Practical projects.

Prerequisites should guide users rather than unnecessarily prevent exploration.

Do not assume that all courses require the same structure.

### Mastery

Keep learning activity and demonstrated knowledge separate.

- XP represents earned progression.
- Mastery represents demonstrated understanding.

Completing lessons can earn XP without necessarily increasing mastery substantially.

Use assessments, exercises, and practical projects to establish mastery.

Never claim that a user has mastered a subject solely because they spent time studying it.

---

## 6. Gamification

Gamification is a first-class feature, not an optional afterthought.

The intended aesthetic is clinical, minimalist, and monochromatic, with RPG-inspired progression.

### XP

Users earn XP through meaningful activities.

Examples:
- Completing lessons.
- Solving exercises.
- Passing assessments.
- Completing units.
- Finishing courses.
- Completing practical projects.

XP awards must be configurable and centrally managed.

Avoid rewarding repetitive or easily exploitable actions.

### Levels

Users accumulate XP to progress through levels.

Level calculations must be deterministic and handled by Rust.

### Skill Trees

Support interconnected skill progression.

Skills may have prerequisites and relationships across subjects.

Do not assume that knowledge always forms a strict hierarchical tree. Prefer graph-based relationships where appropriate.

### Achievements

Achievements should recognize meaningful milestones.

Examples:
- First completed course.
- First programming project.
- First practical experiment.
- Completion of an academic subject.
- Exploration of multiple disciplines.

### Focus Credits

Focus Credits (FC) are a separate, spendable currency.

Users earn FC through designated activities and can exchange them for cosmetics.

FC and XP must never be interchangeable.

### Cosmetic Shop

Support:
- Interface themes.
- Profile avatars.
- Profile frames.
- Achievement decorations.
- Dashboard backgrounds.
- Cosmetic effects.
- Typography options.

Cosmetics must not provide gameplay or educational advantages.

No gambling mechanics, loot boxes, paid XP multipliers, or manipulative purchasing systems.

### Transaction Integrity

Use an auditable transaction ledger.

- XP transactions must be recorded.
- FC transactions must be recorded.
- Duplicate reward transactions must be prevented.
- Purchases must be atomic.
- Progression must survive application restarts.

Never modify balances through arbitrary frontend operations.

---

## 7. Database and Persistence

Use SQLite for local persistence.

Requirements:
- Versioned database migrations.
- Foreign-key constraints where appropriate.
- Transactional updates.
- Appropriate indexes.
- Data validation.
- Safe application shutdown.

Keep progression and economy records auditable.

Do not delete historical XP or currency transactions when users change courses or settings.

Provide a path toward exporting user data.

The application should remain functional without an internet connection.

---

## 8. Security and Privacy

WHITE ROOM should be local-first and privacy-conscious.

Rules:
- No mandatory account system.
- No mandatory cloud connection.
- No keystroke logging.
- No covert screenshots.
- No webcam monitoring.
- No unnecessary telemetry.
- No unrestricted shell execution from the frontend.
- No unrestricted filesystem access.
- No arbitrary application launching through unvalidated user input.

Use Tauri's capability system and grant only the permissions required by each feature.

External application launching must use validated application identifiers.

Terminal functionality must use explicit, controlled process management.

Never expose unrestricted system commands through generic frontend-accessible Tauri commands.

---

## 9. UI and Design Guidelines

WHITE ROOM uses a clinical visual identity.

### Design principles

- Minimalist.
- Monochromatic.
- Spacious layouts.
- Clear information hierarchy.
- Subtle animations.
- Consistent spacing.
- Accessible typography.
- Minimal visual clutter.

Avoid:
- Excessive gradients.
- Excessive visual effects.
- Overly colorful dashboards.
- Unnecessary notifications.
- Aggressive animations.
- Gamification elements that overwhelm the learning experience.

RPG elements should feel integrated into a scientific research interface.

The interface should remain usable for extended sessions.

---

## 10. Development Workflow

Before implementing a feature:

1. Inspect the existing project structure.
2. Read relevant documentation.
3. Identify existing implementations.
4. Understand dependencies and integration points.
5. Propose an implementation plan for non-trivial changes.
6. Identify potential regressions.
7. Implement incrementally.

Do not assume the repository is empty.

Do not replace existing implementations without justification.

Do not silently remove features to make a new implementation easier.

### Code quality

- Write readable, maintainable code.
- Use meaningful names.
- Handle errors explicitly.
- Avoid unnecessary dependencies.
- Document non-obvious decisions.
- Keep functions reasonably focused.
- Avoid unnecessary abstractions.

### Testing

Add tests for important business logic.

Prioritize:
- Session lifecycle.
- Focus mode policies.
- XP calculations.
- Level progression.
- Currency transactions.
- Purchase validation.
- Curriculum progression.
- Assessment scoring.
- Database migrations.
- Recovery and restoration behavior.

Run relevant tests and checks after making changes.

Never claim that tests passed unless they were actually executed.

If a check cannot be performed, explain why.

### Change management

- Keep changes scoped to the requested task.
- Avoid unrelated refactoring.
- Preserve existing behavior unless explicitly asked to change it.
- Do not overwrite user work.
- Do not modify dependency versions unnecessarily.
- Update documentation when architecture or behavior changes.

---

## 11. Agent Communication

When working on WHITE ROOM:

- Explain architectural decisions briefly.
- State assumptions explicitly.
- Identify macOS-specific limitations.
- Distinguish implemented functionality from planned functionality.
- Report incomplete work honestly.
- Prefer incremental, verifiable progress.

When a task is ambiguous, ask for clarification rather than inventing major product requirements.

When a task is too large, break it into smaller implementation stages.

Do not implement future roadmap features unless requested.

---

## 12. Current Development Priorities

Follow this development order unless explicitly instructed otherwise:

1. Tauri application foundation.
2. macOS fullscreen workspace.
3. Safe presentation-mode lifecycle.
4. Standard and Strict modes.
5. Application discovery and launcher.
6. Session manager.
7. Tasks and XP progression.
8. Curriculum and course management.
9. Assessments and mastery.
10. Skill trees and achievements.
11. Focus Credits and cosmetic shop.
12. Embedded learning tools.

The immediate objective is a stable macOS shell with safe entry, exit, and recovery.

Do not build the entire product before validating its foundational architecture.

---

## 13. Deployment Models

WHITE ROOM supports two deployment models.

### Personal Edition — Primary Target

The default deployment model.

Designed for personally owned, ordinary macOS computers without organizational supervision or mandatory device enrollment.

Requirements:
- Standard macOS application installation.
- No mandatory MDM enrollment.
- No device supervision requirement.
- Full access to learning and gamification features.
- Fullscreen productivity workspace.
- Best-effort application restrictions.
- User-controlled focus policies.
- Safe and immediate emergency exit.

Application restrictions must accurately reflect the capabilities available to an ordinary macOS installation.

Never claim that Personal Edition can enforce system-wide application execution restrictions.

### Managed Edition — Advanced Deployment

Designed for supervised Macs and institutional environments.

Potential capabilities:
- Device-management integration.
- OS-enforced application allowlists and denylists.
- Managed configuration policies.
- Application execution restrictions.
- Administrative recovery procedures.

Use Apple's officially supported device-management and Endpoint Security mechanisms where applicable.

Managed functionality must remain optional and must not be required for core WHITE ROOM functionality.

### Restriction Provider Architecture

Application restrictions must use a provider abstraction.

Implement separate providers for:
- Ordinary macOS workspace restrictions.
- Managed macOS application restrictions.

Each provider must report its actual enforcement capabilities.

The application must never represent an unenforced policy as successfully enforced.

### Development Priority

1. Personal Edition.
2. Stable workspace and recovery mechanisms.
3. Personal-mode restriction capabilities.
4. Managed Edition research and prototype.
5. Institutional deployment support.

Personal Edition remains the primary product target.

---

## 14. What to read and do
Follow these instructions unless instructed otherwise:

1. Read PROJECT-BRIEF.md
2. Make a ROADMAP.md if one doesn't exist already.

---

## Final Principle

WHITE ROOM exists to help people learn, explore, experiment, and develop their abilities.

It should make learning engaging without turning productivity into an obligation.

**The user controls the environment. The environment supports the user.**