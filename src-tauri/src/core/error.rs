//! Errors returned to the frontend.

use serde::{Serialize, Serializer};

use crate::{
    database::DatabaseError, sessions::reminders::ReminderError, workspace::WorkspaceError,
};

/// Error type for all Tauri commands.
///
/// Serialized to the frontend as a plain, user-presentable message. Full
/// details are written to the log at the point of conversion.
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("database error")]
    Database(#[from] DatabaseError),

    #[error("workspace error: {0}")]
    Workspace(#[from] WorkspaceError),

    /// Validation messages are written for the user and shown as they are.
    #[error("{0}")]
    Reminder(#[from] ReminderError),

    #[error("the operation was interrupted")]
    Interrupted(String),
}

impl Serialize for CommandError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Log the detailed cause; send only the summary across the IPC boundary.
        log::error!("command failed: {self}: {self:?}");
        serializer.serialize_str(&self.to_string())
    }
}
