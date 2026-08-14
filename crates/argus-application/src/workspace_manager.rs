use std::collections::HashMap;

use argus_domain::shell::{Panel, PanelId, PanelKind, PanelOwner, RegionKind, ShellLayout};
use argus_domain::{Session, SessionId, Workspace, WorkspaceId, WorkspaceStatus};

use crate::ports::{PtyHandleId, WatchHandle};

/// In-memory aggregate of everything the application layer needs to track:
/// live workspaces, the Sessions running inside them, the extensible shell
/// layout they're placed into, the PTY handle backing each Session, and the
/// PATH resolved once at startup.
///
/// Deliberately synchronous with no interior locking of its own — the
/// composition root (src-tauri) is responsible for wrapping this in whatever
/// concurrency primitive its runtime needs (e.g. `Arc<Mutex<WorkspaceManager>>`).
/// Keeping it plain here is what makes it trivial to unit test.
#[derive(Debug, Default)]
pub struct WorkspaceManager {
    workspaces: HashMap<WorkspaceId, Workspace>,
    sessions: HashMap<SessionId, Session>,
    pty_handles: HashMap<SessionId, PtyHandleId>,
    watch_handles: HashMap<WorkspaceId, WatchHandle>,
    layout: ShellLayout,
    resolved_path: Option<String>,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        Self {
            workspaces: HashMap::new(),
            sessions: HashMap::new(),
            pty_handles: HashMap::new(),
            watch_handles: HashMap::new(),
            layout: ShellLayout::new(),
            resolved_path: None,
        }
    }

    pub fn watch_handle_for(&self, id: WorkspaceId) -> Option<WatchHandle> {
        self.watch_handles.get(&id).copied()
    }

    pub fn set_watch_handle(&mut self, id: WorkspaceId, handle: WatchHandle) {
        self.watch_handles.insert(id, handle);
    }

    pub fn resolved_path(&self) -> Option<&str> {
        self.resolved_path.as_deref()
    }

    pub fn set_resolved_path(&mut self, path: String) {
        self.resolved_path = Some(path);
    }

    pub fn get(&self, id: WorkspaceId) -> Option<&Workspace> {
        self.workspaces.get(&id)
    }

    pub fn get_session(&self, id: SessionId) -> Option<&Session> {
        self.sessions.get(&id)
    }

    pub fn pty_handle_for_session(&self, id: SessionId) -> Option<PtyHandleId> {
        self.pty_handles.get(&id).copied()
    }

    pub fn sessions_for_workspace(&self, workspace_id: WorkspaceId) -> Vec<&Session> {
        self.sessions
            .values()
            .filter(|s| s.workspace_id == workspace_id)
            .collect()
    }

    pub fn list(&self) -> Vec<&Workspace> {
        self.workspaces.values().collect()
    }

    pub fn list_sessions(&self) -> Vec<&Session> {
        self.sessions.values().collect()
    }

    pub fn layout(&self) -> &ShellLayout {
        &self.layout
    }

    /// Registers a newly-created workspace: stores it and places its sidebar
    /// panels (FileExplorer, GitPanel — both in SidebarLeft) into the layout.
    /// No Terminal panel is created here — that happens per-Session via
    /// `register_session`, since a Workspace no longer owns a PTY directly
    /// (see ADR-0010). The Editor panel is not created here either; it
    /// appears on demand when the first file is opened (see `open_editor`).
    pub fn register(&mut self, workspace: Workspace) {
        let workspace_id = workspace.id;
        self.workspaces.insert(workspace_id, workspace);
        self.layout.add_panel(Panel::new(
            PanelKind::FileExplorer(workspace_id),
            RegionKind::SidebarLeft,
        ));
        self.layout.add_panel(Panel::new(
            PanelKind::GitPanel(workspace_id),
            RegionKind::SidebarLeft,
        ));
    }

    /// Registers a newly-spawned Session: stores it, records its PTY handle,
    /// and places its Terminal panel into the Grid region.
    pub fn register_session(&mut self, session: Session, pty_handle: PtyHandleId) {
        let session_id = session.id;
        self.pty_handles.insert(session_id, pty_handle);
        self.sessions.insert(session_id, session);
        self.layout.add_panel(Panel::new(
            PanelKind::Terminal(session_id),
            RegionKind::Grid,
        ));
    }

    /// Ensures an Editor panel exists for `id`, creating one in the Grid
    /// region if this is the first file opened for that Workspace. Idempotent.
    pub fn open_editor(&mut self, id: WorkspaceId) {
        let already_open = self
            .layout
            .panels()
            .iter()
            .any(|p| matches!(p.kind, PanelKind::Editor(workspace_id) if workspace_id == id));
        if !already_open {
            self.layout
                .add_panel(Panel::new(PanelKind::Editor(id), RegionKind::Grid));
        }
    }

    pub fn set_status(&mut self, id: WorkspaceId, status: WorkspaceStatus) {
        if let Some(workspace) = self.workspaces.get_mut(&id) {
            workspace.status = status;
        }
    }

    pub fn set_session_status(&mut self, id: SessionId, status: argus_domain::SessionStatus) {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.status = status;
        }
    }

    pub fn rename_session(&mut self, id: SessionId, name: String) {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.name = name;
        }
    }

    /// Removes a Session and its Terminal panel, returning the freed PTY
    /// handle (if any) so the caller can tear down the real process.
    pub fn remove_session(&mut self, id: SessionId) -> Option<PtyHandleId> {
        self.sessions.remove(&id)?;
        let panel_ids: Vec<PanelId> = self
            .layout
            .panels()
            .iter()
            .filter(|p| p.kind.owner() == PanelOwner::Session(id))
            .map(|p| p.id)
            .collect();
        for panel_id in panel_ids {
            self.layout.remove_panel(panel_id);
        }
        self.pty_handles.remove(&id)
    }

    /// Removes a workspace, cascading to every Session it hosts (freeing
    /// each Session's PTY handle) and every panel it owns directly (Editor,
    /// FileExplorer, GitPanel — whichever exist). Returns every freed PTY
    /// handle so the caller can terminate each real process.
    pub fn remove(&mut self, id: WorkspaceId) -> Vec<PtyHandleId> {
        if self.workspaces.remove(&id).is_none() {
            return Vec::new();
        }

        let session_ids: Vec<SessionId> = self
            .sessions_for_workspace(id)
            .into_iter()
            .map(|s| s.id)
            .collect();
        let mut freed_handles: Vec<PtyHandleId> = session_ids
            .into_iter()
            .filter_map(|session_id| self.remove_session(session_id))
            .collect();

        let panel_ids: Vec<PanelId> = self
            .layout
            .panels()
            .iter()
            .filter(|p| p.kind.owner() == PanelOwner::Workspace(id))
            .map(|p| p.id)
            .collect();
        for panel_id in panel_ids {
            self.layout.remove_panel(panel_id);
        }

        freed_handles.shrink_to_fit();
        freed_handles
    }

    /// Removes and returns a Workspace's `WatchHandle`, if one was set, so the
    /// caller can tear down the real filesystem watcher. Separate from
    /// `remove` since the watcher is only started once file-explorer commands
    /// are actually issued for a Workspace, not at registration time.
    pub fn take_watch_handle(&mut self, id: WorkspaceId) -> Option<WatchHandle> {
        self.watch_handles.remove(&id)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use argus_domain::shell::RegionKind;

    fn sample_workspace() -> Workspace {
        Workspace::new(WorkspaceId::new(), PathBuf::from("/tmp/project"))
    }

    fn sample_session(workspace_id: WorkspaceId) -> Session {
        Session::new(SessionId::new(), workspace_id, "Session 1".to_string())
    }

    #[test]
    fn register_adds_workspace_and_sidebar_panels_only() {
        let mut manager = WorkspaceManager::new();
        let workspace = sample_workspace();
        let id = workspace.id;
        manager.register(workspace);

        assert!(manager.get(id).is_some());
        let grid = manager
            .layout()
            .regions()
            .iter()
            .find(|r| r.kind == RegionKind::Grid)
            .unwrap();
        assert!(grid.panels.is_empty());
        let sidebar = manager
            .layout()
            .regions()
            .iter()
            .find(|r| r.kind == RegionKind::SidebarLeft)
            .unwrap();
        assert_eq!(sidebar.panels.len(), 2);
    }

    #[test]
    fn register_session_adds_terminal_panel() {
        let mut manager = WorkspaceManager::new();
        let workspace = sample_workspace();
        let workspace_id = workspace.id;
        manager.register(workspace);

        let session = sample_session(workspace_id);
        let session_id = session.id;
        let pty_handle = PtyHandleId::new();
        manager.register_session(session, pty_handle);

        assert!(manager.get_session(session_id).is_some());
        assert_eq!(manager.pty_handle_for_session(session_id), Some(pty_handle));
        let grid = manager
            .layout()
            .regions()
            .iter()
            .find(|r| r.kind == RegionKind::Grid)
            .unwrap();
        assert_eq!(grid.panels.len(), 1);
    }

    #[test]
    fn open_editor_adds_grid_panel_once() {
        let mut manager = WorkspaceManager::new();
        let workspace = sample_workspace();
        let id = workspace.id;
        manager.register(workspace);

        manager.open_editor(id);
        manager.open_editor(id);

        let editor_panels = manager
            .layout()
            .panels()
            .iter()
            .filter(|p| matches!(p.kind, PanelKind::Editor(workspace_id) if workspace_id == id))
            .count();
        assert_eq!(editor_panels, 1);
    }

    #[test]
    fn remove_session_frees_pty_handle_and_panel() {
        let mut manager = WorkspaceManager::new();
        let workspace = sample_workspace();
        let workspace_id = workspace.id;
        manager.register(workspace);
        let session = sample_session(workspace_id);
        let session_id = session.id;
        let pty_handle = PtyHandleId::new();
        manager.register_session(session, pty_handle);

        let freed = manager.remove_session(session_id);

        assert_eq!(freed, Some(pty_handle));
        assert!(manager.get_session(session_id).is_none());
        let grid = manager
            .layout()
            .regions()
            .iter()
            .find(|r| r.kind == RegionKind::Grid)
            .unwrap();
        assert!(grid.panels.is_empty());
    }

    #[test]
    fn removing_workspace_cascades_to_all_its_sessions() {
        let mut manager = WorkspaceManager::new();
        let workspace = sample_workspace();
        let workspace_id = workspace.id;
        manager.register(workspace);

        let first = sample_session(workspace_id);
        let first_handle = PtyHandleId::new();
        manager.register_session(first.clone(), first_handle);
        let second = sample_session(workspace_id);
        let second_handle = PtyHandleId::new();
        manager.register_session(second.clone(), second_handle);

        let freed = manager.remove(workspace_id);

        assert_eq!(freed.len(), 2);
        assert!(freed.contains(&first_handle));
        assert!(freed.contains(&second_handle));
        assert!(manager.get(workspace_id).is_none());
        assert!(manager.get_session(first.id).is_none());
        assert!(manager.get_session(second.id).is_none());
        assert!(manager.layout().panels().is_empty());
    }

    #[test]
    fn removing_one_workspace_leaves_others_untouched() {
        let mut manager = WorkspaceManager::new();
        let first = sample_workspace();
        let first_id = first.id;
        let second = sample_workspace();
        let second_id = second.id;
        manager.register(first);
        manager.register(second);
        let session = sample_session(second_id);
        manager.register_session(session, PtyHandleId::new());

        manager.remove(first_id);

        assert!(manager.get(first_id).is_none());
        assert!(manager.get(second_id).is_some());
        // second workspace's 2 sidebar panels + 1 terminal panel survive
        assert_eq!(manager.layout().panels().len(), 3);
    }

    #[test]
    fn resolved_path_defaults_to_none_and_can_be_set() {
        let mut manager = WorkspaceManager::new();
        assert_eq!(manager.resolved_path(), None);
        manager.set_resolved_path("/usr/bin:/bin".to_string());
        assert_eq!(manager.resolved_path(), Some("/usr/bin:/bin"));
    }
}
