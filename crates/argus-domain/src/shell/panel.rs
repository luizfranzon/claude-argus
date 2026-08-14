use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::session::SessionId;
use crate::workspace::WorkspaceId;

use super::region::RegionKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PanelId(Uuid);

impl PanelId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PanelId {
    fn default() -> Self {
        Self::new()
    }
}

/// What a panel displays. v1 shipped one variant, tying it directly to the
/// Workspace it backs; v2 adds three more, still each tied to one Workspace;
/// v3 moves `Terminal` to a `SessionId` since the PTY it renders now belongs
/// to a Session, not directly to a Workspace (see ADR-0010) — Editor,
/// FileExplorer and GitPanel stay Workspace-scoped. A future community widget
/// adds another variant here (e.g. `Custom(PluginId)`) — `Panel`/`Region`
/// themselves stay unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanelKind {
    Terminal(SessionId),
    Editor(WorkspaceId),
    FileExplorer(WorkspaceId),
    GitPanel(WorkspaceId),
}

/// What a `PanelKind` belongs to — a Workspace directly, or a Session (which
/// itself belongs to a Workspace). Replaces the old single-type
/// `workspace_id()` extractor now that not every variant maps to a
/// `WorkspaceId` directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelOwner {
    Workspace(WorkspaceId),
    Session(SessionId),
}

impl PanelKind {
    pub fn owner(self) -> PanelOwner {
        match self {
            PanelKind::Terminal(id) => PanelOwner::Session(id),
            PanelKind::Editor(id) | PanelKind::FileExplorer(id) | PanelKind::GitPanel(id) => {
                PanelOwner::Workspace(id)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Panel {
    pub id: PanelId,
    pub kind: PanelKind,
    pub region: RegionKind,
}

impl Panel {
    pub fn new(kind: PanelKind, region: RegionKind) -> Self {
        Self {
            id: PanelId::new(),
            kind,
            region,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_ids_are_unique() {
        assert_ne!(PanelId::new(), PanelId::new());
    }

    #[test]
    fn terminal_panel_carries_its_session_id() {
        let session_id = SessionId::new();
        let panel = Panel::new(PanelKind::Terminal(session_id), RegionKind::Grid);
        assert_eq!(panel.kind, PanelKind::Terminal(session_id));
        assert_eq!(panel.region, RegionKind::Grid);
    }

    #[test]
    fn terminal_owner_is_its_session() {
        let session_id = SessionId::new();
        assert_eq!(
            PanelKind::Terminal(session_id).owner(),
            PanelOwner::Session(session_id)
        );
    }

    #[test]
    fn workspace_scoped_variants_owner_is_their_workspace() {
        let workspace_id = WorkspaceId::new();
        assert_eq!(
            PanelKind::Editor(workspace_id).owner(),
            PanelOwner::Workspace(workspace_id)
        );
        assert_eq!(
            PanelKind::FileExplorer(workspace_id).owner(),
            PanelOwner::Workspace(workspace_id)
        );
        assert_eq!(
            PanelKind::GitPanel(workspace_id).owner(),
            PanelOwner::Workspace(workspace_id)
        );
    }
}
