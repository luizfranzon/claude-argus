use std::path::PathBuf;

use async_trait::async_trait;

/// A syntax-highlighted preview: one entry per line, each a list of (text,
/// rgb foreground) fragments in source order.
pub type HighlightedLines = Vec<Vec<(String, (u8, u8, u8))>>;

/// One content-search hit. `path` is always workspace-relative. `line`/
/// `line_text` are set for a content match; both `None` for a bare file-walk
/// result (the caller fills in fuzzy-match indices itself, since those are a
/// property of how the UI scored the query, not of the search).
#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub path: PathBuf,
    pub line: Option<u64>,
    pub line_text: Option<String>,
}

/// Workspace file search for the fuzzy finder: walking the file tree,
/// grepping file contents, and syntax-highlighting a preview. Deliberately
/// separate from `FileSystemPort` — that port is CRUD over one path at a
/// time; this one is read-only, tree-wide, and backed by a fast gitignore-
/// aware walker rather than shelling out to `git` itself (see ADR-0013).
#[async_trait]
pub trait FileSearchPort: Send + Sync {
    /// Lists every file (not directory) under `root`, as paths relative to
    /// it. `.git` is always skipped regardless of `include_ignored`.
    async fn walk_files(&self, root: PathBuf, include_ignored: bool) -> Vec<PathBuf>;

    /// Literal (non-regex), case-insensitive search for `query` across every
    /// file under `root`. Implementations may cap the number of matches
    /// returned.
    async fn grep_content(&self, root: PathBuf, query: String, include_ignored: bool) -> Vec<SearchMatch>;

    /// Syntax-highlights `contents` using whatever syntax the implementation
    /// resolves from `path`'s extension/content. Returns `None` when no
    /// matching syntax definition exists — callers should fall back to
    /// rendering `contents` unstyled.
    async fn highlight(&self, path: PathBuf, contents: String) -> Option<HighlightedLines>;
}
