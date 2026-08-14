---
status: accepted
---

# Workspace lifecycle: no persistence, no single-instance, always-confirm close

Several related lifecycle behaviors were decided together during planning, each deviating
from what a reader familiar with VS Code/Chrome-style apps might expect by default:

- **No persistence across restarts.** Every launch starts clean; no saved tab list. Simpler
  than serializing PTY sessions, at the cost of losing your tab layout on restart.
- **No single-instance/IPC.** Running `argus <dir>` while argus is already open always spawns
  a brand new OS-level window/process, never focuses an existing one. `tauri-plugin-single-instance`
  is deliberately not added — don't add it out of habit later without revisiting this.
- **Closing a Workspace always asks for confirmation** while it's running (see
  `close_requires_confirmation` in CONTEXT.md), with no idle/busy detection in v1.
- **A `claude` process exiting on its own (crash or normal exit) auto-closes its Workspace**
  with no "process ended — restart?" prompt, matching plain terminal-app behavior rather than
  an IDE-style persistent pane.

These were explicit user choices favoring simplicity over convenience features, made when a
richer alternative (restore tabs, reuse window, smart close detection, restart prompt) was
raised and rejected for each one.
