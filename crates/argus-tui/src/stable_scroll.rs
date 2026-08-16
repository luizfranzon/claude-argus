/// A manual scroll offset that survives redundant re-renders of whatever
/// it's scrolling, but resets to zero the moment the *subject* being viewed
/// actually changes.
///
/// This exists to kill off a whole bug class, not just one instance of it:
/// any pane that (a) lets the user manually scroll it and (b) gets its
/// content re-supplied by something other than the user's own navigation (a
/// background refresh, a filesystem watcher, a debounced re-search, a
/// polling tick) will, if it naively resets scroll on every such re-supply,
/// yank the user's scroll position back on every tick — even though the
/// content on screen never actually changed. The fuzzy finder's preview hit
/// exactly this: a background reindex kept re-requesting the same file's
/// preview, and the naive "reset scroll on every preview request" fired on
/// every tick, not just on real navigation.
///
/// The fix is centralized here instead of hand-rolled per call site: every
/// place that (re)computes what should currently be showing calls
/// [`Self::set_subject`] with a key that identifies it (e.g. a file path, or
/// `(path, line)` for a specific match within a file). The same subject as
/// last time — including a redundant background refresh — leaves the offset
/// alone; a genuinely different subject resets it. Callers never need to
/// reason about *why* they're being asked to show something in order to
/// decide whether to reset scroll — any future scrollable pane fed by both
/// user navigation and background refreshes should use this instead of
/// re-deriving the same "did the target actually change" check by hand.
#[derive(Debug, Default)]
pub struct StableScroll<K> {
    subject: Option<K>,
    offset: i32,
}

impl<K: PartialEq> StableScroll<K> {
    pub fn new() -> Self {
        Self { subject: None, offset: 0 }
    }

    /// Declares what should currently be showing. Resets the offset to 0 iff
    /// `subject` differs from whatever the last call passed (or this is the
    /// first call) — call this every time the target is (re)computed,
    /// whether that's from real user navigation or a redundant background
    /// refresh; only an actual change in subject moves the offset.
    pub fn set_subject(&mut self, subject: K) {
        if self.subject.as_ref() != Some(&subject) {
            self.offset = 0;
        }
        self.subject = Some(subject);
    }

    /// Clears the subject and offset — for when there's nothing to show at
    /// all (e.g. no selection), as opposed to a subject change.
    pub fn reset(&mut self) {
        self.subject = None;
        self.offset = 0;
    }

    /// Adjusts the offset by `delta` (negative scrolls back toward the
    /// subject's natural/auto position, positive scrolls forward), clamped
    /// immediately to `[min, max]` rather than leaving the clamp to the next
    /// render. Without an immediate clamp, scrolling past an edge (e.g.
    /// holding Up past the top of a file) would let the raw offset keep
    /// accumulating unboundedly even though the rendered position is
    /// visually pinned at the edge — so it'd then take just as many
    /// opposite-direction presses to "unwind" back past that edge before the
    /// view visibly moves again. Pass `i32::MIN, i32::MAX` if a caller
    /// genuinely has no bound to enforce yet.
    pub fn scroll_clamped(&mut self, delta: i32, min: i32, max: i32) {
        self.offset = (self.offset + delta).clamp(min, max);
    }

    pub fn offset(&self) -> i32 {
        self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_subject_keeps_offset_across_repeated_calls() {
        let mut s = StableScroll::new();
        s.set_subject("a.rs");
        s.scroll_clamped(5, i32::MIN, i32::MAX);
        s.set_subject("a.rs"); // e.g. a background refresh re-requesting the same file
        assert_eq!(s.offset(), 5);
    }

    #[test]
    fn different_subject_resets_offset() {
        let mut s = StableScroll::new();
        s.set_subject("a.rs");
        s.scroll_clamped(5, i32::MIN, i32::MAX);
        s.set_subject("b.rs");
        assert_eq!(s.offset(), 0);
    }

    #[test]
    fn scroll_clamped_does_not_overshoot_past_the_bound() {
        let mut s = StableScroll::new();
        s.set_subject("a.rs");
        // Scroll up (negative) well past the top edge — same as holding Up
        // longer than there's content above the current position.
        for _ in 0..10 {
            s.scroll_clamped(-1, 0, 20);
        }
        assert_eq!(s.offset(), 0, "offset must not go negative past the clamp");
        // A single step back down must move immediately, not spend several
        // presses "unwinding" an over-scrolled offset that was never applied.
        s.scroll_clamped(1, 0, 20);
        assert_eq!(s.offset(), 1);
    }

    #[test]
    fn reset_zeroes_offset_and_forgets_the_subject() {
        let mut s = StableScroll::new();
        s.set_subject("a.rs");
        s.scroll_clamped(5, i32::MIN, i32::MAX);
        s.reset();
        assert_eq!(s.offset(), 0);
        // Scrolling right after reset (nothing selected yet) shouldn't be
        // silently kept by a later set_subject call for the same file, the
        // way a real subject change never is.
        s.scroll_clamped(2, i32::MIN, i32::MAX);
        s.set_subject("a.rs");
        assert_eq!(s.offset(), 0, "reset must forget the old subject so the next set_subject is treated as a change");
    }
}
