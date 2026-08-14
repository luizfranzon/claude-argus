use std::sync::{Arc, Mutex};

use argus_domain::SessionId;
use thiserror::Error;

use crate::ports::{PtyError, PtyPort};
use crate::workspace_manager::WorkspaceManager;

#[derive(Debug, Error)]
pub enum ConfirmCloseError {
    #[error("failed to terminate process: {0}")]
    KillFailed(PtyError),
}

/// Actually tears a Session down after the user confirmed the close dialog:
/// kills its PTY process, then removes the Session/panel. Mirrors
/// `ConfirmCloseWorkspaceUseCase`.
pub struct ConfirmCloseSessionUseCase<Pty: PtyPort> {
    manager: Arc<Mutex<WorkspaceManager>>,
    pty: Arc<Pty>,
}

impl<Pty: PtyPort> ConfirmCloseSessionUseCase<Pty> {
    pub fn new(manager: Arc<Mutex<WorkspaceManager>>, pty: Arc<Pty>) -> Self {
        Self { manager, pty }
    }

    pub fn execute(&self, session_id: SessionId) -> Result<(), ConfirmCloseError> {
        let pty_handle = { self.manager.lock().unwrap().pty_handle_for_session(session_id) };
        if let Some(handle) = pty_handle {
            self.pty.kill(handle).map_err(ConfirmCloseError::KillFailed)?;
        }
        self.manager.lock().unwrap().remove_session(session_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::testing::FakePtyPort;
    use argus_domain::{Session, Workspace, WorkspaceId};

    #[test]
    fn kills_pty_exactly_once_and_removes_session() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(FakePtyPort::new());
        let workspace = Workspace::new(WorkspaceId::new(), PathBuf::from("/tmp/project"));
        let workspace_id = workspace.id;
        manager.lock().unwrap().register(workspace);
        let session = Session::new(SessionId::new(), workspace_id, "Session 1".to_string());
        let id = session.id;
        let handle = crate::ports::PtyHandleId::new();
        manager.lock().unwrap().register_session(session, handle);

        let use_case = ConfirmCloseSessionUseCase::new(Arc::clone(&manager), Arc::clone(&pty));
        use_case.execute(id).unwrap();

        assert_eq!(pty.kill_calls(), vec![handle]);
        assert!(manager.lock().unwrap().get_session(id).is_none());
    }

    #[test]
    fn closing_unknown_session_does_not_call_kill() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(FakePtyPort::new());
        let use_case = ConfirmCloseSessionUseCase::new(manager, Arc::clone(&pty));

        use_case.execute(SessionId::new()).unwrap();

        assert!(pty.kill_calls().is_empty());
    }
}
