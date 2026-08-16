mod close_confirmation;
mod keys;
mod mouse;
mod pty_output;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use argus_application::ports::{
    BranchInfo, CommitEntry, FileEntry, FileStatusEntry, GitRepository, SyncStatus,
};
use argus_domain::{Session, SessionId, Workspace, WorkspaceId};
use argus_infrastructure::HookEventKind;
use crossterm::event::{KeyCode, KeyEvent};
use uuid::Uuid;

use crate::event::AppEvent;
use crate::fuzzy_finder::{FinderMode, FuzzyFinderState};
use crate::i18n::t;
use crate::notification::NotificationCenter;
use crate::runtime::Runtime;
use crate::ui::hitmap::HitMap;

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
    /// `app::pty_output::scan_osc52`.
    osc52_partial: Vec<u8>,
}

pub struct ExplorerState {
    pub dirs: HashMap<PathBuf, Vec<FileEntry>>,
    pub expanded: HashSet<PathBuf>,
    pub selected: usize,
    /// Cache for `flatten`'s result. Rebuilding it walks every expanded
    /// directory and clones a `PathBuf` per row — cheap once, but `flatten`
    /// used to redo that unconditionally on every render (up to 60/sec via
    /// the redraw tick) even though the tree it describes only actually
    /// changes on expand/collapse or a directory listing arriving. Those are
    /// exactly the two points `invalidate_flatten` is called from; every
    /// other frame just reuses this.
    flat_cache: std::cell::RefCell<Option<Vec<(PathBuf, usize, bool)>>>,
}

impl ExplorerState {
    fn new() -> Self {
        Self {
            dirs: HashMap::new(),
            expanded: HashSet::new(),
            selected: 0,
            flat_cache: std::cell::RefCell::new(None),
        }
    }

