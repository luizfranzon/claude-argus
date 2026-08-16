use std::collections::HashSet;
use std::path::PathBuf;

use argus_application::ports::HighlightedLines;
use argus_domain::WorkspaceId;

use crate::stable_scroll::StableScroll;

/// Identifies what the preview is currently showing: the file, plus which
/// match's line within it (Content mode only — Files mode previews always
/// show the top of the file). Two requests for the same subject — e.g. a
/// background reindex re-showing the same file the user is already looking
/// at — must not reset the user's scroll; see [`StableScroll`].
pub type PreviewSubject = (PathBuf, Option<u64>);

/// Caps how many results a single query keeps around — both to keep the
/// list snappy to render/scroll and because refining the query naturally
/// narrows a huge result set anyway.
pub const RESULT_LIMIT: usize = 200;

/// How long a keystroke in Content mode waits for more typing before firing
/// the actual grep — short enough to feel live, long enough that a fast
/// typist doesn't spawn a search per character.
pub const GREP_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(90);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinderMode {
    Files,
    Content,
}

/// Which pane `Tab` currently routes navigation keys to. `Ctrl+Space` (not
/// `Tab`) switches [`FinderMode`] — see `app::keys::finder::handle_finder_key`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FinderFocus {
    #[default]
    Results,
    Preview,
}

/// How many lines `PageUp`/`PageDown` scroll the preview by when it has
/// focus — arbitrary but reasonable for the finder popup's typical height.
pub const PREVIEW_PAGE_SIZE: i32 = 10;

/// One row in the results list. `path` is always workspace-relative — the
/// form both the `@path` insertion and the on-disk lookup (joined onto the
/// workspace root) need.
#[derive(Debug, Clone)]
pub struct FinderMatch {
    pub path: PathBuf,
    /// Char indices into `path`'s displayed string that the fuzzy matcher
    /// hit — used to bold the matched characters. Empty in Content mode
    /// (the match is a whole line, not a scattered set of characters).
    pub indices: Vec<usize>,
    /// Content mode only: 1-based line number of the match.
    pub line: Option<u64>,
    /// Content mode only: the matched line's text, for the results list.
    pub line_text: Option<String>,
}

pub struct FuzzyFinderState {
    pub workspace_id: WorkspaceId,
    pub root: PathBuf,
    pub mode: FinderMode,
    pub query: String,
    /// Ctrl+G toggle: when true, search `file_index_all` (nothing filtered
    /// out) instead of `file_index` (respects .gitignore).
    pub show_ignored: bool,
    pub results: Vec<FinderMatch>,
    pub selected: usize,
    pub focus: FinderFocus,
    pub marked: HashSet<PathBuf>,
    pub preview: Option<String>,
    pub preview_path: Option<PathBuf>,
    /// Extra lines scrolled beyond the preview's auto-centered position
    /// (positive = further down). Keyed by [`PreviewSubject`] so the offset
    /// only resets when the file/match actually being previewed changes —
    /// not on every redundant re-request (e.g. a background reindex keeping
    /// the same selection) — see `app::keys::finder::request_finder_preview`
    /// and [`StableScroll`].
    pub preview_scroll: StableScroll<PreviewSubject>,
    /// Bumped on every query/mode/toggle change; results and previews that
    /// arrive tagged with a stale generation are discarded — the cheap
    /// substitute for actually cancelling the in-flight grep/read task.
    pub search_gen: u64,
    pub preview_gen: u64,
    /// Syntax-highlighted version of `preview`, one entry per line, each a
    /// list of (text, rgb) fragments in source order. `None` when the
    /// current preview has no matching syntax (unknown extension, read
    /// error) — the UI falls back to rendering `preview` unstyled.
    pub preview_highlighted: Option<HighlightedLines>,
}

