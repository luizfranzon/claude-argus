/// Whether a blinking indicator is in its "on" phase right now, alternating
/// every `period_ms` based on wall-clock time — no per-widget timer state
/// needed, same trick as `agents::spinner_frame`.
pub fn on(period_ms: u128) -> bool {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    (millis / period_ms).is_multiple_of(2)
}
