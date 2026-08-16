use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use tui_term::widget::PseudoTerminal;

use crate::app::{AppState, Focus};
use crate::i18n::t;

pub fn draw(f: &mut Frame, area: Rect, app: &AppState) {
    let Some(session_id) = app.focused_session_id() else {
        // Zero sessions in the active workspace (e.g. the last one just got
        // closed) gets the animated mascot; no active workspace at all keeps
        // the plain placeholder below.
        if app.active_entry().is_some_and(|entry| entry.sessions.is_empty()) {
            let block = Block::default().borders(Borders::ALL);
            let inner = block.inner(area);
            f.render_widget(block, area);
            crate::ui::mascot::draw(f, inner);
            return;
        }
        let placeholder = Paragraph::new(t("terminal.placeholder.no_focused_session", &[]))
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(placeholder, area);
        return;
    };
    let Some(entry) = app.sessions.get(&session_id) else { return };

    let cursor = tui_term::widget::Cursor::default().visibility(app.focus == Focus::Terminal);
    let mut pseudo_term = PseudoTerminal::new(entry.parser.screen()).cursor(cursor);

    // Focus Mode strips the border/title for a fully clean, full-bleed pane
    // (session name + status move into the status line instead — see
    // `ui::draw_statusbar`).
    if !app.focus_mode {
        let status = crate::ui::session_status_text(entry.status);
        let border_color = if app.focus == Focus::Terminal { Color::Cyan } else { Color::DarkGray };
        let title = format!(" {}{} ", entry.session.name, status);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(title);
        pseudo_term = pseudo_term.block(block);
    }

    f.render_widget(pseudo_term, area);
}
