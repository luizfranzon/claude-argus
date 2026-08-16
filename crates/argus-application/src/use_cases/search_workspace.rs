use std::path::PathBuf;
use std::sync::Arc;

use crate::ports::{FileSearchPort, HighlightedLines, SearchMatch};

/// Thin wrapper over `FileSearchPort` for the fuzzy finder's three
/// operations — indexing, content search, and preview highlighting. No
/// caching or orchestration lives here today (unlike, say,
/// `ResolveStartupPathUseCase`'s once-per-process cache); it exists so
/// `Runtime` depends on an application-layer seam instead of reaching past
/// it into `argus-infrastructure` directly, and so the seam is fakeable in
/// tests via `FakeFileSearchPort`.
pub struct SearchWorkspaceUseCase<Port: FileSearchPort> {
    port: Arc<Port>,
}

impl<Port: FileSearchPort> SearchWorkspaceUseCase<Port> {
    pub fn new(port: Arc<Port>) -> Self {
        Self { port }
    }

    pub async fn index_files(&self, root: PathBuf, include_ignored: bool) -> Vec<PathBuf> {
        self.port.walk_files(root, include_ignored).await
    }

    pub async fn search_content(&self, root: PathBuf, query: String, include_ignored: bool) -> Vec<SearchMatch> {
        self.port.grep_content(root, query, include_ignored).await
    }

    pub async fn preview_highlight(&self, path: PathBuf, contents: String) -> Option<HighlightedLines> {
        self.port.highlight(path, contents).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeFileSearchPort;

    #[tokio::test]
    async fn index_files_delegates_to_the_port() {
        let port = Arc::new(FakeFileSearchPort::with_files(vec![PathBuf::from("a.rs")]));
        let use_case = SearchWorkspaceUseCase::new(Arc::clone(&port));

        let files = use_case.index_files(PathBuf::from("/root"), false).await;

        assert_eq!(files, vec![PathBuf::from("a.rs")]);
        assert_eq!(port.walk_calls(), vec![(PathBuf::from("/root"), false)]);
    }

    #[tokio::test]
    async fn search_content_delegates_to_the_port() {
        let port = Arc::new(FakeFileSearchPort::with_matches(vec![SearchMatch {
            path: PathBuf::from("a.rs"),
            line: Some(1),
            line_text: Some("needle".to_string()),
        }]));
        let use_case = SearchWorkspaceUseCase::new(Arc::clone(&port));

        let matches = use_case.search_content(PathBuf::from("/root"), "needle".to_string(), false).await;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line, Some(1));
    }

    #[tokio::test]
    async fn preview_highlight_delegates_to_the_port() {
        let port = Arc::new(FakeFileSearchPort::with_highlight(Some(vec![vec![(
            "fn".to_string(),
            (255, 0, 0),
        )]])));
        let use_case = SearchWorkspaceUseCase::new(port);

        let highlighted = use_case.preview_highlight(PathBuf::from("a.rs"), "fn main() {}".to_string()).await;

        assert!(highlighted.is_some());
    }
}
