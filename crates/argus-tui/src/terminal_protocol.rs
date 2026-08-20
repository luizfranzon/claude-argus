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
    match key.code {
        KeyCode::Char(c) => Some(char_key_bytes(c, key.modifiers)),
        KeyCode::Enter => Some(modified_enter_bytes(key.modifiers)),
        // Plain Backspace is DEL, but Alt+Backspace needs to reach the child
        // as ESC-DEL — the convention readline (and Claude Code's own line
        // editor) binds to backward-kill-word — otherwise it's
        // indistinguishable from a plain single-char delete.
        KeyCode::Backspace => {
            if key.modifiers.contains(KeyModifiers::ALT) {
                Some(vec![0x1b, 0x7f])
            } else {
                Some(vec![0x7f])
            }
        }
        KeyCode::Tab => Some(vec![b'\t']),
        KeyCode::BackTab => Some(b"\x1b[Z".to_vec()),
        KeyCode::Esc => Some(vec![0x1b]),
        KeyCode::Up => Some(modified_csi_bytes('A', key.modifiers)),
        KeyCode::Down => Some(modified_csi_bytes('B', key.modifiers)),
        KeyCode::Right => Some(modified_csi_bytes('C', key.modifiers)),
        KeyCode::Left => Some(modified_csi_bytes('D', key.modifiers)),
        KeyCode::Home => Some(modified_csi_bytes('H', key.modifiers)),
        KeyCode::End => Some(modified_csi_bytes('F', key.modifiers)),
        KeyCode::PageUp => Some(b"\x1b[5~".to_vec()),
        KeyCode::PageDown => Some(b"\x1b[6~".to_vec()),
        KeyCode::Delete => Some(b"\x1b[3~".to_vec()),
        KeyCode::Insert => Some(b"\x1b[2~".to_vec()),
        KeyCode::F(n) => function_key_bytes(n),
        _ => None,
    }
}

/// Encodes a character key with its modifiers. `Ctrl` collapses a letter to
/// its control byte (`Ctrl+A` → `0x01`) or, for underscore, to `0x1f` (the
/// `Ctrl+_`/`Ctrl+Shift+-` undo-last-edit binding — some terminals report the
/// shifted `-` as `Char('_')` directly, others as `Char('-')` with `SHIFT`
/// held, so both are recognized). `Alt` then prefixes the result with `ESC`
/// — the standard "meta sends escape" convention — which is what lets
/// Alt-word-navigation (`Alt+B`/`Alt+F`), `Alt+Y` (paste-history cycling),
/// and the other documented `Alt+<letter>` shortcuts reach the child as
/// something other than a bare keypress.
fn char_key_bytes(c: char, modifiers: KeyModifiers) -> Vec<u8> {
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let shift = modifiers.contains(KeyModifiers::SHIFT);
    let alt = modifiers.contains(KeyModifiers::ALT);
    let mut bytes = if ctrl && c.to_ascii_lowercase().is_ascii_alphabetic() {
        vec![(c.to_ascii_lowercase() as u8) - b'a' + 1]
    } else if ctrl && (c == '_' || (c == '-' && shift)) {
        vec![0x1f]
    } else {
        let mut buf = [0u8; 4];
        c.encode_utf8(&mut buf).as_bytes().to_vec()
    };
    if alt {
        bytes.insert(0, 0x1b);
    }
    bytes
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

/// Cursor-movement keys (arrows, Home/End) encode modifiers as
/// `CSI 1;<code><final>` — the same `xterm`/Kitty modifier convention as
/// `modified_enter_bytes`, but without CSI-u's leading keycode field, since
/// these already have dedicated final bytes. Plain (unmodified) presses omit
/// the `1;<code>` prefix entirely, matching what most terminals send and
/// what child readline/editor implementations expect.
fn modified_csi_bytes(final_byte: char, modifiers: KeyModifiers) -> Vec<u8> {
    if modifiers.is_empty() {
        return format!("\x1b[{final_byte}").into_bytes();
    }
    let shift = modifiers.contains(KeyModifiers::SHIFT);
    let alt = modifiers.contains(KeyModifiers::ALT);
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let modifier_code = 1 + (shift as u8) + (alt as u8) * 2 + (ctrl as u8) * 4;
    format!("\x1b[1;{modifier_code}{final_byte}").into_bytes()
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
    fn alt_char_prefixes_esc_meta_escape() {
        // Alt+B / Alt+F are Claude Code's word-navigation shortcuts.
        assert_eq!(key_to_bytes(&key(KeyCode::Char('b'), KeyModifiers::ALT)), Some(vec![0x1b, b'b']));
        assert_eq!(key_to_bytes(&key(KeyCode::Char('f'), KeyModifiers::ALT)), Some(vec![0x1b, b'f']));
        // Alt+Y cycles paste history after Ctrl+Y.
        assert_eq!(key_to_bytes(&key(KeyCode::Char('y'), KeyModifiers::ALT)), Some(vec![0x1b, b'y']));
    }

    #[test]
    fn ctrl_alt_letter_combines_control_byte_with_meta_escape() {
        let bytes = key_to_bytes(&key(KeyCode::Char('c'), KeyModifiers::CONTROL | KeyModifiers::ALT));
        assert_eq!(bytes, Some(vec![0x1b, 3]));
    }

    #[test]
    fn ctrl_underscore_encodes_undo_byte() {
        assert_eq!(key_to_bytes(&key(KeyCode::Char('_'), KeyModifiers::CONTROL)), Some(vec![0x1f]));
    }

    #[test]
    fn ctrl_shift_dash_encodes_undo_byte() {
        assert_eq!(
            key_to_bytes(&key(KeyCode::Char('-'), KeyModifiers::CONTROL | KeyModifiers::SHIFT)),
            Some(vec![0x1f])
        );
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
    fn ctrl_arrow_encodes_modifier_in_csi_sequence() {
        assert_eq!(
            key_to_bytes(&key(KeyCode::Left, KeyModifiers::CONTROL)),
            Some(b"\x1b[1;5D".to_vec())
        );
        assert_eq!(
            key_to_bytes(&key(KeyCode::Right, KeyModifiers::CONTROL)),
            Some(b"\x1b[1;5C".to_vec())
        );
    }

    #[test]
    fn alt_backspace_encodes_as_esc_del() {
        assert_eq!(key_to_bytes(&key(KeyCode::Backspace, KeyModifiers::ALT)), Some(vec![0x1b, 0x7f]));
    }

    #[test]
    fn plain_backspace_encodes_as_del() {
        assert_eq!(key_to_bytes(&key(KeyCode::Backspace, KeyModifiers::NONE)), Some(vec![0x7f]));
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
