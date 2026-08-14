pub mod hitmap;
pub mod layout;
pub mod modal;
pub mod scroll;
pub mod sidebar;
pub mod terminal;
pub mod topbar;

use ratatui::style::{Color, Style};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{AppState, Focus};
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

    hitmap
}

fn draw_statusbar(f: &mut Frame, area: ratatui::layout::Rect, app: &AppState) {
    let hint = match app.focus {
        Focus::Terminal => "Ctrl+B sidebar  ·  [ ] workspace",
        Focus::Sidebar => "1/2/3 aba  ·  Enter foco terminal  ·  n nova sessão  ·  w novo workspace  ·  q sair",
    };
    let text = if app.status_line.is_empty() {
        hint.to_string()
    } else {
        format!("{}   |   {hint}", app.status_line)
    };
    f.render_widget(Paragraph::new(text).style(Style::default().fg(Color::DarkGray)), area);
}
