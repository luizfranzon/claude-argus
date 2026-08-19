//! Coalesces a fast mouse-wheel flick into small netted chunks instead of
//! forwarding every notch individually.
//!
//! Each wheel notch we forward to `claude` (see `app::forward_wheel_scroll`)
//! costs it a full re-render of its own TUI, same as any full-screen app
//! reacting to a mouse report. A fast flick can queue far more notches than
//! that keeps up with; forwarding — and redrawing Argus itself — for each
//! one individually backs the child's stdin up behind a pile of redraws,
//! which is what makes real keystrokes typed right after feel delayed. This
//! mirrors `paste_coalesce::drain_paste_burst`'s approach: drain what's
//! already queued via `now_or_never` (never awaits — a notch that hasn't
//! actually arrived yet just ends the burst) and net it into a delta.
//!
//! Trackpad momentum scrolling is a long *sequence* of same-direction
//! notches spread over the flick's whole deceleration, not one instantaneous
//! pile-up — a real terminal shows that as a smoothly decelerating scroll
//! because it forwards (and redraws on) each notch as it arrives. Netting
//! the *entire* queued backlog into one delta, with no cap, collapsed that
//! whole gesture into a single instant jump the moment our event loop
//! happened to catch up with however many notches the flick had queued by
//! then — visually a jarring step where a real terminal shows motion.
//! `MAX_NOTCHES_PER_BURST` bounds how much one drain call will net, so a long
//! flick still gets forwarded (and redrawn) across several bursts — one per
//! main-loop iteration — instead of one drain grabbing the whole thing.

use std::collections::VecDeque;
use std::time::Duration;

use crossterm::event::{Event, EventStream, MouseEvent, MouseEventKind};
use futures::{FutureExt, StreamExt};

/// Caps how many notches a single `drain_scroll_burst` call will net
/// together. Small enough that a long flick still renders as several
/// visibly-distinct steps (preserving the felt motion of a real terminal's
/// per-notch redraws) while still absorbing the genuinely-simultaneous
/// pile-ups (many notches the OS/host terminal delivered in one read, faster
/// than our loop can drain) that motivated coalescing in the first place.
const MAX_NOTCHES_PER_BURST: usize = 3;

fn tick(mouse: &MouseEvent) -> Option<i32> {
    match mouse.kind {
        MouseEventKind::ScrollUp => Some(1),
        MouseEventKind::ScrollDown => Some(-1),
        _ => None,
    }
}

/// Drains up to `MAX_NOTCHES_PER_BURST` already-queued wheel-scroll events
/// following `first`, netting them into a signed tick count (positive = net
/// scroll-up notches, negative = net scroll-down) and keeping the most
/// recent event's position (what the mouse was over when the burst ended).
/// Anything pulled off the stream that isn't itself a wheel-scroll event ends
/// the burst early and is pushed onto `lookahead` instead of being dropped,
/// so the caller's next loop iteration processes it — real input right after
/// a scroll flick must not get lost or reordered. Hitting the cap ends the
/// burst the same way a non-scroll event would, but leaves the rest of the
/// flick queued on the stream itself (not drained into `lookahead`) — the
/// caller's next `events.next()` picks it up as a fresh burst, which is what
/// spreads a long flick across multiple redraws instead of collapsing it.
pub async fn drain_scroll_burst(
    events: &mut EventStream,
    first: MouseEvent,
) -> (i32, MouseEvent, VecDeque<Event>) {
    let mut net = tick(&first).unwrap_or(0);
    let mut drained = 1;
    let mut last = first;
    let mut lookahead = VecDeque::new();
    while drained < MAX_NOTCHES_PER_BURST {
        let Some(Some(Ok(event))) = events.next().now_or_never() else { break };
        match &event {
            Event::Mouse(mouse) if tick(mouse).is_some() => {
                net += tick(mouse).unwrap();
                last = *mouse;
                drained += 1;
            }
            _ => {
                lookahead.push_back(event);
                break;
            }
        }
    }
    (net, last, lookahead)
}

/// If consecutive bursts land within this long of each other, the user is
/// still mid-flick rather than having started a fresh, deliberate scroll —
/// `ScrollAccelerator::accelerate` keeps ramping the multiplier up across
/// such bursts instead of resetting it.
const CONTINUATION_WINDOW: Duration = Duration::from_millis(100);

