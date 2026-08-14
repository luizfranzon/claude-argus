use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkspaceId(Uuid);

impl WorkspaceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for WorkspaceId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceStatus {
    Starting,
    Running,
    AwaitingCloseConfirmation,
    Terminating,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub directory: PathBuf,
    pub status: WorkspaceStatus,
}

impl Workspace {
    pub fn new(id: WorkspaceId, directory: PathBuf) -> Self {
        Self {
            id,
            directory,
            status: WorkspaceStatus::Starting,
        }
    }
}

/// Whether closing a workspace in the given status requires user confirmation.
///
/// Always `true` for a live workspace in v1 (no idle/busy detection yet) — kept as
/// its own function so a future "only confirm if busy" rule only touches this one place.
pub fn close_requires_confirmation(status: WorkspaceStatus) -> bool {
    matches!(
        status,
        WorkspaceStatus::Starting | WorkspaceStatus::Running | WorkspaceStatus::AwaitingCloseConfirmation
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_ids_are_unique() {
        assert_ne!(WorkspaceId::new(), WorkspaceId::new());
    }

    #[test]
    fn new_workspace_starts_in_starting_status() {
        let workspace = Workspace::new(WorkspaceId::new(), PathBuf::from("/tmp/project"));
        assert_eq!(workspace.status, WorkspaceStatus::Starting);
    }

    #[test]
    fn running_workspace_requires_close_confirmation() {
        assert!(close_requires_confirmation(WorkspaceStatus::Running));
    }

    #[test]
    fn terminating_workspace_does_not_require_confirmation() {
        assert!(!close_requires_confirmation(WorkspaceStatus::Terminating));
    }
}
