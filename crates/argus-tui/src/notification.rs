use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// How long a toast stays on screen before `NotificationCenter::tick` drops
/// it — matches the status bar's own timeout (`AppState::STATUS_LINE_TIMEOUT`).
const LIFETIME: Duration = Duration::from_secs(4);

/// How long the card takes to fade in from the background when it appears,
/// and to fade back out into it right before `LIFETIME` expires. `ui::notification::draw`
/// reads `Notification::alpha` and blends every color toward the card's
/// background by that fraction — there's no real transparency in a terminal
/// grid, so this is what stands in for it.
const FADE_IN: Duration = Duration::from_millis(120);
const FADE_OUT: Duration = Duration::from_millis(200);

/// Caps how many toasts stack at once so a burst of errors can't fill the
/// whole screen — oldest gets evicted first.
const MAX_VISIBLE: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    Info,
    Warn,
    Error,
}

impl NotificationKind {
    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            NotificationKind::Info => (59, 130, 246),
            NotificationKind::Warn => (245, 158, 11),
            NotificationKind::Error => (239, 68, 68),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub id: u64,
    pub kind: NotificationKind,
    pub title: String,
    pub message: String,
    shown_at: Instant,
}

impl Notification {
    /// How opaque the card should render right now, from `0.0` (fully
    /// blended into the background, i.e. invisible) to `1.0` (fully
    /// opaque) — ramps up over `FADE_IN`, holds at `1.0`, then ramps back
    /// down over the last `FADE_OUT` before `LIFETIME` runs out.
    pub fn alpha(&self) -> f32 {
        let elapsed = self.shown_at.elapsed();
        if elapsed < FADE_IN {
            elapsed.as_secs_f32() / FADE_IN.as_secs_f32()
        } else if let Some(remaining) = LIFETIME.checked_sub(elapsed) {
            if remaining < FADE_OUT {
                remaining.as_secs_f32() / FADE_OUT.as_secs_f32()
            } else {
                1.0
            }
        } else {
            0.0
        }
        .clamp(0.0, 1.0)
    }
}

/// Toast notification engine: any part of the app pushes a card via `info`
/// / `warn` / `error`, `tick` (called from the main loop's existing 250ms
/// redraw tick, same as `AppState::tick`) expires whatever has been visible
/// for `LIFETIME`, and `ui::notification::draw` renders whatever remains.
#[derive(Default)]
pub struct NotificationCenter {
    queue: VecDeque<Notification>,
    next_id: u64,
}

impl NotificationCenter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn info(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.push(NotificationKind::Info, title, message);
    }

    pub fn warn(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.push(NotificationKind::Warn, title, message);
    }

    pub fn error(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.push(NotificationKind::Error, title, message);
    }

    fn push(&mut self, kind: NotificationKind, title: impl Into<String>, message: impl Into<String>) {
        let id = self.next_id;
        self.next_id += 1;
        self.queue.push_back(Notification {
            id,
            kind,
            title: title.into(),
            message: message.into(),
            shown_at: Instant::now(),
        });
        while self.queue.len() > MAX_VISIBLE {
            self.queue.pop_front();
        }
    }

    /// Removes a card immediately — the close button's click handler.
    pub fn dismiss(&mut self, id: u64) {
        self.queue.retain(|n| n.id != id);
    }

    pub fn tick(&mut self) {
        self.queue.retain(|n| n.shown_at.elapsed() < LIFETIME);
    }

    /// Oldest first — `ui::notification::draw` stacks them bottom-up with
    /// the newest closest to the corner.
    pub fn visible(&self) -> impl Iterator<Item = &Notification> {
        self.queue.iter()
    }

    /// Whether any card is currently mid-fade (`alpha` not settled at its
    /// resting `1.0`) — the main loop uses this to pick a fast redraw tick
    /// only while an animation is actually running, instead of paying for
    /// it all the time.
    pub fn is_animating(&self) -> bool {
        self.queue.iter().any(|n| n.alpha() < 1.0)
    }
}
