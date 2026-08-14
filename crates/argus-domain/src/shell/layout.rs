use serde::{Deserialize, Serialize};

use super::panel::{Panel, PanelId};
use super::region::{Region, RegionKind};

/// The whole extensible shell: every named region, each holding its panels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellLayout {
    regions: Vec<Region>,
    panels: Vec<Panel>,
}

impl ShellLayout {
    /// All four regions exist from the start, empty — a panel can be added to
    /// any of them later without first having to "discover" the region.
    pub fn new() -> Self {
        Self {
            regions: vec![
                Region::new(RegionKind::SidebarLeft),
                Region::new(RegionKind::Grid),
                Region::new(RegionKind::TopBar),
                Region::new(RegionKind::BottomBar),
            ],
            panels: Vec::new(),
        }
    }

    pub fn regions(&self) -> &[Region] {
        &self.regions
    }

    pub fn panels(&self) -> &[Panel] {
        &self.panels
    }

    fn region_mut(&mut self, kind: RegionKind) -> &mut Region {
        self.regions
            .iter_mut()
            .find(|region| region.kind == kind)
            .expect("all RegionKind variants are seeded in ShellLayout::new")
    }

    pub fn add_panel(&mut self, panel: Panel) {
        self.region_mut(panel.region).push(panel.id);
        self.panels.push(panel);
    }

    pub fn remove_panel(&mut self, panel_id: PanelId) {
        if let Some(panel) = self.panels.iter().find(|p| p.id == panel_id) {
            let region = panel.region;
            self.region_mut(region).remove(panel_id);
        }
        self.panels.retain(|p| p.id != panel_id);
    }
}

impl Default for ShellLayout {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::panel::PanelKind;
    use crate::workspace::WorkspaceId;

    #[test]
    fn new_layout_seeds_all_regions_empty() {
        let layout = ShellLayout::new();
        assert_eq!(layout.regions().len(), 4);
        assert!(layout.regions().iter().all(|r| r.panels.is_empty()));
    }

    #[test]
    fn add_panel_places_it_in_its_region() {
        let mut layout = ShellLayout::new();
        let panel = Panel::new(PanelKind::Terminal(WorkspaceId::new()), RegionKind::Grid);
        let panel_id = panel.id;
        layout.add_panel(panel);

        let grid = layout
            .regions()
            .iter()
            .find(|r| r.kind == RegionKind::Grid)
            .unwrap();
        assert_eq!(grid.panels, vec![panel_id]);
        assert_eq!(layout.panels().len(), 1);
    }

    #[test]
    fn remove_panel_clears_it_from_region_and_layout() {
        let mut layout = ShellLayout::new();
        let panel = Panel::new(PanelKind::Terminal(WorkspaceId::new()), RegionKind::Grid);
        let panel_id = panel.id;
        layout.add_panel(panel);

        layout.remove_panel(panel_id);

        assert!(layout.panels().is_empty());
        let grid = layout
            .regions()
            .iter()
            .find(|r| r.kind == RegionKind::Grid)
            .unwrap();
        assert!(grid.panels.is_empty());
    }
}
