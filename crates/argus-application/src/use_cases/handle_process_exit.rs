use std::sync::{Arc, Mutex};

use argus_domain::WorkspaceId;

use crate::ports::PtyHandleId;
use crate::workspace_manager::WorkspaceManager;

/// Removes a workspace whose process ended on its own (crash or normal exit).
/// v1 has no "restart" prompt — the workspace/panel is simply removed, matching
/// familiar terminal-app behavior.
pub struct HandleProcessExitUseCase {
    manager: Arc<Mutex<WorkspaceManager>>,
}

impl HandleProcessExitUseCase {
    pub fn new(manager: Arc<Mutex<WorkspaceManager>>) -> Self {
        Self { manager }
    }

    pub fn execute(&self, workspace_id: WorkspaceId) -> Option<PtyHandleId> {
        self.manager.lock().unwrap().remove(workspace_id)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use argus_domain::Workspace;

    #[test]
    fn removes_workspace_on_unexpected_exit() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let workspace = Workspace::new(WorkspaceId::new(), PathBuf::from("/tmp/project"));
        let id = workspace.id;
        manager.lock().unwrap().register(workspace, PtyHandleId::new());

        let use_case = HandleProcessExitUseCase::new(Arc::clone(&manager));
        let freed = use_case.execute(id);

        assert!(freed.is_some());
        assert!(manager.lock().unwrap().get(id).is_none());
    }

    #[test]
    fn unknown_workspace_is_a_no_op() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let use_case = HandleProcessExitUseCase::new(Arc::clone(&manager));

        assert_eq!(use_case.execute(WorkspaceId::new()), None);
    }
}