impl FuzzyFinderState {
    pub fn new(workspace_id: WorkspaceId, root: PathBuf) -> Self {
        Self {
            workspace_id,
            root,
            mode: FinderMode::Files,
            query: String::new(),
            show_ignored: false,
            results: Vec::new(),
            selected: 0,
            focus: FinderFocus::default(),
            marked: HashSet::new(),
            preview: None,
            preview_path: None,
            preview_scroll: StableScroll::new(),
            search_gen: 0,
            preview_gen: 0,
            preview_highlighted: None,
        }
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            FinderMode::Files => FinderMode::Content,
            FinderMode::Content => FinderMode::Files,
        };
        self.results.clear();
        self.selected = 0;
        self.preview_scroll.reset();
    }

    pub fn toggle_show_ignored(&mut self) {
        self.show_ignored = !self.show_ignored;
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            FinderFocus::Results => FinderFocus::Preview,
            FinderFocus::Preview => FinderFocus::Results,
        };
    }

    /// Scrolls the preview by `delta` lines, on top of its auto-centered
    /// base position — see `preview_scroll`. Negative moves up (toward the
    /// top of the file), positive moves down. `min`/`max` bound the
    /// resulting offset so it can't run past either edge of the file — the
    /// caller passes the previous frame's `HitMap::finder_preview_offset_*`,
    /// computed by `ui::fuzzy_finder::draw_preview` from the actual content
    /// length and pane height. Without this immediate clamp, scrolling past
    /// an edge would let the offset keep accumulating unboundedly, so it'd
    /// then take just as many opposite-direction presses to visibly move
    /// again — see `StableScroll::scroll_clamped`.
    pub fn scroll_preview(&mut self, delta: i32, min: i32, max: i32) {
        self.preview_scroll.scroll_clamped(delta, min, max);
    }

    pub fn toggle_mark(&mut self) {
        if let Some(m) = self.results.get(self.selected) {
            let path = m.path.clone();
            if !self.marked.remove(&path) {
                self.marked.insert(path);
            }
        }
    }

    /// Moving the selection doesn't need to touch `preview_scroll` directly
    /// — every caller follows this with `request_finder_preview`, which
    /// declares the new subject and lets `StableScroll` decide whether
    /// anything actually changed.
    pub fn move_selection(&mut self, delta: i32) {
        if self.results.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.results.len() as i32;
        let next = (self.selected as i32 + delta).clamp(0, len - 1);
        self.selected = next as usize;
    }

    /// Absolute path of the currently highlighted row, for loading a preview.
    pub fn selected_abs_path(&self) -> Option<PathBuf> {
        self.results.get(self.selected).map(|m| self.root.join(&m.path))
    }

    /// Paths to insert on confirm: every marked path if any are marked,
    /// otherwise just whatever's currently highlighted.
    pub fn confirm_targets(&self) -> Vec<PathBuf> {
        if !self.marked.is_empty() {
            let mut paths: Vec<PathBuf> = self.marked.iter().cloned().collect();
            paths.sort();
            paths
        } else if let Some(m) = self.results.get(self.selected) {
            vec![m.path.clone()]
        } else {
            Vec::new()
        }
    }
}

/// Fuzzy-matches `query` against workspace-relative `candidates`, returning
/// the top [`RESULT_LIMIT`] by score with per-candidate matched character
/// indices for highlighting. An empty query returns the first
/// [`RESULT_LIMIT`] candidates unscored, in index order — the "browse
/// everything" state before you start typing.
///
/// Pure UI-owned scoring: `candidates` themselves come from
/// `FileSearchPort::walk_files` (see `argus-application::ports`), but
/// ranking a list that's already in hand is a rendering concern, not I/O, so
/// it stays here rather than behind the port.
pub fn match_files(query: &str, candidates: &[PathBuf]) -> Vec<FinderMatch> {
    use argus_domain::strip_diacritics;
    use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
    use nucleo_matcher::{Config, Matcher, Utf32Str};

    if query.trim().is_empty() {
        return candidates
            .iter()
            .take(RESULT_LIMIT)
            .map(|path| FinderMatch { path: path.clone(), indices: Vec::new(), line: None, line_text: None })
            .collect();
    }

    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    // Diacritics are stripped from both the query and each candidate before
    // matching (rather than relying on nucleo's own `Normalization::Smart`,
    // which only folds accents when the *query* has none) so search is
    // accent-insensitive in both directions — see docs/adr search notes.
    // Stripping preserves char count/position 1:1, so indices computed here
    // stay valid against the original (accented) display string used for
    // rendering.
    let normalized_query = strip_diacritics(query);
    let pattern = Pattern::parse(&normalized_query, CaseMatching::Smart, Normalization::Never);

    let mut scored: Vec<(u32, PathBuf, Vec<usize>)> = Vec::new();
    let mut char_buf = Vec::new();
    let mut idx_buf = Vec::new();
    for path in candidates {
        let display = path.to_string_lossy();
        let normalized_display = strip_diacritics(&display);
        char_buf.clear();
        let haystack = Utf32Str::new(&normalized_display, &mut char_buf);
        idx_buf.clear();
        if let Some(score) = pattern.indices(haystack, &mut matcher, &mut idx_buf) {
            let indices = idx_buf.iter().map(|i| *i as usize).collect();
            scored.push((score, path.clone(), indices));
        }
    }
    scored.sort_unstable_by_key(|(score, ..)| std::cmp::Reverse(*score));
    scored
        .into_iter()
        .take(RESULT_LIMIT)
        .map(|(_, path, indices)| FinderMatch { path, indices, line: None, line_text: None })
        .collect()
}

