---
status: accepted
---

# Session Runtime Status via a local HTTP hook server, GET+query not POST+JSON

Showing whether a Session's `claude` process is "thinking" or "idle" needed a signal from
inside `claude` itself, not PTY-output parsing (fragile — breaks on any CLI UI change). Claude
Code's own `UserPromptSubmit`/`Stop` hooks provide exactly that signal, so `CreateSessionUseCase`
now passes each spawned `claude` process `--session-id <uuid>` (Argus's own `SessionId`,
reused directly — no separate id mapping needed) and a `--settings` JSON string wiring both
hooks to call back into a `HookServer`: a `tiny_http`-based server bound to `127.0.0.1:0`
(OS-assigned ephemeral port, one dedicated accept-loop thread, same pattern as
`PortablePtyAdapter`'s output pump) started once at app startup and exposed to the application
layer through a new `HookCallbackPort`.

The hook `command` string is interpreted by whatever shell Claude Code invokes it through
(`cmd.exe` on Windows, `/bin/sh` elsewhere) — a JSON POST body would need different quoting
per platform to survive that shell. The callback URL instead carries `sessionId`/`event` as a
GET query string (`?sessionId=<uuid>&event=prompt_submitted`), which needs no shell-quoting at
all since both values are plain ASCII with no shell-special characters. This trades REST
convention for cross-platform simplicity — worth knowing before "fixing" it to a POST+JSON
body later, since that would reopen the quoting problem this design specifically avoids.