/// How much the multiplier grows per consecutive burst inside
/// `CONTINUATION_WINDOW`, up to `MAX_STREAK` bursts in.
const ACCEL_STEP: f32 = 0.5;

const MAX_STREAK: u32 = 6;

/// Hard ceiling on lines forwarded for one burst, however fast the streak —
/// keeps a very long flick's scroll "fast but readable" instead of a blind
/// teleport to wherever the flick ends.
const MAX_LINES_PER_BURST: i32 = 12;

/// Turns a real terminal's most noticeable scroll trait — the same physical
/// flick moves further the faster it's performed — into an explicit
/// multiplier on top of `drain_scroll_burst`'s netted tick count. A single
/// unhurried notch (the common case: a plain mouse wheel, or the first notch
/// of any gesture) always forwards 1:1, exactly like before this existed;
/// only *consecutive* bursts arriving faster than `CONTINUATION_WINDOW` apart
/// ramp the multiplier up, so an unbroken trackpad flick accelerates the way
/// it does in a real terminal instead of scrolling at the same flat rate
/// however hard it's flicked.
#[derive(Debug, Default)]
pub struct ScrollAccelerator {
    streak: u32,
}

impl ScrollAccelerator {
    pub fn new() -> Self {
        Self { streak: 0 }
    }

    /// `gap` is the time since the previous call that actually forwarded a
    /// burst (`None` for the first scroll of a fresh gesture, or after a
    /// pause long enough to count as one). Returns how many lines to forward
    /// for `ticks` net notches — same sign as `ticks`, magnitude scaled by
    /// how much of a streak of fast, unbroken bursts led up to this one.
    pub fn accelerate(&mut self, ticks: i32, gap: Option<Duration>) -> i32 {
        self.streak = if gap.is_some_and(|g| g <= CONTINUATION_WINDOW) {
            (self.streak + 1).min(MAX_STREAK)
        } else {
            0
        };
        let multiplier = 1.0 + self.streak as f32 * ACCEL_STEP;
        let lines = (ticks as f32 * multiplier).round() as i32;
        lines.clamp(-MAX_LINES_PER_BURST, MAX_LINES_PER_BURST)
    }
}

#[cfg(test)]
mod accelerator_tests {
    use super::*;

    #[test]
    fn a_single_unhurried_notch_is_not_accelerated() {
        let mut accel = ScrollAccelerator::new();
        assert_eq!(accel.accelerate(1, None), 1);
        assert_eq!(accel.accelerate(1, Some(Duration::from_millis(500))), 1);
    }

    #[test]
    fn consecutive_fast_bursts_ramp_up() {
        let mut accel = ScrollAccelerator::new();
        assert_eq!(accel.accelerate(1, None), 1);
        let fast = Some(Duration::from_millis(20));
        assert_eq!(accel.accelerate(1, fast), 2); // streak 1: x1.5 -> rounds to 2
        assert_eq!(accel.accelerate(1, fast), 2); // streak 2: x2.0
        assert_eq!(accel.accelerate(1, fast), 3); // streak 3: x2.5 -> rounds to 3
    }

    #[test]
    fn a_pause_resets_the_streak() {
        let mut accel = ScrollAccelerator::new();
        let fast = Some(Duration::from_millis(20));
        accel.accelerate(1, fast);
        accel.accelerate(1, fast);
        assert!(accel.accelerate(1, fast) > 1, "should be accelerated by now");
        let slow = Some(Duration::from_millis(500));
        assert_eq!(accel.accelerate(1, slow), 1, "a real pause should drop back to 1:1");
    }

    #[test]
    fn output_never_exceeds_the_burst_cap() {
        let mut accel = ScrollAccelerator::new();
        let fast = Some(Duration::from_millis(20));
        let mut last = accel.accelerate(3, None);
        for _ in 0..MAX_STREAK + 4 {
            last = accel.accelerate(3, fast);
        }
        assert_eq!(last, MAX_LINES_PER_BURST);
    }

    #[test]
    fn direction_is_preserved_when_scaled() {
        let mut accel = ScrollAccelerator::new();
        let fast = Some(Duration::from_millis(20));
        accel.accelerate(-1, None);
        let scaled = accel.accelerate(-1, fast);
        assert!(scaled < 0, "scroll-down should stay negative once accelerated");
    }
}
