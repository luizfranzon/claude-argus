---
status: accepted
---

# v2 scope: File Explorer, Monaco Editor, and Git Panel, superseding ADR-0005

ADR-0005 deliberately cut the sidebar, file explorer, and Git tooling from v1. This ADR is
that follow-up: v2 fills `SidebarLeft` with two new panels, `FileExplorer(WorkspaceId)` and
`GitPanel(WorkspaceId)`, and adds `Editor(WorkspaceId)` to `Grid`, split alongside
`Terminal(WorkspaceId)` for the same Workspace (see **Split** in CONTEXT.md). No new Region is
needed — the `RegionKind`/`PanelKind` seam from ADR-0002 absorbs all three additions as plain
enum variants, which is exactly what that seam was built for.

All of this state (open Editor tabs, expanded File Explorer folders, active sidebar tab) is
scoped per-Workspace and, consistent with ADR-0006's no-persistence stance, not saved across
app restarts.

Git integration specifics (backend choice, submodule handling, staging granularity, sync
scope) are recorded separately in ADR-0009, since that's a distinct trade-off from the shell
layout question this ADR answers.
