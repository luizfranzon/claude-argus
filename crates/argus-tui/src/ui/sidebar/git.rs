use argus_application::ports::FileStatusKind;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use unicode_width::UnicodeWidthStr;

use crate::app::WorkspaceEntry;
use crate::i18n::t;
use crate::ui::hitmap::HitMap;

/// Shows the pending commit message, if any (the only thing this row still
/// carries — the key hints now live exclusively in the bottom status bar).
fn hints_line(entry: &WorkspaceEntry) -> Line<'static> {
    if !entry.git.commit_message.is_empty() {
        return Line::from(vec![
            Span::raw(t("sidebar.git.message_label", &[])),
            Span::raw(entry.git.commit_message.clone()),
        ])
        .style(Style::default().fg(Color::DarkGray));
    }

    Line::default()
}

/// How many rows `Paragraph::wrap` will need to lay `line` out at `width`
/// columns, word-wrapped. `text.len() / width` is a lower bound (word
/// wrapping only ever uses *more* rows, never fewer, since a word that
/// doesn't fit the remaining space on a row wraps early) — the `+ 1` is
/// slack against that, cheaper than actually replicating the wrap algorithm
/// just to measure it.
///
/// Uses each span's *display width* (`unicode-width`), not its char count —
/// `entry.git.commit_message` is arbitrary user-authored text, and a CJK
/// commit message's characters each occupy 2 terminal columns, so counting
/// characters instead of columns would undercount the rows this line
/// actually needs to wrap into.
fn wrapped_height(line: &Line, width: u16) -> u16 {
    let len: usize = line.spans.iter().map(|s| s.content.width()).sum();
    let width = width.max(1) as usize;
    (len.div_ceil(width)).max(1) as u16 + 1
}

fn status_letter(kind: FileStatusKind) -> (&'static str, Color) {
    match kind {
        FileStatusKind::Modified => ("M", Color::Yellow),
        FileStatusKind::Added => ("A", Color::Green),
        FileStatusKind::Deleted => ("D", Color::Red),
        FileStatusKind::Renamed => ("R", Color::Magenta),
        FileStatusKind::Untracked => ("?", Color::DarkGray),
        FileStatusKind::Conflicted => ("!", Color::Red),
    }
}

pub fn draw(f: &mut Frame, area: Rect, entry: &WorkspaceEntry, hitmap: &mut HitMap) {
    if entry.git.available == Some(false) {
        f.render_widget(
            Paragraph::new(t("sidebar.git.not_installed", &[])).style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    }
    let Some(repo) = entry.git.active_repo() else {
        f.render_widget(
            Paragraph::new(t("sidebar.git.not_a_repo", &[])).style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    };

    let hints_line = hints_line(entry);
    // Hints wrap at word boundaries, so the row height needed depends on how
    // narrow the (now user-resizable) sidebar is — reserve just enough rows
    // instead of a fixed guess that either wastes space or clips the keys.
    let hints_height = wrapped_height(&hints_line, area.width).clamp(1, 4);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(hints_height), Constraint::Min(0)])
        .split(area);

    // header: repo name (with left/right hint if more than one) + branch + sync
    let repo_label = if entry.git.repos.len() > 1 {
        format!("←{}/{}→ {}", entry.git.selected_repo + 1, entry.git.repos.len(), repo.repo.name)
    } else {
        repo.repo.name.clone()
    };
    let branch = repo.branch.clone().unwrap_or_else(|| "?".to_string());
    let sync = repo
        .sync
        .map(|s| format!(" ↑{} ↓{}", s.ahead, s.behind))
        .unwrap_or_default();
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(repo_label, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  "),
            Span::styled(branch, Style::default().fg(Color::Cyan)),
            Span::styled(sync, Style::default().fg(Color::DarkGray)),
        ])),
        chunks[0],
    );

    f.render_widget(Paragraph::new(hints_line).wrap(Wrap { trim: true }), chunks[1]);

    if entry.git.show_log {
        draw_log(f, chunks[2], repo);
    } else {
        draw_status(f, chunks[2], entry, repo, hitmap);
    }
}

fn draw_status(
    f: &mut Frame,
    area: Rect,
    entry: &WorkspaceEntry,
    repo: &crate::app::GitRepoState,
    hitmap: &mut HitMap,
) {
    if repo.status.is_empty() {
        f.render_widget(
            Paragraph::new(t("sidebar.git.no_changes", &[])).style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    }
    let (visible, offset, visible_selected) =
        crate::ui::scroll::window(&repo.status, entry.git.selected_file, area.height as usize);
    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let selected = i == visible_selected;
            let row = Rect { x: area.x, y: area.y + i as u16, width: area.width, height: 1 };
            hitmap.git_rows.push((row, offset + i, file.path.clone(), file.staged));

            let (letter, color) = status_letter(file.kind);
            let staged_marker = if file.staged { "✓" } else { " " };
            ListItem::new(Line::from(vec![
                Span::raw(if selected { "> " } else { "  " }),
                Span::styled(staged_marker, Style::default().fg(Color::Green)),
                Span::raw(" "),
                Span::styled(letter, Style::default().fg(color)),
                Span::raw(" "),
                Span::raw(file.path.clone()),
            ]))
        })
        .collect();
    f.render_widget(List::new(items), area);
}

fn draw_log(f: &mut Frame, area: Rect, repo: &crate::app::GitRepoState) {
    if repo.log.is_empty() {
        f.render_widget(
            Paragraph::new(t("sidebar.git.loading_log", &[])).style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    }
    let items: Vec<ListItem> = repo
        .log
        .iter()
        .map(|commit| {
            ListItem::new(Line::from(vec![
                Span::styled(commit.short_hash.clone(), Style::default().fg(Color::Yellow)),
                Span::raw(" "),
                Span::raw(commit.summary.clone()),
            ]))
        })
        .collect();
    f.render_widget(List::new(items), area);
}