/// Caps a fuzzy-finder preview to a display-sized excerpt — a file read
/// hands back the whole file, and a multi-megabyte source file has no
/// business being rendered in full into a sidebar-sized pane. Also bounds
/// the cost of syntax highlighting, which runs against the truncated text.
pub const PREVIEW_MAX_LINES: usize = 400;

pub fn truncate_preview(contents: &str) -> String {
    let mut lines = contents.lines().take(PREVIEW_MAX_LINES + 1).collect::<Vec<_>>();
    if lines.len() > PREVIEW_MAX_LINES {
        lines.truncate(PREVIEW_MAX_LINES);
        let mut out = lines.join("\n");
        out.push_str("\n…");
        out
    } else {
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_files_empty_query_returns_candidates_in_order() {
        let candidates = vec![PathBuf::from("b.rs"), PathBuf::from("a.rs")];
        let results = match_files("", &candidates);
        let paths: Vec<_> = results.iter().map(|m| m.path.clone()).collect();
        assert_eq!(paths, candidates);
        assert!(results.iter().all(|m| m.indices.is_empty()));
    }

    #[test]
    fn match_files_ranks_exact_prefix_above_scattered_match() {
        let candidates = vec![PathBuf::from("src/app.rs"), PathBuf::from("src/other/appendix.rs")];
        let results = match_files("app", &candidates);
        assert_eq!(results[0].path, PathBuf::from("src/app.rs"));
    }

    #[test]
    fn match_files_ignores_accents_in_both_directions() {
        let candidates = vec![PathBuf::from("não.rs"), PathBuf::from("outro.rs")];
        assert_eq!(match_files("nao", &candidates)[0].path, PathBuf::from("não.rs"));

        let candidates = vec![PathBuf::from("nao.rs"), PathBuf::from("outro.rs")];
        assert_eq!(match_files("não", &candidates)[0].path, PathBuf::from("nao.rs"));
    }

    #[test]
    fn match_files_drops_non_matching_candidates() {
        let candidates = vec![PathBuf::from("readme.md"), PathBuf::from("src/lib.rs")];
        let results = match_files("zzz", &candidates);
        assert!(results.is_empty());
    }

    #[test]
    fn match_files_respects_result_limit() {
        let candidates: Vec<PathBuf> = (0..RESULT_LIMIT + 50).map(|i| PathBuf::from(format!("file{i}.rs"))).collect();
        let results = match_files("file", &candidates);
        assert_eq!(results.len(), RESULT_LIMIT);
    }

    #[test]
    fn finder_state_toggle_mark_and_confirm_targets() {
        let mut state = FuzzyFinderState::new(WorkspaceId::new(), PathBuf::from("/root"));
        state.results = vec![
            FinderMatch { path: PathBuf::from("a.rs"), indices: vec![], line: None, line_text: None },
            FinderMatch { path: PathBuf::from("b.rs"), indices: vec![], line: None, line_text: None },
        ];

        // Nothing marked: confirm falls back to the current selection.
        assert_eq!(state.confirm_targets(), vec![PathBuf::from("a.rs")]);

        state.toggle_mark();
        state.move_selection(1);
        state.toggle_mark();
        assert_eq!(state.confirm_targets(), vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")]);

        state.selected = 1;
        state.toggle_mark();
        assert_eq!(state.confirm_targets(), vec![PathBuf::from("a.rs")]);
    }
}
