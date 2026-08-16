use ratatui::layout::{Constraint, Direction, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::fuzzy_finder::{FinderFocus, FinderMode, FuzzyFinderState};
use crate::i18n::t;
use crate::icons;
use crate::ui::border::Border;
use crate::ui::hitmap::HitMap;
use crate::ui::overlay::{dim_backdrop, BORDER, HINT, KEY, SURFACE_BG as BG, TITLE_BG};
use crate::ui::scroll;

const MATCH: Color = Color::Rgb(224, 175, 104);
const MARKED: Color = Color::Rgb(158, 206, 106);
/// Border color for whichever pane (results or preview) currently has
/// `Tab`-focus — distinct from the popup's own `BORDER` (cyan) so the two
/// don't read as the same thing.
const PANE_FOCUS: Color = Color::Rgb(122, 162, 247);
/// Background behind the content-mode match's whole line in the preview.
const MATCH_LINE_BG: Color = Color::Rgb(50, 56, 43);

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width)]).flex(Flex::Center).areas(area);
    let [area] = Layout::vertical([Constraint::Length(height)]).flex(Flex::Center).areas(area);
    area
}

pub fn draw(f: &mut Frame, full: Rect, finder: &FuzzyFinderState, hitmap: &mut HitMap) {
    let width = (full.width.saturating_mul(9) / 10).clamp(40, full.width.saturating_sub(2));
    let height = (full.height.saturating_mul(4) / 5).clamp(16, full.height.saturating_sub(2));
    let popup = centered(full, width, height);

    dim_backdrop(f, full, popup);
    f.render_widget(Clear, popup);
    let inner = Border::solid(BORDER).bg(BG).render(f, popup);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // query line
            Constraint::Length(1), // divider
            Constraint::Min(3),    // list | preview
            Constraint::Length(1), // footer
        ])
        .split(inner);

    draw_query_line(f, rows[0], finder);
    f.render_widget(
        Paragraph::new("─".repeat(rows[1].width as usize)).style(Style::default().fg(BORDER).bg(BG)),
        rows[1],
    );

    // Each pane draws its own border (colored by focus) with no separate
    // divider column — the two borders sitting side by side form the split.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(rows[2]);

    draw_results(f, cols[0], finder);
    draw_preview(f, cols[1], finder, hitmap);

    draw_footer(f, rows[3], finder);
}

fn draw_query_line(f: &mut Frame, area: Rect, finder: &FuzzyFinderState) {
    let mode_label = match finder.mode {
        FinderMode::Files => t("finder.query.mode_files", &[]),
        FinderMode::Content => t("finder.query.mode_content", &[]),
    };
    let ignored_label = if finder.show_ignored { t("finder.query.showing_all", &[]) } else { String::new() };
    let prompt = format!(" [{mode_label}{ignored_label}] {}_", finder.query);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            prompt,
            Style::default().fg(Color::White).bg(TITLE_BG).add_modifier(Modifier::BOLD),
        )))
        .style(Style::default().bg(TITLE_BG)),
        area,
    );
}

fn draw_results(f: &mut Frame, area: Rect, finder: &FuzzyFinderState) {
    let border_color = if finder.focus == FinderFocus::Results { PANE_FOCUS } else { Color::DarkGray };
    let inner = Border::solid(border_color).bg(BG).render(f, area);

    let (visible, offset, visible_selected) = scroll::window(&finder.results, finder.selected, inner.height as usize);

    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let selected = i == visible_selected;
            let marked = finder.marked.contains(&m.path);
            let mut spans = vec![
                Span::raw(if selected { "> " } else { "  " }),
                Span::styled(if marked { "● " } else { "  " }, Style::default().fg(MARKED)),
            ];

            match finder.mode {
                FinderMode::Files => {
                    let display = m.path.to_string_lossy().into_owned();
                    let (icon, color) = icons::for_file(&display);
                    spans.push(Span::styled(format!("{icon} "), Style::default().fg(color)));
                    spans.extend(highlighted(&display, &m.indices, selected));
                }
                FinderMode::Content => {
                    let path = m.path.to_string_lossy();
                    let line = m.line.unwrap_or(0);
                    let text = m.line_text.as_deref().unwrap_or("").trim();
                    spans.push(Span::styled(
                        format!("{path}:{line}  "),
                        Style::default().fg(Color::Rgb(122, 162, 247)),
                    ));
                    spans.push(Span::styled(
                        text.to_string(),
                        if selected { Style::default().add_modifier(Modifier::BOLD) } else { Style::default() },
                    ));
                }
            }

            let base = if selected { Style::default().bg(Color::DarkGray) } else { Style::default() };
            ListItem::new(Line::from(spans)).style(base)
        })
        .collect();
    let _ = offset;

    let list = if items.is_empty() {
        let placeholder = if finder.query.is_empty() {
            t("finder.results.placeholder_type_to_search", &[])
        } else {
            t("finder.results.placeholder_no_results", &[])
        };
        List::new(vec![ListItem::new(Span::styled(placeholder, Style::default().fg(HINT)))])
    } else {
        List::new(items)
    };
    f.render_widget(list, inner);
}

