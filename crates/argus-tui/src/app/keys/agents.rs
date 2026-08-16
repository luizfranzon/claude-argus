use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{AppState, Focus, Modal};

impl AppState {
    pub(crate) fn handle_agents_key(&mut self, key: KeyEvent) {
        let Some(workspace_id) = self.active_workspace else { return };
        let Some(entry) = self.workspace_entries.get_mut(&workspace_id) else { return };
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                entry.agents_selected = entry.agents_selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if entry.agents_selected + 1 < entry.sessions.len() {
                    entry.agents_selected += 1;
                }
            }
            KeyCode::Char('n') => {
                self.runtime.spawn_session(workspace_id, None);
            }
            KeyCode::Char('x') => {
                if let Some(id) = entry.sessions.get(entry.agents_selected).copied() {
                    self.request_close_session(id);
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if let Some(id) = entry.sessions.get(entry.agents_selected).copied() {
                    self.modal = Some(Modal::RenameSession { session_id: id, input: String::new() });
                }
            }
            KeyCode::Enter => {
                if let Some(id) = entry.sessions.get(entry.agents_selected).copied() {
                    entry.focused_session = Some(id);
                    self.focus = Focus::Terminal;
                    self.resize_focused_session();
                }
            }
            _ => {}
        }
    }
}
