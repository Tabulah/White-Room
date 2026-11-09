//! Application lifecycle, configuration, logging, and shared infrastructure.
//!
//! Note: refer to this module as `crate::core` to avoid confusion with the
//! standard library's `core` crate.

pub mod config;
pub mod error;
pub mod lifecycle;
pub mod logging;
#[cfg(debug_assertions)]
pub mod smoke_test;
