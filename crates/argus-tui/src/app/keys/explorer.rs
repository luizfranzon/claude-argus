use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{AppState, Modal};

impl AppState {
    pub(crate) fn handle_explorer_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('f') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.open_fuzzy_finder();
            return;
        }
        let Some(workspace_id) = self.active_workspace else { return };
        let Some(entry) = self.workspace_entries.get_mut(&workspace_id) else { return };
        let root = entry.workspace.directory.clone();
        // Cloned out of the cache (rather than held as a `Ref`) since this
        // function goes on to mutate `entry.explorer` itself (selection,
        // expand/collapse) in the same scope — cache lookup still skips the
        // tree walk, this just detaches the borrow immediately afterward.
        let rows: Vec<(PathBuf, usize, bool)> = entry.explorer.flatten(&root).clone();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                entry.explorer.selected = entry.explorer.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if entry.explorer.selected + 1 < rows.len() {
                    entry.explorer.selected += 1;
                }
            }
            KeyCode::Char(' ') => {
                if let Some((path, _, is_dir)) = rows.get(entry.explorer.selected).cloned() {
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
            KeyCode::Enter => {
                if let Some((path, ..)) = rows.get(entry.explorer.selected).cloned() {
                    let rel = path.strip_prefix(&root).unwrap_or(&path).to_path_buf();
                    self.insert_prompt_paths(workspace_id, vec![rel]);
                }
            }
            KeyCode::Char('a') => {
                self.modal = Some(Modal::NewFile { workspace_id, dir: dir_for_selection(&rows, entry.explorer.selected, &root), input: String::new() });
            }
            KeyCode::Char('A') => {
                self.modal = Some(Modal::NewDir { workspace_id, dir: dir_for_selection(&rows, entry.explorer.selected, &root), input: String::new() });
            }
            KeyCode::Char('r') => {
                if let Some((path, ..)) = rows.get(entry.explorer.selected).cloned() {
                    self.modal = Some(Modal::RenamePath { workspace_id, from: path, input: String::new() });
                }
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                if let Some((path, ..)) = rows.get(entry.explorer.selected).cloned() {
                    let parent = path.parent().map(PathBuf::from).unwrap_or_else(|| root.clone());
                    self.modal = Some(Modal::ConfirmDeletePath { workspace_id, path, parent });
                }
            }
            _ => {}
        }
    }
}

fn dir_for_selection(rows: &[(PathBuf, usize, bool)], selected: usize, root: &PathBuf) -> PathBuf {
    match rows.get(selected) {
        Some((path, _, true)) => path.clone(),
        Some((path, _, false)) => path.parent().map(PathBuf::from).unwrap_or_else(|| root.clone()),
        None => root.clone(),
    }
}
