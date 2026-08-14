use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
/// Workspace it backs; v2 adds three more, still each tied to one Workspace.
/// A future community widget adds another variant here (e.g. `Custom(PluginId)`)
/// — `Panel`/`Region` themselves stay unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanelKind {
    Terminal(WorkspaceId),
    Editor(WorkspaceId),
    FileExplorer(WorkspaceId),
    GitPanel(WorkspaceId),
}

impl PanelKind {
    pub fn workspace_id(self) -> WorkspaceId {
        match self {
            PanelKind::Terminal(id)
            | PanelKind::Editor(id)
            | PanelKind::FileExplorer(id)
            | PanelKind::GitPanel(id) => id,
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
    fn terminal_panel_carries_its_workspace_id() {
        let workspace_id = WorkspaceId::new();
        let panel = Panel::new(PanelKind::Terminal(workspace_id), RegionKind::Grid);
        assert_eq!(panel.kind, PanelKind::Terminal(workspace_id));
        assert_eq!(panel.region, RegionKind::Grid);
    }

    #[test]
    fn workspace_id_extracts_from_every_variant() {
        let workspace_id = WorkspaceId::new();
        assert_eq!(PanelKind::Terminal(workspace_id).workspace_id(), workspace_id);
        assert_eq!(PanelKind::Editor(workspace_id).workspace_id(), workspace_id);
        assert_eq!(
            PanelKind::FileExplorer(workspace_id).workspace_id(),
            workspace_id
        );
        assert_eq!(PanelKind::GitPanel(workspace_id).workspace_id(), workspace_id);
    }
}
