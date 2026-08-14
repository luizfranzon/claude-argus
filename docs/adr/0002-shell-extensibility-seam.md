---
status: accepted
---

# Model the shell as generic Regions/Panels even though v1 only has Terminal

The long-term plan is a community-installable widget system where third-party panels can
appear in the sidebar, the grid, or the top/bottom bars. The actual plugin technology
(iframe, WASM, etc.) is deliberately undecided. Rather than hardcode "there is only ever a
terminal," `argus-domain::shell` models the shell as four named `RegionKind`s
(`SidebarLeft`, `Grid`, `TopBar`, `BottomBar`), each holding a list of `Panel`s identified by
an open-ended `PanelKind` enum. v1 ships exactly one variant,
`PanelKind::Terminal(WorkspaceId)`.

This is a light seam (~150 lines across `region.rs`/`panel.rs`/`layout.rs`), not a plugin
subsystem — no loading, sandboxing, or plugin API exists yet. The payoff is that adding a
second panel kind later is an enum variant, not a redesign of `Workspace`/`ShellLayout`/the
Tauri command surface.
