pub mod blink;
pub mod fuzzy_finder;
pub mod hitmap;
pub mod layout;
pub mod mascot;
pub mod modal;
pub mod notification;
pub mod overlay;
pub mod scroll;
pub mod sidebar;
pub mod terminal;
pub mod topbar;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{AppState, Focus, RuntimeStatus, SidebarTab};
use crate::i18n::t;
pub use hitmap::HitMap;

pub fn draw(f: &mut Frame, app: &AppState) -> HitMap {
    let mut hitmap = HitMap::default();
    let full = f.area();
    let regions = layout::compute(full, app.sidebar_width, app.focus_mode);
    hitmap.terminal_area = regions.terminal;
    hitmap.full = full;

    if !app.focus_mode {
        topbar::draw(f, regions.topbar, app, &mut hitmap);
        sidebar::draw(f, regions.sidebar, app, &mut hitmap);
    }
    terminal::draw(f, regions.terminal, app);
    draw_statusbar(f, regions.statusbar, app);

    if let Some(m) = &app.modal {
        modal::draw(f, f.area(), m);
    }

    if let Some(finder) = &app.fuzzy_finder {
        fuzzy_finder::draw(f, f.area(), finder);
    }

    let toasts: Vec<_> = app.notifications.visible().collect();
    notification::draw(f, f.area(), &toasts, app.hovered_notification, &mut hitmap);

    hitmap
}

/// Session status glyph + translated label, shared by the Focus Mode status
/// line here and the terminal pane's own title (`ui::terminal::draw`).
pub fn session_status_text(status: Option<RuntimeStatus>) -> String {
    match status {
        Some(RuntimeStatus::Thinking) => format!(" ● {}", t("session.status.thinking", &[])),
        Some(RuntimeStatus::Idle) => format!(" ○ {}", t("session.status.idle", &[])),
        Some(RuntimeStatus::Waiting) => format!(" ◆ {}", t("session.status.waiting", &[])),
        None => String::new(),
    }
}

fn draw_statusbar(f: &mut Frame, area: ratatui::layout::Rect, app: &AppState) {
    // Each context's hints are pre-grouped into two rows by function, rather
    // than split mechanically, so related keys (navigation vs. destructive
    // actions vs. global workspace controls) land on the same line.
    let terminal_row1 = vec![("Ctrl+B", t("statusbar.terminal.sidebar", &[])), ("[ ]", t("statusbar.terminal.workspace", &[]))];
    let terminal_row2: Vec<(&str, String)> = Vec::new();

    let agents_row1 = vec![
        ("j/k", t("statusbar.agents.navigate", &[])),
        ("Enter", t("statusbar.agents.focus_terminal", &[])),
        ("n", t("statusbar.agents.new_session", &[])),
        ("1/2/3", t("statusbar.agents.tab", &[])),
    ];
    let agents_row2 = vec![
        ("r", t("statusbar.agents.rename", &[])),
        ("x", t("statusbar.agents.close", &[])),
        ("w", t("statusbar.agents.new_workspace", &[])),
        ("W", t("statusbar.agents.close_workspace", &[])),
        ("q", t("statusbar.agents.quit", &[])),
    ];

    let explorer_row1 = vec![
        ("j/k", t("statusbar.explorer.navigate", &[])),
        ("Enter", t("statusbar.explorer.insert_path", &[])),
        ("Space", t("statusbar.explorer.toggle_expand", &[])),
        ("Ctrl+F", t("statusbar.explorer.find_files", &[])),
        ("1/2/3", t("statusbar.explorer.tab", &[])),
    ];
    let explorer_row2 = vec![
        ("a", t("statusbar.explorer.new_file", &[])),
        ("A", t("statusbar.explorer.new_folder", &[])),
        ("r", t("statusbar.explorer.rename", &[])),
        ("x", t("statusbar.explorer.delete", &[])),
        ("w", t("statusbar.explorer.new_workspace", &[])),
        ("W", t("statusbar.explorer.close_workspace", &[])),
        ("q", t("statusbar.explorer.quit", &[])),
    ];

    let git_row1 = vec![
        ("←/→", t("statusbar.git.repo", &[])),
        ("j/k", t("statusbar.git.file", &[])),
        ("Space", t("statusbar.git.stage_toggle", &[])),
        ("c", t("statusbar.git.commit", &[])),
        ("b", t("statusbar.git.branch", &[])),
        ("1/2/3", t("statusbar.git.tab", &[])),
    ];
    let git_row2 = vec![
        ("f", t("statusbar.git.fetch", &[])),
        ("p/P", t("statusbar.git.pull_push", &[])),
        ("l", t("statusbar.git.log", &[])),
        ("w", t("statusbar.git.new_workspace", &[])),
        ("W", t("statusbar.git.close_workspace", &[])),
        ("q", t("statusbar.git.quit", &[])),
    ];

    let key_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(Color::DarkGray);

    if app.focus_mode {
        let mut spans = vec![Span::raw(" "), Span::styled(t("statusbar.focus_mode.label", &[]), key_style)];
        if let Some(entry) = app.focused_session_id().and_then(|id| app.sessions.get(&id)) {
            let status = session_status_text(entry.status);
            spans.push(Span::styled("   |   ", label_style));
            spans.push(Span::styled(format!("{}{}", entry.session.name, status), label_style));
        }
        spans.push(Span::styled("   |   ", label_style));
        spans.push(Span::styled("F8", key_style));
        spans.push(Span::styled(format!(" {}", t("statusbar.focus_mode.exit", &[])), label_style));
        spans.push(Span::raw(" "));
        f.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }

    let (row1, row2): (&[(&str, String)], &[(&str, String)]) = match app.focus {
        Focus::Terminal => (&terminal_row1, &terminal_row2),
        Focus::Sidebar => match app.active_entry().map(|w| w.sidebar_tab) {
            Some(SidebarTab::Agents) | None => (&agents_row1, &agents_row2),
            Some(SidebarTab::Explorer) => (&explorer_row1, &explorer_row2),
            Some(SidebarTab::Git) => (&git_row1, &git_row2),
        },
    };

    let rows = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([ratatui::layout::Constraint::Length(1), ratatui::layout::Constraint::Length(1)])
        .split(area);

    let mut line1_spans = Vec::with_capacity(row1.len() * 3 + 2);
    line1_spans.push(Span::raw(" "));
    if !app.status_line.is_empty() {
        line1_spans.push(Span::styled(app.status_line.clone(), label_style));
        line1_spans.push(Span::styled("   |   ", label_style));
    }
    push_hints(&mut line1_spans, row1, key_style, label_style);
    f.render_widget(Paragraph::new(Line::from(line1_spans)), rows[0]);

    let mut line2_spans = Vec::with_capacity(row2.len() * 3 + 2);
    line2_spans.push(Span::raw(" "));
    push_hints(&mut line2_spans, row2, key_style, label_style);
    f.render_widget(Paragraph::new(Line::from(line2_spans)), rows[1]);
}

fn push_hints<'a>(spans: &mut Vec<Span<'a>>, hints: &[(&'a str, String)], key_style: Style, label_style: Style) {
    for (i, (key, label)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", label_style));
        }
        spans.push(Span::styled(*key, key_style));
        spans.push(Span::styled(format!(" {label}"), label_style));
    }
}
