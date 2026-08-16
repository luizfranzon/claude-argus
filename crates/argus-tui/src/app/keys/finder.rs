use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use argus_domain::WorkspaceId;

use crate::app::{AppState, Focus};
use crate::fuzzy_finder::{self, FinderFocus, FinderMode, FuzzyFinderState, PREVIEW_PAGE_SIZE};
use crate::ui::hitmap::HitMap;

impl AppState {
    pub(crate) fn open_fuzzy_finder(&mut self) {
        let Some(workspace_id) = self.active_workspace else { return };
        let Some(entry) = self.workspace_entries.get(&workspace_id) else { return };
        let root = entry.workspace.directory.clone();
        if entry.file_index.is_none() {
            self.runtime.spawn_index_files(workspace_id, root.clone(), false);
        }
        self.fuzzy_finder = Some(FuzzyFinderState::new(workspace_id, root));
        self.refresh_finder_results();
    }

    fn close_fuzzy_finder(&mut self) {
        self.fuzzy_finder = None;
    }

    /// Re-runs whichever search Files/Content mode calls for against the
    /// current query, mode and gitignore toggle. Files mode matches
    /// synchronously against the cached index (near-instant even for large
    /// projects); Content mode bumps the search generation and hands the
    /// grep off to a debounced background task — see
    /// `Runtime::spawn_finder_grep`.
    pub(crate) fn refresh_finder_results(&mut self) {
        self.refresh_finder_results_impl(true);
    }

    /// Same as [`Self::refresh_finder_results`] but keeps whichever row is
    /// currently highlighted selected (by path) instead of jumping back to
    /// the top — for background-triggered refreshes (a reindex arriving)
    /// where nothing the user did should move their cursor.
    pub(crate) fn refresh_finder_results_keep_selection(&mut self) {
        self.refresh_finder_results_impl(false);
    }

    fn refresh_finder_results_impl(&mut self, reset_selection: bool) {
        let Some(finder) = self.fuzzy_finder.as_mut() else { return };
        finder.search_gen += 1;
        let kept_path = if reset_selection { None } else { finder.results.get(finder.selected).map(|m| m.path.clone()) };
        match finder.mode {
            FinderMode::Files => {
                let entry = self.workspace_entries.get(&finder.workspace_id);
                let index = entry.and_then(|w| if finder.show_ignored { w.file_index_all.as_ref() } else { w.file_index.as_ref() });
                finder.results = index.map(|c| fuzzy_finder::match_files(&finder.query, c)).unwrap_or_default();
                finder.selected = kept_path.and_then(|p| finder.results.iter().position(|m| m.path == p)).unwrap_or(0);
                self.request_finder_preview();
            }
            FinderMode::Content => {
                if finder.query.trim().is_empty() {
                    finder.results.clear();
                    finder.selected = 0;
                    finder.preview = None;
                    finder.preview_path = None;
                    return;
                }
                self.runtime.spawn_finder_grep(
                    finder.workspace_id,
                    finder.root.clone(),
                    finder.query.clone(),
                    finder.show_ignored,
                    finder.search_gen,
                );
            }
        }
    }

    pub(crate) fn request_finder_preview(&mut self) {
        let Some(finder) = self.fuzzy_finder.as_mut() else { return };
        finder.preview_gen += 1;
        let Some(path) = finder.selected_abs_path() else {
            finder.preview = None;
            finder.preview_path = None;
            finder.preview_scroll.reset();
            return;
        };
        // Declares what the preview should now be showing; `StableScroll`
        // itself decides whether that's actually a change (real navigation)
        // or a no-op re-request (e.g. a filesystem-watcher reindex arriving
        // while Files mode is open, which keeps the current selection but
        // still calls this) — see `FuzzyFinderState::preview_scroll`.
        let line = finder.results.get(finder.selected).and_then(|m| m.line);
        finder.preview_scroll.set_subject((path.clone(), line));
        self.runtime.spawn_finder_preview(path, finder.preview_gen);
    }