/// Splits `text` into spans, bolding + coloring the characters at `indices`
/// (char positions from the fuzzy matcher) so the matched letters stand out
/// against the rest of the path.
fn highlighted(text: &str, indices: &[usize], selected: bool) -> Vec<Span<'static>> {
    if indices.is_empty() {
        let style = if selected { Style::default().add_modifier(Modifier::BOLD) } else { Style::default() };
        return vec![Span::styled(text.to_string(), style)];
    }
    let base_style = if selected { Style::default().add_modifier(Modifier::BOLD) } else { Style::default() };
    let match_style = Style::default().fg(MATCH).add_modifier(Modifier::BOLD);
    let mut spans = Vec::new();
    let mut buf = String::new();
    for (i, c) in text.chars().enumerate() {
        if indices.contains(&i) {
            if !buf.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut buf), base_style));
            }
            spans.push(Span::styled(c.to_string(), match_style));
        } else {
            buf.push(c);
        }
    }
    if !buf.is_empty() {
        spans.push(Span::styled(buf, base_style));
    }
    spans
}

/// How many visual (wrapped) rows a line of display-`width` cells takes up
/// once `Paragraph` wraps it at `wrap_width` columns — `Paragraph::scroll`'s
/// y offset is counted in these, not in logical `Line`s, so the preview's
/// scroll math has to work in the same unit or it undercounts how far down a
/// match sits behind any earlier line that wraps. A width-0 (empty) line
/// still occupies one row, same as ratatui's own word-wrapper.
fn line_visual_rows(width: usize, wrap_width: u16) -> i32 {
    (width.max(1) as u16).div_ceil(wrap_width.max(1)) as i32
}

/// Finds the char range of `query` inside `haystack`, accent- and case-
/// insensitively in both directions — same folding rule as the grep itself
/// (`RipgrepSearchAdapter::accent_insensitive_pattern`), so the highlighted
/// span always corresponds to what the search actually matched. Both sides
/// are normalized to lowercase ASCII-diacritic-stripped text first; since
/// `strip_diacritics` preserves char count 1:1, a byte offset found in the
/// normalized haystack converts to the same char index in the original.
fn find_match_char_range(query: &str, haystack: &str) -> Option<(usize, usize)> {
    if query.trim().is_empty() {
        return None;
    }
    let normalized_haystack = argus_domain::strip_diacritics(haystack).to_ascii_lowercase();
    let normalized_query = argus_domain::strip_diacritics(query).to_ascii_lowercase();
    let byte_start = normalized_haystack.find(&normalized_query)?;
    let char_start = normalized_haystack[..byte_start].chars().count();
    let char_len = normalized_query.chars().count();
    Some((char_start, char_start + char_len))
}

/// Style for the exact matched substring inside the preview — a solid block
/// that fully replaces whatever style the underlying text had (syntax fg,
/// the whole-line match background, …) rather than patching just the
/// foreground on top of it. The search match is the reason the preview
/// scrolled here in the first place, so it has to read as unambiguously more
/// important than syntax highlighting, not just differently colored from
/// it — a patched fg can end up close in hue/brightness to some syntax
/// token color and blend in instead of standing out.
fn match_range_style() -> Style {
    Style::default().bg(MATCH).fg(Color::Black).add_modifier(Modifier::BOLD)
}

