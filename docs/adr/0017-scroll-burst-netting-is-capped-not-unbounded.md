---
status: accepted
---

# Wheel-scroll burst netting is capped, not unbounded

`scroll_coalesce::drain_scroll_burst` was written to fix a real problem: forwarding every
mouse-wheel notch to `claude` individually, each one costing a full re-render of `claude`'s own
TUI, backs its stdin up behind a pile of redraws during a fast flick — real keystrokes typed
right after the flick would land behind that backlog and feel delayed. The fix was to drain
whatever wheel-scroll events were already queued (via `now_or_never`, never awaiting) and net
them into one signed delta before forwarding, same shape as `paste_coalesce::drain_paste_burst`.

That netting had no upper bound: one `drain_scroll_burst` call would drain the *entire* backlog
queued at that instant, however large. Trackpad momentum scrolling is a long sequence of
same-direction notches spread across the whole deceleration of the gesture — not one
instantaneous pile-up — and a real terminal shows that as smooth, visibly-decelerating motion
because it forwards (and the attached app redraws on) each notch roughly as it arrives. Netting
the whole queued backlog collapsed that motion into a single instant jump the moment Argus's
event loop happened to catch up with however many notches had queued by then: correct end
position, but none of the felt motion a real terminal shows during the same gesture — which is
what read as "less fluid than the real thing" even though nothing was actually stalling.

`MAX_NOTCHES_PER_BURST` (3) now bounds how many notches one `drain_scroll_burst` call will net.
Hitting the cap ends the burst the same way a non-scroll event does, but — unlike a non-scroll
event — leaves the rest of the flick queued on the `EventStream` itself rather than draining it
into `lookahead`; the main loop's next `events.next()` picks it up as a fresh burst. A long flick
now spreads across several small bursts (and their redraws) instead of collapsing into one,
recovering the stepped-but-visible motion a real terminal shows, while still absorbing the
genuinely-simultaneous pile-ups (many notches the OS/host terminal delivered in one read, faster
than the loop can drain) that motivated coalescing in the first place — 3 is small enough to keep
per-burst redraw/write cost negligible, and large enough that a literal one-notch-at-a-time mouse
wheel (not a trackpad flick) still nets to 1 almost every time, since nothing else is typically
queued yet when that call fires.

The alternative considered was removing the cap in a different way — pacing forwarded ticks over
wall-clock time instead (e.g. queue the net delta in `AppState` and drain a few ticks per 16ms
redraw tick, regardless of how they were netted). Rejected as unnecessary complexity: the actual
problem was that one `drain_scroll_burst` call was allowed to drain arbitrarily far ahead of what
had really "already happened" physically, not that ticks needed artificial pacing once forwarded
— bounding the drain itself is enough to make each burst correspond to roughly one slice of the
gesture instead of the whole thing, without adding a second stateful queue for something the
`EventStream` already queues for us.

## Follow-up: bursts also accelerate with flick speed, not just cap out

Capping the drain fixed the "one big jump" problem, but left a second, related gap: a real
terminal's scroll doesn't just move smoothly, it moves *further* the faster the same physical
flick is performed — the felt "it knows I'm scrolling fast" behavior. Argus forwarded exactly
`ticks` lines per burst regardless of how quickly bursts were arriving, so a fast flick and a slow
one covered the same ground per unit of wheel movement — flat, not accelerating, which read as its
own kind of "less fluid than the real thing" even once the jump was gone.

`scroll_coalesce::ScrollAccelerator` (`app::mouse::on_scroll_burst`) now sits between the netted
`ticks` from `drain_scroll_burst` and the SGR reports actually written to the child: it tracks the
gap since the previous burst that scrolled the terminal content (`AppState::last_terminal_scroll_at`)
and ramps a multiplier up for each consecutive burst that lands within `CONTINUATION_WINDOW`
(100ms) of the last one, resetting to 1:1 the moment a gap is longer than that — i.e. the instant
the user pauses or starts a fresh, deliberate scroll. A single unhurried wheel notch is therefore
never accelerated (the common case, and mouse-wheel users who want line-by-line control), while an
unbroken trackpad flick ramps up to `MAX_LINES_PER_BURST` (12) lines per burst the longer it runs,
same shape as the real terminal's own felt acceleration. This only applies to scrolling the
terminal's own content — the sidebar and the fuzzy finder's preview scroll by moving a selection
index (see `scroll_sidebar`, `FuzzyFinderState::scroll_preview`), not by an analog scroll amount,
so accelerating them wouldn't map onto anything a real terminal does; `on_scroll_burst` only feeds
`ScrollAccelerator` on the branch that forwards to the focused session's PTY.
