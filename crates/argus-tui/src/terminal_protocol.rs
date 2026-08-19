//! Pure xterm terminal-protocol encoding: translating crossterm's key/mouse
//! events into the raw bytes a real terminal would send down the wire. No
//! `AppState` dependency — every function here is a plain `(input) -> bytes`
//! translation, which is what makes it independently unit-testable without
//! spinning up any session/workspace state.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};

/// Maps a crossterm `MouseEventKind` to its SGR `(button code, is release)`
/// pair — `Drag` adds the `32` motion offset xterm uses to distinguish a
/// move-while-held from a fresh press, `Moved` reports the same motion
/// offset with the "no button" base code (`3`) since it's a hover with
/// nothing held, and wheel ticks are their own pseudo-buttons
/// (`64`/`65`/`66`/`67`) that are always "presses", never released.
pub fn sgr_mouse_button(kind: MouseEventKind) -> Option<(u8, bool)> {
    let base = |b: MouseButton| match b {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    };
    match kind {
        MouseEventKind::Down(b) => Some((base(b), false)),
        MouseEventKind::Drag(b) => Some((base(b) + 32, false)),
        MouseEventKind::Up(b) => Some((base(b), true)),
        MouseEventKind::Moved => Some((3 + 32, false)),
        MouseEventKind::ScrollUp => Some((64, false)),
        MouseEventKind::ScrollDown => Some((65, false)),
        MouseEventKind::ScrollLeft => Some((66, false)),
        MouseEventKind::ScrollRight => Some((67, false)),
    }
}

/// Translates a key press into the raw bytes a real terminal would send down
/// the wire — there is no browser terminal emulator underneath a TUI to do
/// this for us, so it's re-derived here.
pub fn key_to_bytes(key: &KeyEvent) -> Option<Vec<u8>> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        if let KeyCode::Char(c) = key.code {
            let c = c.to_ascii_lowercase();
            if c.is_ascii_alphabetic() {
                let byte = (c as u8) - b'a' + 1;
                return Some(vec![byte]);
            }
        }
    }
    match key.code {
        KeyCode::Char(c) => {
            let mut buf = [0u8; 4];
            Some(c.encode_utf8(&mut buf).as_bytes().to_vec())
        }
        KeyCode::Enter => Some(modified_enter_bytes(key.modifiers)),
        KeyCode::Backspace => Some(vec![0x7f]),
        KeyCode::Tab => Some(vec![b'\t']),
        KeyCode::BackTab => Some(b"\x1b[Z".to_vec()),
        KeyCode::Esc => Some(vec![0x1b]),
        KeyCode::Up => Some(b"\x1b[A".to_vec()),
        KeyCode::Down => Some(b"\x1b[B".to_vec()),
        KeyCode::Right => Some(b"\x1b[C".to_vec()),
        KeyCode::Left => Some(b"\x1b[D".to_vec()),
        KeyCode::Home => Some(b"\x1b[H".to_vec()),
        KeyCode::End => Some(b"\x1b[F".to_vec()),
        KeyCode::PageUp => Some(b"\x1b[5~".to_vec()),
        KeyCode::PageDown => Some(b"\x1b[6~".to_vec()),
        KeyCode::Delete => Some(b"\x1b[3~".to_vec()),
        KeyCode::Insert => Some(b"\x1b[2~".to_vec()),
        KeyCode::F(n) => function_key_bytes(n),
        _ => None,
    }
}

/// Plain Enter is `\r`, but Shift/Alt/Shift+Alt+Enter need to reach the child
/// PTY as a *distinct* sequence — apps like Claude Code use that to insert a
/// newline instead of submitting. We encode them as CSI-u (`\x1b[13;N u`),
/// the modifier convention xterm's `modifyOtherKeys`/the Kitty keyboard
/// protocol use for the Enter key (code 13), so terminal-aware children can
/// tell modified Enter apart from a plain one.
fn modified_enter_bytes(modifiers: KeyModifiers) -> Vec<u8> {
    let shift = modifiers.contains(KeyModifiers::SHIFT);
    let alt = modifiers.contains(KeyModifiers::ALT);
    if !shift && !alt {
        return vec![b'\r'];
    }
    // CSI-u modifier codes are 1 + bitmask(shift=1, alt=2, ctrl=4).
    let modifier_code = 1 + (shift as u8) + (alt as u8) * 2;
    format!("\x1b[13;{modifier_code}u").into_bytes()
}

