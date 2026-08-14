use std::sync::{Arc, Mutex};

use argus_domain::WorkspaceId;
use thiserror::Error;

use crate::ports::{PtyError, PtyPort};
use crate::workspace_manager::WorkspaceManager;

#[derive(Debug, Error)]
pub enum ConfirmCloseError {
    #[error("failed to terminate process: {0}")]
    KillFailed(PtyError),
}

/// Actually tears a workspace down after the user confirmed the close dialog:
/// kills every one of its Sessions' PTY processes (a Workspace can host
/// several, see ADR-0010), then removes the workspace and its panels. One
/// confirmation covers the whole Workspace, not one per Session.
pub struct ConfirmCloseWorkspaceUseCase<Pty: PtyPort> {
    manager: Arc<Mutex<WorkspaceManager>>,
    pty: Arc<Pty>,
}

impl<Pty: PtyPort> ConfirmCloseWorkspaceUseCase<Pty> {
    pub fn new(manager: Arc<Mutex<WorkspaceManager>>, pty: Arc<Pty>) -> Self {
        Self { manager, pty }
    }

    pub fn execute(&self, workspace_id: WorkspaceId) -> Result<(), ConfirmCloseError> {
        let freed_handles = self.manager.lock().unwrap().remove(workspace_id);
        for handle in freed_handles {
            self.pty.kill(handle).map_err(ConfirmCloseError::KillFailed)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::testing::FakePtyPort;
    use argus_domain::{Session, SessionId, Workspace};

    #[test]
    fn kills_every_session_pty_and_removes_workspace() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(FakePtyPort::new());
        let workspace = Workspace::new(WorkspaceId::new(), PathBuf::from("/tmp/project"));
        let id = workspace.id;
        manager.lock().unwrap().register(workspace);
        let first = Session::new(SessionId::new(), id, "Session 1".to_string());
        let first_handle = crate::ports::PtyHandleId::new();
        manager.lock().unwrap().register_session(first, first_handle);
        let second = Session::new(SessionId::new(), id, "Session 2".to_string());
        let second_handle = crate::ports::PtyHandleId::new();
        manager.lock().unwrap().register_session(second, second_handle);

        let use_case = ConfirmCloseWorkspaceUseCase::new(Arc::clone(&manager), Arc::clone(&pty));
        use_case.execute(id).unwrap();

        let killed = pty.kill_calls();
        assert_eq!(killed.len(), 2);
        assert!(killed.contains(&first_handle));
        assert!(killed.contains(&second_handle));
        assert!(manager.lock().unwrap().get(id).is_none());
    }

    #[test]
    fn closing_unknown_workspace_does_not_call_kill() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(FakePtyPort::new());
        let use_case = ConfirmCloseWorkspaceUseCase::new(manager, Arc::clone(&pty));

        use_case.execute(WorkspaceId::new()).unwrap();

        assert!(pty.kill_calls().is_empty());
    }
}
