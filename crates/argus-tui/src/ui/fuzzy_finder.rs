use ratatui::layout::{Constraint, Direction, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::fuzzy_finder::{FinderMode, FuzzyFinderState};
use crate::i18n::t;
use crate::icons;
use crate::ui::overlay::{dim_backdrop, BORDER, HINT, KEY, SURFACE_BG as BG, TITLE_BG};
use crate::ui::scroll;

const MATCH: Color = Color::Rgb(224, 175, 104);
const MARKED: Color = Color::Rgb(158, 206, 106);

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width)]).flex(Flex::Center).areas(area);
    let [area] = Layout::vertical([Constraint::Length(height)]).flex(Flex::Center).areas(area);
    area
}

pub fn draw(f: &mut Frame, full: Rect, finder: &FuzzyFinderState) {
    let width = (full.width.saturating_mul(9) / 10).clamp(40, full.width.saturating_sub(2));
    let height = (full.height.saturating_mul(4) / 5).clamp(16, full.height.saturating_sub(2));
    let popup = centered(full, width, height);

    dim_backdrop(f, full, popup);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(BG));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

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

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Length(1), Constraint::Percentage(45)])
        .split(rows[2]);

    draw_results(f, cols[0], finder);
    f.render_widget(
        Paragraph::new("│\n".repeat(cols[1].height as usize)).style(Style::default().fg(BORDER).bg(BG)),
        cols[1],
    );
    draw_preview(f, cols[2], finder);

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
    let (visible, offset, visible_selected) = scroll::window(&finder.results, finder.selected, area.height as usize);

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
    f.render_widget(list, area);
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

fn draw_preview(f: &mut Frame, area: Rect, finder: &FuzzyFinderState) {
    let title = finder
        .results
        .get(finder.selected)
        .map(|m| m.path.to_string_lossy().into_owned())
        .unwrap_or_default();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(format!(" {title} "), Style::default().fg(Color::DarkGray)))
        .style(Style::default().bg(BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let target_line = finder.results.get(finder.selected).and_then(|m| m.line).unwrap_or(0) as u16;
    let scroll_y = target_line.saturating_sub(inner.height / 2);

    let text: Text = match &finder.preview_highlighted {
        Some(lines) => Text::from(
            lines
                .iter()
                .map(|spans| {
                    Line::from(
                        spans
                            .iter()
                            .map(|(text, (r, g, b))| Span::styled(text.clone(), Style::default().fg(Color::Rgb(*r, *g, *b))))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>(),
        ),
        None => Text::raw(finder.preview.as_deref().unwrap_or("")),
    };

    f.render_widget(
        Paragraph::new(text).wrap(Wrap { trim: false }).scroll((scroll_y, 0)).style(Style::default().fg(Color::Gray)),
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
