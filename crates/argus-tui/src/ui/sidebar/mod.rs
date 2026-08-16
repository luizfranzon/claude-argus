pub mod agents;
pub mod explorer;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{AppState, Focus, SidebarTab};
use crate::i18n::t;
use crate::ui::border::Border;
use crate::ui::hitmap::HitMap;
use crate::ui::layout::equal_columns;

pub fn draw(f: &mut Frame, area: Rect, app: &AppState, hitmap: &mut HitMap) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    draw_tabs(f, chunks[0], app, hitmap);

    let Some(entry) = app.active_entry() else {
        let placeholder = Paragraph::new(t("sidebar.placeholder.no_workspace", &[]))
            .style(Style::default().fg(Color::DarkGray))
            .block(Border::solid(Color::DarkGray).into_block());
        f.render_widget(placeholder, chunks[1]);
        return;
    };

    // Same focus convention as the terminal pane: the flowing blue gradient
    // marks whichever side currently has input focus, resizing gets its own
    // solid color since it's a distinct (temporary) state, and everything
    // else falls back to a flat gray.
    let inner = if app.resizing_sidebar {
        Border::solid(Color::Yellow).render(f, chunks[1])
    } else if app.focus == Focus::Sidebar {
        Border::blue().animated(true).render(f, chunks[1])
    } else {
        Border::solid(Color::DarkGray).render(f, chunks[1])
    };
    hitmap.sidebar_content_area = inner;

    match entry.sidebar_tab {
        SidebarTab::Agents => agents::draw(f, inner, app, entry, hitmap),
        SidebarTab::Explorer => explorer::draw(f, inner, app, entry, hitmap),
    }
}

fn draw_tabs(f: &mut Frame, area: Rect, app: &AppState, hitmap: &mut HitMap) {
    let active_tab = app.active_entry().map(|w| w.sidebar_tab);
    let labels = [
        (t("sidebar.tabs.agents", &[]), SidebarTab::Agents),
        (t("sidebar.tabs.explorer", &[]), SidebarTab::Explorer),
    ];
    let columns = equal_columns(area, labels.len());

    for (col, (label, tab)) in columns.iter().zip(labels.iter()) {
        let style = if active_tab == Some(*tab) {
            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        f.render_widget(Paragraph::new(Line::from(format!(" {label} "))).style(style), *col);
        hitmap.sidebar_tabs.push((*col, *tab));
    }
}
