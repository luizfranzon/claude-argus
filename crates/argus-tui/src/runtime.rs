use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use argus_application::ports::{
    ExitReason, FileSystemPort, FileWatcherPort, GitPort, PtyPort, WatchHandle,
};
use argus_application::use_cases::{
    ConfirmCloseError, ConfirmCloseSessionError, ConfirmCloseSessionUseCase,
    ConfirmCloseWorkspaceUseCase, CreateSessionUseCase, CreateWorkspaceUseCase,
    HandleSessionProcessExitUseCase, RequestCloseSessionUseCase, RequestCloseWorkspaceUseCase,
    ResolveStartupPathUseCase, SearchWorkspaceUseCase,
};
use argus_application::WorkspaceManager;
use argus_domain::{SessionId, WorkspaceId};
use argus_infrastructure::{
    claude_sessions_dir, GitCliAdapter, HomeDirResolver, HookServer, NotifyWatcherAdapter,
    PlatformHomeDirResolver, PlatformPathResolver, PortablePtyAdapter, RipgrepSearchAdapter,
    StdFsAdapter,
};
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::event::AppEvent;

/// Appends a timestamped line to `~/.claude/argus-watch-errors.log` when a
/// `FileWatcherPort::watch()` call fails. These failures are otherwise
/// silent (see `watch_claude_sessions`/`watch_workspace`) — Argus keeps
/// running without live-refresh rather than crashing — but silent means
/// undiagnosable, so this gives you *something* to check when a Session
/// rename or file-explorer refresh mysteriously never shows up. Best-effort:
/// if the log itself can't be written, there's nothing more useful to do.
fn log_watch_failure(what: &str, err: &argus_application::ports::WatchError) {
    use std::io::Write;
    let Some(home) = PlatformHomeDirResolver.home_dir() else { return };
    let path = home.join(".claude/argus-watch-errors.log");
    let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = writeln!(file, "[{now}] failed to watch {what}: {err}");
}

/// Diffs `~/.claude/sessions/*.json` against the manager's live Sessions and
/// emits `ClaudeSessionRenamed` for each mismatch. A free function (not a
/// `Runtime` method) so the watch callback — which only has `manager`/`tx`,
/// not a whole `Runtime` — can call it directly without cloning one.
fn sync_claude_session_names(
    manager: &Arc<Mutex<WorkspaceManager>>,
    dir: &PathBuf,
    tx: &UnboundedSender<AppEvent>,
) {
    let claude_names = argus_infrastructure::read_claude_session_names(dir);
    if claude_names.is_empty() {
        return;
    }

    let manager = manager.lock().unwrap();
    for (claude_session_id, name) in claude_names {
        let session_id = SessionId::from(claude_session_id);
        let Some(session) = manager.get_session(session_id) else { continue };
        if session.name != name {
            let _ = tx.send(AppEvent::ClaudeSessionRenamed(session_id, name));
        }
    }
}

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
    search_workspace: Arc<SearchWorkspaceUseCase<RipgrepSearchAdapter>>,
    tx: UnboundedSender<AppEvent>,
}

