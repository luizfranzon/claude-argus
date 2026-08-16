//! Centralized border engine for argus. Every panel and popup in the app
//! draws its border through this module so the whole UI shares one visual
//! language: rounded corners everywhere, focus expressed purely as a color
//! change (a flat color for most panes, the flowing blue gradient reserved
//! for whichever pane currently has input focus).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders};
use ratatui::Frame;

/// Blue tones the focused-pane gradient cycles through, sky blue -> royal
/// blue -> deep indigo and smoothly back to sky blue.
const BLUE_STOPS: [(u8, u8, u8); 3] = [
    (96, 165, 250),  // sky blue
    (59, 130, 246),  // royal blue
    (30, 64, 175),   // deep indigo-blue
];

enum Paint {
    Solid(Color),
    Blue,
}

/// Which way an animated gradient's colors travel around the perimeter.
/// Only meaningful together with `.animated(true)` — a still gradient has no
/// direction to speak of.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Spin {
    #[default]
    Clockwise,
    // No current call site spins counter-clockwise; kept as part of the
    // builder's public surface for whichever pane wants it next.
    #[allow(dead_code)]
    CounterClockwise,
}

/// Builder for a rounded-corner border. Start with [`Border::solid`] (most
/// panes) or [`Border::blue`] (the focused pane's flowing gradient), then
/// chain `title`/`bg`/`animated`/`spin` as needed, and finish with either
/// `render` (draws straight into the frame, returns the inner content rect)
/// or `into_block` (hands back a `ratatui::Block` for widgets that own their
/// own border, like `Paragraph` or `PseudoTerminal` — solid colors only).
pub struct Border<'a> {
    paint: Paint,
    title: Option<Span<'a>>,
    bg: Option<Color>,
    animated: bool,
    spin: Spin,
}

impl<'a> Border<'a> {
    pub fn solid(color: Color) -> Self {
        Self { paint: Paint::Solid(color), title: None, bg: None, animated: false, spin: Spin::default() }
    }

    /// The flowing blue gradient reserved for the currently focused pane.
    pub fn blue() -> Self {
        Self { paint: Paint::Blue, title: None, bg: None, animated: false, spin: Spin::default() }
    }

    pub fn title(mut self, title: impl Into<Span<'a>>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    /// Whether the gradient's colors travel around the perimeter over time
    /// instead of sitting still. Only affects `Border::blue()` — a solid
    /// border has no gradient to animate, so this is a no-op there. Defaults
    /// to clockwise (see [`Spin`]); override with `.spin(...)`.
    pub fn animated(mut self, animated: bool) -> Self {
        self.animated = animated;
        self
    }

    /// Which way the animation spins. Has no effect unless `.animated(true)`
    /// is also set.
    #[allow(dead_code)]
    pub fn spin(mut self, spin: Spin) -> Self {
        self.spin = spin;
        self
    }

    /// Builds a plain `ratatui::Block` for callers that attach the border to
    /// another widget via that widget's own `.block(...)`. Solid colors
    /// only — the gradient paint can't be expressed as a flat `Block`
    /// style, so callers with `Border::blue()` must use `render` instead.
    pub fn into_block(self) -> Block<'a> {
        let Paint::Solid(color) = self.paint else {
            unreachable!("gradient borders must be drawn with Border::render, not into_block")
        };
        let mut block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(color));
        if let Some(bg) = self.bg {
            block = block.style(Style::default().bg(bg));
        }
        if let Some(title) = self.title {
            block = block.title(title);
        }
        block
    }

    /// Draws the border (and its background fill, if set) straight into
    /// `area` and returns the inner rect content should be rendered into.
    pub fn render(self, f: &mut Frame, area: Rect) -> Rect {
        match self.paint {
            Paint::Solid(_) => {
                let block = self.into_block();
                let inner = block.inner(area);
                f.render_widget(block, area);
                inner
            }
            Paint::Blue => {
                if let Some(bg) = self.bg {
                    f.render_widget(Block::default().style(Style::default().bg(bg)), area);
                }
                let title = self.title.as_ref().map(|s| s.content.as_ref());
                render_blue_gradient(f.buffer_mut(), area, title, self.animated, self.spin);
                inner_rect(area)
            }
        }
    }
}

fn inner_rect(area: Rect) -> Rect {
    Block::default().borders(Borders::ALL).inner(area)
}

fn lerp(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color::Rgb(mix(a.0, b.0), mix(a.1, b.1), mix(a.2, b.2))
}