/// Rewrites `line`'s spans so chars in `[start, end)` (char indices) get
/// [`match_range_style`] — a full style replacement, not a patch — splitting
/// spans at the boundary as needed. Mirrors `highlighted()`'s per-character
/// approach, but operates on an already-styled multi-span `Line` instead of
/// building spans from scratch against plain text.
fn highlight_match_range(line: &mut Line<'static>, start: usize, end: usize) {
    if start >= end {
        return;
    }
    let match_style = match_range_style();
    let mut new_spans = Vec::with_capacity(line.spans.len() + 2);
    let mut char_idx = 0usize;
    for span in std::mem::take(&mut line.spans) {
        let mut buf = String::new();
        let mut buf_style = span.style;
        for c in span.content.chars() {
            let this_style = if char_idx >= start && char_idx < end { match_style } else { span.style };
            if !buf.is_empty() && this_style != buf_style {
                new_spans.push(Span::styled(std::mem::take(&mut buf), buf_style));
            }
            buf_style = this_style;
            buf.push(c);
            char_idx += 1;
        }
        if !buf.is_empty() {
            new_spans.push(Span::styled(buf, buf_style));
        }
    }
    line.spans = new_spans;
}

fn draw_preview(f: &mut Frame, area: Rect, finder: &FuzzyFinderState, hitmap: &mut HitMap) {
    let title = finder
        .results
        .get(finder.selected)
        .map(|m| m.path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let border_color = if finder.focus == FinderFocus::Preview { PANE_FOCUS } else { Color::DarkGray };
    let inner = Border::solid(border_color)
        .title(Span::styled(format!(" {title} "), Style::default().fg(Color::DarkGray)))
        .bg(BG)
        .render(f, area);
    hitmap.finder_preview_area = inner;

    let mut lines: Vec<Line> = match &finder.preview_highlighted {
        Some(hl) => hl
            .iter()
            .map(|spans| {
                Line::from(
                    spans
                        .iter()
                        .map(|(text, (r, g, b))| Span::styled(text.clone(), Style::default().fg(Color::Rgb(*r, *g, *b))))
                        .collect::<Vec<_>>(),
                )
            })
            .collect(),
        None => finder.preview.as_deref().unwrap_or("").lines().map(|l| Line::from(l.to_string())).collect(),
    };

    // Content-mode match: highlight the whole matched line with a
    // background band, and center the auto-scroll on it. Files mode has no
    // line (a whole-file match), so both stay at the top (index 0).
    let match_line_idx = if finder.mode == FinderMode::Content {
        finder.results.get(finder.selected).and_then(|m| m.line).map(|line| line.saturating_sub(1) as usize)
    } else {
        None
    };
    if let Some(idx) = match_line_idx {
        if let Some(line) = lines.get_mut(idx) {
            if line.spans.is_empty() {
                line.spans.push(Span::raw(" ".repeat(inner.width as usize)));
            }
            for span in &mut line.spans {
                span.style = span.style.bg(MATCH_LINE_BG);
            }
            // On top of the whole-line background, also pick out the exact
            // matched substring — same treatment as Files mode's per-
            // character highlighting, but found fresh against this line's
            // actual text since content-search only hands back a line
            // number, not a column.
            let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            if let Some((start, end)) = find_match_char_range(&finder.query, &line_text) {
                highlight_match_range(line, start, end);
            }
        }
    }

    // `Paragraph::scroll`'s y offset counts *wrapped/visual* rows, not
    // logical lines in `lines` (see ratatui's `render_text`, which advances
    // its row counter once per `composer.next_line()` — one call per
    // post-wrap row). A logical line whose text is wider than the pane
    // wraps into several of those rows, so centering/clamping against a
    // logical line index undercounts how far down the real match sits
    // whenever an earlier line wraps — the match then renders below the
    // computed center, requiring extra manual scrolling to reach it. Convert
    // to visual rows here instead of assuming one row per line.
    let wrap_width = inner.width.max(1);
    let total_lines: i32 = lines.iter().map(|l| line_visual_rows(l.width(), wrap_width)).sum();
    let target_line: i32 = match match_line_idx {
        Some(idx) => lines.iter().take(idx).map(|l| line_visual_rows(l.width(), wrap_width)).sum(),
        None => 0,
    };
    let max_scroll = (total_lines - inner.height as i32).max(0);
    // Manual scrolling (`preview_scroll`) is applied on top of the
    // *already-clamped* auto-centered position, not the raw centering math —
    // otherwise, whenever the raw center falls outside the visible range
    // (e.g. the match is near the top of the file, or Files mode always
    // centers on line 0), the first several scroll steps would just cancel
    // out that off-screen offset instead of visibly moving anything.
    let raw_center = target_line - (inner.height as i32 / 2);
    let auto_scroll_y = raw_center.clamp(0, max_scroll);
    // Published for the next key/mouse-scroll event to clamp against
    // immediately (see `HitMap::finder_preview_offset_min/max`) instead of
    // letting the raw offset overshoot past an edge.
    hitmap.finder_preview_offset_min = -auto_scroll_y;
    hitmap.finder_preview_offset_max = max_scroll - auto_scroll_y;
    let scroll_y = (auto_scroll_y + finder.preview_scroll.offset()).clamp(0, max_scroll) as u16;

    f.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .scroll((scroll_y, 0))
            .style(Style::default().fg(Color::Gray)),
        inner,
    );
}

