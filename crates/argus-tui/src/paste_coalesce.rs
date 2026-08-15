//! Windows-only workaround for a `crossterm` limitation: on Windows,
//! `crossterm` reads input via the console's `ReadConsoleInput`, which
//! delivers a clipboard paste as a burst of individual `KeyEvent`s
//! indistinguishable from typing — bracketed-paste sequence detection only
//! happens on the Unix raw-byte parse path, so `Event::Paste` never fires.
//! Left alone, every pasted character trickles in as a normal keystroke and
//! any embedded newline is read as "submit now" by `handle_terminal_key`.
//!
//! The Windows console enqueues that whole burst synchronously in one shot,
//! so immediately after the first pasted character arrives, the rest are
//! typically already sitting in the event stream ready to read with zero
//! wait — unlike real typing, or OS key-repeat (which is timer-driven and
//! queues one event at a time). `drain_paste_burst` exploits that: it pulls
//! everything already queued and, if more than the seed character came
//! through, the caller treats the result as a synthetic paste instead of a
//! keystroke.

use std::collections::VecDeque;

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::{FutureExt, StreamExt};

enum DrainStep {
    Text(char),
    Ignore,
    Boundary,
}

/// The character `key` contributes to a coalesced paste burst, or `None` if
/// it isn't paste-shaped text. Only plain characters (optionally Shift'd,
/// for capitals/symbols), `Enter` (→ `\n`) and `Tab` (→ `\t`, so
/// pasted/indented code doesn't fragment the burst) qualify — anything else
/// (arrows, Ctrl/Alt combos, function keys) is real, distinct input that
/// must end the burst rather than be folded into it.
fn text_char(key: &KeyEvent) -> Option<char> {
    if !key.modifiers.difference(KeyModifiers::SHIFT).is_empty() {
        return None;
    }
    match key.code {
        KeyCode::Char(c) => Some(c),
        KeyCode::Enter => Some('\n'),
        KeyCode::Tab => Some('\t'),
        _ => None,
    }
}

fn classify(event: &Event) -> DrainStep {
    let Event::Key(key) = event else {
        return DrainStep::Boundary;
    };
    if key.kind == KeyEventKind::Release {
        return DrainStep::Ignore;
    }
    match text_char(key) {
        Some(c) => DrainStep::Text(c),
        None => DrainStep::Boundary,
    }
}

/// Whether `key` is a plausible first character of a paste burst — the same
/// check `run()` uses to decide whether `drain_paste_burst` is worth
/// calling at all.
pub fn seed_char(key: &KeyEvent) -> Option<char> {
    text_char(key)
}

/// Pulls every event already queued on `events` via `now_or_never` (never
/// awaits — a key that hasn't actually arrived yet just ends the burst),
/// folding paste-shaped ones (see `text_char`) into `first`'s string.
/// Anything pulled that isn't paste-shaped text ends the burst and is
/// pushed onto `lookahead` instead of being dropped, so the caller's next
/// loop iteration processes it — real input right after a paste must not
/// get lost or reordered.
pub async fn drain_paste_burst(
    events: &mut EventStream,
    first: char,
    lookahead: &mut VecDeque<Event>,
) -> String {
    let mut text = String::new();
    text.push(first);
    while let Some(Some(Ok(event))) = events.next().now_or_never() {
        match classify(&event) {
            DrainStep::Text(c) => text.push(c),
            DrainStep::Ignore => {}
            DrainStep::Boundary => {
                lookahead.push_back(event);
                break;
            }
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn text_char_accepts_plain_and_shifted_chars() {
        assert_eq!(text_char(&key(KeyCode::Char('a'), KeyModifiers::NONE)), Some('a'));
        assert_eq!(text_char(&key(KeyCode::Char('A'), KeyModifiers::SHIFT)), Some('A'));
    }

    #[test]
    fn text_char_maps_enter_and_tab() {
        assert_eq!(text_char(&key(KeyCode::Enter, KeyModifiers::NONE)), Some('\n'));
        assert_eq!(text_char(&key(KeyCode::Tab, KeyModifiers::NONE)), Some('\t'));
    }

    #[test]
    fn text_char_rejects_control_combos_and_navigation() {
        assert_eq!(text_char(&key(KeyCode::Char('c'), KeyModifiers::CONTROL)), None);
        assert_eq!(text_char(&key(KeyCode::Left, KeyModifiers::NONE)), None);
        assert_eq!(text_char(&key(KeyCode::Backspace, KeyModifiers::NONE)), None);
    }

    #[test]
    fn classify_ignores_release_without_ending_the_burst() {
        let mut release = key(KeyCode::Char('a'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert!(matches!(classify(&Event::Key(release)), DrainStep::Ignore));
    }

    #[test]
    fn classify_treats_non_key_events_as_a_boundary() {
        assert!(matches!(classify(&Event::Resize(80, 24)), DrainStep::Boundary));
    }

    #[test]
    fn classify_treats_navigation_keys_as_a_boundary() {
        let arrow = key(KeyCode::Left, KeyModifiers::NONE);
        assert!(matches!(classify(&Event::Key(arrow)), DrainStep::Boundary));
    }
}
