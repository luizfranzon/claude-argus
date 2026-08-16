use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::{AppState, RuntimeStatus, WorkspaceEntry};
use crate::i18n::t;
use crate::ui::hitmap::HitMap;
use crate::ui::scroll;
use crate::ui::blink;

/// Alternation period for the "unread" sidebar dot.
const UNREAD_BLINK_MS: u128 = 500;

const SPINNER_FRAMES: [char; 6] = ['⠻', '⠽', '⠾', '⠷', '⠯', '⠟'];

fn spinner_frame() -> char {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let idx = (millis / 120) as usize % SPINNER_FRAMES.len();
    SPINNER_FRAMES[idx]
}

pub fn draw(f: &mut Frame, area: Rect, app: &AppState, entry: &WorkspaceEntry, hitmap: &mut HitMap) {
    if entry.sessions.is_empty() {
        let target = Rect {
            x: area.x,
            y: area.y + area.height.saturating_sub(1) / 2,
            width: area.width,
            height: 1.min(area.height),
        };
        let paragraph = Paragraph::new(t("sidebar.agents.empty", &[]))
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(paragraph, target);
        return;
    }

    let (visible, offset, visible_selected) =
        scroll::window(&entry.sessions, entry.agents_selected, area.height as usize);
    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(i, session_id)| {
            let selected = i == visible_selected;
            let focused = entry.focused_session == Some(*session_id);

            let row = Rect { x: area.x, y: area.y + i as u16, width: area.width, height: 1 };
            hitmap.agents_rows.push((row, offset + i, *session_id));

            let Some(session_entry) = app.sessions.get(session_id) else {
                return ListItem::new("…");
            };
            let (dot, dot_color): (String, Color) = if session_entry.unread {
                if blink::on(UNREAD_BLINK_MS) {
                    ("○".to_string(), Color::Green)
                } else {
                    (" ".to_string(), Color::Green)
                }
            } else {
                match session_entry.status {
                    Some(RuntimeStatus::Thinking) => {
                        (spinner_frame().to_string(), Color::Rgb(255, 165, 0))
                    }
                    Some(RuntimeStatus::Idle) => ("○".to_string(), Color::Green),
                    Some(RuntimeStatus::Waiting) => ("◆".to_string(), Color::Magenta),
                    None => ("○".to_string(), Color::DarkGray),
                }
            };
            let name_style = if selected {
                Style::default().add_modifier(Modifier::BOLD).bg(Color::DarkGray)
            } else if focused {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::raw(if selected { "> " } else { "  " }),
                Span::styled(dot, Style::default().fg(dot_color)),
                Span::raw(" "),
                Span::styled(session_entry.session.name.clone(), name_style),
            ]))
        })
        .collect();

    f.render_widget(List::new(items), area);
}