fn draw_footer(f: &mut Frame, area: Rect, finder: &FuzzyFinderState) {
    let key_style = Style::default().fg(KEY).add_modifier(Modifier::BOLD);
    let hint_style = Style::default().fg(HINT);
    let marked_hint = if finder.marked.is_empty() {
        String::new()
    } else {
        format!("  ·  {}", t("finder.footer.marked_count", &[("count", &finder.marked.len().to_string())]))
    };
    let spans = vec![
        Span::raw(" "),
        Span::styled("Enter", key_style),
        Span::styled(format!(" {}  ", t("finder.footer.confirm", &[])), hint_style),
        Span::styled("Tab", key_style),
        Span::styled(format!(" {}  ", t("finder.footer.focus", &[])), hint_style),
        Span::styled("Ctrl+Space", key_style),
        Span::styled(format!(" {}  ", t("finder.footer.mode", &[])), hint_style),
        Span::styled("Ctrl+T", key_style),
        Span::styled(format!(" {}  ", t("finder.footer.mark", &[])), hint_style),
        Span::styled("Ctrl+G", key_style),
        Span::styled(format!(" {}  ", t("finder.footer.ignored", &[])), hint_style),
        Span::styled("Esc", key_style),
        Span::styled(format!(" {}", t("finder.footer.cancel", &[])), hint_style),
        Span::styled(marked_hint, Style::default().fg(MARKED)),
    ];
    f.render_widget(Paragraph::new(Line::from(spans)).style(Style::default().bg(BG)), area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_visual_rows_counts_wrapped_rows_not_logical_lines() {
        assert_eq!(line_visual_rows(0, 80), 1, "an empty line still occupies one row");
        assert_eq!(line_visual_rows(80, 80), 1, "exactly one width's worth fits on one row");
        assert_eq!(line_visual_rows(81, 80), 2, "one cell over wraps into a second row");
        assert_eq!(line_visual_rows(250, 80), 4, "a long line spans ceil(250/80) rows");
    }

    #[test]
    fn find_match_char_range_is_accent_and_case_insensitive() {
        assert_eq!(find_match_char_range("nao", "isso não funciona"), Some((5, 8)));
        assert_eq!(find_match_char_range("NÃO", "isso nao funciona"), Some((5, 8)));
        assert_eq!(find_match_char_range("xyz", "no match here"), None);
    }

    #[test]
    fn highlight_match_range_splits_spans_and_fully_overrides_the_matched_range() {
        let mut line = Line::from(vec![Span::styled("hello world".to_string(), Style::default().bg(Color::Red))]);
        highlight_match_range(&mut line, 6, 11);
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(rendered, "hello world");
        // Text outside the match keeps whatever style it already had...
        assert_eq!(line.spans[0].style.bg, Some(Color::Red));
        // ...but the matched substring fully replaces it — bg and fg both —
        // so it reads as more important than any underlying style (syntax
        // color, the match-line background), not just differently colored.
        let matched = line.spans.last().unwrap();
        assert_eq!(matched.content.as_ref(), "world");
        assert_eq!(matched.style.bg, Some(MATCH));
        assert_eq!(matched.style.fg, Some(Color::Black));
    }
}
