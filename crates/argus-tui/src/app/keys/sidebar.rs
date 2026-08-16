use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{AppState, Focus, Modal, SidebarTab};

impl AppState {
    pub(crate) fn handle_sidebar_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('1') => self.set_sidebar_tab(SidebarTab::Agents),
            KeyCode::Char('2') => self.set_sidebar_tab(SidebarTab::Explorer),
            KeyCode::Tab => self.cycle_sidebar_tab(),
            KeyCode::Char('[') => self.cycle_workspace(-1),
            KeyCode::Char(']') => self.cycle_workspace(1),
            KeyCode::Char('w') => {
                self.modal = Some(Modal::NewWorkspacePath { input: String::new() })
            }
            KeyCode::Char('W') => {
                if let Some(id) = self.active_workspace {
                    self.request_close_workspace(id);
                }
            }
            // Returns to whichever session was already focused, without
            // touching the Agents list highlight — unlike Enter, which both
            // selects and focuses.
            KeyCode::Esc if self.focused_session_id().is_some() => {
                self.focus = Focus::Terminal;
            }
            _ => {
                let tab = self.active_entry().map(|w| w.sidebar_tab);
                match tab {
                    Some(SidebarTab::Agents) => self.handle_agents_key(key),
                    Some(SidebarTab::Explorer) => self.handle_explorer_key(key),
                    None => {}
                }
            }
        }
    }

    pub(crate) fn set_sidebar_tab(&mut self, tab: SidebarTab) {
        if let Some(w) = self.active_entry_mut() {
            w.sidebar_tab = tab;
        }
    }

    fn cycle_sidebar_tab(&mut self) {
        if let Some(w) = self.active_entry_mut() {
            w.sidebar_tab = match w.sidebar_tab {
                SidebarTab::Agents => SidebarTab::Explorer,
                SidebarTab::Explorer => SidebarTab::Agents,
            };
        }
    }

    fn cycle_workspace(&mut self, dir: i32) {
        if self.workspaces.is_empty() {
            return;
        }
        let current = self
            .active_workspace
            .and_then(|id| self.workspaces.iter().position(|w| *w == id))
            .unwrap_or(0) as i32;
        let len = self.workspaces.len() as i32;
        let next = (current + dir).rem_euclid(len);
        self.active_workspace = Some(self.workspaces[next as usize]);
        self.resize_focused_session();
    }
}
