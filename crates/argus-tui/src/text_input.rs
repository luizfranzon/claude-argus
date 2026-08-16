use crossterm::event::{KeyCode, KeyEvent};

/// What a modal's caller should do after a key was routed through
/// [`apply`]: keep editing, run the modal's commit action, or discard it.
pub enum TextInputAction {
    Continue,
    Submit,
    Cancel,
}

/// Appends a paste's contents to a single-line modal `input`. These fields
/// have no notion of a line break, so embedded newlines (a multi-line
/// clipboard paste, or the trailing newline many terminals add) are dropped
/// rather than being forwarded — without this a pasted multi-line value
/// would otherwise need `apply`'s `KeyCode::Enter` handling, which submits
/// the modal instead of inserting a line break.
pub fn apply_paste(input: &mut String, text: &str) {
    input.extend(text.chars().filter(|c| *c != '\n' && *c != '\r'));
}

/// The shared text-editing keymap for every `Modal` variant's free-text
/// `input` field: Enter submits, Esc cancels, Backspace/Char edit `input` in
/// place, anything else is ignored. One interface for all six `Modal`
/// variants, instead of each re-implementing this match. The fuzzy finder's
/// query field does *not* go through this — it interleaves Tab/Ctrl+T/Ctrl+G
/// with finder-specific Enter/Esc semantics that don't fit a plain
/// submit/cancel shape, so `app::keys::finder::handle_finder_key` has its
/// own compound keymap instead.
pub fn apply(input: &mut String, key: KeyEvent) -> TextInputAction {
    match key.code {
        KeyCode::Enter => TextInputAction::Submit,
        KeyCode::Esc => TextInputAction::Cancel,
        KeyCode::Backspace => {
            input.pop();
            TextInputAction::Continue
        }
        KeyCode::Char(c) => {
            input.push(c);
            TextInputAction::Continue
        }
        _ => TextInputAction::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn char_appends_and_continues() {
        let mut input = "ab".to_string();
        let action = apply(&mut input, key(KeyCode::Char('c')));
        assert!(matches!(action, TextInputAction::Continue));
        assert_eq!(input, "abc");
    }

    #[test]
    fn backspace_pops_and_continues() {
        let mut input = "abc".to_string();
        let action = apply(&mut input, key(KeyCode::Backspace));
        assert!(matches!(action, TextInputAction::Continue));
        assert_eq!(input, "ab");
    }

    #[test]
    fn backspace_on_empty_input_stays_empty() {
        let mut input = String::new();
        let action = apply(&mut input, key(KeyCode::Backspace));
        assert!(matches!(action, TextInputAction::Continue));
        assert_eq!(input, "");
    }

    #[test]
    fn enter_submits_without_touching_input() {
        let mut input = "abc".to_string();
        let action = apply(&mut input, key(KeyCode::Enter));
        assert!(matches!(action, TextInputAction::Submit));
        assert_eq!(input, "abc");
    }

    #[test]
    fn esc_cancels_without_touching_input() {
        let mut input = "abc".to_string();
        let action = apply(&mut input, key(KeyCode::Esc));
        assert!(matches!(action, TextInputAction::Cancel));
        assert_eq!(input, "abc");
    }

    #[test]
    fn other_keys_are_ignored() {
        let mut input = "abc".to_string();
        let action = apply(&mut input, key(KeyCode::Left));
        assert!(matches!(action, TextInputAction::Continue));
        assert_eq!(input, "abc");
    }
}
