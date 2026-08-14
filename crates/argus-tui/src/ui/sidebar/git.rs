use argus_application::ports::FileStatusKind;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::WorkspaceEntry;
use crate::ui::hitmap::HitMap;

/// Builds the key-hints row as styled spans (key bold-cyan, label dim-gray)
/// so `Paragraph::wrap` can flow it across as many lines as the sidebar's
/// current width actually needs, instead of one long unstyled string that
/// just got truncated at narrower widths.
fn hints_line(entry: &WorkspaceEntry) -> Line<'static> {
    if !entry.git.commit_message.is_empty() {
        return Line::from(vec![
            Span::raw("mensagem: "),
            Span::raw(entry.git.commit_message.clone()),
        ])
        .style(Style::default().fg(Color::DarkGray));
    }

    const HINTS: &[(&str, &str)] = &[
        ("␣", "stage"),
        ("c", "commit"),
        ("f", "fetch"),
        ("p", "pull"),
        ("P", "push"),
        ("b", "branch"),
        ("l", "log"),
    ];
    let key_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(Color::DarkGray);
    let mut spans = Vec::with_capacity(HINTS.len() * 3);
    for (key, label) in HINTS {
        spans.push(Span::styled(*key, key_style));
        spans.push(Span::styled(format!(" {label}  "), label_style));
    }
    Line::from(spans)
}

/// How many rows `Paragraph::wrap` will need to lay `line` out at `width`
/// columns, word-wrapped. `text.len() / width` is a lower bound (word
/// wrapping only ever uses *more* rows, never fewer, since a word that
/// doesn't fit the remaining space on a row wraps early) — the `+ 1` is
/// slack against that, cheaper than actually replicating the wrap algorithm
/// just to measure it.
fn wrapped_height(line: &Line, width: u16) -> u16 {
    let len: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
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
            Paragraph::new("git não está instalado").style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    }
    let Some(repo) = entry.git.active_repo() else {
        f.render_widget(
            Paragraph::new("não é um repositório git").style(Style::default().fg(Color::DarkGray)),
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
            Paragraph::new("sem mudanças").style(Style::default().fg(Color::DarkGray)),
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
            Paragraph::new("carregando histórico…").style(Style::default().fg(Color::DarkGray)),
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
