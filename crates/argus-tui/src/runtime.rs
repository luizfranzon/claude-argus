use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use argus_application::ports::{ExitReason, FileSystemPort, FileWatcherPort, GitPort, PtyPort};
use argus_application::use_cases::{
    ConfirmCloseError, ConfirmCloseSessionError, ConfirmCloseSessionUseCase,
    ConfirmCloseWorkspaceUseCase, CreateSessionUseCase, CreateWorkspaceUseCase,
    HandleSessionProcessExitUseCase, RequestCloseSessionUseCase, RequestCloseWorkspaceUseCase,
    ResolveStartupPathUseCase,
};
use argus_application::WorkspaceManager;
use argus_domain::{SessionId, WorkspaceId};
use argus_infrastructure::{
    GitCliAdapter, HookServer, NotifyWatcherAdapter, PlatformPathResolver,
    PortablePtyAdapter, StdFsAdapter,
};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::event::AppEvent;

type TuiCreateSessionUseCase = CreateSessionUseCase<PortablePtyAdapter, HookServer>;
type TuiCreateWorkspaceUseCase = CreateWorkspaceUseCase<PortablePtyAdapter, HookServer>;

/// Composition root: owns every adapter/use case, dispatches results back
/// into the ratatui event loop over `tx`. `Clone` is cheap (every field is an
/// `Arc` or a `Copy`/cloneable use case wrapper) so it can be handed to
/// spawned tokio tasks freely.
#[derive(Clone)]
pub struct Runtime {
    pub manager: Arc<Mutex<WorkspaceManager>>,
    pub pty: Arc<PortablePtyAdapter>,
    pub fs: Arc<StdFsAdapter>,
    pub watcher: Arc<NotifyWatcherAdapter>,
    pub git: Arc<GitCliAdapter>,
    #[allow(dead_code)]
    hook_server: Arc<HookServer>,
    create_workspace: Arc<TuiCreateWorkspaceUseCase>,
    create_session: Arc<TuiCreateSessionUseCase>,
    request_close_workspace: Arc<RequestCloseWorkspaceUseCase>,
    confirm_close_workspace: Arc<ConfirmCloseWorkspaceUseCase<PortablePtyAdapter>>,
    request_close_session: Arc<RequestCloseSessionUseCase>,
    confirm_close_session: Arc<ConfirmCloseSessionUseCase<PortablePtyAdapter>>,
    resolve_startup_path: Arc<ResolveStartupPathUseCase<PlatformPathResolver>>,
    tx: UnboundedSender<AppEvent>,
}

impl Runtime {
    pub fn new(tx: UnboundedSender<AppEvent>) -> std::io::Result<Self> {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(PortablePtyAdapter::new());
        let fs = Arc::new(StdFsAdapter::new());
        let watcher = Arc::new(NotifyWatcherAdapter::new());
        let git = Arc::new(GitCliAdapter::new());
        let path_resolver = Arc::new(PlatformPathResolver);
        let session_process_exit =
            Arc::new(HandleSessionProcessExitUseCase::new(Arc::clone(&manager)));

        let hook_tx = tx.clone();
        let hook_server = Arc::new(HookServer::start(move |session_id, kind| {
            let _ = hook_tx.send(AppEvent::HookStatus(session_id, kind));
        })?);

        let create_session = Arc::new(CreateSessionUseCase::new(
            Arc::clone(&manager),
            Arc::clone(&pty),
            Arc::clone(&hook_server),
            session_process_exit,
        ));
        let create_workspace = Arc::new(CreateWorkspaceUseCase::new(
            Arc::clone(&manager),
            Arc::clone(&create_session),
        ));

        Ok(Self {
            request_close_workspace: Arc::new(RequestCloseWorkspaceUseCase::new(Arc::clone(
                &manager,
            ))),
            confirm_close_workspace: Arc::new(ConfirmCloseWorkspaceUseCase::new(
                Arc::clone(&manager),
                Arc::clone(&pty),
            )),
            request_close_session: Arc::new(RequestCloseSessionUseCase::new(Arc::clone(
                &manager,
            ))),
            confirm_close_session: Arc::new(ConfirmCloseSessionUseCase::new(
                Arc::clone(&manager),
                Arc::clone(&pty),
            )),
            resolve_startup_path: Arc::new(ResolveStartupPathUseCase::new(
                Arc::clone(&manager),
                path_resolver,
            )),
            manager,
            pty,
            fs,
            watcher,
            git,
            hook_server,
            create_workspace,
            create_session,
            tx,
        })
    }

    pub async fn resolve_startup_path(&self) {
        let _ = self.resolve_startup_path.execute().await;
    }

