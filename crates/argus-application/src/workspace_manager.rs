use std::collections::HashMap;

use argus_domain::shell::{Panel, PanelId, PanelKind, RegionKind, ShellLayout};
use argus_domain::{Workspace, WorkspaceId, WorkspaceStatus};

use crate::ports::{PtyHandleId, WatchHandle};

/// In-memory aggregate of everything the application layer needs to track:
/// live workspaces, the extensible shell layout they're placed into, the PTY
/// handle backing each workspace, and the PATH resolved once at startup.
///
/// Deliberately synchronous with no interior locking of its own — the
/// composition root (src-tauri) is responsible for wrapping this in whatever
/// concurrency primitive its runtime needs (e.g. `Arc<Mutex<WorkspaceManager>>`).
/// Keeping it plain here is what makes it trivial to unit test.
#[derive(Debug, Default)]
pub struct WorkspaceManager {
    workspaces: HashMap<WorkspaceId, Workspace>,
    pty_handles: HashMap<WorkspaceId, PtyHandleId>,
    watch_handles: HashMap<WorkspaceId, WatchHandle>,
    layout: ShellLayout,
    resolved_path: Option<String>,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        Self {
            workspaces: HashMap::new(),
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

    pub fn pty_handle_for(&self, id: WorkspaceId) -> Option<PtyHandleId> {
        self.pty_handles.get(&id).copied()
    }

    pub fn list(&self) -> Vec<&Workspace> {
        self.workspaces.values().collect()
    }

    pub fn layout(&self) -> &ShellLayout {
        &self.layout
    }

    /// Registers a newly-spawned workspace: stores it, records its PTY handle,
    /// and places its Terminal panel (Grid) and its sidebar panels
    /// (FileExplorer, GitPanel — both in SidebarLeft) into the layout. The
    /// Editor panel is not created here; it appears on demand when the first
    /// file is opened (see `open_editor`).
    pub fn register(&mut self, workspace: Workspace, pty_handle: PtyHandleId) {
        let workspace_id = workspace.id;
        self.pty_handles.insert(workspace_id, pty_handle);
        self.workspaces.insert(workspace_id, workspace);
        self.layout.add_panel(Panel::new(
            PanelKind::Terminal(workspace_id),
            RegionKind::Grid,
        ));
        self.layout.add_panel(Panel::new(
            PanelKind::FileExplorer(workspace_id),
            RegionKind::SidebarLeft,
        ));
        self.layout.add_panel(Panel::new(
            PanelKind::GitPanel(workspace_id),
            RegionKind::SidebarLeft,
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

    /// Removes a workspace and every panel it owns (Terminal, Editor,
    /// FileExplorer, GitPanel — whichever exist), returning the freed PTY
    /// handle (if any) so the caller can tear down the real process.
    pub fn remove(&mut self, id: WorkspaceId) -> Option<PtyHandleId> {
        self.workspaces.remove(&id)?;
        let panel_ids: Vec<PanelId> = self
            .layout
            .panels()
            .iter()
            .filter(|p| p.kind.workspace_id() == id)
            .map(|p| p.id)
            .collect();
        for panel_id in panel_ids {
            self.layout.remove_panel(panel_id);
        }
        self.pty_handles.remove(&id)
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

    #[test]
    fn register_adds_workspace_grid_and_sidebar_panels() {
        let mut manager = WorkspaceManager::new();
        let workspace = sample_workspace();
        let id = workspace.id;
        manager.register(workspace, PtyHandleId::new());

        assert!(manager.get(id).is_some());
        let grid = manager
            .layout()
            .regions()
            .iter()
            .find(|r| r.kind == RegionKind::Grid)
            .unwrap();
        assert_eq!(grid.panels.len(), 1);
        let sidebar = manager
            .layout()
            .regions()
            .iter()
            .find(|r| r.kind == RegionKind::SidebarLeft)
            .unwrap();
        assert_eq!(sidebar.panels.len(), 2);
    }

    #[test]
    fn open_editor_adds_grid_panel_once() {
        let mut manager = WorkspaceManager::new();
        let workspace = sample_workspace();
        let id = workspace.id;
        manager.register(workspace, PtyHandleId::new());

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
    fn remove_clears_workspace_panel_and_pty_handle() {
        let mut manager = WorkspaceManager::new();
        let workspace = sample_workspace();
        let id = workspace.id;
        let pty_handle = PtyHandleId::new();
        manager.register(workspace, pty_handle);

        let freed_handle = manager.remove(id);

        assert_eq!(freed_handle, Some(pty_handle));
        assert!(manager.get(id).is_none());
        assert!(manager.layout().panels().is_empty());
    }

    #[test]
    fn removing_one_workspace_leaves_others_untouched() {
        let mut manager = WorkspaceManager::new();
        let first = sample_workspace();
        let first_id = first.id;
        let second = sample_workspace();
        let second_id = second.id;
        manager.register(first, PtyHandleId::new());
        manager.register(second, PtyHandleId::new());

        manager.remove(first_id);

        assert!(manager.get(first_id).is_none());
        assert!(manager.get(second_id).is_some());
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
