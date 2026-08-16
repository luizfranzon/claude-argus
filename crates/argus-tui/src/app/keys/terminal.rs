use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{AppState, Focus, RuntimeStatus};
use crate::terminal_protocol::key_to_bytes;

impl AppState {
    pub(crate) fn handle_terminal_key(&mut self, key: KeyEvent, full: ratatui::layout::Rect) {
        // Ctrl+B is the escape hatch back to sidebar navigation, same
        // "leader key" idea as tmux — chosen because a `claude`/shell session
        // running inside essentially never needs literal Ctrl+B itself. Also
        // the escape hatch out of Focus Mode, since the sidebar it's headed
        // to is exactly what Focus Mode hides.
        if key.code == KeyCode::Char('b') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.focus = Focus::Sidebar;
            if self.focus_mode {
                self.focus_mode = false;
                self.resize_for_current_layout(full);
            }
            self.sync_agents_selection_to_focused();
            return;
        }
        if let Some(session_id) = self.focused_session_id() {
            if let Some(bytes) = key_to_bytes(&key) {
                self.runtime.write_to_session(session_id, &bytes);
                // While `Waiting` (blocked on a prompt or permission
                // picker), only a key that actually resolves the picker
                // should move the status off `Waiting` — Claude Code won't
                // necessarily fire a `Notification`/`Stop` hook for that
                // (e.g. Esc), so the status would otherwise stay purple
                // forever. Esc or Ctrl+C cancels the prompt outright, so
                // those go straight to `Idle`; Enter answers it, so that
                // optimistically resumes `Thinking`. Anything else — arrow
                // keys, digits, Tab — is just navigating within the picker
                // and hasn't confirmed anything yet, so it leaves the
                // status at `Waiting`. Either way, the next real hook event
                // corrects it.
                let is_cancel = key.code == KeyCode::Esc
                    || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL));
                if let Some(entry) = self.sessions.get_mut(&session_id) {
                    if entry.status == Some(RuntimeStatus::Waiting) {
                        if is_cancel {
                            entry.status = Some(RuntimeStatus::Idle);
                        } else if key.code == KeyCode::Enter {
                            entry.status = Some(RuntimeStatus::Thinking);
                        }
                    }
                }
            }
        }
    }

    /// Moves the Agents list highlight onto whichever session is currently
    /// focused, so leaving the terminal for the sidebar always shows the
    /// highlight on the session you were just looking at instead of wherever
    /// it was last left by arrow-key navigation.
    fn sync_agents_selection_to_focused(&mut self) {
        if let Some(entry) = self.active_entry_mut() {
            if let Some(focused) = entry.focused_session {
                if let Some(index) = entry.sessions.iter().position(|id| *id == focused) {
                    entry.agents_selected = index;
                }
            }
        }
    }
}
