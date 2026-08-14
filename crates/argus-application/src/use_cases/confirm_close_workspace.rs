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
/// kills its PTY process, then removes the workspace/panel.
pub struct ConfirmCloseWorkspaceUseCase<Pty: PtyPort> {
    manager: Arc<Mutex<WorkspaceManager>>,
    pty: Arc<Pty>,
}

impl<Pty: PtyPort> ConfirmCloseWorkspaceUseCase<Pty> {
    pub fn new(manager: Arc<Mutex<WorkspaceManager>>, pty: Arc<Pty>) -> Self {
        Self { manager, pty }
    }

    pub fn execute(&self, workspace_id: WorkspaceId) -> Result<(), ConfirmCloseError> {
        let pty_handle = { self.manager.lock().unwrap().pty_handle_for(workspace_id) };
        if let Some(handle) = pty_handle {
            self.pty.kill(handle).map_err(ConfirmCloseError::KillFailed)?;
        }
        self.manager.lock().unwrap().remove(workspace_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::testing::FakePtyPort;
    use argus_domain::Workspace;

    #[test]
    fn kills_pty_exactly_once_and_removes_workspace() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(FakePtyPort::new());
        let workspace = Workspace::new(WorkspaceId::new(), PathBuf::from("/tmp/project"));
        let id = workspace.id;
        let handle = crate::ports::PtyHandleId::new();
        manager.lock().unwrap().register(workspace, handle);

        let use_case = ConfirmCloseWorkspaceUseCase::new(Arc::clone(&manager), Arc::clone(&pty));
        use_case.execute(id).unwrap();

        assert_eq!(pty.kill_calls(), vec![handle]);
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
