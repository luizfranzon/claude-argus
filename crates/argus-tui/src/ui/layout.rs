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

/// Computes the frame's regions. When `focus_mode` is set, the topbar and
/// sidebar are collapsed to nothing and the terminal pane takes the whole
/// frame above the status line — see ADR/Focus Mode design: everything but
/// the currently-focused session and the status line is hidden.
pub fn compute(full: Rect, sidebar_width: u16, focus_mode: bool) -> Regions {
    if focus_mode {
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(full);
        return Regions {
            topbar: Rect::default(),
            sidebar: Rect::default(),
            terminal: vertical[0],
            statusbar: vertical[1],
        };
    }

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(2)])
        .split(full);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(sidebar_width), Constraint::Min(0)])
        .split(vertical[1]);

    // The sidebar reserves a top row for its tab strip before its bordered
    // box begins, which pushes its content one row below the terminal pane's.
    // Mirror that here so both panes' content lines up.
    let terminal_column = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(body[1]);

    Regions {
        topbar: vertical[0],
        sidebar: body[0],
        terminal: terminal_column[1],
        statusbar: vertical[2],
    }
}

/// Clamps a candidate sidebar width against `full`'s total width, keeping
/// both the sidebar and the terminal pane at least their respective minimums.
pub fn clamp_sidebar_width(width: u16, full_width: u16) -> u16 {
    let max = full_width.saturating_sub(MIN_TERMINAL_WIDTH).max(MIN_SIDEBAR_WIDTH);
    width.clamp(MIN_SIDEBAR_WIDTH, max)
}

/// The PTY content size for a given terminal pane `Rect`. `bordered` should
/// be `false` in Focus Mode, where `ui::terminal` skips the
/// `Block::bordered()` wrapper and the session gets the full `terminal_area`.
pub fn pty_content_size(terminal_area: Rect, bordered: bool) -> (u16, u16) {
    if bordered {
        (terminal_area.width.saturating_sub(2), terminal_area.height.saturating_sub(2))
    } else {
        (terminal_area.width, terminal_area.height)
    }
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
