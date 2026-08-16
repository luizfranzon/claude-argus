---
status: accepted
---

# The redraw loop only calls `terminal.draw()` when something changed

The main loop's `redraw` tick (`crates/argus-tui/src/main.rs`) fires every
16ms — that part is unchanged, and still exists to cap redraw work at a
steady rate instead of one full-frame redraw per keystroke/PTY chunk (see the
comment above `redraw.tick()`). What changed is what happens *inside* that
tick: it used to call `terminal.draw()` unconditionally on every fire, which
meant Argus repainted the whole frame — walking the file explorer tree,
re-rendering every visible list, diffing the full `ratatui` buffer, writing
to the real terminal — 60 times a second forever, including while sitting
completely idle with a static screen and nothing to show for it. That's pure
waste: the same frame, produced again, for no visible difference.

`AppState` now carries a `dirty: bool` (`crates/argus-tui/src/app/mod.rs`).
Every discrete input event and `AppEvent` the main loop processes calls
`state.mark_dirty()`; the redraw tick only calls `terminal.draw()` when
`state.dirty` is set (or `state.is_animating()`, below), and clears it right
after. A genuinely idle app — no keystrokes, no PTY output, no background
task reporting in — now skips the draw call entirely on most ticks instead of
producing an identical frame over and over.

## The wrinkle: wall-clock animations have no discrete event to hang `mark_dirty` off of

Three things animate purely by reading the system clock at render time, not
by reacting to an event: the "thinking" spinner (`ui::sidebar::agents::spinner_frame`),
the unread-session blink dot (`ui::blink::on`, used in both `topbar.rs` and
`sidebar::agents.rs`), and a toast's fade in/out (`Notification::alpha`). None
of these have a moment where "the thing that will look different next frame"
happens — they just look different because time passed. A pure dirty-flag
would leave a spinner frozen on its first frame, or a toast stuck fully
opaque past when it should be fading out.

`AppState::is_animating()` covers this: it's true whenever a session is
`RuntimeStatus::Thinking`, any session is unread, a toast is visible, or the
status line is counting down to its own expiry. The redraw tick draws when
`dirty || is_animating()` — so these keep animating at the full 16ms cadence
exactly as before, and the skip only ever applies to the actually-idle case
(no animation in flight, no event since the last frame).

One more subtlety worth naming: `AppState::tick()` (status line expiry,
notification-queue pruning) and `sync_focused_read()` (clearing `unread` on
focus) both run *before* the `dirty || is_animating()` check on every tick.
If either of them causes a transition — a status message expiring, an unread
flag clearing — that happens *before* the check reads `is_animating()`, so a
transition-to-not-animating would otherwise be invisible to the check that's
about to decide whether to draw it. Both methods call `self.dirty = true`
themselves when they actually change something, specifically to cover this:
the check has to see the change as "dirty," not read the post-change state
and conclude nothing needs to be redrawn.

## Considered options

**Reduce the tick rate instead of adding a dirty flag** (e.g. 16ms → 100ms
while idle) was the fallback option. Simpler — no dirty-tracking, no
animation carve-out — but it still redraws unconditionally, just less often,
so it trades away some waste for less than the dirty-flag approach reclaims,
while also making the app feel very slightly less responsive to the first
keystroke after a long idle stretch (up to one stale tick's worth of lag
before the faster-cadence timer catches up, if throttling were also
attempted). Rejected: the dirty-flag gets a strictly better result (zero
wasted draws when idle, and the exact same 16ms responsiveness the app
already had) for a bounded, well-understood amount of extra bookkeeping.
