---
status: superseded by ADR-0008
---

# v1 scope is terminal + Workspace tabs only — no grid, sidebar, or Git tools

The full product vision includes a resizable grid of multiple Claude Code instances, a file
explorer, Git tooling with a diff viewer, and eventually community widgets. During planning,
the user was explicitly asked to name the smallest useful v1, and chose "terminal + tabs
only": one directory, one `claude` process, one xterm.js view per Workspace tab — nothing
else. This was a deliberate product-scope decision, not a technical limitation; the
`RegionKind`/`PanelKind` seam (ADR 0002) exists specifically so the grid and sidebar can be
added later without redesigning the domain model.

Anyone reading this codebase and wondering where the file explorer or Git panel is: they were
scoped out of v1 on purpose, not forgotten.
