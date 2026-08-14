use crossterm::event::{KeyCode, KeyEvent};

/// What a modal's caller should do after a key was routed through
/// [`apply`]: keep editing, run the modal's commit action, or discard it.
pub enum TextInputAction {
    Continue,
    Submit,
    Cancel,
}

/// The single text-editing keymap every modal in `app.rs` shares: Enter
/// submits, Esc cancels, Backspace/Char edit `input` in place, anything
/// else is ignored. One interface for all six `Modal` variants that carry
/// a free-text `input` field, instead of each re-implementing this match.
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
