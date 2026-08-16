//! Per-concern key handling, split the same way `ui::sidebar` splits the
//! sidebar's own tabs into separate render modules — one file per input
//! context (`Focus::Terminal`, each `SidebarTab`, each `Modal` shape, the
//! fuzzy finder), so a change to one context's keymap touches one small
//! file.

mod agents;
mod explorer;
mod finder;
mod modal;
mod sidebar;
mod terminal;
