---
status: accepted
---

# Split the Rust core into domain/application/infrastructure crates

argus needs Rust to own all business logic (Workspace lifecycle, PTY session management,
close-confirmation rules), per an explicit user requirement for Clean Architecture. We split
Rust into three crates — `argus-domain` (pure entities and rules, no I/O or async runtime),
`argus-application` (use cases and ports as traits), `argus-infrastructure` (real adapters:
`portable-pty`, PATH resolution, git CLI, filesystem) — with `argus-tui` as a thin composition
root that wires adapters to use cases and renders the terminal UI.

The dependency direction is enforced by the Cargo dependency graph itself: `argus-domain`
depends on nothing project-specific, `argus-application` depends only on `argus-domain`, and
`argus-infrastructure` depends on both to implement their ports. This also makes
`argus-domain`/`argus-application` `cargo test`-able with fakes, with zero `portable-pty` or
UI dependency in their tree, and keeps the door open to reusing the core in a future headless
mode.

## Considered Options

A single crate with modules instead of separate crates was considered and rejected — Cargo
modules don't enforce the dependency direction the way separate crates with scoped
`Cargo.toml` dependencies do, so nothing would stop domain code from silently reaching into
`portable-pty` or UI concerns over time.
