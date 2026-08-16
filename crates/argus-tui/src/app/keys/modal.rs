use std::path::PathBuf;

use crossterm::event::KeyEvent;

use crate::app::{AppState, Modal};

impl Modal {
    /// Routes a bracketed paste into whichever `input` field this modal
    /// carries, if any — the confirm variants have nothing to paste into.
    pub(crate) fn paste(&mut self, text: &str) {
        let input = match self {
            Modal::NewWorkspacePath { input }
            | Modal::RenameSession { input, .. }
            | Modal::NewFile { input, .. }
            | Modal::NewDir { input, .. }
            | Modal::RenamePath { input, .. } => input,
            Modal::ConfirmCloseSession { .. }
            | Modal::ConfirmCloseWorkspace { .. }
            | Modal::ConfirmDeletePath { .. } => return,
        };
        crate::text_input::apply_paste(input, text);
    }
}

impl AppState {
    pub(crate) fn handle_modal_key(&mut self, key: KeyEvent) {
        use crate::text_input::{apply as apply_text_input, TextInputAction};

        let Some(modal) = self.modal.take() else { return };
        match modal {
            Modal::NewWorkspacePath { mut input } => match apply_text_input(&mut input, key) {
                TextInputAction::Submit => {
                    if !input.trim().is_empty() {
                        self.spawn_initial_workspace(PathBuf::from(input.trim()));
                    }
                }
                TextInputAction::Cancel => {}
                TextInputAction::Continue => self.modal = Some(Modal::NewWorkspacePath { input }),
            },
            Modal::RenameSession { session_id, mut input } => match apply_text_input(&mut input, key) {
                TextInputAction::Submit => {
                    if !input.trim().is_empty() {
                        self.runtime.rename_session(session_id, input.trim().to_string());
                        if let Some(entry) = self.sessions.get_mut(&session_id) {
                            entry.session.name = input.trim().to_string();
                        }
                    }
                }
                TextInputAction::Cancel => {}
                TextInputAction::Continue => {
                    self.modal = Some(Modal::RenameSession { session_id, input });
                }
            },
            Modal::NewFile { workspace_id, dir, mut input } => match apply_text_input(&mut input, key) {
                TextInputAction::Submit => {
                    if !input.trim().is_empty() {
                        let path = dir.join(input.trim());
                        self.runtime.spawn_create_file(workspace_id, path, dir);
                    }
                }
                TextInputAction::Cancel => {}
                TextInputAction::Continue => {
                    self.modal = Some(Modal::NewFile { workspace_id, dir, input });
                }
            },
            Modal::NewDir { workspace_id, dir, mut input } => match apply_text_input(&mut input, key) {
                TextInputAction::Submit => {
                    if !input.trim().is_empty() {
                        let path = dir.join(input.trim());
                        self.runtime.spawn_create_dir(workspace_id, path, dir);
                    }
                }
                TextInputAction::Cancel => {}
                TextInputAction::Continue => {
                    self.modal = Some(Modal::NewDir { workspace_id, dir, input });
                }
            },
            Modal::RenamePath { workspace_id, from, mut input } => match apply_text_input(&mut input, key) {
                TextInputAction::Submit => {
                    if !input.trim().is_empty() {
                        let parent = from.parent().map(PathBuf::from).unwrap_or_else(|| from.clone());
                        let to = parent.join(input.trim());
                        self.runtime.spawn_rename_path(workspace_id, from, to, parent);
                    }
                }
                TextInputAction::Cancel => {}
                TextInputAction::Continue => {
                    self.modal = Some(Modal::RenamePath { workspace_id, from, input });
                }
            },
            Modal::ConfirmCloseSession { session_id } => match key.code {
                crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Enter => {
                    self.confirm_close_session(session_id)
                }
                _ => {}
            },
            Modal::ConfirmCloseWorkspace { workspace_id } => match key.code {
                crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Enter => {
                    self.confirm_close_workspace(workspace_id)
                }
                _ => {}
            },
            Modal::ConfirmDeletePath { workspace_id, path, parent } => match key.code {
                crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Enter => {
                    self.runtime.spawn_delete_path(workspace_id, path, parent);
                }
                _ => {}
            },
        }
    }
}
