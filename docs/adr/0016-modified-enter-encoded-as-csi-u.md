---
status: accepted
---

# Modified Enter (Shift/Alt) is encoded as CSI-u, not collapsed to plain `\r`

`terminal_protocol::key_to_bytes` re-derives the raw bytes a real terminal would send down the
wire for each `crossterm::KeyEvent`, since there's no browser/OS terminal emulator underneath
the Session's PTY to do this translation for us. `KeyCode::Enter` was mapped unconditionally to
`vec![b'\r']`, discarding `key.modifiers` entirely — the only modifier handled anywhere in this
file was `CONTROL`, and only for `Char`. A `claude` process running inside a Session's terminal
panel therefore could never tell Shift+Enter, Alt+Enter, or Shift+Alt+Enter apart from a plain
Enter: all four collapsed to the same `\r` byte, so the CLI's "insert newline instead of submit"
binding never fired inside argus even though it works in a real terminal outside argus.

`KeyCode::Enter` now goes through `modified_enter_bytes`, which checks `SHIFT`/`ALT` and, when
either is set, emits `\x1b[13;{modifier_code}u` instead of `\r` — CSI-u, the modifier-reporting
convention used by xterm's `modifyOtherKeys` and the Kitty keyboard protocol, where key code 13
is Enter and the modifier code is `1 + shift(1) + alt(2) + ctrl(4)`. Plain Enter (no modifiers)
is untouched and still sends bare `\r`, so normal submit behavior for every other terminal
consumer is unaffected. `CONTROL` was left out of the modifier code on purpose: Ctrl+Enter has
no established meaning in this CLI's keybindings, so widening the encoding to cover it would add
an untested code path with nothing depending on it — it can be added the same way if a real use
shows up.

The alternative considered was decoding the modifier at the argus layer and injecting a
literal `\n` (or a multi-byte escape the target CLI's readline is known to accept) directly into
the child's stdin instead of relying on the child to interpret CSI-u itself. Rejected because
`terminal_protocol` is deliberately a dumb, protocol-level translation (`(input) -> bytes`, no
`AppState`, per the module's own doc comment) — special-casing "what does the process on the
other end of this PTY do with Enter" would leak child-specific behavior into a layer that has no
way to know what's running there. CSI-u is what a modifier-aware real terminal would send in the
same situation, so emitting it here keeps the layer's job unchanged: look like a normal
terminal, and let whatever is attached decide what to do with the sequence.
