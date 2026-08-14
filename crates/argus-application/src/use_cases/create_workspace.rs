use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use argus_domain::{Workspace, WorkspaceId, WorkspaceStatus};
use thiserror::Error;

use crate::ports::{DirectoryPicker, ExitReason, PtyError, PtyPort, SpawnSpec};
use crate::use_cases::handle_process_exit::HandleProcessExitUseCase;
use crate::workspace_manager::WorkspaceManager;

pub type OutputSink = Box<dyn Fn(Vec<u8>) + Send + Sync>;
pub type ExitSink = Box<dyn Fn(WorkspaceId, ExitReason) + Send + Sync>;

#[derive(Debug, Error)]
pub enum CreateWorkspaceError {
    #[error("failed to spawn claude process: {0}")]
    PtySpawnFailed(PtyError),
}

/// Creates a Workspace either via the native folder picker or by duplicating
/// an existing workspace's directory (which never opens the picker).
pub struct CreateWorkspaceUseCase<Pty: PtyPort, Picker: DirectoryPicker> {
    manager: Arc<Mutex<WorkspaceManager>>,
    pty: Arc<Pty>,
    picker: Arc<Picker>,
    process_exit: Arc<HandleProcessExitUseCase>,
}

impl<Pty: PtyPort, Picker: DirectoryPicker> CreateWorkspaceUseCase<Pty, Picker> {
    pub fn new(
        manager: Arc<Mutex<WorkspaceManager>>,
        pty: Arc<Pty>,
        picker: Arc<Picker>,
        process_exit: Arc<HandleProcessExitUseCase>,
    ) -> Self {
        Self {
            manager,
            pty,
            picker,
            process_exit,
        }
    }

    /// Opens the native folder picker. Returns `Ok(None)` if the user cancels.
    pub async fn create_via_picker(
        &self,
        on_output: OutputSink,
        on_exit: ExitSink,
    ) -> Result<Option<Workspace>, CreateWorkspaceError> {
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
        on_exit: ExitSink,
    ) -> Result<Option<Workspace>, CreateWorkspaceError> {
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
        on_exit: ExitSink,
    ) -> Result<Workspace, CreateWorkspaceError> {
        self.spawn_workspace(directory, on_output, on_exit).await
    }

    async fn spawn_workspace(
        &self,
        directory: PathBuf,
        on_output: OutputSink,
        on_exit: ExitSink,
    ) -> Result<Workspace, CreateWorkspaceError> {
        let workspace_id = WorkspaceId::new();
        let env_path = {
            self.manager
                .lock()
                .unwrap()
                .resolved_path()
                .unwrap_or_default()
                .to_string()
        };
        let process_exit = Arc::clone(&self.process_exit);

        let spec = SpawnSpec {
            program: "claude".to_string(),
            args: Vec::new(),
            cwd: directory.clone(),
            env_path,
            on_output,
            on_exit: Box::new(move |reason| {
                process_exit.execute(workspace_id);
                on_exit(workspace_id, reason);
            }),
        };

        let pty_handle = self
            .pty
            .spawn(spec)
            .await
            .map_err(CreateWorkspaceError::PtySpawnFailed)?;

        let mut workspace = Workspace::new(workspace_id, directory);
        workspace.status = WorkspaceStatus::Running;
        self.manager
            .lock()
            .unwrap()
            .register(workspace.clone(), pty_handle);

        Ok(workspace)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::testing::{FakeDirectoryPicker, FakePtyPort};

    fn noop_sinks() -> (OutputSink, ExitSink) {
        (Box::new(|_| {}), Box::new(|_, _| {}))
    }

    #[tokio::test]
    async fn create_via_picker_registers_a_running_workspace() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(FakePtyPort::new());
        let picker = Arc::new(FakeDirectoryPicker::returning(Some(PathBuf::from("/tmp/project"))));
        let process_exit = Arc::new(HandleProcessExitUseCase::new(Arc::clone(&manager)));
        let use_case = CreateWorkspaceUseCase::new(
            Arc::clone(&manager),
            Arc::clone(&pty),
            Arc::clone(&picker),
            process_exit,
        );

        let (on_output, on_exit) = noop_sinks();
        let workspace = use_case
            .create_via_picker(on_output, on_exit)
            .await
            .unwrap()
            .expect("picker returned a directory");

        assert_eq!(workspace.status, WorkspaceStatus::Running);
        assert_eq!(workspace.directory, PathBuf::from("/tmp/project"));
        assert!(manager.lock().unwrap().get(workspace.id).is_some());
    }

    #[tokio::test]
    async fn create_via_picker_returns_none_when_cancelled() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(FakePtyPort::new());
        let picker = Arc::new(FakeDirectoryPicker::returning(None));
        let process_exit = Arc::new(HandleProcessExitUseCase::new(Arc::clone(&manager)));
        let use_case = CreateWorkspaceUseCase::new(manager, pty, picker, process_exit);

        let (on_output, on_exit) = noop_sinks();
        let result = use_case.create_via_picker(on_output, on_exit).await.unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn duplicate_never_calls_the_picker() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(FakePtyPort::new());
        let picker = Arc::new(FakeDirectoryPicker::returning(Some(PathBuf::from("/should-not-be-used"))));
        let process_exit = Arc::new(HandleProcessExitUseCase::new(Arc::clone(&manager)));
        let use_case = CreateWorkspaceUseCase::new(
            Arc::clone(&manager),
            pty,
            Arc::clone(&picker),
            process_exit,
        );

        let (on_output, on_exit) = noop_sinks();
        let source = use_case
            .create_with_directory(PathBuf::from("/tmp/source"), on_output, on_exit)
            .await
            .unwrap();

        let (on_output, on_exit) = noop_sinks();
        let duplicate = use_case
            .duplicate(source.id, on_output, on_exit)
            .await
            .unwrap()
            .expect("source workspace exists");

        assert_eq!(duplicate.directory, PathBuf::from("/tmp/source"));
        assert_eq!(picker.call_count(), 0);
    }

    #[tokio::test]
    async fn pty_exit_removes_the_workspace_via_handle_process_exit() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(FakePtyPort::new());
        let picker = Arc::new(FakeDirectoryPicker::returning(None));
        let process_exit = Arc::new(HandleProcessExitUseCase::new(Arc::clone(&manager)));
        let use_case = CreateWorkspaceUseCase::new(
            Arc::clone(&manager),
            Arc::clone(&pty),
            picker,
            process_exit,
        );

        let (on_output, on_exit) = noop_sinks();
        let workspace = use_case
            .create_with_directory(PathBuf::from("/tmp/project"), on_output, on_exit)
            .await
            .unwrap();
        let handle = manager.lock().unwrap().pty_handle_for(workspace.id).unwrap();

        pty.trigger_exit(handle, ExitReason::Crashed);

        assert!(manager.lock().unwrap().get(workspace.id).is_none());
    }
}