    fn output_sink(&self, stream_id: Uuid) -> argus_application::use_cases::OutputSink {
        let tx = self.tx.clone();
        Box::new(move |data: Vec<u8>| {
            let _ = tx.send(AppEvent::PtyOutput(stream_id, data));
        })
    }

    fn exit_sink(&self) -> argus_application::use_cases::SessionExitSink {
        let tx = self.tx.clone();
        Box::new(move |session_id, reason: ExitReason| {
            let _ = tx.send(AppEvent::SessionExited(session_id, reason));
        })
    }

    /// Spawns the initial workspace + its first session. Returns the
    /// `stream_id` the first session's PTY output will be tagged with, so the
    /// caller can pre-register a buffer for it before any output arrives.
    pub fn spawn_workspace(&self, directory: PathBuf) -> Uuid {
        let stream_id = Uuid::new_v4();
        let on_output = self.output_sink(stream_id);
        let on_exit = self.exit_sink();
        let create_workspace = Arc::clone(&self.create_workspace);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = create_workspace
                .create_with_directory(directory, on_output, on_exit)
                .await;
            let _ = tx.send(AppEvent::WorkspaceSpawned { stream_id, result });
        });
        stream_id
    }

    pub fn spawn_session(&self, workspace_id: WorkspaceId, name: Option<String>) -> Uuid {
        let stream_id = Uuid::new_v4();
        let on_output = self.output_sink(stream_id);
        let on_exit = self.exit_sink();
        let create_session = Arc::clone(&self.create_session);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = create_session
                .execute(workspace_id, name, on_output, on_exit)
                .await;
            let _ = tx.send(AppEvent::SessionSpawned {
                stream_id,
                workspace_id,
                result,
            });
        });
        stream_id
    }

    pub fn write_to_session(&self, session_id: SessionId, data: &[u8]) {
        if let Some(handle) = self.manager.lock().unwrap().pty_handle_for_session(session_id) {
            let _ = self.pty.write(handle, data);
        }
    }

    pub fn resize_session(&self, session_id: SessionId, cols: u16, rows: u16) {
        if let Some(handle) = self.manager.lock().unwrap().pty_handle_for_session(session_id) {
            let _ = self.pty.resize(handle, cols, rows);
        }
    }

    pub fn request_close_session(
        &self,
        session_id: SessionId,
    ) -> argus_application::use_cases::CloseDecision {
        self.request_close_session.execute(session_id)
    }

    pub fn confirm_close_session(&self, session_id: SessionId) -> Result<(), ConfirmCloseSessionError> {
        self.confirm_close_session.execute(session_id)
    }

    pub fn request_close_workspace(
        &self,
        workspace_id: WorkspaceId,
    ) -> argus_application::use_cases::CloseDecision {
        self.request_close_workspace.execute(workspace_id)
    }

    pub fn confirm_close_workspace(&self, workspace_id: WorkspaceId) -> Result<(), ConfirmCloseError> {
        self.confirm_close_workspace.execute(workspace_id)
    }

    pub fn rename_session(&self, session_id: SessionId, name: String) {
        self.manager.lock().unwrap().rename_session(session_id, name);
    }

    pub fn watch_workspace(&self, workspace_id: WorkspaceId, root: PathBuf) {
        let tx = self.tx.clone();
        if let Ok(handle) = self.watcher.watch(
            root,
            Box::new(move || {
                let _ = tx.send(AppEvent::FsChanged(workspace_id));
            }),
        ) {
            self.manager.lock().unwrap().set_watch_handle(workspace_id, handle);
        }
    }

    pub fn unwatch_workspace(&self, workspace_id: WorkspaceId) {
        if let Some(handle) = self.manager.lock().unwrap().take_watch_handle(workspace_id) {
            self.watcher.unwatch(handle);
        }
    }

    pub fn spawn_list_dir(&self, workspace_id: WorkspaceId, path: PathBuf) {
        let fs = Arc::clone(&self.fs);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = fs.list_dir(path.clone()).await;
            let _ = tx.send(AppEvent::DirLoaded(workspace_id, path, result));
        });
    }

    pub fn spawn_create_file(&self, workspace_id: WorkspaceId, path: PathBuf, parent: PathBuf) {
        let fs = Arc::clone(&self.fs);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = fs.create_file(path).await;
            let _ = tx.send(AppEvent::FsOpDone(workspace_id, parent, result));
        });
    }

    pub fn spawn_create_dir(&self, workspace_id: WorkspaceId, path: PathBuf, parent: PathBuf) {
        let fs = Arc::clone(&self.fs);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = fs.create_dir(path).await;
            let _ = tx.send(AppEvent::FsOpDone(workspace_id, parent, result));
        });
    }

    pub fn spawn_rename_path(
        &self,
        workspace_id: WorkspaceId,
        from: PathBuf,
        to: PathBuf,
        parent: PathBuf,
    ) {
        let fs = Arc::clone(&self.fs);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = fs.rename(from, to).await;
            let _ = tx.send(AppEvent::FsOpDone(workspace_id, parent, result));
        });
    }

    pub fn spawn_delete_path(&self, workspace_id: WorkspaceId, path: PathBuf, parent: PathBuf) {
        let fs = Arc::clone(&self.fs);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = fs.delete(path).await;
            let _ = tx.send(AppEvent::FsOpDone(workspace_id, parent, result));
        });
    }

    pub fn spawn_git_available(&self) {
        let git = Arc::clone(&self.git);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let available = git.is_git_available().await;
            let _ = tx.send(AppEvent::GitAvailable(available));
        });
    }

    pub fn spawn_git_list_repositories(&self, workspace_id: WorkspaceId, root: PathBuf) {
        let git = Arc::clone(&self.git);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let repos = git.list_repositories(root).await;
            let _ = tx.send(AppEvent::GitReposLoaded(workspace_id, repos));
        });
    }

    pub fn spawn_git_refresh(&self, workspace_id: WorkspaceId, repo: PathBuf) {
        let git = Arc::clone(&self.git);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let status = git.status(repo.clone()).await;
            let branch = git.current_branch(repo.clone()).await;
            let branches = git.list_branches(repo.clone()).await;
            let sync = git.sync_status(repo.clone()).await;
            let _ = tx.send(AppEvent::GitRefreshed {
                workspace_id,
                repo,
                status,
                branch,
                branches,
                sync,
            });
        });
    }

    pub fn spawn_git_log(&self, workspace_id: WorkspaceId, repo: PathBuf, skip: u32, limit: u32) {
        let git = Arc::clone(&self.git);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let entries = git.log(repo.clone(), skip, limit).await;
            let _ = tx.send(AppEvent::GitLogLoaded {
                workspace_id,
                repo,
                skip,
                entries,
            });
        });
    }

    pub fn spawn_git_stage(&self, workspace_id: WorkspaceId, repo: PathBuf, files: Vec<String>) {
        let git = Arc::clone(&self.git);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = git.stage(repo.clone(), files).await;
            let _ = tx.send(AppEvent::GitActionDone {
                workspace_id,
                repo,
                action: "stage",
                result,
            });
        });
    }

    pub fn spawn_git_unstage(&self, workspace_id: WorkspaceId, repo: PathBuf, files: Vec<String>) {
        let git = Arc::clone(&self.git);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = git.unstage(repo.clone(), files).await;
            let _ = tx.send(AppEvent::GitActionDone {
                workspace_id,
                repo,
                action: "unstage",
                result,
            });
        });
    }

    pub fn spawn_git_commit(&self, workspace_id: WorkspaceId, repo: PathBuf, message: String) {
        let git = Arc::clone(&self.git);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = git.commit(repo.clone(), message).await;
            let _ = tx.send(AppEvent::GitActionDone {
                workspace_id,
                repo,
                action: "commit",
                result,
            });
        });
    }

    pub fn spawn_git_switch_branch(&self, workspace_id: WorkspaceId, repo: PathBuf, name: String) {
        let git = Arc::clone(&self.git);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = git.switch_branch(repo.clone(), name).await;
            let _ = tx.send(AppEvent::GitActionDone {
                workspace_id,
                repo,
                action: "switch_branch",
                result,
            });
        });
    }

    pub fn spawn_git_push(&self, workspace_id: WorkspaceId, repo: PathBuf) {
        self.spawn_git_remote_action(workspace_id, repo, "push");
    }

    pub fn spawn_git_pull(&self, workspace_id: WorkspaceId, repo: PathBuf) {
        self.spawn_git_remote_action(workspace_id, repo, "pull");
    }

    pub fn spawn_git_fetch(&self, workspace_id: WorkspaceId, repo: PathBuf) {
        self.spawn_git_remote_action(workspace_id, repo, "fetch");
    }

    fn spawn_git_remote_action(
        &self,
        workspace_id: WorkspaceId,
        repo: PathBuf,
        action: &'static str,
    ) {
        let git = Arc::clone(&self.git);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let result = match action {
                "push" => git.push(repo.clone()).await,
                "pull" => git.pull(repo.clone()).await,
                _ => git.fetch(repo.clone()).await,
            };
            let _ = tx.send(AppEvent::GitActionDone {
                workspace_id,
                repo,
                action,
                result,
            });
        });
    }
}
