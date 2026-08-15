---
status: accepted
---

# Notification hook is matcher-scoped, not treated as a blanket "blocked" signal

`Waiting` (ADR-0011's `Session Runtime Status`, extended) is meant to mean "the `claude`
process is blocked on a real user decision" — a tool permission prompt or an option picker.
Wiring the `Notification` hook with no `matcher` initially seemed to capture that, since the
hook fires whenever Claude Code "sends a notification to the user." In practice it also fires
for `idle_prompt` — a ~60s-idle reminder with nothing pending — so a Session that received one
prompt and then just sat there would flip to `Waiting` with no decision actually blocking it.

Claude Code's `Notification` hook config supports a `matcher` field that filters by
notification type *before* the hook command runs, same mechanism `PostToolUse` already uses
(e.g. `"Edit|Write"`). `CreateSessionUseCase::hook_args` now scopes the `Notification` entry to
`"permission_prompt|elicitation_dialog|elicitation_url_dialog|agent_needs_input"` — the four
types that block on an actual decision — and leaves the other five (`idle_prompt`,
`auth_success`, `elicitation_complete`, `elicitation_response`, `agent_completed`) unmatched, so
they never call back into argus at all. This was picked over reading the hook's JSON body for a
distinguishing field: matcher filtering needs no stdin/POST-body plumbing (ADR-0011 already
flagged `--settings`-embedded JSON bodies as a shell-quoting hazard across `cmd.exe`/`/bin/sh`),
and it fails closed — an unmatched notification type just leaves the Session's status wherever
`Stop`/`UserPromptSubmit` last put it, rather than requiring every notification type to be
explicitly classified in argus's own code.
