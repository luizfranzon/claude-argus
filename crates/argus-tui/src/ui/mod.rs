pub mod hitmap;
pub mod layout;
pub mod modal;
pub mod notification;
pub mod scroll;
pub mod sidebar;
pub mod terminal;
pub mod topbar;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{AppState, Focus, SidebarTab};
pub use hitmap::HitMap;

pub fn draw(f: &mut Frame, app: &AppState) -> HitMap {
    let mut hitmap = HitMap::default();
    let full = f.area();
    let regions = layout::compute(full, app.sidebar_width);
    hitmap.terminal_area = regions.terminal;
    hitmap.full = full;

    topbar::draw(f, regions.topbar, app, &mut hitmap);
    sidebar::draw(f, regions.sidebar, app, &mut hitmap);
    terminal::draw(f, regions.terminal, app);
    draw_statusbar(f, regions.statusbar, app);

    if let Some(m) = &app.modal {
        modal::draw(f, f.area(), m);
    }

    let toasts: Vec<_> = app.notifications.visible().collect();
    notification::draw(f, f.area(), &toasts, app.hovered_notification, &mut hitmap);

    hitmap
}

fn draw_statusbar(f: &mut Frame, area: ratatui::layout::Rect, app: &AppState) {
    const TERMINAL_HINTS: &[(&str, &str)] = &[("Ctrl+B", "sidebar"), ("[ ]", "workspace")];
    const SIDEBAR_COMMON: &[(&str, &str)] = &[("1/2/3", "aba"), ("w", "novo workspace"), ("q", "sair")];
    const AGENTS_HINTS: &[(&str, &str)] = &[
        ("j/k", "navegar"),
        ("Enter", "foco terminal"),
        ("n", "nova sessão"),
        ("r", "renomear"),
        ("x", "fechar"),
    ];
    const EXPLORER_HINTS: &[(&str, &str)] = &[
        ("j/k", "navegar"),
        ("Enter/Space", "abrir/expandir"),
        ("a", "novo arquivo"),
        ("A", "nova pasta"),
        ("r", "renomear"),
        ("x", "excluir"),
    ];
    const GIT_HINTS: &[(&str, &str)] = &[
        ("←/→", "repo"),
        ("j/k", "arquivo"),
        ("Space", "stage/unstage"),
        ("c", "commit"),
        ("f", "fetch"),
        ("p/P", "pull/push"),
        ("b", "branch"),
        ("l", "log"),
    ];

    let hints: Vec<(&str, &str)> = match app.focus {
        Focus::Terminal => TERMINAL_HINTS.to_vec(),
        Focus::Sidebar => {
            let tab_hints = match app.active_entry().map(|w| w.sidebar_tab) {
                Some(SidebarTab::Agents) | None => AGENTS_HINTS,
                Some(SidebarTab::Explorer) => EXPLORER_HINTS,
                Some(SidebarTab::Git) => GIT_HINTS,
            };
            tab_hints.iter().chain(SIDEBAR_COMMON.iter()).copied().collect()
        }
    };

    let key_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(Color::DarkGray);

    let mut spans = Vec::with_capacity(hints.len() * 3);
    if !app.status_line.is_empty() {
        spans.push(Span::styled(app.status_line.clone(), label_style));
        spans.push(Span::styled("   |   ", label_style));
    }
    for (i, (key, label)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", label_style));
        }
        spans.push(Span::styled(*key, key_style));
        spans.push(Span::styled(format!(" {label}"), label_style));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