impl Runtime {
    pub fn new(tx: UnboundedSender<AppEvent>) -> std::io::Result<Self> {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(PortablePtyAdapter::new());
        let fs = Arc::new(StdFsAdapter::new());
        let watcher = Arc::new(NotifyWatcherAdapter::new());
        let git = Arc::new(GitCliAdapter::new());
        let search_workspace = Arc::new(SearchWorkspaceUseCase::new(Arc::new(RipgrepSearchAdapter::new())));
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
            search_workspace,
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

    /// Watches `~/.claude/sessions` so a `/rename` typed inside a `claude`
    /// session (Argus's own session file, since `--session-id` makes Argus's
    /// `SessionId` *be* Claude Code's session id) gets picked up here too.
    /// A no-op (returns `None`) if `HOME` can't be resolved or the directory
    /// can't be watched — Argus works fine without this, it just misses the
    /// auto-rename. Runs one initial sync so already-renamed sessions (e.g.
    /// `/rename`d before Argus started watching) catch up immediately.
    pub fn watch_claude_sessions(&self) -> Option<WatchHandle> {
        let dir = claude_sessions_dir()?;
        sync_claude_session_names(&self.manager, &dir, &self.tx);

        let tx = self.tx.clone();
        let manager = Arc::clone(&self.manager);
        let watch_dir = dir.clone();
        let display_dir = dir.clone();
        self.watcher
            .watch(
                dir,
                Box::new(move || sync_claude_session_names(&manager, &watch_dir, &tx)),
            )
            .inspect_err(|e| log_watch_failure(&format!("claude sessions dir {display_dir:?}"), e))
            .ok()
    }

    pub fn watch_workspace(&self, workspace_id: WorkspaceId, root: PathBuf) {
        let tx = self.tx.clone();
        let display_root = root.clone();
        match self.watcher.watch(
            root,
            Box::new(move || {
                let _ = tx.send(AppEvent::FsChanged(workspace_id));
            }),
        ) {
            Ok(handle) => self.manager.lock().unwrap().set_watch_handle(workspace_id, handle),
            Err(e) => log_watch_failure(&format!("workspace root {display_root:?}"), &e),
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
            // Four independent `git` subprocess calls — each pays its own
            // process-spawn cost, so running them concurrently rather than
            // one after another cuts the wall-clock latency of a refresh
            // roughly 4x instead of paying that cost four times over.
            let (status, branch, branches, sync) = tokio::join!(
                git.status(repo.clone()),
                git.current_branch(repo.clone()),
                git.list_branches(repo.clone()),
                git.sync_status(repo.clone()),
            );
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

    /// Walks the workspace's file tree for the fuzzy finder's Files-mode
    /// index, via `SearchWorkspaceUseCase` — see `FileSearchPort` (ADR-0013
    /// for why this doesn't go through `GitPort` instead).
    pub fn spawn_index_files(&self, workspace_id: WorkspaceId, root: PathBuf, include_ignored: bool) {
        let search_workspace = Arc::clone(&self.search_workspace);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let files = search_workspace.index_files(root, include_ignored).await;
            let _ = tx.send(AppEvent::FinderIndexed { workspace_id, all: include_ignored, files });
        });
    }

    /// Runs a Content-mode grep across the workspace via
    /// `SearchWorkspaceUseCase`. `generation` lets the caller discard a
    /// result that arrives after the query has already moved on.
    pub fn spawn_finder_grep(
        &self,
        workspace_id: WorkspaceId,
        root: PathBuf,
        query: String,
        include_ignored: bool,
        generation: u64,
    ) {
        let search_workspace = Arc::clone(&self.search_workspace);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(crate::fuzzy_finder::GREP_DEBOUNCE).await;
            let matches = search_workspace
                .search_content(root, query, include_ignored)
                .await
                .into_iter()
                .map(|m| crate::fuzzy_finder::FinderMatch {
                    path: m.path,
                    indices: Vec::new(),
                    line: m.line,
                    line_text: m.line_text,
                })
                .collect();
            let _ = tx.send(AppEvent::FinderSearchResult { workspace_id, generation, matches });
        });
    }

    /// Loads a preview of `path`'s contents for the fuzzy finder's preview
    /// pane. `FileSystemPort` has no partial-read API, so the whole file is
    /// read first; truncation is pure display formatting and stays on this
    /// task, while syntax highlighting (real regex-driven parsing work) goes
    /// through `SearchWorkspaceUseCase`, which dispatches it to a
    /// blocking-pool thread — running it on the UI loop's own task
    /// previously froze the whole app for the duration of every preview.
    pub fn spawn_finder_preview(&self, path: PathBuf, generation: u64) {
        let fs = Arc::clone(&self.fs);
        let search_workspace = Arc::clone(&self.search_workspace);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let read_result = fs.read_file(path.clone()).await;
            let result = match read_result {
                Ok(contents) => {
                    let truncated = crate::fuzzy_finder::truncate_preview(&contents);
                    let highlighted = search_workspace.preview_highlight(path.clone(), truncated.clone()).await;
                    Ok((truncated, highlighted))
                }
                Err(e) => Err(e),
            };
            let _ = tx.send(AppEvent::FinderPreviewLoaded { generation, path, result });
        });
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
