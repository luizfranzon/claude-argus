//! Mouse handling: click hit-testing against the last frame's `HitMap`,
//! sidebar-resize dragging, and forwarding clicks/drags/scroll to the
//! focused session's own mouse tracking (e.g. `claude`'s "Jump to bottom"
//! button) when it has requested one.

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::terminal_protocol;
use crate::ui::hitmap::{self, HitMap};

use super::{AppState, SidebarTab};

impl AppState {
    /// Routes a mouse event either into the sidebar-resize drag or into
    /// click hit-testing against the regions `ui::draw` recorded while
    /// rendering the last frame. Ignored while a modal is open (the modal
    /// doesn't register any click targets of its own yet).
    /// Applies a coalesced mouse-wheel flick (see `scroll_coalesce`) as a
    /// single write of `ticks.abs()` repeated SGR wheel reports — same
    /// position/gating math as routing one `ScrollUp`/`ScrollDown` through
    /// `on_mouse`, just batched into one syscall and one redraw instead of
    /// one per notch, which is what keeps a fast flick from backing up
    /// `claude`'s stdin behind a pile of redraws and delaying real keystrokes
    /// typed right after.
    pub fn on_scroll_burst(&mut self, hitmap: &HitMap, mouse: MouseEvent, ticks: i32) {
        if ticks == 0 {
            return;
        }
        // The fuzzy finder's preview scrolls by hover alone, independent of
        // which pane (results/preview) currently has Tab-focus — see
        // `FuzzyFinderState::scroll_preview`. Everything else about the
        // finder (and any other modal) ignores the wheel, same as before.
        if let Some(finder) = self.fuzzy_finder.as_mut() {
            if hitmap::hit(hitmap.finder_preview_area, mouse.column, mouse.row) {
                finder.scroll_preview(-ticks, hitmap.finder_preview_offset_min, hitmap.finder_preview_offset_max);
            }
            return;
        }
        if self.modal.is_some() {
            return;
        }
        if hitmap::hit(hitmap.sidebar_content_area, mouse.column, mouse.row) {
            self.scroll_sidebar(ticks);
            return;
        }
        let content = self.terminal_content_rect(hitmap);
        if !hitmap::hit(content, mouse.column, mouse.row) || !self.focused_wants_mouse() {
            return;
        }
        let Some(session_id) = self.focused_session_id() else { return };
        let button = if ticks > 0 { 64 } else { 65 };
        let col = mouse.column.saturating_sub(content.x).saturating_add(1).max(1);
        let row = mouse.row.saturating_sub(content.y).saturating_add(1).max(1);
        let mut bytes = Vec::new();
        for _ in 0..ticks.unsigned_abs() {
            bytes.extend_from_slice(format!("\x1b[<{button};{col};{row}M").as_bytes());
        }
        self.runtime.write_to_session(session_id, &bytes);
    }

    /// Moves the Explorer's selection under the cursor by `ticks` (positive
    /// = wheel-up, negative = wheel-down) instead of forwarding the wheel
    /// anywhere — the Explorer has no independent scroll offset of its own
    /// (see `scroll::window`), it auto-scrolls to keep whichever row is
    /// `selected` in view, so moving the selection *is* how it scrolls.
    fn scroll_sidebar(&mut self, ticks: i32) {
        let Some(entry) = self.active_entry_mut() else { return };
        if entry.sidebar_tab != SidebarTab::Explorer {
            return;
        }
        let root = entry.workspace.directory.clone();
        let len = entry.explorer.flatten(&root).len();
        if len == 0 {
            return;
        }
        let next = (entry.explorer.selected as i32 - ticks).clamp(0, len as i32 - 1);
        entry.explorer.selected = next as usize;
    }

