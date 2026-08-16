//! Shared visuals for popup overlays (`ui::modal`, `ui::fuzzy_finder`):
//! dimming everything already drawn this frame outside the popup, and the
//! surface colors every popup uses. Both overlay modules used to define
//! their own copy of this — a real seam only once a second overlay actually
//! existed, per the "one adapter is hypothetical, two is real" rule; now
//! that there are two, they share it.

use ratatui::style::Color;
use ratatui::Frame;
use ratatui::layout::Rect;

/// Dark-navy fill for a popup's body — deliberately a shade darker than
/// whatever the terminal's own background is, so the popup reads as a
/// distinct surface rather than a transparent box outline.
pub const SURFACE_BG: Color = Color::Rgb(13, 17, 28);
/// Slightly lighter band behind a popup's title/query row.
pub const TITLE_BG: Color = Color::Rgb(30, 41, 59);
pub const BORDER: Color = Color::Cyan;
pub const KEY: Color = Color::Cyan;
pub const HINT: Color = Color::DarkGray;

/// How much of each cell's original brightness survives the dim — lower is
/// darker. Applied to everything already drawn this frame (topbar, sidebar,
/// terminal, statusbar) outside `popup`, so the popup reads as the one thing
/// in focus. Ratatui cells have no real alpha channel, so this fakes it by
/// scaling each cell's actual RGB toward black in place.
const DIM_FACTOR: f32 = 0.35;

pub fn dim_backdrop(f: &mut Frame, area: Rect, popup: Rect) {
    let buf = f.buffer_mut();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            let inside_popup = x >= popup.x
                && x < popup.x.saturating_add(popup.width)
                && y >= popup.y
                && y < popup.y.saturating_add(popup.height);
            if inside_popup {
                continue;
            }
            let Some(cell) = buf.cell_mut((x, y)) else { continue };
            cell.fg = dim(cell.fg);
            cell.bg = dim(cell.bg);
        }
    }
}

fn dim(color: Color) -> Color {
    let (r, g, b) = to_rgb(color);
    Color::Rgb(
        (f32::from(r) * DIM_FACTOR) as u8,
        (f32::from(g) * DIM_FACTOR) as u8,
        (f32::from(b) * DIM_FACTOR) as u8,
    )
}

/// Approximates every named/indexed `Color` variant as RGB so it can be
/// scaled uniformly — this app's own styles are almost all `Color::Rgb(..)`
/// already, but named ANSI colors (`Cyan`, `DarkGray`, …) still show up here
/// and there and need *some* real triplet to dim consistently.
fn to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (205, 0, 0),
        Color::Green => (0, 205, 0),
        Color::Yellow => (205, 205, 0),
        Color::Blue => (0, 0, 238),
        Color::Magenta => (205, 0, 205),
        Color::Cyan => (0, 205, 205),
        Color::Gray => (229, 229, 229),
        Color::DarkGray => (127, 127, 127),
        Color::LightRed => (255, 0, 0),
        Color::LightGreen => (0, 255, 0),
        Color::LightYellow => (255, 255, 0),
        Color::LightBlue => (92, 92, 255),
        Color::LightMagenta => (255, 0, 255),
        Color::LightCyan => (0, 255, 255),
        Color::White => (255, 255, 255),
        _ => (40, 40, 40),
    }
}