fn function_key_bytes(n: u8) -> Option<Vec<u8>> {
    let code = match n {
        1 => "OP",
        2 => "OQ",
        3 => "OR",
        4 => "OS",
        5 => "[15~",
        6 => "[17~",
        7 => "[18~",
        8 => "[19~",
        9 => "[20~",
        10 => "[21~",
        11 => "[23~",
        12 => "[24~",
        _ => return None,
    };
    Some(format!("\x1b{code}").into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent { code, modifiers, kind: KeyEventKind::Press, state: KeyEventState::NONE }
    }

    #[test]
    fn ctrl_letter_encodes_as_control_byte() {
        let bytes = key_to_bytes(&key(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(bytes, Some(vec![3]));
    }

    #[test]
    fn plain_char_encodes_as_utf8() {
        let bytes = key_to_bytes(&key(KeyCode::Char('a'), KeyModifiers::NONE));
        assert_eq!(bytes, Some(b"a".to_vec()));
    }

    #[test]
    fn enter_encodes_as_carriage_return() {
        let bytes = key_to_bytes(&key(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(bytes, Some(vec![b'\r']));
    }

    #[test]
    fn shift_enter_encodes_as_csi_u_sequence() {
        let bytes = key_to_bytes(&key(KeyCode::Enter, KeyModifiers::SHIFT));
        assert_eq!(bytes, Some(b"\x1b[13;2u".to_vec()));
    }

    #[test]
    fn alt_enter_encodes_as_csi_u_sequence() {
        let bytes = key_to_bytes(&key(KeyCode::Enter, KeyModifiers::ALT));
        assert_eq!(bytes, Some(b"\x1b[13;3u".to_vec()));
    }

    #[test]
    fn shift_alt_enter_encodes_as_csi_u_sequence() {
        let bytes = key_to_bytes(&key(KeyCode::Enter, KeyModifiers::SHIFT | KeyModifiers::ALT));
        assert_eq!(bytes, Some(b"\x1b[13;4u".to_vec()));
    }

    #[test]
    fn arrow_keys_encode_as_csi_sequences() {
        assert_eq!(key_to_bytes(&key(KeyCode::Up, KeyModifiers::NONE)), Some(b"\x1b[A".to_vec()));
        assert_eq!(key_to_bytes(&key(KeyCode::Down, KeyModifiers::NONE)), Some(b"\x1b[B".to_vec()));
    }

    #[test]
    fn function_keys_encode_known_range() {
        assert_eq!(key_to_bytes(&key(KeyCode::F(1), KeyModifiers::NONE)), Some(b"\x1bOP".to_vec()));
        assert_eq!(key_to_bytes(&key(KeyCode::F(5), KeyModifiers::NONE)), Some(b"\x1b[15~".to_vec()));
        assert_eq!(key_to_bytes(&key(KeyCode::F(13), KeyModifiers::NONE)), None);
    }

    #[test]
    fn sgr_mouse_button_down_is_not_a_release() {
        assert_eq!(sgr_mouse_button(MouseEventKind::Down(MouseButton::Left)), Some((0, false)));
    }

    #[test]
    fn sgr_mouse_button_up_is_a_release() {
        assert_eq!(sgr_mouse_button(MouseEventKind::Up(MouseButton::Right)), Some((2, true)));
    }

    #[test]
    fn sgr_mouse_button_drag_adds_motion_offset() {
        assert_eq!(sgr_mouse_button(MouseEventKind::Drag(MouseButton::Left)), Some((32, false)));
    }

    #[test]
    fn sgr_mouse_button_wheel_ticks_are_pseudo_buttons() {
        assert_eq!(sgr_mouse_button(MouseEventKind::ScrollUp), Some((64, false)));
        assert_eq!(sgr_mouse_button(MouseEventKind::ScrollDown), Some((65, false)));
    }
}
