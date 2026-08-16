---
status: accepted
---

# Session, not Workspace, owns the PTY

ADR-0002 and ADR-0003 modeled `PanelKind::Terminal` around a `WorkspaceId` and gave each
Workspace exactly one Tauri Channel streaming exactly one PTY's output — a Workspace *was* its
one running `claude` process. The Agents sidebar tab and multi-terminal `Grid` (v3) need a
Workspace to run several `claude` processes concurrently, grouped and filtered by feature, so a
new unit — `Session` — now owns the PTY and the Channel instead. A Workspace becomes a
container of N Sessions rather than a 1:1 wrapper around one.

Concretely: `PanelKind::Terminal` now carries a `SessionId`, not a `WorkspaceId`; the
per-unit Tauri Channel from ADR-0003 is created per-Session; and `close_requires_confirmation`
(ADR-0006) applies per-Session, with closing a Workspace cascading to close all its Sessions.
The "no persistence across restarts" policy from ADR-0006 is unchanged and now covers Sessions
and Feature Groups too — nothing about Session lifecycle survives an app restart, same as
Workspace today.

The alternative considered was keeping one PTY per Workspace and letting a user open multiple
Workspaces on the same directory to get parallel agents. Rejected because Workspace already
means "one directory tab" throughout the UI (File Explorer, Editor are all
Workspace-scoped by directory) — duplicating tabs per directory would have duplicated those
panels too, for no benefit, instead of just adding a second dimension (Session) under the
existing Workspace.
