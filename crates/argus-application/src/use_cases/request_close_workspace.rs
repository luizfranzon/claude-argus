use std::sync::{Arc, Mutex};

use argus_domain::{close_requires_confirmation, WorkspaceId};

use crate::workspace_manager::WorkspaceManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseDecision {
    RequiresConfirmation,
    AlreadyClosed,
}

/// Pure decision, no side effects — tells the caller whether to show a
/// confirmation dialog before actually tearing anything down.
pub struct RequestCloseWorkspaceUseCase {
    manager: Arc<Mutex<WorkspaceManager>>,
}

impl RequestCloseWorkspaceUseCase {
    pub fn new(manager: Arc<Mutex<WorkspaceManager>>) -> Self {
        Self { manager }
    }

    pub fn execute(&self, workspace_id: WorkspaceId) -> CloseDecision {
        let manager = self.manager.lock().unwrap();
        match manager.get(workspace_id) {
            Some(workspace) if close_requires_confirmation(workspace.status) => {
                CloseDecision::RequiresConfirmation
            }
            _ => CloseDecision::AlreadyClosed,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use argus_domain::Workspace;
    use crate::ports::PtyHandleId;

    #[test]
    fn running_workspace_requires_confirmation() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let workspace = Workspace::new(WorkspaceId::new(), PathBuf::from("/tmp/project"));
        let id = workspace.id;
        manager.lock().unwrap().register(workspace, PtyHandleId::new());

        let use_case = RequestCloseWorkspaceUseCase::new(manager);
        assert_eq!(use_case.execute(id), CloseDecision::RequiresConfirmation);
    }

    #[test]
    fn unknown_workspace_is_already_closed() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let use_case = RequestCloseWorkspaceUseCase::new(manager);

        assert_eq!(
            use_case.execute(WorkspaceId::new()),
            CloseDecision::AlreadyClosed
        );
    }
}
