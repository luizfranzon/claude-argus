use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use argus_application::ports::{
    BranchInfo, CommitEntry, FileEntry, FileStatusEntry, GitRepository, SyncStatus,
};
use argus_application::use_cases::CloseDecision;
use argus_domain::{Session, SessionId, Workspace, WorkspaceId};
use argus_infrastructure::HookEventKind;
use base64::Engine;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use uuid::Uuid;

use crate::event::AppEvent;
use crate::notification::NotificationCenter;
use crate::runtime::Runtime;
use crate::ui::hitmap::{self, HitMap};

/// Scrollback kept per session's virtual screen. Generous since it's cheap
/// (plain cells in memory, not a rendered widget) and sessions stay alive for
/// the whole app run.
const SCROLLBACK: usize = 5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarTab {
    Agents,
    Explorer,
    Git,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeStatus {
    Thinking,
    Idle,
    /// Blocked on the user — a tool permission prompt or a proposed-response
    /// picker (e.g. `AskUserQuestion`). Fired by Claude Code's `Notification`
    /// hook, matcher-scoped (see ADR-0012) to skip `idle_prompt` and other
    /// non-blocking notification types.
    Waiting,
}

pub struct SessionEntry {
    pub session: Session,
    pub parser: vt100::Parser,
    pub status: Option<RuntimeStatus>,
    /// Set when the session finishes (`Stopped`, i.e. goes `Idle`) while it
    /// isn't the currently displayed session. Cleared once the session
    /// becomes `focused_session_id()` again — see `sync_focused_read`.
    pub unread: bool,
    /// Bytes carried over between `on_pty_output` calls while an OSC 52
    /// clipboard-set sequence from the child process is mid-stream — see
    /// `scan_osc52`.
    osc52_partial: Vec<u8>,
}

pub struct ExplorerState {
    pub dirs: HashMap<PathBuf, Vec<FileEntry>>,
    pub expanded: HashSet<PathBuf>,
    pub selected: usize,
}

impl ExplorerState {
    fn new() -> Self {
        Self {
            dirs: HashMap::new(),
            expanded: HashSet::new(),
            selected: 0,
        }
    }

    /// Flattens the currently-expanded tree into the rows the Explorer view
    /// draws top to bottom: `(path, depth, is_dir)`. Root's own entries start
    /// at depth 0; lazy — a directory only contributes rows once it has been
    /// expanded and its listing has arrived.
    pub fn flatten(&self, root: &PathBuf) -> Vec<(PathBuf, usize, bool)> {
        let mut rows = Vec::new();
        if let Some(entries) = self.dirs.get(root) {
            self.push_entries(entries, 0, &mut rows);
        }
        rows
    }

    fn push_entries(&self, entries: &[FileEntry], depth: usize, rows: &mut Vec<(PathBuf, usize, bool)>) {
        for entry in entries {
            rows.push((entry.path.clone(), depth, entry.is_dir));
            if entry.is_dir && self.expanded.contains(&entry.path) {
                if let Some(children) = self.dirs.get(&entry.path) {
                    self.push_entries(children, depth + 1, rows);
                }
            }
        }
    }
}

pub struct GitRepoState {
    pub repo: GitRepository,
    pub status: Vec<FileStatusEntry>,
    pub branch: Option<String>,
    pub branches: Vec<BranchInfo>,
    pub sync: Option<SyncStatus>,
    pub log: Vec<CommitEntry>,
    pub log_loading: bool,
    pub log_complete: bool,
}

impl GitRepoState {
    fn new(repo: GitRepository) -> Self {
        Self {
            repo,
            status: Vec::new(),
            branch: None,
            branches: Vec::new(),
            sync: None,
            log: Vec::new(),
            log_loading: false,
            log_complete: false,
        }
    }
}

pub struct GitState {
    pub available: Option<bool>,
    pub repos: Vec<GitRepoState>,
    pub selected_repo: usize,
    pub selected_file: usize,
    pub commit_message: String,
    pub show_log: bool,
}

impl GitState {
    fn new() -> Self {
        Self {
            available: None,
            repos: Vec::new(),
            selected_repo: 0,
            selected_file: 0,
            commit_message: String::new(),
            show_log: false,
        }
    }

    pub fn active_repo(&self) -> Option<&GitRepoState> {
        self.repos.get(self.selected_repo)
    }
}

pub struct WorkspaceEntry {
    pub workspace: Workspace,
    pub sessions: Vec<SessionId>,
    pub focused_session: Option<SessionId>,
    pub sidebar_tab: SidebarTab,
    pub agents_selected: usize,
    pub explorer: ExplorerState,
    pub git: GitState,
}

impl WorkspaceEntry {
    fn new(workspace: Workspace) -> Self {
        Self {
            workspace,
            sessions: Vec::new(),
            focused_session: None,
            sidebar_tab: SidebarTab::Agents,
            agents_selected: 0,
            explorer: ExplorerState::new(),
            git: GitState::new(),
        }
    }
}

pub enum Modal {
    NewWorkspacePath { input: String },
    RenameSession { session_id: SessionId, input: String },
    NewFile { workspace_id: WorkspaceId, dir: PathBuf, input: String },
    NewDir { workspace_id: WorkspaceId, dir: PathBuf, input: String },
    RenamePath { workspace_id: WorkspaceId, from: PathBuf, input: String },
    CommitMessage { workspace_id: WorkspaceId, repo: PathBuf, input: String },
    ConfirmCloseSession { session_id: SessionId },
    ConfirmCloseWorkspace { workspace_id: WorkspaceId },
    ConfirmDeletePath { workspace_id: WorkspaceId, path: PathBuf, parent: PathBuf },
}

impl Modal {
    /// Routes a bracketed paste into whichever `input` field this modal
    /// carries, if any — the confirm variants have nothing to paste into.
    fn paste(&mut self, text: &str) {
        let input = match self {
            Modal::NewWorkspacePath { input }
            | Modal::RenameSession { input, .. }
            | Modal::NewFile { input, .. }
            | Modal::NewDir { input, .. }
            | Modal::RenamePath { input, .. }
            | Modal::CommitMessage { input, .. } => input,
            Modal::ConfirmCloseSession { .. }
            | Modal::ConfirmCloseWorkspace { .. }
            | Modal::ConfirmDeletePath { .. } => return,
        };
        crate::text_input::apply_paste(input, text);
    }
}

pub struct AppState {
    pub runtime: Runtime,
    pub workspaces: Vec<WorkspaceId>,
    pub workspace_entries: HashMap<WorkspaceId, WorkspaceEntry>,
    pub sessions: HashMap<SessionId, SessionEntry>,
    pub active_workspace: Option<WorkspaceId>,
    pub focus: Focus,
    pub modal: Option<Modal>,
    pub status_line: String,
    status_line_at: Option<std::time::Instant>,
    pub should_quit: bool,
    stream_to_session: HashMap<Uuid, SessionId>,
    pending_output: HashMap<Uuid, Vec<u8>>,
    pending_workspace_stream: Option<Uuid>,
    /// Content area (cols, rows) last handed to the focused session's PTY —
    /// used to size a session's `vt100::Parser` the moment it's created.
    pub terminal_size: (u16, u16),
    pub sidebar_width: u16,
    /// Set while the user is dragging the sidebar/terminal divider (mouse
    /// button down on it and still held) — highlights the border and routes
    /// further mouse-move events to resizing instead of click hit-testing.
    pub resizing_sidebar: bool,
    /// Whether crossterm mouse capture is (meant to be) enabled. Starts
    /// `true` so TUI mouse clicks (tabs, sidebar, drag-resize) work out of
    /// the box, and so clicks/drags/scroll over a session's PTY content
    /// reach `claude`'s own mouse-tracking UI (see `focused_wants_mouse`) —
    /// its own copy-on-select keeps working regardless of this flag since
    /// `scan_osc52` forwards it straight from the raw PTY stream. Toggled via
    /// F9 for whoever wants the terminal emulator's native mouse handling
    /// instead. The main loop watches this flag and issues the actual
    /// Enable/DisableMouseCapture escape sequence when it changes.
    pub mouse_capture_enabled: bool,
    /// Set for the duration of a button press that started while the focused
    /// session had its own mouse tracking enabled (`vt100::MouseProtocolMode`
    /// != `None`, e.g. `claude`'s own "Jump to bottom" button) — every
    /// `Drag`/`Up` until release is forwarded to the child as a raw SGR mouse
    /// report, even if the drag strays outside the terminal content rect, so
    /// the child never sees a press with no matching release.
    forwarding_mouse: bool,
    /// Text waiting for the main loop to push to the system clipboard: a
    /// payload decoded out of a session's raw PTY byte stream when the child
    /// process (`claude`) emits its own OSC 52 "set clipboard" escape
    /// (`scan_osc52`) — `vt100::Parser` has no hook for OSC 52 and silently
    /// drops it, so argus has to notice and act on it itself. Goes out
    /// through both an OSC 52 write to the real terminal and, best-effort, a
    /// direct call to a local clipboard tool (`xsel` / `xclip` / `wl-copy`) —
    /// GNOME Terminal's VTE deliberately does not implement OSC 52
    /// clipboard-set, so the escape sequence alone doesn't get the text into
    /// the clipboard on that terminal.
    pub clipboard_copy_requested: Option<String>,
    /// Toast notification engine — see `notification::NotificationCenter`
    /// and the `notify_*` methods below for the service other parts of the
    /// app use to raise one.
    pub notifications: NotificationCenter,
    /// Id of whichever toast's close button the mouse is currently over —
    /// `ui::notification::draw` renders that one `X` in red. Updated on
    /// every `MouseEventKind::Moved` in `on_mouse`.
    pub hovered_notification: Option<u64>,
}

impl AppState {
    pub fn new(runtime: Runtime, terminal_size: (u16, u16)) -> Self {
        Self {
            runtime,
            workspaces: Vec::new(),
            workspace_entries: HashMap::new(),
            sessions: HashMap::new(),
            active_workspace: None,
            focus: Focus::Terminal,
            modal: None,
            status_line: String::new(),
            status_line_at: None,
            should_quit: false,
            stream_to_session: HashMap::new(),
            pending_output: HashMap::new(),
            pending_workspace_stream: None,
            terminal_size,
            sidebar_width: crate::ui::layout::DEFAULT_SIDEBAR_WIDTH,
            resizing_sidebar: false,
            mouse_capture_enabled: true,
            forwarding_mouse: false,
            clipboard_copy_requested: None,
            notifications: NotificationCenter::new(),
            hovered_notification: None,
        }
    }

    // ---- notifications --------------------------------------------------

    /// Raises an info toast (blue border) — the default kind for routine,
    /// non-actionable feedback.
    pub fn notify_info(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.notifications.info(title, message);
    }

    /// Raises a warning toast (orange border) — something worth the user's
    /// attention but not a failure.
    pub fn notify_warn(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.notifications.warn(title, message);
    }

    /// Raises an error toast (red border) — an operation failed.
    pub fn notify_error(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.notifications.error(title, message);
    }

    const STATUS_LINE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(4);

    /// Shows a message in the status bar for `STATUS_LINE_TIMEOUT` — call
    /// `tick` periodically (the main loop already redraws every 250ms) to
    /// actually clear it once expired.
    pub fn set_status(&mut self, message: String) {
        self.status_line = message;
        self.status_line_at = Some(std::time::Instant::now());
    }

    /// Clears an expired status message. No-op otherwise.
    pub fn tick(&mut self) {
        if let Some(at) = self.status_line_at {
            if at.elapsed() >= Self::STATUS_LINE_TIMEOUT {
                self.status_line.clear();
                self.status_line_at = None;
            }
        }
        self.notifications.tick();
    }

    pub fn spawn_initial_workspace(&mut self, directory: PathBuf) {
        let stream_id = self.runtime.spawn_workspace(directory);
        self.pending_workspace_stream = Some(stream_id);
        self.pending_output.insert(stream_id, Vec::new());
    }

    pub fn active_entry(&self) -> Option<&WorkspaceEntry> {
        self.active_workspace.and_then(|id| self.workspace_entries.get(&id))
    }

    pub fn active_entry_mut(&mut self) -> Option<&mut WorkspaceEntry> {
        self.active_workspace.and_then(|id| self.workspace_entries.get_mut(&id))
    }

    pub fn focused_session_id(&self) -> Option<SessionId> {
        self.active_entry().and_then(|w| w.focused_session)
    }

    /// Marks the currently displayed session as read. Called once per event
    /// loop iteration (see `main.rs`) so any path that changes which session
    /// is focused — direct selection, auto-focus on close, switching back to
    /// a workspace whose focused session didn't change — clears `unread`
    /// uniformly, without each call site needing to know about it.
    pub fn sync_focused_read(&mut self) {
        if let Some(id) = self.focused_session_id() {
            if let Some(entry) = self.sessions.get_mut(&id) {
                entry.unread = false;
            }
        }
    }

    // ---- backend event handling ----------------------------------------

    pub fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::PtyOutput(stream_id, data) => self.on_pty_output(stream_id, data),
            AppEvent::SessionSpawned { stream_id, workspace_id, result } => {
                self.on_session_spawned(stream_id, workspace_id, result)
            }
            AppEvent::SessionExited(session_id, _reason) => self.on_session_gone(session_id),
            AppEvent::WorkspaceSpawned { stream_id, result } => {
                self.on_workspace_spawned(stream_id, result)
            }
            AppEvent::FsChanged(workspace_id) => self.on_fs_changed(workspace_id),
            AppEvent::ClaudeSessionRenamed(session_id, name) => {
                self.runtime.rename_session(session_id, name.clone());
                if let Some(entry) = self.sessions.get_mut(&session_id) {
                    entry.session.name = name;
                }
            }
            AppEvent::HookStatus(session_id, kind) => {
                let is_focused = self.focused_session_id() == Some(session_id);
                if let Some(entry) = self.sessions.get_mut(&session_id) {
                    entry.status = Some(match kind {
                        HookEventKind::PromptSubmitted => RuntimeStatus::Thinking,
                        HookEventKind::Stopped => RuntimeStatus::Idle,
                        HookEventKind::Notification => RuntimeStatus::Waiting,
                    });
                    entry.unread = kind == HookEventKind::Stopped && !is_focused;
                }
            }
            AppEvent::DirLoaded(workspace_id, path, result) => match result {
                Ok(entries) => {
                    if let Some(entry) = self.workspace_entries.get_mut(&workspace_id) {
                        entry.explorer.dirs.insert(path, entries);
                    }
                }
                Err(e) => {
                    self.set_status(format!("erro ao listar diretório: {e}"));
                    self.notify_warn("Erro ao listar diretório", e.to_string());
                }
            },
            AppEvent::FsOpDone(workspace_id, parent, result) => {
                if let Err(e) = result {
                    self.set_status(format!("erro no arquivo: {e}"));
                    self.notify_warn("Erro no arquivo", e.to_string());
                }
                self.runtime.spawn_list_dir(workspace_id, parent);
            }
            AppEvent::GitAvailable(available) => {
                if let Some(id) = self.active_workspace {
                    if let Some(w) = self.workspace_entries.get_mut(&id) {
                        w.git.available = Some(available);
                    }
                }
            }
            AppEvent::GitReposLoaded(workspace_id, repos) => {
                if let Some(w) = self.workspace_entries.get_mut(&workspace_id) {
                    for repo in repos {
                        if !w.git.repos.iter().any(|r| r.repo.path == repo.path) {
                            w.git.repos.push(GitRepoState::new(repo));
                        }
                    }
                }
                let paths: Vec<PathBuf> = self
                    .workspace_entries
                    .get(&workspace_id)
                    .map(|w| w.git.repos.iter().map(|r| r.repo.path.clone()).collect())
                    .unwrap_or_default();
                for repo in paths {
                    self.runtime.spawn_git_refresh(workspace_id, repo);
                }
            }
            AppEvent::GitRefreshed { workspace_id, repo, status, branch, branches, sync } => {
                if let Some(repo_state) = self.repo_state_mut(workspace_id, &repo) {
                    if let Ok(status) = status {
                        repo_state.status = status;
                    }
                    if let Ok(branch) = branch {
                        repo_state.branch = branch;
                    }
                    if let Ok(branches) = branches {
                        repo_state.branches = branches;
                    }
                    if let Ok(sync) = sync {
                        repo_state.sync = Some(sync);
                    }
                }
            }
            AppEvent::GitLogLoaded { workspace_id, repo, skip, entries } => {
                if let Some(repo_state) = self.repo_state_mut(workspace_id, &repo) {
                    repo_state.log_loading = false;
                    match entries {
                        Ok(entries) => {
                            if entries.len() < 30 {
                                repo_state.log_complete = true;
                            }
                            if skip == 0 {
                                repo_state.log = entries;
                            } else {
                                repo_state.log.extend(entries);
                            }
                        }
                        Err(e) => self.set_status(format!("erro no log: {e}")),
                    }
                }
            }
            AppEvent::GitActionDone { workspace_id, repo, action, result } => {
                if let Err(e) = result {
                    self.set_status(format!("git {action} falhou: {e}"));
                    self.notify_error(format!("git {action} falhou"), e.to_string());
                } else if action == "commit" {
                    if let Some(w) = self.workspace_entries.get_mut(&workspace_id) {
                        w.git.commit_message.clear();
                    }
                    self.notify_info("Commit realizado", format!("{}", repo.display()));
                }
                self.runtime.spawn_git_refresh(workspace_id, repo);
            }
        }
    }

    fn repo_state_mut(&mut self, workspace_id: WorkspaceId, repo: &PathBuf) -> Option<&mut GitRepoState> {
        self.workspace_entries
            .get_mut(&workspace_id)?
            .git
            .repos
            .iter_mut()
            .find(|r| &r.repo.path == repo)
    }

    fn on_pty_output(&mut self, stream_id: Uuid, data: Vec<u8>) {
        if let Some(session_id) = self.stream_to_session.get(&stream_id).copied() {
            if let Some(entry) = self.sessions.get_mut(&session_id) {
                if let Some(text) = scan_osc52(&mut entry.osc52_partial, &data) {
                    self.clipboard_copy_requested = Some(text);
                }
                entry.parser.process(&data);
            }
        } else {
            self.pending_output.entry(stream_id).or_default().extend(data);
        }
    }

    fn new_parser_for(&mut self, stream_id: Uuid) -> vt100::Parser {
        let (cols, rows) = self.terminal_size;
        let mut parser = vt100::Parser::new(rows.max(1), cols.max(1), SCROLLBACK);
        if let Some(buffered) = self.pending_output.remove(&stream_id) {
            parser.process(&buffered);
        }
        parser
    }

    fn on_session_spawned(
        &mut self,
        stream_id: Uuid,
        workspace_id: WorkspaceId,
        result: Result<Session, argus_application::use_cases::CreateSessionError>,
    ) {
        match result {
            Ok(session) => {
                let session_id = session.id;
                self.stream_to_session.insert(stream_id, session_id);
                let parser = self.new_parser_for(stream_id);
                self.sessions.insert(session_id, SessionEntry { session, parser, status: None, unread: false, osc52_partial: Vec::new() });
                if let Some(w) = self.workspace_entries.get_mut(&workspace_id) {
                    w.sessions.push(session_id);
                    w.focused_session = Some(session_id);
                    w.agents_selected = w.sessions.len() - 1;
                }
                if self.active_workspace == Some(workspace_id) {
                    self.resize_focused_session();
                }
            }
            Err(e) => {
                self.pending_output.remove(&stream_id);
                self.set_status(format!("falha ao criar sessão: {e}"));
                self.notify_error("Falha ao criar sessão", e.to_string());
            }
        }
    }

    fn on_workspace_spawned(
        &mut self,
        stream_id: Uuid,
        result: Result<
            argus_application::use_cases::CreatedWorkspace,
            argus_application::use_cases::CreateWorkspaceError,
        >,
    ) {
        self.pending_workspace_stream = None;
        match result {
            Ok(created) => {
                let workspace = created.workspace;
                let session = created.first_session;
                let workspace_id = workspace.id;
                let session_id = session.id;

                self.stream_to_session.insert(stream_id, session_id);
                let parser = self.new_parser_for(stream_id);
                self.sessions.insert(session_id, SessionEntry { session, parser, status: None, unread: false, osc52_partial: Vec::new() });

                let mut entry = WorkspaceEntry::new(workspace.clone());
                entry.sessions.push(session_id);
                entry.focused_session = Some(session_id);
                self.workspace_entries.insert(workspace_id, entry);
                self.workspaces.push(workspace_id);
                self.active_workspace = Some(workspace_id);
                self.focus = Focus::Terminal;

                self.runtime.watch_workspace(workspace_id, workspace.directory.clone());
                self.runtime.spawn_list_dir(workspace_id, workspace.directory.clone());
                self.runtime.spawn_git_available();
                self.runtime.spawn_git_list_repositories(workspace_id, workspace.directory.clone());
                self.resize_focused_session();
            }
            Err(e) => {
                self.pending_output.remove(&stream_id);
                self.set_status(format!("falha ao criar workspace: {e}"));
                self.notify_error("Falha ao criar workspace", e.to_string());
                self.should_quit = self.workspaces.is_empty();
            }
        }
    }

    fn on_session_gone(&mut self, session_id: SessionId) {
        self.sessions.remove(&session_id);
        self.stream_to_session.retain(|_, id| *id != session_id);
        for entry in self.workspace_entries.values_mut() {
            entry.sessions.retain(|id| *id != session_id);
            if entry.focused_session == Some(session_id) {
                entry.focused_session = entry.sessions.first().copied();
            }
        }
        self.resize_focused_session();
    }

    fn on_fs_changed(&mut self, workspace_id: WorkspaceId) {
        let Some(entry) = self.workspace_entries.get(&workspace_id) else { return };
        let loaded_dirs: Vec<PathBuf> = entry.explorer.dirs.keys().cloned().collect();
        for dir in loaded_dirs {
            self.runtime.spawn_list_dir(workspace_id, dir);
        }
        let repo_paths: Vec<PathBuf> = entry.git.repos.iter().map(|r| r.repo.path.clone()).collect();
        for repo in repo_paths {
            self.runtime.spawn_git_refresh(workspace_id, repo);
        }
    }

    // ---- close flows (synchronous use cases, no AppEvent round-trip) ---

    pub fn request_close_session(&mut self, session_id: SessionId) {
        match self.runtime.request_close_session(session_id) {
            CloseDecision::RequiresConfirmation => {
                self.modal = Some(Modal::ConfirmCloseSession { session_id });
            }
            CloseDecision::AlreadyClosed => self.on_session_gone(session_id),
        }
    }

    pub fn confirm_close_session(&mut self, session_id: SessionId) {
        if let Err(e) = self.runtime.confirm_close_session(session_id) {
            self.set_status(format!("erro ao fechar sessão: {e}"));
        }
        self.on_session_gone(session_id);
    }

    pub fn request_close_workspace(&mut self, workspace_id: WorkspaceId) {
        match self.runtime.request_close_workspace(workspace_id) {
            CloseDecision::RequiresConfirmation => {
                self.modal = Some(Modal::ConfirmCloseWorkspace { workspace_id });
            }
            CloseDecision::AlreadyClosed => self.remove_workspace(workspace_id),
        }
    }

    pub fn confirm_close_workspace(&mut self, workspace_id: WorkspaceId) {
        if let Err(e) = self.runtime.confirm_close_workspace(workspace_id) {
            self.set_status(format!("erro ao fechar workspace: {e}"));
        }
        self.runtime.unwatch_workspace(workspace_id);
        self.remove_workspace(workspace_id);
    }

    fn remove_workspace(&mut self, workspace_id: WorkspaceId) {
        if let Some(entry) = self.workspace_entries.remove(&workspace_id) {
            for session_id in entry.sessions {
                self.sessions.remove(&session_id);
                self.stream_to_session.retain(|_, id| *id != session_id);
            }
        }
        self.workspaces.retain(|id| *id != workspace_id);
        if self.active_workspace == Some(workspace_id) {
            self.active_workspace = self.workspaces.first().copied();
            self.resize_focused_session();
        }
        if self.workspaces.is_empty() {
            self.should_quit = true;
        }
    }

    // ---- terminal sizing --------------------------------------------------

    pub fn set_terminal_size(&mut self, cols: u16, rows: u16) {
        self.terminal_size = (cols, rows);
        self.resize_focused_session();
    }

    pub fn resize_focused_session(&mut self) {
        let (cols, rows) = self.terminal_size;
        if let Some(session_id) = self.focused_session_id() {
            if let Some(entry) = self.sessions.get_mut(&session_id) {
                entry.parser.set_size(rows.max(1), cols.max(1));
            }
            self.runtime.resize_session(session_id, cols.max(1), rows.max(1));
        }
    }

    // ---- key handling ------------------------------------------------

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::F(9) {
            self.mouse_capture_enabled = !self.mouse_capture_enabled;
            self.set_status(if self.mouse_capture_enabled {
                "Mouse capture on — TUI clicks enabled".to_string()
            } else {
                "Mouse capture off — native terminal selection".to_string()
            });
            return;
        }

        if self.modal.is_some() {
            self.handle_modal_key(key);
            return;
        }

        match self.focus {
            Focus::Terminal => self.handle_terminal_key(key),
            Focus::Sidebar => self.handle_sidebar_key(key),
        }
    }

    // ---- mouse handling ------------------------------------------------

    /// Routes a mouse event either into the sidebar-resize drag or into
    /// click hit-testing against the regions `ui::draw` recorded while
    /// rendering the last frame. Ignored while a modal is open (the modal
    /// doesn't register any click targets of its own yet).
    /// Applies a coalesced mouse-wheel flick (see `scroll_coalesce`) as a
    /// single write of `ticks.abs()` repeated SGR wheel reports — same
    /// position/gating math as routing one `ScrollUp`/`ScrollDown` through
    /// `on_mouse`, just batched into one syscall and one redraw instead of
    /// one per notch, which is what keeps a fast flick from backing up
    /// `claude`'s stdin behind a pile of redraws and delaying real keystrokes
    /// typed right after.
    pub fn on_scroll_burst(&mut self, hitmap: &HitMap, mouse: MouseEvent, ticks: i32) {
        if self.modal.is_some() || ticks == 0 {
            return;
        }
        let content = Self::terminal_content_rect(hitmap);
        if !hitmap::hit(content, mouse.column, mouse.row) || !self.focused_wants_mouse() {
            return;
        }
        let Some(session_id) = self.focused_session_id() else { return };
        let button = if ticks > 0 { 64 } else { 65 };
        let col = mouse.column.saturating_sub(content.x).saturating_add(1).max(1);
        let row = mouse.row.saturating_sub(content.y).saturating_add(1).max(1);
        let mut bytes = Vec::new();
        for _ in 0..ticks.unsigned_abs() {
            bytes.extend_from_slice(format!("\x1b[<{button};{col};{row}M").as_bytes());
        }
        self.runtime.write_to_session(session_id, &bytes);
    }

    pub fn on_mouse(&mut self, mouse: MouseEvent, hitmap: &HitMap) {
        if self.modal.is_some() {
            return;
        }
        if mouse.kind == MouseEventKind::Moved {
            self.hovered_notification = hitmap
                .notification_close
                .iter()
                .find(|(r, _)| hitmap::hit(*r, mouse.column, mouse.row))
                .map(|(_, id)| *id);
            return;
        }
        let content = Self::terminal_content_rect(hitmap);

        // Once the focused session's own TUI has claimed the mouse — it
        // declared an xterm mouse-tracking mode, which is how `claude` makes
        // its own click targets like "Jump to bottom", and its own
        // copy-on-select (see `scan_osc52`), work — every event over the
        // content rect belongs to it, not to Argus. `forwarding_mouse` keeps
        // that true for the rest of a press even if a drag strays outside
        // `content`, so the child never sees a press with no matching
        // release.
        let over_content = hitmap::hit(content, mouse.column, mouse.row);
        if self.forwarding_mouse || (over_content && self.focused_wants_mouse()) {
            match mouse.kind {
                MouseEventKind::Down(_) => self.forwarding_mouse = true,
                MouseEventKind::Up(_) => self.forwarding_mouse = false,
                _ => {}
            }
            self.forward_mouse_report(content, mouse);
            return;
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some((_, id)) =
                    hitmap.notification_close.iter().find(|(r, _)| hitmap::hit(*r, mouse.column, mouse.row))
                {
                    self.notifications.dismiss(*id);
                } else if self.on_resize_handle(mouse.column, hitmap) {
                    self.resizing_sidebar = true;
                } else {
                    self.handle_click(mouse.column, mouse.row, hitmap);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) if self.resizing_sidebar => {
                self.set_sidebar_width(mouse.column, hitmap.full);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.resizing_sidebar = false;
            }
            _ => {}
        }
    }

    /// Whether the focused session's own TUI has requested xterm mouse
    /// tracking with SGR encoding (`\x1b[?1000h`/`1002`/`1003` plus `1006`) —
    /// `claude`'s interactive mouse-driven UI (e.g. its "Jump to bottom"
    /// button) only works once this is true, since only then does it parse
    /// mouse reports off its own stdin. Mouse events go nowhere while this is
    /// false (e.g. a plain shell session that never asked for mouse input) —
    /// Argus doesn't try to substitute its own click/selection handling.
    fn focused_wants_mouse(&self) -> bool {
        self.focused_session_id()
            .and_then(|id| self.sessions.get(&id))
            .is_some_and(|entry| {
                entry.parser.screen().mouse_protocol_mode() != vt100::MouseProtocolMode::None
                    && entry.parser.screen().mouse_protocol_encoding() == vt100::MouseProtocolEncoding::Sgr
            })
    }

    /// Forwards `mouse` to the focused session as a standard SGR mouse
    /// report (`ESC [ < Cb ; Cx ; Cy M`/`m`), the same encoding any real
    /// terminal emulator sends once an app has requested mouse tracking.
    /// Covers wheel scroll and clicks/drags alike — `vt100`'s own scrollback
    /// can't stand in for the wheel case, since `claude` draws through the
    /// alternate screen buffer (like any full-screen TUI), which never
    /// accumulates scrollback in any terminal, real or emulated, so scrolling
    /// the conversation has to be handled by `claude` itself, same as it
    /// already does for `PageUp`.
    fn forward_mouse_report(&mut self, content: ratatui::layout::Rect, mouse: MouseEvent) {
        let Some(session_id) = self.focused_session_id() else { return };
        let Some((button, release)) = Self::sgr_mouse_button(mouse.kind) else { return };
        let col = mouse.column.saturating_sub(content.x).saturating_add(1).max(1);
        let row = mouse.row.saturating_sub(content.y).saturating_add(1).max(1);
        let suffix = if release { 'm' } else { 'M' };
        let bytes = format!("\x1b[<{button};{col};{row}{suffix}").into_bytes();
        self.runtime.write_to_session(session_id, &bytes);
    }

    /// Maps a crossterm `MouseEventKind` to its SGR `(button code, is
    /// release)` pair — `Drag` adds the `32` motion offset xterm uses to
    /// distinguish a move-while-held from a fresh press, and wheel ticks are
    /// their own pseudo-buttons (`64`/`65`/`66`/`67`) that are always
    /// "presses", never released.
    fn sgr_mouse_button(kind: MouseEventKind) -> Option<(u8, bool)> {
        let base = |b: MouseButton| match b {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
        };
        match kind {
            MouseEventKind::Down(b) => Some((base(b), false)),
            MouseEventKind::Drag(b) => Some((base(b) + 32, false)),
            MouseEventKind::Up(b) => Some((base(b), true)),
            MouseEventKind::ScrollUp => Some((64, false)),
            MouseEventKind::ScrollDown => Some((65, false)),
            MouseEventKind::ScrollLeft => Some((66, false)),
            MouseEventKind::ScrollRight => Some((67, false)),
            MouseEventKind::Moved => None,
        }
    }

    /// The PTY content rect inside the terminal pane's border (see
    /// `ui::terminal`'s `Block::bordered()` and `layout::pty_content_size`).
    fn terminal_content_rect(hitmap: &HitMap) -> ratatui::layout::Rect {
        let area = hitmap.terminal_area;
        ratatui::layout::Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        }
    }

    /// The sidebar/terminal divider is a two-column grab target: the
    /// sidebar's own right border plus the terminal pane's left border
    /// drawn right next to it — same forgiving width most terminal
    /// multiplexers give a drag handle.
    fn on_resize_handle(&self, column: u16, hitmap: &HitMap) -> bool {
        let border_x = hitmap.full.x + self.sidebar_width.saturating_sub(1);
        column == border_x || column == border_x.saturating_add(1)
    }

    fn set_sidebar_width(&mut self, column: u16, full: ratatui::layout::Rect) {
        let width = column.saturating_sub(full.x).saturating_add(1);
        self.sidebar_width = crate::ui::layout::clamp_sidebar_width(width, full.width);
        let regions = crate::ui::layout::compute(full, self.sidebar_width);
        let (cols, rows) = crate::ui::layout::pty_content_size(regions.terminal);
        self.set_terminal_size(cols, rows);
    }

    fn handle_click(&mut self, x: u16, y: u16, hitmap: &HitMap) {
        if let Some((_, workspace_id)) = hitmap.topbar_tabs.iter().find(|(r, _)| hitmap::hit(*r, x, y)) {
            self.active_workspace = Some(*workspace_id);
            self.resize_focused_session();
            return;
        }

        if let Some((_, tab)) = hitmap.sidebar_tabs.iter().find(|(r, _)| hitmap::hit(*r, x, y)) {
            self.focus = Focus::Sidebar;
            self.set_sidebar_tab(*tab);
            return;
        }

        if let Some(&(_, index, session_id)) = hitmap.agents_rows.iter().find(|(r, ..)| hitmap::hit(*r, x, y)) {
            if let Some(entry) = self.active_entry_mut() {
                entry.agents_selected = index;
                entry.focused_session = Some(session_id);
            }
            self.focus = Focus::Terminal;
            self.resize_focused_session();
            return;
        }

        if let Some((_, index, path, is_dir)) =
            hitmap.explorer_rows.iter().find(|(r, ..)| hitmap::hit(*r, x, y)).cloned()
        {
            self.focus = Focus::Sidebar;
            if let Some(workspace_id) = self.active_workspace {
                if let Some(entry) = self.workspace_entries.get_mut(&workspace_id) {
                    entry.explorer.selected = index;
                    if is_dir {
                        if entry.explorer.expanded.contains(&path) {
                            entry.explorer.expanded.remove(&path);
                        } else {
                            entry.explorer.expanded.insert(path.clone());
                            if !entry.explorer.dirs.contains_key(&path) {
                                self.runtime.spawn_list_dir(workspace_id, path);
                            }
                        }
                    }
                }
            }
            return;
        }

        if let Some((_, index, path, staged)) =
            hitmap.git_rows.iter().find(|(r, ..)| hitmap::hit(*r, x, y)).cloned()
        {
            self.focus = Focus::Sidebar;
            if let Some(workspace_id) = self.active_workspace {
                if let Some(entry) = self.workspace_entries.get_mut(&workspace_id) {
                    entry.git.selected_file = index;
                }
                let repo_path = self
                    .workspace_entries
                    .get(&workspace_id)
                    .and_then(|w| w.git.active_repo())
                    .map(|r| r.repo.path.clone());
                if let Some(repo_path) = repo_path {
                    if staged {
                        self.runtime.spawn_git_unstage(workspace_id, repo_path, vec![path]);
                    } else {
                        self.runtime.spawn_git_stage(workspace_id, repo_path, vec![path]);
                    }
                }
            }
            return;
        }

        if hitmap::hit(hitmap.terminal_area, x, y) && self.focused_session_id().is_some() {
            self.focus = Focus::Terminal;
        }
    }

    /// Moves the Agents list highlight onto whichever session is currently
    /// focused, so leaving the terminal for the sidebar always shows the
    /// highlight on the session you were just looking at instead of wherever
    /// it was last left by arrow-key navigation.
    fn sync_agents_selection_to_focused(&mut self) {
        if let Some(entry) = self.active_entry_mut() {
            if let Some(focused) = entry.focused_session {
                if let Some(index) = entry.sessions.iter().position(|id| *id == focused) {
                    entry.agents_selected = index;
                }
            }
        }
    }

    /// Handles a `crossterm::event::Event::Paste` (bracketed paste, enabled
    /// in `main.rs`): the whole clipboard contents arrive as one string
    /// instead of a `KeyEvent` per character. Without this, paste falls back
    /// to individual `Char`/`Enter` key events — visibly trickling in one
    /// letter at a time, and (worse) every embedded newline hits
    /// `handle_terminal_key`'s `\r` and gets read by the session as "submit
    /// now", firing off a partial message mid-paste.
    ///
    /// Forwarded to the focused session wrapped in the same
    /// `ESC[200~...ESC[201~` markers a real terminal would use, so a
    /// bracketed-paste-aware program inside (a shell, `claude` itself) treats
    /// it as one paste rather than as typed submits.
    pub fn on_paste(&mut self, text: String) {
        if let Some(modal) = self.modal.as_mut() {
            modal.paste(&text);
            return;
        }
        if self.focus == Focus::Terminal {
            if let Some(session_id) = self.focused_session_id() {
                let mut bytes = Vec::with_capacity(text.len() + 12);
                bytes.extend_from_slice(b"\x1b[200~");
                bytes.extend_from_slice(text.as_bytes());
                bytes.extend_from_slice(b"\x1b[201~");
                self.runtime.write_to_session(session_id, &bytes);
            }
        }
    }

    fn handle_terminal_key(&mut self, key: KeyEvent) {
        // Ctrl+B is the escape hatch back to sidebar navigation, same
        // "leader key" idea as tmux — chosen because a `claude`/shell session
        // running inside essentially never needs literal Ctrl+B itself.
        if key.code == KeyCode::Char('b') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.focus = Focus::Sidebar;
            self.sync_agents_selection_to_focused();
            return;
        }
        if let Some(session_id) = self.focused_session_id() {
            if let Some(bytes) = key_to_bytes(&key) {
                self.runtime.write_to_session(session_id, &bytes);
                // While `Waiting` (blocked on a prompt or permission
                // picker), only a key that actually resolves the picker
                // should move the status off `Waiting` — Claude Code won't
                // necessarily fire a `Notification`/`Stop` hook for that
                // (e.g. Esc), so the status would otherwise stay purple
                // forever. Esc or Ctrl+C cancels the prompt outright, so
                // those go straight to `Idle`; Enter answers it, so that
                // optimistically resumes `Thinking`. Anything else — arrow
                // keys, digits, Tab — is just navigating within the picker
                // and hasn't confirmed anything yet, so it leaves the
                // status at `Waiting`. Either way, the next real hook event
                // corrects it.
                let is_cancel = key.code == KeyCode::Esc
                    || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL));
                if let Some(entry) = self.sessions.get_mut(&session_id) {
                    if entry.status == Some(RuntimeStatus::Waiting) {
                        if is_cancel {
                            entry.status = Some(RuntimeStatus::Idle);
                        } else if key.code == KeyCode::Enter {
                            entry.status = Some(RuntimeStatus::Thinking);
                        }
                    }
                }
            }
        }
    }

    fn handle_sidebar_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('1') => self.set_sidebar_tab(SidebarTab::Agents),
            KeyCode::Char('2') => self.set_sidebar_tab(SidebarTab::Explorer),
            KeyCode::Char('3') => self.set_sidebar_tab(SidebarTab::Git),
            KeyCode::Tab => self.cycle_sidebar_tab(),
            KeyCode::Char('[') => self.cycle_workspace(-1),
            KeyCode::Char(']') => self.cycle_workspace(1),
            KeyCode::Char('w') => {
                self.modal = Some(Modal::NewWorkspacePath { input: String::new() })
            }
            KeyCode::Char('W') => {
                if let Some(id) = self.active_workspace {
                    self.request_close_workspace(id);
                }
            }
            // Returns to whichever session was already focused, without
            // touching the Agents list highlight — unlike Enter, which both
            // selects and focuses.
            KeyCode::Esc if self.focused_session_id().is_some() => {
                self.focus = Focus::Terminal;
            }
            _ => {
                let tab = self.active_entry().map(|w| w.sidebar_tab);
                match tab {
                    Some(SidebarTab::Agents) => self.handle_agents_key(key),
                    Some(SidebarTab::Explorer) => self.handle_explorer_key(key),
                    Some(SidebarTab::Git) => self.handle_git_key(key),
                    None => {}
                }
            }
        }
    }

    fn set_sidebar_tab(&mut self, tab: SidebarTab) {
        if let Some(w) = self.active_entry_mut() {
            w.sidebar_tab = tab;
        }
    }

    fn cycle_sidebar_tab(&mut self) {
        if let Some(w) = self.active_entry_mut() {
            w.sidebar_tab = match w.sidebar_tab {
                SidebarTab::Agents => SidebarTab::Explorer,
                SidebarTab::Explorer => SidebarTab::Git,
                SidebarTab::Git => SidebarTab::Agents,
            };
        }
    }

    fn cycle_workspace(&mut self, dir: i32) {
        if self.workspaces.is_empty() {
            return;
        }
        let current = self
            .active_workspace
            .and_then(|id| self.workspaces.iter().position(|w| *w == id))
            .unwrap_or(0) as i32;
        let len = self.workspaces.len() as i32;
        let next = (current + dir).rem_euclid(len);
        self.active_workspace = Some(self.workspaces[next as usize]);
        self.resize_focused_session();
    }

    fn handle_agents_key(&mut self, key: KeyEvent) {
        let Some(workspace_id) = self.active_workspace else { return };
        let Some(entry) = self.workspace_entries.get_mut(&workspace_id) else { return };
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                entry.agents_selected = entry.agents_selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if entry.agents_selected + 1 < entry.sessions.len() {
                    entry.agents_selected += 1;
                }
            }
            KeyCode::Char('n') => {
                self.runtime.spawn_session(workspace_id, None);
            }
            KeyCode::Char('x') => {
                if let Some(id) = entry.sessions.get(entry.agents_selected).copied() {
                    self.request_close_session(id);
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if let Some(id) = entry.sessions.get(entry.agents_selected).copied() {
                    self.modal = Some(Modal::RenameSession { session_id: id, input: String::new() });
                }
            }
            KeyCode::Enter => {
                if let Some(id) = entry.sessions.get(entry.agents_selected).copied() {
                    entry.focused_session = Some(id);
                    self.focus = Focus::Terminal;
                    self.resize_focused_session();
                }
            }
            _ => {}
        }
    }

    fn handle_explorer_key(&mut self, key: KeyEvent) {
        let Some(workspace_id) = self.active_workspace else { return };
        let Some(entry) = self.workspace_entries.get_mut(&workspace_id) else { return };
        let root = entry.workspace.directory.clone();
        let rows = entry.explorer.flatten(&root);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                entry.explorer.selected = entry.explorer.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if entry.explorer.selected + 1 < rows.len() {
                    entry.explorer.selected += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some((path, _, is_dir)) = rows.get(entry.explorer.selected).cloned() {
                    if is_dir {
                        if entry.explorer.expanded.contains(&path) {
                            entry.explorer.expanded.remove(&path);
                        } else {
                            entry.explorer.expanded.insert(path.clone());
                            if !entry.explorer.dirs.contains_key(&path) {
                                self.runtime.spawn_list_dir(workspace_id, path);
                            }
                        }
                    }
                }
            }
            KeyCode::Char('a') => {
                self.modal = Some(Modal::NewFile { workspace_id, dir: dir_for_selection(&rows, entry.explorer.selected, &root), input: String::new() });
            }
            KeyCode::Char('A') => {
                self.modal = Some(Modal::NewDir { workspace_id, dir: dir_for_selection(&rows, entry.explorer.selected, &root), input: String::new() });
            }
            KeyCode::Char('r') => {
                if let Some((path, ..)) = rows.get(entry.explorer.selected).cloned() {
                    self.modal = Some(Modal::RenamePath { workspace_id, from: path, input: String::new() });
                }
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                if let Some((path, ..)) = rows.get(entry.explorer.selected).cloned() {
                    let parent = path.parent().map(PathBuf::from).unwrap_or_else(|| root.clone());
                    self.modal = Some(Modal::ConfirmDeletePath { workspace_id, path, parent });
                }
            }
            _ => {}
        }
    }

    fn handle_git_key(&mut self, key: KeyEvent) {
        let Some(workspace_id) = self.active_workspace else { return };
        let Some(entry) = self.workspace_entries.get_mut(&workspace_id) else { return };
        let repo_path = entry.git.active_repo().map(|r| r.repo.path.clone());
        match key.code {
            KeyCode::Left => {
                entry.git.selected_repo = entry.git.selected_repo.saturating_sub(1);
                entry.git.selected_file = 0;
            }
            KeyCode::Right => {
                if entry.git.selected_repo + 1 < entry.git.repos.len() {
                    entry.git.selected_repo += 1;
                    entry.git.selected_file = 0;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                entry.git.selected_file = entry.git.selected_file.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(repo) = entry.git.active_repo() {
                    if entry.git.selected_file + 1 < repo.status.len() {
                        entry.git.selected_file += 1;
                    }
                }
            }
            KeyCode::Char(' ') => {
                if let (Some(repo_path), Some(repo)) = (repo_path.clone(), entry.git.active_repo()) {
                    if let Some(file) = repo.status.get(entry.git.selected_file) {
                        if file.staged {
                            self.runtime.spawn_git_unstage(workspace_id, repo_path, vec![file.path.clone()]);
                        } else {
                            self.runtime.spawn_git_stage(workspace_id, repo_path, vec![file.path.clone()]);
                        }
                    }
                }
            }
            KeyCode::Char('c') => {
                if let Some(repo_path) = repo_path {
                    self.modal = Some(Modal::CommitMessage { workspace_id, repo: repo_path, input: entry.git.commit_message.clone() });
                }
            }
            KeyCode::Char('f') => {
                if let Some(repo_path) = repo_path {
                    self.runtime.spawn_git_fetch(workspace_id, repo_path);
                }
            }
            KeyCode::Char('p') => {
                if let Some(repo_path) = repo_path {
                    self.runtime.spawn_git_pull(workspace_id, repo_path);
                }
            }
            KeyCode::Char('P') => {
                if let Some(repo_path) = repo_path {
                    self.runtime.spawn_git_push(workspace_id, repo_path);
                }
            }
            KeyCode::Char('b') => {
                if let (Some(repo_path), Some(repo)) = (repo_path, entry.git.active_repo()) {
                    if repo.branches.len() > 1 {
                        let current = repo.branches.iter().position(|b| b.is_current).unwrap_or(0);
                        let next = (current + 1) % repo.branches.len();
                        let name = repo.branches[next].name.clone();
                        self.runtime.spawn_git_switch_branch(workspace_id, repo_path, name);
                    }
                }
            }
            KeyCode::Char('l') => {
                if let Some(repo_path) = repo_path {
                    let log_empty = entry.git.active_repo().map(|r| r.log.is_empty()).unwrap_or(true);
                    entry.git.show_log = !entry.git.show_log;
                    if entry.git.show_log && log_empty {
                        self.runtime.spawn_git_log(workspace_id, repo_path, 0, 30);
                    }
                }
            }
            KeyCode::Char('m') => {
                if entry.git.show_log {
                    if let (Some(repo_path), Some(repo)) = (repo_path, entry.git.active_repo()) {
                        if !repo.log_complete && !repo.log_loading {
                            let skip = repo.log.len() as u32;
                            self.runtime.spawn_git_log(workspace_id, repo_path, skip, 30);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_modal_key(&mut self, key: KeyEvent) {
        use crate::text_input::{apply as apply_text_input, TextInputAction};

        let Some(modal) = self.modal.take() else { return };
        match modal {
            Modal::NewWorkspacePath { mut input } => match apply_text_input(&mut input, key) {
                TextInputAction::Submit => {
                    if !input.trim().is_empty() {
                        self.spawn_initial_workspace(PathBuf::from(input.trim()));
                    }
                }
                TextInputAction::Cancel => {}
                TextInputAction::Continue => self.modal = Some(Modal::NewWorkspacePath { input }),
            },
            Modal::RenameSession { session_id, mut input } => match apply_text_input(&mut input, key) {
                TextInputAction::Submit => {
                    if !input.trim().is_empty() {
                        self.runtime.rename_session(session_id, input.trim().to_string());
                        if let Some(entry) = self.sessions.get_mut(&session_id) {
                            entry.session.name = input.trim().to_string();
                        }
                    }
                }
                TextInputAction::Cancel => {}
                TextInputAction::Continue => {
                    self.modal = Some(Modal::RenameSession { session_id, input });
                }
            },
            Modal::NewFile { workspace_id, dir, mut input } => match apply_text_input(&mut input, key) {
                TextInputAction::Submit => {
                    if !input.trim().is_empty() {
                        let path = dir.join(input.trim());
                        self.runtime.spawn_create_file(workspace_id, path, dir);
                    }
                }
                TextInputAction::Cancel => {}
                TextInputAction::Continue => {
                    self.modal = Some(Modal::NewFile { workspace_id, dir, input });
                }
            },
            Modal::NewDir { workspace_id, dir, mut input } => match apply_text_input(&mut input, key) {
                TextInputAction::Submit => {
                    if !input.trim().is_empty() {
                        let path = dir.join(input.trim());
                        self.runtime.spawn_create_dir(workspace_id, path, dir);
                    }
                }
                TextInputAction::Cancel => {}
                TextInputAction::Continue => {
                    self.modal = Some(Modal::NewDir { workspace_id, dir, input });
                }
            },
            Modal::RenamePath { workspace_id, from, mut input } => match apply_text_input(&mut input, key) {
                TextInputAction::Submit => {
                    if !input.trim().is_empty() {
                        let parent = from.parent().map(PathBuf::from).unwrap_or_else(|| from.clone());
                        let to = parent.join(input.trim());
                        self.runtime.spawn_rename_path(workspace_id, from, to, parent);
                    }
                }
                TextInputAction::Cancel => {}
                TextInputAction::Continue => {
                    self.modal = Some(Modal::RenamePath { workspace_id, from, input });
                }
            },
            Modal::CommitMessage { workspace_id, repo, mut input } => match apply_text_input(&mut input, key) {
                TextInputAction::Submit => {
                    if !input.trim().is_empty() {
                        self.runtime.spawn_git_commit(workspace_id, repo, input.trim().to_string());
                    }
                }
                TextInputAction::Cancel => {}
                TextInputAction::Continue => {
                    self.modal = Some(Modal::CommitMessage { workspace_id, repo, input });
                }
            },
            Modal::ConfirmCloseSession { session_id } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.confirm_close_session(session_id),
                _ => {}
            },
            Modal::ConfirmCloseWorkspace { workspace_id } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.confirm_close_workspace(workspace_id),
                _ => {}
            },
            Modal::ConfirmDeletePath { workspace_id, path, parent } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.runtime.spawn_delete_path(workspace_id, path, parent);
                }
                _ => {}
            },
        }
    }
}

fn dir_for_selection(rows: &[(PathBuf, usize, bool)], selected: usize, root: &PathBuf) -> PathBuf {
    match rows.get(selected) {
        Some((path, _, true)) => path.clone(),
        Some((path, _, false)) => path.parent().map(PathBuf::from).unwrap_or_else(|| root.clone()),
        None => root.clone(),
    }
}

/// Longest possible prefix of an OSC 52 introducer (`ESC ] 5 2 ;`) — used to
/// decide how much of a trailing, not-yet-matched tail is worth keeping
/// across chunk boundaries.
const OSC52_PREFIX: &[u8] = b"\x1b]52;";

/// Hard cap on how many bytes `scan_osc52` will carry over while waiting for
/// a sequence to terminate, so a malformed or adversarial stream that opens
/// `ESC ] 52 ;` and never closes it can't pin unbounded memory.
const OSC52_MAX_PENDING: usize = 1 << 16;

/// Scans raw PTY bytes for a complete OSC 52 "set clipboard" escape sequence
/// and, if one is found, base64-decodes its payload and returns the
/// resulting text, ready to feed into the same clipboard pipeline as a
/// manual selection (`AppState::clipboard_copy_requested`).
///
/// This exists because `vt100::Parser` has no hook for OSC 52 — it treats
/// selection-copy escapes as an "unhandled osc sequence" and silently drops
/// them — so without this scan, a `claude` session running inside argus's
/// embedded PTY loses the copy-on-select behavior it has when run directly
/// in a real terminal.
///
/// `partial` carries bytes across calls: a session's byte stream can split
/// a single escape sequence across two `on_pty_output` events (e.g. a large
/// selection's base64 payload landing on a PTY read boundary), so an
/// in-progress, not-yet-terminated sequence is buffered here until either it
/// completes or the pending-byte cap is hit.
fn scan_osc52(partial: &mut Vec<u8>, data: &[u8]) -> Option<String> {
    partial.extend_from_slice(data);

    let mut found = None;
    loop {
        let Some(start) = find_subslice(partial, OSC52_PREFIX) else {
            // No introducer anywhere in the buffer — keep only a tail that
            // could still grow into one on the next chunk.
            let keep = partial.len().min(OSC52_PREFIX.len() - 1);
            let from = partial.len() - keep;
            if keep > 0 && OSC52_PREFIX.starts_with(&partial[from..]) {
                partial.drain(..from);
            } else {
                partial.clear();
            }
            break;
        };
        partial.drain(..start);

        let body = &partial[OSC52_PREFIX.len()..];
        let bel = body.iter().position(|&b| b == 0x07).map(|i| (i, i + 1));
        let st = find_subslice(body, b"\x1b\\").map(|i| (i, i + 2));
        let terminator = match (bel, st) {
            (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        let Some((payload_end, seq_end)) = terminator else {
            // Sequence not finished yet — wait for more data, unless it's
            // grown unreasonably large (malformed stream).
            if partial.len() > OSC52_MAX_PENDING {
                partial.clear();
            }
            break;
        };

        let seq_len = OSC52_PREFIX.len() + seq_end;
        let payload = partial[OSC52_PREFIX.len()..OSC52_PREFIX.len() + payload_end].to_vec();
        partial.drain(..seq_len);

        // Skip clipboard *queries* (payload `?`) — argus can't answer them,
        // and a real copy-on-select never sends this form.
        if payload == b"?" || payload.ends_with(b";?") {
            continue;
        }
        // Payload is `Pc;Pd` (or bare `Pd`) — Pd is what we want, base64-encoded.
        let b64 = match payload.iter().position(|&b| b == b';') {
            Some(i) => &payload[i + 1..],
            None => &payload[..],
        };
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(b64) {
            found = Some(String::from_utf8_lossy(&decoded).into_owned());
        }
    }
    found
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Translates a key press into the raw bytes a real terminal would send down
/// the wire — there is no browser terminal emulator underneath a TUI to do
/// this for us, so it's re-derived here.
fn key_to_bytes(key: &KeyEvent) -> Option<Vec<u8>> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char(c) = key.code {
            let c = c.to_ascii_lowercase();
            if c.is_ascii_alphabetic() {
                let byte = (c as u8) - b'a' + 1;
                return Some(vec![byte]);
            }
        }
    }
    match key.code {
        KeyCode::Char(c) => {
            let mut buf = [0u8; 4];
            Some(c.encode_utf8(&mut buf).as_bytes().to_vec())
        }
        KeyCode::Enter => Some(vec![b'\r']),
        KeyCode::Backspace => Some(vec![0x7f]),
        KeyCode::Tab => Some(vec![b'\t']),
        KeyCode::BackTab => Some(b"\x1b[Z".to_vec()),
        KeyCode::Esc => Some(vec![0x1b]),
        KeyCode::Up => Some(b"\x1b[A".to_vec()),
        KeyCode::Down => Some(b"\x1b[B".to_vec()),
        KeyCode::Right => Some(b"\x1b[C".to_vec()),
        KeyCode::Left => Some(b"\x1b[D".to_vec()),
        KeyCode::Home => Some(b"\x1b[H".to_vec()),
        KeyCode::End => Some(b"\x1b[F".to_vec()),
        KeyCode::PageUp => Some(b"\x1b[5~".to_vec()),
        KeyCode::PageDown => Some(b"\x1b[6~".to_vec()),
        KeyCode::Delete => Some(b"\x1b[3~".to_vec()),
        KeyCode::Insert => Some(b"\x1b[2~".to_vec()),
        KeyCode::F(n) => function_key_bytes(n),
        _ => None,
    }
}

fn function_key_bytes(n: u8) -> Option<Vec<u8>> {
    let code = match n {
        1 => "OP",
        2 => "OQ",
        3 => "OR",
        4 => "OS",
        5 => "[15~",
        6 => "[17~",
        7 => "[18~",
        8 => "[19~",
        9 => "[20~",
        10 => "[21~",
        11 => "[23~",
        12 => "[24~",
        _ => return None,
    };
    Some(format!("\x1b{code}").into_bytes())
}

#[cfg(test)]
mod osc52_tests {
    use super::scan_osc52;

    #[test]
    fn finds_a_bel_terminated_sequence() {
        let mut partial = Vec::new();
        let found = scan_osc52(&mut partial, b"before\x1b]52;c;aGVsbG8=\x07after");
        assert_eq!(found.as_deref(), Some("hello"));
        assert!(partial.is_empty());
    }

    #[test]
    fn finds_a_string_terminator_sequence() {
        let mut partial = Vec::new();
        let found = scan_osc52(&mut partial, b"\x1b]52;c;aGVsbG8=\x1b\\rest");
        assert_eq!(found.as_deref(), Some("hello"));
    }

    #[test]
    fn reassembles_a_sequence_split_across_chunks() {
        let mut partial = Vec::new();
        assert_eq!(scan_osc52(&mut partial, b"noise \x1b]52;c;aGVs"), None);
        assert_eq!(scan_osc52(&mut partial, b"bG8=\x07 more noise").as_deref(), Some("hello"));
    }

    #[test]
    fn ignores_a_clipboard_query() {
        let mut partial = Vec::new();
        assert_eq!(scan_osc52(&mut partial, b"\x1b]52;c;?\x07"), None);
        assert!(partial.is_empty());
    }

    #[test]
    fn returns_the_last_of_multiple_sequences_in_one_chunk() {
        let mut partial = Vec::new();
        let found = scan_osc52(&mut partial, b"\x1b]52;c;Zmlyc3Q=\x07\x1b]52;c;c2Vjb25k\x07");
        assert_eq!(found.as_deref(), Some("second"));
    }

    #[test]
    fn caps_pending_bytes_on_an_unterminated_sequence() {
        let mut partial = Vec::new();
        let huge = vec![b'a'; super::OSC52_MAX_PENDING + 1];
        let mut chunk = b"\x1b]52;c;".to_vec();
        chunk.extend_from_slice(&huge);
        assert_eq!(scan_osc52(&mut partial, &chunk), None);
        assert!(partial.is_empty(), "pending buffer should be dropped once it exceeds the cap");
    }

    #[test]
    fn plain_output_leaves_no_residue() {
        let mut partial = Vec::new();
        assert_eq!(scan_osc52(&mut partial, b"just some regular claude output\n"), None);
        assert!(partial.is_empty());
    }
}
