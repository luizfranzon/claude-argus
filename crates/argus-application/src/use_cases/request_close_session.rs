use std::sync::{Arc, Mutex};

use argus_domain::{session_close_requires_confirmation, SessionId};

use crate::use_cases::request_close_workspace::CloseDecision;
use crate::workspace_manager::WorkspaceManager;

/// Pure decision, no side effects — tells the caller whether to show a
/// confirmation dialog before actually tearing a Session down. Mirrors
/// `RequestCloseWorkspaceUseCase`.
pub struct RequestCloseSessionUseCase {
    manager: Arc<Mutex<WorkspaceManager>>,
}

impl RequestCloseSessionUseCase {
    pub fn new(manager: Arc<Mutex<WorkspaceManager>>) -> Self {
        Self { manager }
    }

    pub fn execute(&self, session_id: SessionId) -> CloseDecision {
        let manager = self.manager.lock().unwrap();
        match manager.get_session(session_id) {
            Some(session) if session_close_requires_confirmation(session.status) => {
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
    use argus_domain::{Session, Workspace, WorkspaceId};
    use crate::ports::PtyHandleId;

    #[test]
    fn running_session_requires_confirmation() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let workspace = Workspace::new(WorkspaceId::new(), PathBuf::from("/tmp/project"));
        let workspace_id = workspace.id;
        manager.lock().unwrap().register(workspace);
        let session = Session::new(SessionId::new(), workspace_id, "Session 1".to_string());
        let id = session.id;
        manager.lock().unwrap().register_session(session, PtyHandleId::new());

        let use_case = RequestCloseSessionUseCase::new(manager);
        assert_eq!(use_case.execute(id), CloseDecision::RequiresConfirmation);
    }

    #[test]
    fn unknown_session_is_already_closed() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let use_case = RequestCloseSessionUseCase::new(manager);

        assert_eq!(use_case.execute(SessionId::new()), CloseDecision::AlreadyClosed);
    }
}
