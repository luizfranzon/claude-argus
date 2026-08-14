use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use tui_term::widget::PseudoTerminal;

use crate::app::{AppState, Focus, RuntimeStatus};

pub fn draw(f: &mut Frame, area: Rect, app: &AppState) {
    let Some(session_id) = app.focused_session_id() else {
        let placeholder = Paragraph::new("Nenhuma sessão em foco — crie uma na aba Agents (a)")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(placeholder, area);
        return;
    };
    let Some(entry) = app.sessions.get(&session_id) else { return };

    let status = match entry.status {
        Some(RuntimeStatus::Thinking) => " ● thinking",
        Some(RuntimeStatus::Idle) => " ○ idle",
        None => "",
    };
    let border_color = if app.focus == Focus::Terminal { Color::Cyan } else { Color::DarkGray };
    let title = format!(" {}{} ", entry.session.name, status);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(title);

    let cursor = tui_term::widget::Cursor::default().visibility(app.focus == Focus::Terminal);
    let pseudo_term = PseudoTerminal::new(entry.parser.screen()).block(block).cursor(cursor);
    f.render_widget(pseudo_term, area);
}