    pub fn on_mouse(&mut self, mouse: MouseEvent, hitmap: &HitMap) {
        if self.modal.is_some() || self.fuzzy_finder.is_some() {
            return;
        }
        if mouse.kind == MouseEventKind::Moved {
            self.hovered_notification = hitmap
                .notification_close
                .iter()
                .find(|(r, _)| hitmap::hit(*r, mouse.column, mouse.row))
                .map(|(_, id)| *id);
        }
        let content = self.terminal_content_rect(hitmap);

        // Once the focused session's own TUI has claimed the mouse — it
        // declared an xterm mouse-tracking mode, which is how `claude` makes
        // its own click targets like "Jump to bottom", and its own
        // copy-on-select (see `pty_output::scan_osc52`), work — every event
        // over the content rect belongs to it, not to Argus. `forwarding_mouse`
        // keeps that true for the rest of a press even if a drag strays
        // outside `content`, so the child never sees a press with no
        // matching release. Bare hover motion (no button held) is only
        // forwarded when the child asked for `AnyMotion` (mode 1003)
        // specifically — a child that only asked for click/drag tracking
        // never expects hover reports, e.g. hover-highlighted buttons.
        let over_content = hitmap::hit(content, mouse.column, mouse.row);
        let wants_this_event = if mouse.kind == MouseEventKind::Moved {
            self.focused_wants_motion()
        } else {
            self.focused_wants_mouse()
        };
        if mouse.kind == MouseEventKind::Moved && !(over_content && wants_this_event) {
            return;
        }
        if self.forwarding_mouse || (over_content && wants_this_event) {
            match mouse.kind {
                MouseEventKind::Down(_) => self.forwarding_mouse = true,
                MouseEventKind::Up(_) => self.forwarding_mouse = false,
                _ => {}
            }
            self.forward_mouse_report(content, mouse);
            return;
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some((_, id)) =
                    hitmap.notification_close.iter().find(|(r, _)| hitmap::hit(*r, mouse.column, mouse.row))
                {
                    self.notifications.dismiss(*id);
                } else if self.on_resize_handle(mouse.column, hitmap) {
                    self.resizing_sidebar = true;
                } else {
                    self.handle_click(mouse.column, mouse.row, hitmap);
                }
            }
            MouseEventKind::Drag(MouseButton::Left) if self.resizing_sidebar => {
                self.set_sidebar_width(mouse.column, hitmap.full);
            }
            MouseEventKind::Up(MouseButton::Left) => {
                self.resizing_sidebar = false;
            }
            _ => {}
        }
    }

    /// Whether the focused session's own TUI has requested xterm mouse
    /// tracking with SGR encoding (`\x1b[?1000h`/`1002`/`1003` plus `1006`) —
    /// `claude`'s interactive mouse-driven UI (e.g. its "Jump to bottom"
    /// button) only works once this is true, since only then does it parse
    /// mouse reports off its own stdin. Mouse events go nowhere while this is
    /// false (e.g. a plain shell session that never asked for mouse input) —
    /// Argus doesn't try to substitute its own click/selection handling.
    fn focused_wants_mouse(&self) -> bool {
        self.focused_session_id()
            .and_then(|id| self.sessions.get(&id))
            .is_some_and(|entry| {
                entry.parser.screen().mouse_protocol_mode() != vt100::MouseProtocolMode::None
                    && entry.parser.screen().mouse_protocol_encoding() == vt100::MouseProtocolEncoding::Sgr
            })
    }

    /// Whether the focused session asked for `AnyMotion` (mode `1003`)
    /// specifically — the mode that reports mouse movement even with no
    /// button held, which is what drives hover-highlight UI (e.g. `claude`
    /// highlighting a button under the cursor). `ButtonMotion` (`1002`) and
    /// weaker modes only ever expect click/drag reports, so bare hover is
    /// withheld from them.
    fn focused_wants_motion(&self) -> bool {
        self.focused_session_id().and_then(|id| self.sessions.get(&id)).is_some_and(|entry| {
            entry.parser.screen().mouse_protocol_mode() == vt100::MouseProtocolMode::AnyMotion
                && entry.parser.screen().mouse_protocol_encoding() == vt100::MouseProtocolEncoding::Sgr
        })
    }

    /// Forwards `mouse` to the focused session as a standard SGR mouse
    /// report (`ESC [ < Cb ; Cx ; Cy M`/`m`), the same encoding any real
    /// terminal emulator sends once an app has requested mouse tracking.
    /// Covers wheel scroll and clicks/drags alike — `vt100`'s own scrollback
    /// can't stand in for the wheel case, since `claude` draws through the
    /// alternate screen buffer (like any full-screen TUI), which never
    /// accumulates scrollback in any terminal, real or emulated, so scrolling
    /// the conversation has to be handled by `claude` itself, same as it
    /// already does for `PageUp`.
    fn forward_mouse_report(&mut self, content: ratatui::layout::Rect, mouse: MouseEvent) {
        let Some(session_id) = self.focused_session_id() else { return };
        let Some((button, release)) = terminal_protocol::sgr_mouse_button(mouse.kind) else { return };
        let col = mouse.column.saturating_sub(content.x).saturating_add(1).max(1);
        let row = mouse.row.saturating_sub(content.y).saturating_add(1).max(1);
        let suffix = if release { 'm' } else { 'M' };
        let bytes = format!("\x1b[<{button};{col};{row}{suffix}").into_bytes();
        self.runtime.write_to_session(session_id, &bytes);
    }

    /// The PTY content rect inside the terminal pane's border (see
    /// `ui::terminal`'s `Block::bordered()` and `layout::pty_content_size`).
    pub(super) fn terminal_content_rect(&self, hitmap: &HitMap) -> ratatui::layout::Rect {
        let area = hitmap.terminal_area;
        if self.focus_mode {
            // No border in Focus Mode (see `ui::terminal::draw`) — the pane
            // itself is the content rect.
            return area;
        }
        ratatui::layout::Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        }
    }

    /// The sidebar/terminal divider is a two-column grab target: the
    /// sidebar's own right border plus the terminal pane's left border
    /// drawn right next to it — same forgiving width most terminal
    /// multiplexers give a drag handle.
    fn on_resize_handle(&self, column: u16, hitmap: &HitMap) -> bool {
        let border_x = hitmap.full.x + self.sidebar_width.saturating_sub(1);
        column == border_x || column == border_x.saturating_add(1)
    }

    fn set_sidebar_width(&mut self, column: u16, full: ratatui::layout::Rect) {
        let width = column.saturating_sub(full.x).saturating_add(1);
        self.sidebar_width = crate::ui::layout::clamp_sidebar_width(width, full.width);
        self.resize_for_current_layout(full);
    }

    fn handle_click(&mut self, x: u16, y: u16, hitmap: &HitMap) {
        if let Some((_, workspace_id)) = hitmap.topbar_tabs.iter().find(|(r, _)| hitmap::hit(*r, x, y)) {
            self.active_workspace = Some(*workspace_id);
            self.resize_focused_session();
            return;
        }

        if let Some((_, tab)) = hitmap.sidebar_tabs.iter().find(|(r, _)| hitmap::hit(*r, x, y)) {
            self.focus = super::Focus::Sidebar;
            self.set_sidebar_tab(*tab);
            return;
        }

        if let Some(&(_, index, session_id)) = hitmap.agents_rows.iter().find(|(r, ..)| hitmap::hit(*r, x, y)) {
            if let Some(entry) = self.active_entry_mut() {
                entry.agents_selected = index;
                entry.focused_session = Some(session_id);
            }
            self.focus = super::Focus::Terminal;
            self.resize_focused_session();
            return;
        }

        if let Some((_, index, path, is_dir)) =
            hitmap.explorer_rows.iter().find(|(r, ..)| hitmap::hit(*r, x, y)).cloned()
        {
            self.focus = super::Focus::Sidebar;
            if let Some(workspace_id) = self.active_workspace {
                if let Some(entry) = self.workspace_entries.get_mut(&workspace_id) {
                    entry.explorer.selected = index;
                    if is_dir {
                        if entry.explorer.expanded.contains(&path) {
                            entry.explorer.expanded.remove(&path);
                        } else {
                            entry.explorer.expanded.insert(path.clone());
                            if !entry.explorer.dirs.contains_key(&path) {
                                self.runtime.spawn_list_dir(workspace_id, path);
                            }
                        }
                        entry.explorer.invalidate_flatten();
                    }
                }
            }
            return;
        }

        if hitmap::hit(hitmap.sidebar_content_area, x, y) {
            self.focus = super::Focus::Sidebar;
            return;
        }

        if hitmap::hit(hitmap.terminal_area, x, y) {
            self.focus_terminal();
        }
    }
}
