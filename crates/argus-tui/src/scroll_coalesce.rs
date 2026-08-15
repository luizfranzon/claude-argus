//! Coalesces a fast mouse-wheel flick into one net scroll amount instead of
//! forwarding every notch individually.
//!
//! Each wheel notch we forward to `claude` (see `app::forward_wheel_scroll`)
//! costs it a full re-render of its own TUI, same as any full-screen app
//! reacting to a mouse report. A fast flick can queue far more notches than
//! that keeps up with; forwarding — and redrawing Argus itself — for each
//! one individually backs the child's stdin up behind a pile of redraws,
//! which is what makes real keystrokes typed right after feel delayed. This
//! mirrors `paste_coalesce::drain_paste_burst`'s approach: drain everything
//! already queued via `now_or_never` (never awaits — a notch that hasn't
//! actually arrived yet just ends the burst) and net it into a single delta.

use std::collections::VecDeque;

use crossterm::event::{Event, EventStream, MouseEvent, MouseEventKind};
use futures::{FutureExt, StreamExt};

fn tick(mouse: &MouseEvent) -> Option<i32> {
    match mouse.kind {
        MouseEventKind::ScrollUp => Some(1),
        MouseEventKind::ScrollDown => Some(-1),
        _ => None,
    }
}

/// Drains every already-queued wheel-scroll event following `first`, netting
/// them into a signed tick count (positive = net scroll-up notches, negative
/// = net scroll-down) and keeping the most recent event's position (what the
/// mouse was over when the flick ended). Anything pulled off the stream that
/// isn't itself a wheel-scroll event ends the burst and is pushed onto
/// `lookahead` instead of being dropped, so the caller's next loop iteration
/// processes it — real input right after a scroll flick must not get lost or
/// reordered.
pub async fn drain_scroll_burst(
    events: &mut EventStream,
    first: MouseEvent,
) -> (i32, MouseEvent, VecDeque<Event>) {
    let mut net = tick(&first).unwrap_or(0);
    let mut last = first;
    let mut lookahead = VecDeque::new();
    while let Some(Some(Ok(event))) = events.next().now_or_never() {
        match &event {
            Event::Mouse(mouse) if tick(mouse).is_some() => {
                net += tick(mouse).unwrap();
                last = *mouse;
            }
            _ => {
                lookahead.push_back(event);
                break;
            }
        }
    }
    (net, last, lookahead)
}
