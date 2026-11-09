//! macOS-specific functionality (presentation options, application discovery,
//! notifications).
//!
//! Platform code stays behind well-defined interfaces so the rest of the
//! backend does not depend on macOS APIs directly.
//!
//! Status: planned — see ROADMAP.md Phases 2–4. Phase 1 needed no macOS-specific
//! code beyond small `cfg` branches in `workspace`.
