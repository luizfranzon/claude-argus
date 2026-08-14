use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use argus_domain::{Session, Workspace, WorkspaceId, WorkspaceStatus};
use thiserror::Error;

use crate::ports::{DirectoryPicker, HookCallbackPort, PtyPort};
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

/// Creates a Workspace either via the native folder picker or by duplicating
/// an existing workspace's directory (which never opens the picker), and
/// auto-spawns its first Session.
pub struct CreateWorkspaceUseCase<Pty: PtyPort, Picker: DirectoryPicker, Hooks: HookCallbackPort> {
    manager: Arc<Mutex<WorkspaceManager>>,
    picker: Arc<Picker>,
    create_session: Arc<CreateSessionUseCase<Pty, Hooks>>,
}

impl<Pty: PtyPort, Picker: DirectoryPicker, Hooks: HookCallbackPort> CreateWorkspaceUseCase<Pty, Picker, Hooks> {
    pub fn new(
        manager: Arc<Mutex<WorkspaceManager>>,
        picker: Arc<Picker>,
        create_session: Arc<CreateSessionUseCase<Pty, Hooks>>,
    ) -> Self {
        Self {
            manager,
            picker,
            create_session,
        }
    }

    /// Opens the native folder picker. Returns `Ok(None)` if the user cancels.
    pub async fn create_via_picker(
        &self,
        on_output: OutputSink,
        on_exit: SessionExitSink,
    ) -> Result<Option<CreatedWorkspace>, CreateWorkspaceError> {
        let Some(dir) = self.picker.pick_folder(None).await else {
            return Ok(None);
        };
        self.spawn_workspace(dir, on_output, on_exit).await.map(Some)
    }

    /// Reuses `source_id`'s directory. Never calls the picker.
    pub async fn duplicate(
        &self,
        source_id: WorkspaceId,
        on_output: OutputSink,
        on_exit: SessionExitSink,
    ) -> Result<Option<CreatedWorkspace>, CreateWorkspaceError> {
        let dir = {
            let manager = self.manager.lock().unwrap();
            manager.get(source_id).map(|w| w.directory.clone())
        };
        let Some(dir) = dir else {
            return Ok(None);
        };
        self.spawn_workspace(dir, on_output, on_exit).await.map(Some)
    }

    /// Used for the very first workspace on CLI launch (directory already
    /// known from cwd/argv) and for the GUI-launch directory-picker screen.
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
    use crate::testing::{FakeDirectoryPicker, FakeHookCallbackPort, FakePtyPort};
    use crate::use_cases::handle_process_exit::HandleSessionProcessExitUseCase;
    use argus_domain::WorkspaceStatus;

    fn noop_sinks() -> (OutputSink, SessionExitSink) {
        (Box::new(|_| {}), Box::new(|_, _| {}))
    }

    fn use_case(
        manager: Arc<Mutex<WorkspaceManager>>,
        pty: Arc<FakePtyPort>,
        picker: Arc<FakeDirectoryPicker>,
    ) -> CreateWorkspaceUseCase<FakePtyPort, FakeDirectoryPicker, FakeHookCallbackPort> {
        let process_exit = Arc::new(HandleSessionProcessExitUseCase::new(Arc::clone(&manager)));
        let hooks = Arc::new(FakeHookCallbackPort::new("http://127.0.0.1:9999/hook"));
        let create_session = Arc::new(CreateSessionUseCase::new(Arc::clone(&manager), pty, hooks, process_exit));
        CreateWorkspaceUseCase::new(manager, picker, create_session)
    }

    #[tokio::test]
    async fn create_via_picker_registers_a_running_workspace_with_first_session() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(FakePtyPort::new());
        let picker = Arc::new(FakeDirectoryPicker::returning(Some(PathBuf::from("/tmp/project"))));
        let use_case = use_case(Arc::clone(&manager), pty, picker);

        let (on_output, on_exit) = noop_sinks();
        let created = use_case
            .create_via_picker(on_output, on_exit)
            .await
            .unwrap()
            .expect("picker returned a directory");

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
    async fn create_via_picker_returns_none_when_cancelled() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(FakePtyPort::new());
        let picker = Arc::new(FakeDirectoryPicker::returning(None));
        let use_case = use_case(manager, pty, picker);

        let (on_output, on_exit) = noop_sinks();
        let result = use_case.create_via_picker(on_output, on_exit).await.unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn duplicate_never_calls_the_picker() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(FakePtyPort::new());
        let picker = Arc::new(FakeDirectoryPicker::returning(Some(PathBuf::from("/should-not-be-used"))));
        let use_case = use_case(Arc::clone(&manager), pty, Arc::clone(&picker));

        let (on_output, on_exit) = noop_sinks();
        let source = use_case
            .create_with_directory(PathBuf::from("/tmp/source"), on_output, on_exit)
            .await
            .unwrap();

        let (on_output, on_exit) = noop_sinks();
        let duplicate = use_case
            .duplicate(source.workspace.id, on_output, on_exit)
            .await
            .unwrap()
            .expect("source workspace exists");

        assert_eq!(duplicate.workspace.directory, PathBuf::from("/tmp/source"));
        assert_eq!(picker.call_count(), 0);
    }

    #[tokio::test]
    async fn session_pty_exit_removes_only_the_session_not_the_workspace() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(FakePtyPort::new());
        let picker = Arc::new(FakeDirectoryPicker::returning(None));
        let use_case = use_case(Arc::clone(&manager), Arc::clone(&pty), picker);

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
