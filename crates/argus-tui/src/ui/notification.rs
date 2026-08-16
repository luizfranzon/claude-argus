use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::notification::Notification;
use crate::ui::border::Border;

use super::HitMap;

const CARD_WIDTH: u16 = 42;
const CARD_HEIGHT: u16 = 4;
const MARGIN: u16 = 1;
/// Extra clearance above the status bar so the stack doesn't hug the very
/// bottom edge of the screen.
const BOTTOM_MARGIN: u16 = 3;
const GAP: u16 = 1;

/// Same dark-navy surface the modal popup uses, so a toast reads as the same
/// kind of floating card rather than a one-off style. Also the color every
/// other color fades from/to — a terminal grid has no real alpha channel, so
/// "transparent" here just means "blended into this".
const BG_RGB: (u8, u8, u8) = (13, 17, 28);
const BG: Color = Color::Rgb(BG_RGB.0, BG_RGB.1, BG_RGB.2);
const TITLE_RGB: (u8, u8, u8) = (255, 255, 255);
const MESSAGE_RGB: (u8, u8, u8) = (200, 200, 200);
const CLOSE_RGB: (u8, u8, u8) = (148, 163, 184);
const CLOSE_HOVER_RGB: (u8, u8, u8) = (239, 68, 68);

fn lerp_channel(from: u8, to: u8, t: f32) -> u8 {
    (from as f32 + (to as f32 - from as f32) * t).round() as u8
}

/// Blends `to` toward `BG_RGB` by `alpha` — `alpha == 1.0` is `to` at full
/// strength, `alpha == 0.0` is indistinguishable from the card's own
/// background, which is what a fade-in/fade-out looks like without real
/// transparency.
fn faded(to: (u8, u8, u8), alpha: f32) -> Color {
    Color::Rgb(
        lerp_channel(BG_RGB.0, to.0, alpha),
        lerp_channel(BG_RGB.1, to.1, alpha),
        lerp_channel(BG_RGB.2, to.2, alpha),
    )
}

/// Draws the toast stack anchored to `area`'s bottom-left corner and records
/// each card's close button into `hitmap.notification_close` for
/// `AppState::on_mouse` to hit-test against. `notifications` is oldest-first;
/// rendering walks it in reverse so the newest card sits closest to the
/// corner and older ones stack upward above it. `hovered` is whichever
/// notification's close button the mouse was last over — used to render
/// that one `X` in red.
pub fn draw(f: &mut Frame, area: Rect, notifications: &[&Notification], hovered: Option<u64>, hitmap: &mut HitMap) {
    let width = CARD_WIDTH.min(area.width.saturating_sub(MARGIN * 2));
    if width == 0 || notifications.is_empty() {
        return;
    }

    let mut y = area.y + area.height.saturating_sub(BOTTOM_MARGIN + CARD_HEIGHT);
    for notification in notifications.iter().rev() {
        if y < area.y {
            break;
        }
        let card = Rect { x: area.x + MARGIN, y, width, height: CARD_HEIGHT };
        draw_card(f, card, notification, hovered == Some(notification.id), hitmap);

        let step = CARD_HEIGHT + GAP;
        if y < area.y + step {
            break;
        }
        y -= step;
    }
}

fn draw_card(f: &mut Frame, area: Rect, notification: &Notification, hovered: bool, hitmap: &mut HitMap) {
    let alpha = notification.alpha();
    let border = faded(notification.kind.rgb(), alpha);
    let title_fg = faded(TITLE_RGB, alpha);
    let message_fg = faded(MESSAGE_RGB, alpha);
    let close_fg = faded(if hovered { CLOSE_HOVER_RGB } else { CLOSE_RGB }, alpha);

    f.render_widget(Clear, area);
    let inner = Border::solid(border).bg(BG).render(f, area);

    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(inner);
    let title_row = Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]).split(rows[0]);
    let title_area = title_row[0];
    let close_area = title_row[1];

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            notification.title.clone(),
            Style::default().fg(title_fg).add_modifier(Modifier::BOLD),
        )))
        .style(Style::default().bg(BG)),
        title_area,
    );
    f.render_widget(
        Paragraph::new("x").style(Style::default().fg(close_fg).bg(BG).add_modifier(Modifier::BOLD)),
        close_area,
    );
    hitmap.notification_close.push((close_area, notification.id));

    f.render_widget(
        Paragraph::new(notification.message.clone())
            .style(Style::default().fg(message_fg).bg(BG))
            .wrap(Wrap { trim: true }),
        rows[1],
    );
}
