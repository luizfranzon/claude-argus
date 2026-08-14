use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub const DEFAULT_SIDEBAR_WIDTH: u16 = 34;
pub const MIN_SIDEBAR_WIDTH: u16 = 20;
/// How many columns the terminal pane keeps at minimum when the sidebar is
/// dragged wide — prevents dragging the divider so far right the terminal
/// pane collapses to nothing.
pub const MIN_TERMINAL_WIDTH: u16 = 30;

pub struct Regions {
    pub topbar: Rect,
    pub sidebar: Rect,
    pub terminal: Rect,
    pub statusbar: Rect,
}

pub fn compute(full: Rect, sidebar_width: u16) -> Regions {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
        .split(full);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(sidebar_width), Constraint::Min(0)])
        .split(vertical[1]);

    Regions {
        topbar: vertical[0],
        sidebar: body[0],
        terminal: body[1],
        statusbar: vertical[2],
    }
}

/// Clamps a candidate sidebar width against `full`'s total width, keeping
/// both the sidebar and the terminal pane at least their respective minimums.
pub fn clamp_sidebar_width(width: u16, full_width: u16) -> u16 {
    let max = full_width.saturating_sub(MIN_TERMINAL_WIDTH).max(MIN_SIDEBAR_WIDTH);
    width.clamp(MIN_SIDEBAR_WIDTH, max)
}

/// The PTY content size for a given terminal pane `Rect` — one cell of
/// border on every side (see `ui::terminal`'s `Block::bordered()`).
pub fn pty_content_size(terminal_area: Rect) -> (u16, u16) {
    (terminal_area.width.saturating_sub(2), terminal_area.height.saturating_sub(2))
}

/// Splits `area` into `n` equal-width horizontal columns — shared by every
/// tab bar (topbar workspaces, sidebar Agents/Explorer/Git) so the Rects used
/// to render each tab are the exact same ones used to hit-test clicks on it.
pub fn equal_columns(area: Rect, n: usize) -> Vec<Rect> {
    if n == 0 {
        return Vec::new();
    }
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Ratio(1, n as u32); n])
        .split(area)
        .to_vec()
}
