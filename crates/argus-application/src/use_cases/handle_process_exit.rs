use std::sync::{Arc, Mutex};

use argus_domain::SessionId;

use crate::ports::PtyHandleId;
use crate::workspace_manager::WorkspaceManager;

/// Removes a Session whose process ended on its own (crash or normal exit).
/// v1/v2 had no "restart" prompt for a Workspace's single process; v3 keeps
/// the same behavior per-Session — the Session/panel is simply removed,
/// matching familiar terminal-app behavior. Only the Session that actually
/// exited is affected; its Workspace and sibling Sessions are untouched.
pub struct HandleSessionProcessExitUseCase {
    manager: Arc<Mutex<WorkspaceManager>>,
}

impl HandleSessionProcessExitUseCase {
    pub fn new(manager: Arc<Mutex<WorkspaceManager>>) -> Self {
        Self { manager }
    }

    pub fn execute(&self, session_id: SessionId) -> Option<PtyHandleId> {
        self.manager.lock().unwrap().remove_session(session_id)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use argus_domain::{Session, Workspace, WorkspaceId};

    #[test]
    fn removes_session_on_unexpected_exit() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let workspace = Workspace::new(WorkspaceId::new(), PathBuf::from("/tmp/project"));
        let workspace_id = workspace.id;
        manager.lock().unwrap().register(workspace);
        let session = Session::new(SessionId::new(), workspace_id, "Session 1".to_string());
        let session_id = session.id;
        manager
            .lock()
            .unwrap()
            .register_session(session, PtyHandleId::new());

        let use_case = HandleSessionProcessExitUseCase::new(Arc::clone(&manager));
        let freed = use_case.execute(session_id);

        assert!(freed.is_some());
        assert!(manager.lock().unwrap().get_session(session_id).is_none());
    }

    #[test]
    fn unknown_session_is_a_no_op() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let use_case = HandleSessionProcessExitUseCase::new(Arc::clone(&manager));

        assert_eq!(use_case.execute(SessionId::new()), None);
    }
}
