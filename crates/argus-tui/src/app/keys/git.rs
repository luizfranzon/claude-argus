use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{AppState, Modal};

impl AppState {
    pub(crate) fn handle_git_key(&mut self, key: KeyEvent) {
        let Some(workspace_id) = self.active_workspace else { return };
        let Some(entry) = self.workspace_entries.get_mut(&workspace_id) else { return };
        let repo_path = entry.git.active_repo().map(|r| r.repo.path.clone());
        match key.code {
            KeyCode::Left => {
                entry.git.selected_repo = entry.git.selected_repo.saturating_sub(1);
                entry.git.selected_file = 0;
            }
            KeyCode::Right => {
                if entry.git.selected_repo + 1 < entry.git.repos.len() {
                    entry.git.selected_repo += 1;
                    entry.git.selected_file = 0;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                entry.git.selected_file = entry.git.selected_file.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(repo) = entry.git.active_repo() {
                    if entry.git.selected_file + 1 < repo.status.len() {
                        entry.git.selected_file += 1;
                    }
                }
            }
            KeyCode::Char(' ') => {
                if let (Some(repo_path), Some(repo)) = (repo_path.clone(), entry.git.active_repo()) {
                    if let Some(file) = repo.status.get(entry.git.selected_file) {
                        if file.staged {
                            self.runtime.spawn_git_unstage(workspace_id, repo_path, vec![file.path.clone()]);
                        } else {
                            self.runtime.spawn_git_stage(workspace_id, repo_path, vec![file.path.clone()]);
                        }
                    }
                }
            }
            KeyCode::Char('c') => {
                if let Some(repo_path) = repo_path {
                    self.modal = Some(Modal::CommitMessage { workspace_id, repo: repo_path, input: entry.git.commit_message.clone() });
                }
            }
            KeyCode::Char('f') => {
                if let Some(repo_path) = repo_path {
                    self.runtime.spawn_git_fetch(workspace_id, repo_path);
                }
            }
            KeyCode::Char('p') => {
                if let Some(repo_path) = repo_path {
                    self.runtime.spawn_git_pull(workspace_id, repo_path);
                }
            }
            KeyCode::Char('P') => {
                if let Some(repo_path) = repo_path {
                    self.runtime.spawn_git_push(workspace_id, repo_path);
                }
            }
            KeyCode::Char('b') => {
                if let (Some(repo_path), Some(repo)) = (repo_path, entry.git.active_repo()) {
                    if repo.branches.len() > 1 {
                        let current = repo.branches.iter().position(|b| b.is_current).unwrap_or(0);
                        let next = (current + 1) % repo.branches.len();
                        let name = repo.branches[next].name.clone();
                        self.runtime.spawn_git_switch_branch(workspace_id, repo_path, name);
                    }
                }
            }
            KeyCode::Char('l') => {
                if let Some(repo_path) = repo_path {
                    let log_empty = entry.git.active_repo().map(|r| r.log.is_empty()).unwrap_or(true);
                    entry.git.show_log = !entry.git.show_log;
                    if entry.git.show_log && log_empty {
                        self.runtime.spawn_git_log(workspace_id, repo_path, 0, 30);
                    }
                }
            }
            KeyCode::Char('m') => {
                if entry.git.show_log {
                    if let (Some(repo_path), Some(repo)) = (repo_path, entry.git.active_repo()) {
                        if !repo.log_complete && !repo.log_loading {
                            let skip = repo.log.len() as u32;
                            self.runtime.spawn_git_log(workspace_id, repo_path, skip, 30);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
