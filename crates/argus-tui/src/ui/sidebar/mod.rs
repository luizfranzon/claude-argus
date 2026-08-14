pub mod agents;
pub mod explorer;
pub mod git;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{AppState, Focus, SidebarTab};
use crate::ui::hitmap::HitMap;
use crate::ui::layout::equal_columns;

pub fn draw(f: &mut Frame, area: Rect, app: &AppState, hitmap: &mut HitMap) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    draw_tabs(f, chunks[0], app, hitmap);

    let Some(entry) = app.active_entry() else {
        let placeholder = Paragraph::new("Sem workspace ativo — w para abrir um diretório")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(placeholder, chunks[1]);
        return;
    };

    let border_color = if app.resizing_sidebar {
        Color::Yellow
    } else if app.focus == Focus::Sidebar {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let block = Block::default().borders(Borders::ALL).border_style(Style::default().fg(border_color));
    let inner = block.inner(chunks[1]);
    f.render_widget(block, chunks[1]);

    match entry.sidebar_tab {
        SidebarTab::Agents => agents::draw(f, inner, app, entry, hitmap),
        SidebarTab::Explorer => explorer::draw(f, inner, app, entry, hitmap),
        SidebarTab::Git => git::draw(f, inner, entry, hitmap),
    }
}

fn draw_tabs(f: &mut Frame, area: Rect, app: &AppState, hitmap: &mut HitMap) {
    let active_tab = app.active_entry().map(|w| w.sidebar_tab);
    let labels = [("1 Agents", SidebarTab::Agents), ("2 Explorer", SidebarTab::Explorer), ("3 Git", SidebarTab::Git)];
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
