use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use argus_application::ports::{FileSearchPort, HighlightedLines, SearchMatch};
use async_trait::async_trait;

/// Caps how many content-search matches a single grep returns — keeps a
/// huge workspace from producing an unbounded result list.
pub const RESULT_LIMIT: usize = 200;

/// Longest single line highlighting will attempt to color. Minified/
/// generated files routinely pack an entire file onto one line, and
/// syntect's regex-based highlighter (backed by `fancy-regex`, which
/// supports backtracking) can be pathologically slow against that — so past
/// this length highlighting is skipped for the whole file rather than risk
/// a long-running background task.
const MAX_HIGHLIGHT_LINE_LEN: usize = 500;

/// `FileSearchPort` backed by the `ignore`/`grep-*`/`syntect` crates — the
/// same libraries ripgrep itself is built on. Every method is synchronous,
/// CPU/IO-bound work; each dispatches to the blocking thread pool so it
/// never stalls a tokio worker.
#[derive(Default)]
pub struct RipgrepSearchAdapter;

impl RipgrepSearchAdapter {
    pub fn new() -> Self {
        Self
    }
}

/// One `ignore::WalkBuilder` policy shared by both walking and grepping:
/// skip `.git` unconditionally, and toggle the standard gitignore/hidden
/// filters by `include_ignored`.
fn workspace_walk_builder(root: &Path, include_ignored: bool) -> ignore::WalkBuilder {
    let mut builder = ignore::WalkBuilder::new(root);
    builder.standard_filters(!include_ignored).hidden(false);
    builder.filter_entry(|entry| entry.file_name() != ".git");
    builder
}

fn walk_files_blocking(root: &Path, include_ignored: bool) -> Vec<PathBuf> {
    let builder = workspace_walk_builder(root, include_ignored);
    let mut out = Vec::new();
    for entry in builder.build().filter_map(Result::ok) {
        if entry.file_type().is_some_and(|t| t.is_file()) {
            if let Ok(rel) = entry.path().strip_prefix(root) {
                out.push(rel.to_path_buf());
            }
        }
    }
    out
}

/// Regex character class (no brackets) covering every case/diacritic variant
/// of `base` for the Latin vowels plus `c`/`n`/`y` — the letters that
/// commonly carry a diacritic in Portuguese and other Latin European
/// languages. Kept in sync with `argus_domain::text::strip_diacritics`,
/// which reduces any of these variants back down to `base`.
fn accent_class(base: char) -> Option<&'static str> {
    match base {
        'a' | 'A' => Some("aàáâãäåāăąAÀÁÂÃÄÅĀĂĄ"),
        'e' | 'E' => Some("eèéêëēĕėęěEÈÉÊËĒĔĖĘĚ"),
        'i' | 'I' => Some("iìíîïĩīĭįıIÌÍÎÏĨĪĬĮİ"),
        'o' | 'O' => Some("oòóôõöøōŏőOÒÓÔÕÖØŌŎŐ"),
        'u' | 'U' => Some("uùúûüũūŭůűųUÙÚÛÜŨŪŬŮŰŲ"),
        'c' | 'C' => Some("cçćĉċčCÇĆĈĊČ"),
        'n' | 'N' => Some("nñńņňŉNÑŃŅŇ"),
        'y' | 'Y' => Some("yýÿŷYÝŸŶ"),
        _ => None,
    }
}

/// Builds a regex pattern that matches `query` literally but accent-
/// insensitively in both directions ("nao" finds "não" and vice versa):
/// every character is first reduced to its plain-ASCII base via
/// `strip_diacritics`, then either expanded into a bracket class of all its
/// case/diacritic variants (for the letters `accent_class` covers) or
/// escaped and matched as-is. Content search is "search for this text", not
/// a regex prompt, so a stray `(`/`.`/`*` mid-typing must still match
/// literally rather than turn into an error or a surprising match.
fn accent_insensitive_pattern(query: &str) -> String {
    let normalized = argus_domain::strip_diacritics(query);
    let mut out = String::with_capacity(query.len() * 2);
    for c in normalized.chars() {
        if let Some(variants) = accent_class(c) {
            out.push('[');
            out.push_str(variants);
            out.push(']');
        } else {
            if "\\.+*?()|[]{}^$".contains(c) {
                out.push('\\');
            }
            out.push(c);
        }
    }
    out
}

fn grep_content_blocking(root: &Path, query: &str, include_ignored: bool) -> Vec<SearchMatch> {
    use grep_regex::RegexMatcherBuilder;
    use grep_searcher::sinks::UTF8;
    use grep_searcher::SearcherBuilder;

    let mut results = Vec::new();
    if query.trim().is_empty() {
        return results;
    }
    let Ok(matcher) = RegexMatcherBuilder::new().case_insensitive(true).build(&accent_insensitive_pattern(query)) else {
        return results;
    };

    let builder = workspace_walk_builder(root, include_ignored);

    'walk: for entry in builder.build().filter_map(Result::ok) {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(root) else { continue };
        let mut searcher = SearcherBuilder::new().line_number(true).build();
        let rel = rel.to_path_buf();
        let search_result = searcher.search_path(
            &matcher,
            entry.path(),
            UTF8(|lnum, line| {
                results.push(SearchMatch {
                    path: rel.clone(),
                    line: Some(lnum),
                    line_text: Some(line.trim_end().to_string()),
                });
                Ok(results.len() < RESULT_LIMIT)
            }),
        );
        let _ = search_result;
        if results.len() >= RESULT_LIMIT {
            break 'walk;
        }
    }
    results
}

