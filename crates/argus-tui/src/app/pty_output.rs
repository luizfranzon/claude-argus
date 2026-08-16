//! Raw PTY byte handling: feeding output into a session's `vt100::Parser`
//! and scanning it for OSC 52 "set clipboard" escapes, which `vt100::Parser`
//! itself silently drops.

use base64::Engine;
use uuid::Uuid;

use super::{AppState, SCROLLBACK};

impl AppState {
    pub(crate) fn on_pty_output(&mut self, stream_id: Uuid, data: Vec<u8>) {
        if let Some(session_id) = self.stream_to_session.get(&stream_id).copied() {
            if let Some(entry) = self.sessions.get_mut(&session_id) {
                if let Some(text) = scan_osc52(&mut entry.osc52_partial, &data) {
                    self.clipboard_copy_requested = Some(text);
                }
                entry.parser.process(&data);
            }
        } else {
            self.pending_output.entry(stream_id).or_default().extend(data);
        }
    }

    pub(crate) fn new_parser_for(&mut self, stream_id: Uuid) -> vt100::Parser {
        let (cols, rows) = self.terminal_size;
        let mut parser = vt100::Parser::new(rows.max(1), cols.max(1), SCROLLBACK);
        if let Some(buffered) = self.pending_output.remove(&stream_id) {
            parser.process(&buffered);
        }
        parser
    }
}

/// Longest possible prefix of an OSC 52 introducer (`ESC ] 5 2 ;`) — used to
/// decide how much of a trailing, not-yet-matched tail is worth keeping
/// across chunk boundaries.
const OSC52_PREFIX: &[u8] = b"\x1b]52;";

/// Hard cap on how many bytes `scan_osc52` will carry over while waiting for
/// a sequence to terminate, so a malformed or adversarial stream that opens
/// `ESC ] 52 ;` and never closes it can't pin unbounded memory.
const OSC52_MAX_PENDING: usize = 1 << 16;

/// Scans raw PTY bytes for a complete OSC 52 "set clipboard" escape sequence
/// and, if one is found, base64-decodes its payload and returns the
/// resulting text, ready to feed into the same clipboard pipeline as a
/// manual selection (`AppState::clipboard_copy_requested`).
///
/// This exists because `vt100::Parser` has no hook for OSC 52 — it treats
/// selection-copy escapes as an "unhandled osc sequence" and silently drops
/// them — so without this scan, a `claude` session running inside argus's
/// embedded PTY loses the copy-on-select behavior it has when run directly
/// in a real terminal.
///
/// `partial` carries bytes across calls: a session's byte stream can split
/// a single escape sequence across two `on_pty_output` events (e.g. a large
/// selection's base64 payload landing on a PTY read boundary), so an
/// in-progress, not-yet-terminated sequence is buffered here until either it
/// completes or the pending-byte cap is hit.
fn scan_osc52(partial: &mut Vec<u8>, data: &[u8]) -> Option<String> {
    partial.extend_from_slice(data);

    let mut found = None;
    loop {
        let Some(start) = find_subslice(partial, OSC52_PREFIX) else {
            // No introducer anywhere in the buffer — keep only a tail that
            // could still grow into one on the next chunk.
            let keep = partial.len().min(OSC52_PREFIX.len() - 1);
            let from = partial.len() - keep;
            if keep > 0 && OSC52_PREFIX.starts_with(&partial[from..]) {
                partial.drain(..from);
            } else {
                partial.clear();
            }
            break;
        };
        partial.drain(..start);

        let body = &partial[OSC52_PREFIX.len()..];
        let bel = body.iter().position(|&b| b == 0x07).map(|i| (i, i + 1));
        let st = find_subslice(body, b"\x1b\\").map(|i| (i, i + 2));
        let terminator = match (bel, st) {
            (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        let Some((payload_end, seq_end)) = terminator else {
            // Sequence not finished yet — wait for more data, unless it's
            // grown unreasonably large (malformed stream).
            if partial.len() > OSC52_MAX_PENDING {
                partial.clear();
            }
            break;
        };

        let seq_len = OSC52_PREFIX.len() + seq_end;
        let payload = partial[OSC52_PREFIX.len()..OSC52_PREFIX.len() + payload_end].to_vec();
        partial.drain(..seq_len);

        // Skip clipboard *queries* (payload `?`) — argus can't answer them,
        // and a real copy-on-select never sends this form.
        if payload == b"?" || payload.ends_with(b";?") {
            continue;
        }
        // Payload is `Pc;Pd` (or bare `Pd`) — Pd is what we want, base64-encoded.
        let b64 = match payload.iter().position(|&b| b == b';') {
            Some(i) => &payload[i + 1..],
            None => &payload[..],
        };
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(b64) {
            found = Some(String::from_utf8_lossy(&decoded).into_owned());
        }
    }
    found
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::scan_osc52;

    #[test]
    fn finds_a_bel_terminated_sequence() {
        let mut partial = Vec::new();
        let found = scan_osc52(&mut partial, b"before\x1b]52;c;aGVsbG8=\x07after");
        assert_eq!(found.as_deref(), Some("hello"));
        assert!(partial.is_empty());
    }

    #[test]
    fn finds_a_string_terminator_sequence() {
        let mut partial = Vec::new();
        let found = scan_osc52(&mut partial, b"\x1b]52;c;aGVsbG8=\x1b\\rest");
        assert_eq!(found.as_deref(), Some("hello"));
    }

    #[test]
    fn reassembles_a_sequence_split_across_chunks() {
        let mut partial = Vec::new();
        assert_eq!(scan_osc52(&mut partial, b"noise \x1b]52;c;aGVs"), None);
        assert_eq!(scan_osc52(&mut partial, b"bG8=\x07 more noise").as_deref(), Some("hello"));
    }

    #[test]
    fn ignores_a_clipboard_query() {
        let mut partial = Vec::new();
        assert_eq!(scan_osc52(&mut partial, b"\x1b]52;c;?\x07"), None);
        assert!(partial.is_empty());
    }

    #[test]
    fn returns_the_last_of_multiple_sequences_in_one_chunk() {
        let mut partial = Vec::new();
        let found = scan_osc52(&mut partial, b"\x1b]52;c;Zmlyc3Q=\x07\x1b]52;c;c2Vjb25k\x07");
        assert_eq!(found.as_deref(), Some("second"));
    }

    #[test]
    fn caps_pending_bytes_on_an_unterminated_sequence() {
        let mut partial = Vec::new();
        let huge = vec![b'a'; super::OSC52_MAX_PENDING + 1];
        let mut chunk = b"\x1b]52;c;".to_vec();
        chunk.extend_from_slice(&huge);
        assert_eq!(scan_osc52(&mut partial, &chunk), None);
        assert!(partial.is_empty(), "pending buffer should be dropped once it exceeds the cap");
    }

    #[test]
    fn plain_output_leaves_no_residue() {
        let mut partial = Vec::new();
        assert_eq!(scan_osc52(&mut partial, b"just some regular claude output\n"), None);
        assert!(partial.is_empty());
    }
}
