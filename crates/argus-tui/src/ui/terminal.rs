use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use tui_term::widget::PseudoTerminal;

use crate::app::{AppState, Focus};
use crate::i18n::t;
use crate::ui::border::Border;

pub fn draw(f: &mut Frame, area: Rect, app: &AppState) {
    let Some(session_id) = app.focused_session_id() else {
        // Zero sessions in the active workspace (e.g. the last one just got
        // closed) gets the animated mascot; no active workspace at all keeps
        // the plain placeholder below.
        if app.active_entry().is_some_and(|entry| entry.sessions.is_empty()) {
            let inner = Border::solid(Color::DarkGray).render(f, area);
            crate::ui::mascot::draw(f, inner);
            return;
        }
        let placeholder = Paragraph::new(t("terminal.placeholder.no_focused_session", &[]))
            .style(ratatui::style::Style::default().fg(Color::DarkGray))
            .block(Border::solid(Color::DarkGray).into_block());
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
        let title = format!(" {}{} ", entry.session.name, status);
        if app.focus == Focus::Terminal {
            // The focused pane gets the flowing blue gradient to stand out
            // from the rest of the (flat-colored) chrome.
            let inner = Border::blue().animated(true).title(title).render(f, area);
            f.render_widget(pseudo_term, inner);
            return;
        }
        pseudo_term = pseudo_term.block(Border::solid(Color::DarkGray).title(title).into_block());
    }

    f.render_widget(pseudo_term, area);
}