fn syntax_set() -> &'static syntect::parsing::SyntaxSet {
    static SYNTAX_SET: OnceLock<syntect::parsing::SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(syntect::parsing::SyntaxSet::load_defaults_newlines)
}

fn theme() -> &'static syntect::highlighting::Theme {
    static THEME: OnceLock<syntect::highlighting::Theme> = OnceLock::new();
    THEME.get_or_init(|| {
        syntect::highlighting::ThemeSet::load_defaults()
            .themes
            .remove("base16-ocean.dark")
            .expect("syntect bundles base16-ocean.dark by default")
    })
}

fn highlight_lines_blocking(path: &Path, contents: &str) -> Option<HighlightedLines> {
    use syntect::easy::HighlightLines;
    use syntect::util::LinesWithEndings;

    if contents.lines().any(|line| line.len() > MAX_HIGHLIGHT_LINE_LEN) {
        return None;
    }

    let ss = syntax_set();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let syntax = ss
        .find_syntax_by_extension(ext)
        .or_else(|| ss.find_syntax_by_first_line(contents))?;

    let mut highlighter = HighlightLines::new(syntax, theme());
    let mut out = Vec::new();
    for line in LinesWithEndings::from(contents) {
        let ranges = highlighter.highlight_line(line, ss).ok()?;
        let spans = ranges
            .into_iter()
            .map(|(style, text)| {
                let text = text.trim_end_matches(['\n', '\r']).to_string();
                (text, (style.foreground.r, style.foreground.g, style.foreground.b))
            })
            .collect();
        out.push(spans);
    }
    Some(out)
}

#[async_trait]
impl FileSearchPort for RipgrepSearchAdapter {
    async fn walk_files(&self, root: PathBuf, include_ignored: bool) -> Vec<PathBuf> {
        tokio::task::spawn_blocking(move || walk_files_blocking(&root, include_ignored))
            .await
            .unwrap_or_default()
    }

    async fn grep_content(&self, root: PathBuf, query: String, include_ignored: bool) -> Vec<SearchMatch> {
        tokio::task::spawn_blocking(move || grep_content_blocking(&root, &query, include_ignored))
            .await
            .unwrap_or_default()
    }

    async fn highlight(&self, path: PathBuf, contents: String) -> Option<HighlightedLines> {
        tokio::task::spawn_blocking(move || highlight_lines_blocking(&path, &contents))
            .await
            .unwrap_or(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn grep_content_finds_matching_line_with_number() {
        let dir = std::env::temp_dir().join(format!("argus-search-adapter-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("needle.txt"), "first line\nhas a needle here\nlast line\n").unwrap();

        let adapter = RipgrepSearchAdapter::new();
        let results = adapter.grep_content(dir.clone(), "needle".to_string(), false).await;

        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, PathBuf::from("needle.txt"));
        assert_eq!(results[0].line, Some(2));
        assert_eq!(results[0].line_text.as_deref(), Some("has a needle here"));
    }

    #[tokio::test]
    async fn walk_files_skips_git_directory() {
        let dir = std::env::temp_dir().join(format!("argus-search-adapter-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        std::fs::write(dir.join(".git").join("HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(dir.join("real.rs"), "fn main() {}\n").unwrap();

        let adapter = RipgrepSearchAdapter::new();
        let files = adapter.walk_files(dir.clone(), true).await;

        std::fs::remove_dir_all(&dir).ok();

        assert!(files.contains(&PathBuf::from("real.rs")));
        assert!(!files.iter().any(|p| p.starts_with(".git")));
    }

    #[tokio::test]
    async fn highlight_colors_known_extension() {
        let adapter = RipgrepSearchAdapter::new();
        let lines = adapter
            .highlight(PathBuf::from("main.rs"), "fn main() {}\n".to_string())
            .await
            .expect("rust syntax should be bundled");
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].is_empty());
        assert!(lines[0].iter().any(|(text, _)| text.contains("fn")));
    }

    #[tokio::test]
    async fn highlight_returns_none_for_unknown_extension() {
        let adapter = RipgrepSearchAdapter::new();
        let result = adapter
            .highlight(PathBuf::from("data.unknownext12345"), "gibberish content\n".to_string())
            .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn highlight_skips_pathologically_long_lines() {
        let minified = format!("fn main() {{ {} }}\n", "x".repeat(MAX_HIGHLIGHT_LINE_LEN + 1));
        let adapter = RipgrepSearchAdapter::new();
        let result = adapter.highlight(PathBuf::from("bundle.rs"), minified).await;
        assert!(result.is_none());
    }

    #[test]
    fn accent_insensitive_pattern_neutralizes_metacharacters() {
        assert_eq!(accent_insensitive_pattern("a.b*c"), "[aàáâãäåāăąAÀÁÂÃÄÅĀĂĄ]\\.b\\*[cçćĉċčCÇĆĈĊČ]");
        assert_eq!(accent_insensitive_pattern("xyz"), "x[yýÿŷYÝŸŶ]z");
    }

    #[tokio::test]
    async fn grep_content_ignores_accents_in_both_directions() {
        let dir = std::env::temp_dir().join(format!("argus-search-adapter-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "isso não funciona\n").unwrap();
        std::fs::write(dir.join("b.txt"), "isso nao devia falhar\n").unwrap();

        let adapter = RipgrepSearchAdapter::new();
        let unaccented_query = adapter.grep_content(dir.clone(), "nao".to_string(), false).await;
        let accented_query = adapter.grep_content(dir.clone(), "não".to_string(), false).await;

        std::fs::remove_dir_all(&dir).ok();

        assert_eq!(unaccented_query.len(), 2, "query without accent should match both files");
        assert_eq!(accented_query.len(), 2, "query with accent should match both files");
    }
}