    /// Flattens the currently-expanded tree into the rows the Explorer view
    /// draws top to bottom: `(path, depth, is_dir)`. Root's own entries start
    /// at depth 0; lazy — a directory only contributes rows once it has been
    /// expanded and its listing has arrived.
    ///
    /// Cached (see `flat_cache`) — `&self`, not `&mut self`, so the render
    /// path can call it without a mutable borrow; interior mutability is
    /// what lets a `&self` method still memoize.
    pub fn flatten(&self, root: &PathBuf) -> std::cell::Ref<'_, Vec<(PathBuf, usize, bool)>> {
        if self.flat_cache.borrow().is_none() {
            let mut rows = Vec::new();
            if let Some(entries) = self.dirs.get(root) {
                self.push_entries(entries, 0, &mut rows);
            }
            *self.flat_cache.borrow_mut() = Some(rows);
        }
        std::cell::Ref::map(self.flat_cache.borrow(), |cached| cached.as_ref().unwrap())
    }

    /// Drops the cached `flatten` result. Call after anything that changes
    /// which rows it would produce: a directory expanding/collapsing, or a
    /// directory listing arriving.
    pub fn invalidate_flatten(&mut self) {
        *self.flat_cache.get_mut() = None;
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
    /// Fuzzy finder's Files-mode index, workspace-relative paths,
    /// respecting `.gitignore` — `None` until the first walk completes.
    /// Kept fresh by re-walking on every `AppEvent::FsChanged`.
    pub file_index: Option<Vec<PathBuf>>,
    /// The "show everything" counterpart to `file_index`, built lazily the
    /// first time Ctrl+G is pressed inside the finder and refreshed
    /// alongside it from then on.
    pub file_index_all: Option<Vec<PathBuf>>,
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
            file_index: None,
            file_index_all: None,
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
    /// Focus Mode: hides the topbar and sidebar, giving the currently
    /// focused session's pane the whole frame above the status line. Toggled
    /// with F8; forces `focus` to `Focus::Terminal` when turned on, and is
    /// itself turned back off if the user leaves the terminal via Ctrl+B.
    pub focus_mode: bool,
    pub modal: Option<Modal>,
    /// The Explorer's Ctrl+F fuzzy-finder overlay — mutually exclusive with
    /// `modal` (opening one never happens while the other is up), but kept
    /// as its own field rather than a `Modal` variant since it needs a
    /// live-filtered list + preview layout `ui::modal`'s fixed layout
    /// doesn't support.
    pub fuzzy_finder: Option<FuzzyFinderState>,
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
    /// reach `claude`'s own mouse-tracking UI (see `mouse::focused_wants_mouse`) —
    /// its own copy-on-select keeps working regardless of this flag since
    /// `pty_output::scan_osc52` forwards it straight from the raw PTY stream.
    /// Toggled via F9 for whoever wants the terminal emulator's native mouse
    /// handling instead. The main loop watches this flag and issues the
    /// actual Enable/DisableMouseCapture escape sequence when it changes.
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
    /// (`pty_output::scan_osc52`) — `vt100::Parser` has no hook for OSC 52 and
    /// silently drops it, so argus has to notice and act on it itself. Goes
    /// out through both an OSC 52 write to the real terminal and,
    /// best-effort, a direct call to a local clipboard tool
    /// (`xsel` / `xclip` / `wl-copy`) — GNOME Terminal's VTE deliberately does
    /// not implement OSC 52 clipboard-set, so the escape sequence alone
    /// doesn't get the text into the clipboard on that terminal.
    pub clipboard_copy_requested: Option<String>,
    /// Toast notification engine — see `notification::NotificationCenter`
    /// and the `notify_*` methods below for the service other parts of the
    /// app use to raise one.
    pub notifications: NotificationCenter,
    /// Id of whichever toast's close button the mouse is currently over —
    /// `ui::notification::draw` renders that one `X` in red. Updated on
    /// every `MouseEventKind::Moved` in `app::mouse::on_mouse`.
    pub hovered_notification: Option<u64>,
    /// Set whenever anything the frame renders might have changed; the main
    /// loop's redraw tick draws and clears it, and skips `terminal.draw()`
    /// entirely otherwise. Coarse-grained on purpose — every discrete input
    /// or `AppEvent` sets it via `mark_dirty`, same as the old unconditional
    /// 60Hz redraw would have repainted for any of them — the only thing
    /// this changes is no longer repainting an unchanged frame when nothing
    /// happened at all. Starts `true` so the first frame always draws.
    pub dirty: bool,
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
            focus_mode: false,
            modal: None,
            fuzzy_finder: None,
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
            dirty: true,
        }
    }

    /// Marks the frame as needing a redraw — called by the main loop for
    /// every discrete input event and `AppEvent` it processes.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Whether something is animating purely on wall-clock time (no
    /// discrete event to hang a `mark_dirty` off of) and therefore still
    /// needs periodic redraws even while otherwise idle: a "thinking"
    /// spinner, an unread blink dot, a fading toast, or a status-line
    /// message counting down to its own clearing.
    pub fn is_animating(&self) -> bool {
        self.status_line_at.is_some()
            || self.notifications.visible().next().is_some()
            || self
                .sessions
                .values()
                .any(|s| s.unread || s.status == Some(RuntimeStatus::Thinking))
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
    ///
    /// Marks the frame dirty itself when it actually changes something —
    /// this runs right before the main loop's `dirty || is_animating()`
    /// redraw check, so a transition it makes (e.g. status line just
    /// expiring) has to flip `dirty` here or that check would read the
    /// *post*-clear state and wrongly conclude nothing needs a redraw,
    /// leaving the stale text on screen.
    pub fn tick(&mut self) {
        if let Some(at) = self.status_line_at {
            if at.elapsed() >= Self::STATUS_LINE_TIMEOUT {
                self.status_line.clear();
                self.status_line_at = None;
                self.dirty = true;
            }
        }
        let notifications_before = self.notifications.visible().count();
        self.notifications.tick();
        if self.notifications.visible().count() != notifications_before {
            self.dirty = true;
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

    /// Marks the currently displayed session as read. Called once per event
    /// loop iteration (see `main.rs`) so any path that changes which session
    /// is focused — direct selection, auto-focus on close, switching back to
    /// a workspace whose focused session didn't change — clears `unread`
    /// uniformly, without each call site needing to know about it.
    pub fn sync_focused_read(&mut self) {
        if let Some(id) = self.focused_session_id() {
            if let Some(entry) = self.sessions.get_mut(&id) {
                if entry.unread {
                    entry.unread = false;
                    self.dirty = true;
                }
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
                        entry.explorer.invalidate_flatten();
                    }
                }
                Err(e) => {
                    self.set_status(t("explorer.list.error_status", &[("error", &e.to_string())]));
                    self.notify_warn(t("explorer.list.error_title", &[]), e.to_string());
                }
            },
            AppEvent::FsOpDone(workspace_id, parent, result) => {
                if let Err(e) = result {
                    self.set_status(t("explorer.op.error_status", &[("error", &e.to_string())]));
                    self.notify_warn(t("explorer.op.error_title", &[]), e.to_string());
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
                        Err(e) => self.set_status(t("git.log.error_status", &[("error", &e.to_string())])),
                    }
                }
            }
            AppEvent::GitActionDone { workspace_id, repo, action, result } => {
                if let Err(e) = result {
                    self.set_status(t("git.action.error_status", &[("action", action), ("error", &e.to_string())]));
                    self.notify_error(t("git.action.error_title", &[("action", action)]), e.to_string());
                } else if action == "commit" {
                    if let Some(w) = self.workspace_entries.get_mut(&workspace_id) {
                        w.git.commit_message.clear();
                    }
                    self.notify_info(t("git.commit.success_title", &[]), format!("{}", repo.display()));
                }
                self.runtime.spawn_git_refresh(workspace_id, repo);
            }
            AppEvent::FinderIndexed { workspace_id, all, files } => {
                if let Some(w) = self.workspace_entries.get_mut(&workspace_id) {
                    if all {
                        w.file_index_all = Some(files);
                    } else {
                        w.file_index = Some(files);
                    }
                }
                // Only re-run the match if the finder is showing this exact
                // index (Files mode, same show_ignored index that was just
                // rebuilt) — Content mode never reads the cached index, and
                // reacting unconditionally here reset the selection back to
                // the top on every background fs-watcher reindex, which made
                // arrow-key navigation look broken while the finder was open.
                if self
                    .fuzzy_finder
                    .as_ref()
                    .is_some_and(|f| f.workspace_id == workspace_id && f.mode == FinderMode::Files && f.show_ignored == all)
                {
                    self.refresh_finder_results_keep_selection();
                }
            }
            AppEvent::FinderSearchResult { workspace_id, generation, matches } => {
                if let Some(finder) = self.fuzzy_finder.as_mut() {
                    if finder.workspace_id == workspace_id && finder.search_gen == generation {
                        finder.results = matches;
                        finder.selected = 0;
                        self.request_finder_preview();
                    }
                }
            }
            AppEvent::FinderPreviewLoaded { generation, path, result } => {
                if let Some(finder) = self.fuzzy_finder.as_mut() {
                    if finder.preview_gen == generation {
                        match result {
                            Ok((truncated, highlighted)) => {
                                finder.preview_highlighted = highlighted;
                                finder.preview = Some(truncated);
                            }
                            Err(e) => {
                                finder.preview_highlighted = None;
                                finder.preview = Some(t("finder.preview.error", &[("error", &e.to_string())]));
                            }
                        }
                        finder.preview_path = Some(path);
                    }
                }
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
                self.set_status(t("session.create.error_status", &[("error", &e.to_string())]));
                self.notify_error(t("session.create.error_title", &[]), e.to_string());
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
                self.runtime.spawn_index_files(workspace_id, workspace.directory.clone(), false);
                self.resize_focused_session();
            }
            Err(e) => {
                self.pending_output.remove(&stream_id);
                self.set_status(t("workspace.create.error_status", &[("error", &e.to_string())]));
                self.notify_error(t("workspace.create.error_title", &[]), e.to_string());
                self.should_quit = self.workspaces.is_empty();
            }
        }
    }

    pub(crate) fn on_session_gone(&mut self, session_id: SessionId) {
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
        let root = entry.workspace.directory.clone();
        self.runtime.spawn_index_files(workspace_id, root.clone(), false);
        if entry.file_index_all.is_some() {
            self.runtime.spawn_index_files(workspace_id, root, true);
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

    /// Recomputes the terminal pane's size for the current `sidebar_width`
    /// and `focus_mode`, and resizes the focused session's PTY to match —
    /// called whenever `focus_mode` flips, since that changes how much of
    /// `full` the terminal pane gets and whether it has a border.
    pub(crate) fn resize_for_current_layout(&mut self, full: ratatui::layout::Rect) {
        let regions = crate::ui::layout::compute(full, self.sidebar_width, self.focus_mode);
        let (cols, rows) = crate::ui::layout::pty_content_size(regions.terminal, !self.focus_mode);
        self.set_terminal_size(cols, rows);
    }

    // ---- key handling ------------------------------------------------

    pub fn on_key(&mut self, key: KeyEvent, hitmap: &HitMap) {
        if key.code == KeyCode::F(9) {
            self.mouse_capture_enabled = !self.mouse_capture_enabled;
            self.set_status(if self.mouse_capture_enabled {
                t("statusbar.mouse_capture.on", &[])
            } else {
                t("statusbar.mouse_capture.off", &[])
            });
            return;
        }

        if self.modal.is_some() {
            self.handle_modal_key(key);
            return;
        }

        if self.fuzzy_finder.is_some() {
            self.handle_finder_key(key);
            return;
        }

        // Global toggle for Focus Mode — checked before Focus routing so it
        // works from both Focus::Terminal and Focus::Sidebar. Modal takes
        // priority (handled above), same as every other global key.
        if key.code == KeyCode::F(8) {
            self.focus_mode = !self.focus_mode;
            if self.focus_mode {
                self.focus = Focus::Terminal;
            }
            self.resize_for_current_layout(hitmap.full);
            return;
        }

        match self.focus {
            Focus::Terminal => self.handle_terminal_key(key, hitmap.full),
            Focus::Sidebar => self.handle_sidebar_key(key),
        }
    }

    /// Handles a `crossterm::event::Event::Paste` (bracketed paste, enabled
    /// in `main.rs`): the whole clipboard contents arrive as one string
    /// instead of a `KeyEvent` per character. Without this, paste falls back
    /// to individual `Char`/`Enter` key events — visibly trickling in one
    /// letter at a time, and (worse) every embedded newline hits
    /// `keys::terminal::handle_terminal_key`'s `\r` and gets read by the
    /// session as "submit now", firing off a partial message mid-paste.
    ///
    /// Forwarded to the focused session via [`Self::write_bracketed_paste`]
    /// so a bracketed-paste-aware program inside (a shell, `claude` itself)
    /// treats it as one paste rather than as typed submits.
    pub fn on_paste(&mut self, text: String) {
        if let Some(modal) = self.modal.as_mut() {
            modal.paste(&text);
            return;
        }
        if let Some(finder) = self.fuzzy_finder.as_mut() {
            crate::text_input::apply_paste(&mut finder.query, &text);
            self.refresh_finder_results();
            return;
        }
        if self.focus == Focus::Terminal {
            if let Some(session_id) = self.focused_session_id() {
                self.write_bracketed_paste(session_id, &text);
            }
        }
    }

    /// Wraps `text` in the same `ESC[200~...ESC[201~` bracketed-paste
    /// markers a real terminal uses and writes it to `session_id`'s PTY —
    /// the one place that encoding happens, shared by a real clipboard paste
    /// ([`Self::on_paste`]) and the fuzzy finder's Enter-to-insert
    /// (`keys::finder::insert_finder_targets`), which is functionally the
    /// same "insert this text as a paste" operation via a second entry
    /// point.
    pub(crate) fn write_bracketed_paste(&self, session_id: SessionId, text: &str) {
        let mut bytes = Vec::with_capacity(text.len() + 12);
        bytes.extend_from_slice(b"\x1b[200~");
        bytes.extend_from_slice(text.as_bytes());
        bytes.extend_from_slice(b"\x1b[201~");
        self.runtime.write_to_session(session_id, &bytes);
    }
}
