use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use tui_term::widget::PseudoTerminal;

use crate::app::{AppState, Focus, RuntimeStatus, Selection};

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
        Some(RuntimeStatus::Waiting) => " ◆ waiting",
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

    if let Some(selection) = app.selection {
        if selection.session_id == session_id {
            highlight_selection(f, area, selection);
        }
    }
}

/// Paints the `REVERSED` modifier over the buffer cells covered by a
/// selection, in the same row-major (start-row/start-col .. end-row/end-col)
/// shape `vt100::Screen::contents_between` reads text out of — so what's
/// highlighted always matches what gets copied.
fn highlight_selection(f: &mut Frame, area: Rect, selection: Selection) {
    // One cell of border on every side — matches `layout::pty_content_size`.
    let content = Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    if content.width == 0 || content.height == 0 {
        return;
    }
    let ((start_row, start_col), (end_row, end_col)) = selection.ordered();
    let buffer = f.buffer_mut();
    for row in start_row..=end_row {
        if row >= content.height {
            break;
        }
        let (col_start, col_end) = if start_row == end_row {
            (start_col, end_col)
        } else if row == start_row {
            (start_col, content.width)
        } else if row == end_row {
            (0, end_col)
        } else {
            (0, content.width)
        };
        for col in col_start..col_end.min(content.width) {
            if let Some(cell) = buffer.cell_mut((content.x + col, content.y + row)) {
                cell.set_style(Style::default().add_modifier(Modifier::REVERSED));
            }
        }
    }
}