    /// The finder's query field is a compound keymap (Tab/Ctrl+T/Ctrl+G
    /// interleaved with finder-specific Enter/Esc semantics), not a plain
    /// submit/cancel text field — it deliberately does not route through
    /// `text_input::apply`, which only covers the six `Modal` variants that
    /// carry a free-text `input` (see `text_input.rs`'s module doc comment).
    pub(crate) fn handle_finder_key(&mut self, key: KeyEvent, hitmap: &HitMap) {
        let (min, max) = (hitmap.finder_preview_offset_min, hitmap.finder_preview_offset_max);
        let Some(finder) = self.fuzzy_finder.as_mut() else { return };
        match key.code {
            KeyCode::Esc => {
                self.close_fuzzy_finder();
            }
            KeyCode::Enter => {
                let targets = finder.confirm_targets();
                let workspace_id = finder.workspace_id;
                self.close_fuzzy_finder();
                self.insert_finder_targets(workspace_id, targets);
            }
            // Tab switches which pane arrow keys/PageUp/PageDown drive —
            // the results list or the preview (for scrolling it). Mode
            // (Files/Content) moved to Ctrl+Space so Tab could take this
            // over; see the Ctrl+Space arm below.
            KeyCode::Tab => {
                finder.toggle_focus();
            }
            KeyCode::Char(' ') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                finder.toggle_mode();
                self.refresh_finder_results();
            }
            // Ctrl+T (not bare Space) marks/unmarks the highlighted row —
            // Space has to stay a literal query character, since both a
            // Content-mode search phrase and a nucleo multi-atom Files
            // pattern routinely contain spaces.
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                finder.toggle_mark();
            }
            KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                finder.toggle_show_ignored();
                let workspace_id = finder.workspace_id;
                let root = finder.root.clone();
                let need_index = finder.show_ignored
                    && self.workspace_entries.get(&workspace_id).is_some_and(|w| w.file_index_all.is_none());
                if need_index {
                    self.runtime.spawn_index_files(workspace_id, root, true);
                }
                self.refresh_finder_results();
            }
            KeyCode::Up if finder.focus == FinderFocus::Preview => {
                finder.scroll_preview(-1, min, max);
            }
            KeyCode::Down if finder.focus == FinderFocus::Preview => {
                finder.scroll_preview(1, min, max);
            }
            KeyCode::PageUp if finder.focus == FinderFocus::Preview => {
                finder.scroll_preview(-PREVIEW_PAGE_SIZE, min, max);
            }
            KeyCode::PageDown if finder.focus == FinderFocus::Preview => {
                finder.scroll_preview(PREVIEW_PAGE_SIZE, min, max);
            }
            KeyCode::Up => {
                finder.move_selection(-1);
                self.request_finder_preview();
            }
            KeyCode::Down => {
                finder.move_selection(1);
                self.request_finder_preview();
            }
            KeyCode::Backspace => {
                finder.query.pop();
                self.refresh_finder_results();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                finder.query.push(c);
                self.refresh_finder_results();
            }
            _ => {}
        }
    }

    /// Writes `@relative/path` (space-joined for a multi-select confirm) to
    /// `workspace_id`'s active session's PTY via
    /// [`AppState::write_bracketed_paste`] — the same bracket-wrapping a
    /// real clipboard paste (`on_paste`) uses, so `claude`'s own input
    /// handling treats it as one atomic insertion — then hands keyboard
    /// focus to the terminal so typing can continue immediately.
    ///
    /// The fuzzy finder always confirms once and is done, so it goes
    /// straight to the terminal. See [`Self::insert_prompt_paths`] for the
    /// Explorer's Enter key, which stays in the sidebar so several files can
    /// be queued into the same prompt before switching over.
    pub(crate) fn insert_finder_targets(&mut self, workspace_id: WorkspaceId, targets: Vec<PathBuf>) {
        if !self.insert_prompt_paths(workspace_id, targets) {
            return;
        }
        self.focus = Focus::Terminal;
        self.resize_focused_session();
    }

    /// Writes `@relative/path` for each of `targets` to `workspace_id`'s
    /// active session's PTY, same as [`Self::insert_finder_targets`], but
    /// leaves focus wherever it already is. Returns whether anything was
    /// written (a session has to be focused, and `targets` non-empty).
    pub(crate) fn insert_prompt_paths(&mut self, workspace_id: WorkspaceId, targets: Vec<PathBuf>) -> bool {
        if targets.is_empty() {
            return false;
        }
        let Some(session_id) = self.workspace_entries.get(&workspace_id).and_then(|w| w.focused_session) else {
            return false;
        };
        let text = targets
            .iter()
            .map(|p| format!("@{}", p.display()))
            .collect::<Vec<_>>()
            .join(" ");
        self.write_bracketed_paste(session_id, &format!("{text} "));
        true
    }
}
