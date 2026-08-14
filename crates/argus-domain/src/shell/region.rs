use serde::{Deserialize, Serialize};

use super::panel::PanelId;

/// A named, extensible area of the app shell that can host panels.
///
/// v1 only populates `Grid` (one panel per open Workspace). The other regions
/// exist as recognized-but-empty slots so a future community-widget system can
/// place panels in them without any change to this enum's callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RegionKind {
    SidebarLeft,
    Grid,
    TopBar,
    BottomBar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    pub kind: RegionKind,
    pub panels: Vec<PanelId>,
}

impl Region {
    pub fn new(kind: RegionKind) -> Self {
        Self {
            kind,
            panels: Vec::new(),
        }
    }

    pub fn push(&mut self, panel_id: PanelId) {
        self.panels.push(panel_id);
    }

    pub fn remove(&mut self, panel_id: PanelId) {
        self.panels.retain(|id| *id != panel_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_region_has_no_panels() {
        let region = Region::new(RegionKind::Grid);
        assert!(region.panels.is_empty());
    }

    #[test]
    fn push_then_remove_leaves_region_empty() {
        let mut region = Region::new(RegionKind::Grid);
        let panel_id = PanelId::new();
        region.push(panel_id);
        assert_eq!(region.panels, vec![panel_id]);

        region.remove(panel_id);
        assert!(region.panels.is_empty());
    }

    #[test]
    fn panel_ordering_is_preserved() {
        let mut region = Region::new(RegionKind::Grid);
        let first = PanelId::new();
        let second = PanelId::new();
        region.push(first);
        region.push(second);
        assert_eq!(region.panels, vec![first, second]);
    }
}
