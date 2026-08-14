use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::workspace::WorkspaceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for SessionId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

/// Plain UUID text (e.g. `3fa85f64-5717-4562-b3fc-2c963f66afa6`), no
/// wrapping — used as `claude --session-id <this>` and to correlate hook
/// callbacks back to a Session (see docs/adr/0010).
impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Starting,
    Running,
    AwaitingCloseConfirmation,
    Terminating,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: SessionId,
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub status: SessionStatus,
}

impl Session {
    pub fn new(id: SessionId, workspace_id: WorkspaceId, name: String) -> Self {
        Self {
            id,
            workspace_id,
            name,
            status: SessionStatus::Starting,
        }
    }
}

/// Whether closing a session in the given status requires user confirmation.
/// Same rule as `Workspace`'s (see `workspace::close_requires_confirmation`) —
/// kept as a separate function since the two status enums are distinct types.
pub fn close_requires_confirmation(status: SessionStatus) -> bool {
    matches!(
        status,
        SessionStatus::Starting | SessionStatus::Running | SessionStatus::AwaitingCloseConfirmation
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_ids_are_unique() {
        assert_ne!(SessionId::new(), SessionId::new());
    }

    #[test]
    fn new_session_starts_in_starting_status() {
        let session = Session::new(SessionId::new(), WorkspaceId::new(), "Session 1".to_string());
        assert_eq!(session.status, SessionStatus::Starting);
    }

    #[test]
    fn running_session_requires_close_confirmation() {
        assert!(close_requires_confirmation(SessionStatus::Running));
    }

    #[test]
    fn terminating_session_does_not_require_confirmation() {
        assert!(!close_requires_confirmation(SessionStatus::Terminating));
    }
}
