use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::i18n::t;

const LINE1: &str = " ▐▛███▜▌";
const LINE2: &str = "▝▜█████▛▘";
const LINE3: &str = "▝▝ ▘▘";

/// Draws the static "no sessions" mascot centered in `area`: the Claude
/// ASCII mark, a blank line, then the empty-state message.
pub fn draw(f: &mut Frame, area: Rect) {
    let ascii_style = Style::default().fg(Color::Cyan);
    let message_style = Style::default().fg(Color::DarkGray);

    let lines = vec![
        Line::styled(LINE1, ascii_style),
        Line::styled(LINE2, ascii_style),
        Line::styled(LINE3, ascii_style),
        Line::raw(""),
        Line::styled(t("sidebar.agents.empty", &[]), message_style),
    ];

    let content_height = lines.len() as u16;
    let target = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(content_height) / 2,
        width: area.width,
        height: content_height.min(area.height),
    };

    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), target);
}
