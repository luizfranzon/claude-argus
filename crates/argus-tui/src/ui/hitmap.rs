use std::path::PathBuf;

use argus_domain::{SessionId, WorkspaceId};
use ratatui::layout::Rect;

use crate::app::SidebarTab;

/// Click targets recorded while drawing the current frame — rebuilt every
/// `ui::draw` call and consumed by `AppState::on_mouse` for the next mouse
/// event. Positions only make sense against the frame they were built from,
/// same as any immediate-mode UI's hit-testing.
#[derive(Default)]
pub struct HitMap {
    pub topbar_tabs: Vec<(Rect, WorkspaceId)>,
    pub sidebar_tabs: Vec<(Rect, SidebarTab)>,
    /// `(row, absolute index into WorkspaceEntry::sessions, session id)`.
    pub agents_rows: Vec<(Rect, usize, SessionId)>,
    /// `(row, absolute index into the flattened tree, path, is_dir)`.
    pub explorer_rows: Vec<(Rect, usize, PathBuf, bool)>,
    /// `(row, absolute index into GitRepoState::status, file path, staged)`.
    pub git_rows: Vec<(Rect, usize, String, bool)>,
    pub terminal_area: Rect,
    /// The whole frame — kept so a sidebar-resize drag can re-run
    /// `ui::layout::compute` against it without the caller needing to track
    /// its own copy of the current terminal size.
    pub full: Rect,
}

pub fn hit(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.x.saturating_add(rect.width) && y >= rect.y && y < rect.y.saturating_add(rect.height)
}
