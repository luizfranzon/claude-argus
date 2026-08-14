use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use argus_application::ports::{
    BranchInfo, CommitEntry, FileEntry, FileStatusEntry, GitRepository, SyncStatus,
};
use argus_application::use_cases::CloseDecision;
use argus_domain::{Session, SessionId, Workspace, WorkspaceId};
use argus_infrastructure::HookEventKind;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use uuid::Uuid;

use crate::event::AppEvent;
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
}

pub struct SessionEntry {
    pub session: Session,
    pub parser: vt100::Parser,
    pub status: Option<RuntimeStatus>,
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
        }
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
            AppEvent::HookStatus(session_id, kind) => {
                if let Some(entry) = self.sessions.get_mut(&session_id) {
                    entry.status = Some(match kind {
                        HookEventKind::PromptSubmitted => RuntimeStatus::Thinking,
                        HookEventKind::Stopped => RuntimeStatus::Idle,
                    });
                }
            }
            AppEvent::DirLoaded(workspace_id, path, result) => match result {
                Ok(entries) => {
                    if let Some(entry) = self.workspace_entries.get_mut(&workspace_id) {
                        entry.explorer.dirs.insert(path, entries);
                    }
                }
                Err(e) => self.set_status(format!("erro ao listar diretório: {e}")),
            },
            AppEvent::FsOpDone(workspace_id, parent, result) => {
                if let Err(e) = result {
                    self.set_status(format!("erro no arquivo: {e}"));
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
                } else if action == "commit" {
                    if let Some(w) = self.workspace_entries.get_mut(&workspace_id) {
                        w.git.commit_message.clear();
                    }
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
                self.sessions.insert(session_id, SessionEntry { session, parser, status: None });
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
                self.sessions.insert(session_id, SessionEntry { session, parser, status: None });

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
    pub fn on_mouse(&mut self, mouse: MouseEvent, hitmap: &HitMap) {
        if self.modal.is_some() {
            return;
        }
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if self.on_resize_handle(mouse.column, hitmap) {
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
