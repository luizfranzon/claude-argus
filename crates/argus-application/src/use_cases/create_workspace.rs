use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use argus_domain::{Session, Workspace, WorkspaceId, WorkspaceStatus};
use thiserror::Error;

use crate::ports::{HookCallbackPort, PtyPort};
use crate::use_cases::create_session::{CreateSessionError, CreateSessionUseCase, OutputSink, SessionExitSink};
use crate::workspace_manager::WorkspaceManager;

#[derive(Debug, Error)]
pub enum CreateWorkspaceError {
    #[error("failed to create initial session: {0}")]
    SessionSpawnFailed(CreateSessionError),
}

/// A newly-created Workspace together with the first Session that was
/// auto-spawned inside it — a Workspace is never returned "empty", matching
/// today's UX of "open a folder, an agent starts" (see ADR-0010).
#[derive(Debug, Clone)]
pub struct CreatedWorkspace {
    pub workspace: Workspace,
    pub first_session: Session,
}

/// Auto-spawns a Workspace's first Session for a directory already known
/// (from cwd/argv on launch, or typed into a modal).
pub struct CreateWorkspaceUseCase<Pty: PtyPort, Hooks: HookCallbackPort> {
    manager: Arc<Mutex<WorkspaceManager>>,
    create_session: Arc<CreateSessionUseCase<Pty, Hooks>>,
}

impl<Pty: PtyPort, Hooks: HookCallbackPort> CreateWorkspaceUseCase<Pty, Hooks> {
    pub fn new(
        manager: Arc<Mutex<WorkspaceManager>>,
        create_session: Arc<CreateSessionUseCase<Pty, Hooks>>,
    ) -> Self {
        Self {
            manager,
            create_session,
        }
    }

    pub async fn create_with_directory(
        &self,
        directory: PathBuf,
        on_output: OutputSink,
        on_exit: SessionExitSink,
    ) -> Result<CreatedWorkspace, CreateWorkspaceError> {
        self.spawn_workspace(directory, on_output, on_exit).await
    }

    async fn spawn_workspace(
        &self,
        directory: PathBuf,
        on_output: OutputSink,
        on_exit: SessionExitSink,
    ) -> Result<CreatedWorkspace, CreateWorkspaceError> {
        let workspace_id = WorkspaceId::new();
        let workspace = Workspace::new(workspace_id, directory);
        self.manager.lock().unwrap().register(workspace);

        let first_session = self
            .create_session
            .execute(workspace_id, Some("Session 1".to_string()), on_output, on_exit)
            .await
            .map_err(CreateWorkspaceError::SessionSpawnFailed)?;

        let workspace = {
            let mut manager = self.manager.lock().unwrap();
            manager.set_status(workspace_id, WorkspaceStatus::Running);
            manager
                .get(workspace_id)
                .cloned()
                .expect("just registered above")
        };

        Ok(CreatedWorkspace {
            workspace,
            first_session,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::testing::{FakeHookCallbackPort, FakePtyPort};
    use crate::use_cases::handle_process_exit::HandleSessionProcessExitUseCase;
    use argus_domain::WorkspaceStatus;

    fn noop_sinks() -> (OutputSink, SessionExitSink) {
        (Box::new(|_| {}), Box::new(|_, _| {}))
    }

    fn use_case(
        manager: Arc<Mutex<WorkspaceManager>>,
        pty: Arc<FakePtyPort>,
    ) -> CreateWorkspaceUseCase<FakePtyPort, FakeHookCallbackPort> {
        let process_exit = Arc::new(HandleSessionProcessExitUseCase::new(Arc::clone(&manager)));
        let hooks = Arc::new(FakeHookCallbackPort::new("http://127.0.0.1:9999/hook"));
        let create_session = Arc::new(CreateSessionUseCase::new(Arc::clone(&manager), pty, hooks, process_exit));
        CreateWorkspaceUseCase::new(manager, create_session)
    }

    #[tokio::test]
    async fn create_with_directory_registers_a_running_workspace_with_first_session() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(FakePtyPort::new());
        let use_case = use_case(Arc::clone(&manager), pty);

        let (on_output, on_exit) = noop_sinks();
        let created = use_case
            .create_with_directory(PathBuf::from("/tmp/project"), on_output, on_exit)
            .await
            .unwrap();

        assert_eq!(created.workspace.status, WorkspaceStatus::Running);
        assert_eq!(created.workspace.directory, PathBuf::from("/tmp/project"));
        assert_eq!(created.first_session.name, "Session 1");
        assert!(manager.lock().unwrap().get(created.workspace.id).is_some());
        assert!(manager
            .lock()
            .unwrap()
            .get_session(created.first_session.id)
            .is_some());
    }

    #[tokio::test]
    async fn session_pty_exit_removes_only_the_session_not_the_workspace() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(FakePtyPort::new());
        let use_case = use_case(Arc::clone(&manager), Arc::clone(&pty));

        let (on_output, on_exit) = noop_sinks();
        let created = use_case
            .create_with_directory(PathBuf::from("/tmp/project"), on_output, on_exit)
            .await
            .unwrap();
        let handle = manager
            .lock()
            .unwrap()
            .pty_handle_for_session(created.first_session.id)
            .unwrap();

        pty.trigger_exit(handle, crate::ports::ExitReason::Crashed);

        assert!(manager
            .lock()
            .unwrap()
            .get_session(created.first_session.id)
            .is_none());
        assert!(manager.lock().unwrap().get(created.workspace.id).is_some());
    }
}