/// Eases `t` (0.0..=1.0) so a color mix accelerates out of one stop and
/// decelerates into the next, instead of moving at a constant linear rate —
/// this is what keeps the stop boundaries themselves from reading as a kink.
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Samples the blue gradient at `t` (0.0..=1.0), where `t` is the fractional
/// position along the border's perimeter (clockwise from the top-left
/// corner). The stops are treated as a cycle — after the last stop it eases
/// back into the first — so the color at `t == 0.0` and `t == 1.0` match and
/// the seam where the perimeter walk closes on itself is invisible.
fn sample(t: f32) -> Color {
    let n = BLUE_STOPS.len();
    let scaled = t.clamp(0.0, 1.0) * n as f32;
    let seg = (scaled.floor() as usize) % n;
    let next = (seg + 1) % n;
    let local_t = smoothstep(scaled - scaled.floor());
    lerp(BLUE_STOPS[seg], BLUE_STOPS[next], local_t)
}

/// One full revolution of the animated gradient takes this long — slow
/// enough to read as a smooth ambient motion rather than a distraction, fast
/// enough to notice at the ~250ms redraw cadence the main loop already runs.
const ROTATION_PERIOD_MS: u128 = 4000;

/// The animation's current position in its cycle, 0.0..=1.0, derived from
/// wall-clock time — same trick as `blink::on`, no per-widget timer state
/// needed. Always 0.0 (no shift) when `animated` is false.
fn phase(animated: bool) -> f32 {
    if !animated {
        return 0.0;
    }
    let millis =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    (millis % ROTATION_PERIOD_MS) as f32 / ROTATION_PERIOD_MS as f32
}

/// Draws a rounded-corner box border around `area` directly into `buf`,
/// with the border color flowing through shades of blue starting at the
/// top-left corner. When `animated` is true, the whole gradient additionally
/// rotates around the perimeter over time, spinning the direction `spin`
/// says (clockwise by default). `title`, if given, is rendered on the top
/// edge styled bold-white to stay legible against every gradient stop.
fn render_blue_gradient(buf: &mut Buffer, area: Rect, title: Option<&str>, animated: bool, spin: Spin) {
    if area.width < 2 || area.height < 2 {
        return;
    }

    let left = area.left();
    let right = area.right() - 1;
    let top = area.top();
    let bottom = area.bottom() - 1;

    let perimeter = 2 * (area.width as usize - 1) + 2 * (area.height as usize - 1);
    let perimeter = perimeter.max(1);
    let phase = phase(animated);

    // Cumulative distance walked clockwise from the top-left corner,
    // converted to 0.0..=1.0 for `sample`. The perimeter walk itself is
    // always clockwise (it's just how the cells are visited); `spin` only
    // decides which way the *phase offset* moves that base position, which
    // is what actually makes the pattern travel one way or the other.
    let mut dist: usize = 0;
    let mut put = |x: u16, y: u16, symbol: &str, dist: usize| {
        let base_t = dist as f32 / perimeter as f32;
        let t = match spin {
            Spin::Clockwise => (base_t + phase).rem_euclid(1.0),
            Spin::CounterClockwise => (phase - base_t).rem_euclid(1.0),
        };
        if let Some(cell) = buf.cell_mut((x, y)) {
            cell.set_symbol(symbol).set_style(Style::default().fg(sample(t)));
        }
    };

    // Top edge, left -> right.
    for x in left..=right {
        let symbol = if x == left { "╭" } else if x == right { "╮" } else { "─" };
        put(x, top, symbol, dist);
        dist += 1;
    }
    // Right edge, top -> bottom.
    for y in (top + 1)..=bottom {
        let symbol = if y == bottom { "╯" } else { "│" };
        put(right, y, symbol, dist);
        dist += 1;
    }
    // Bottom edge, right -> left.
    if bottom > top {
        for x in (left..right).rev() {
            let symbol = if x == left { "╰" } else { "─" };
            put(x, bottom, symbol, dist);
            dist += 1;
        }
    }
    // Left edge, bottom -> top.
    if right > left {
        for y in ((top + 1)..bottom).rev() {
            put(left, y, "│", dist);
            dist += 1;
        }
    }

    if let Some(title) = title {
        let max_width = area.width.saturating_sub(4) as usize;
        let truncated: String = title.chars().take(max_width).collect();
        let start_x = left + 2;
        for (i, ch) in truncated.chars().enumerate() {
            let x = start_x + i as u16;
            if x >= right {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, top)) {
                cell.set_char(ch).set_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
            }
        }
    }
}
