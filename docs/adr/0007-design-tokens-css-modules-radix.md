---
status: accepted
---

# Reuse an existing design token set via CSS Modules + Radix, not Tailwind

The user had a pre-existing design token export (`theme.css`/`reset.css`) from another
project, as plain CSS custom properties (`--bg`, `--accent`, `--status-working`, etc.)
supporting multiple themes via `[data-theme]`. Since these are plain CSS variables rather than
a Tailwind config, argus consumes them directly through CSS Modules (`var(--token)` in every
component's stylesheet) instead of introducing Tailwind as a translation layer. `@radix-ui`
supplies unstyled primitives (dialogs, etc.) styled the same way; `lucide-react` supplies
icons.

Only the `dark` theme block was copied into `src/styles/theme.css` — the other eleven themes
in the original export were intentionally dropped, not lost. Adding one back is pasting another
`[data-theme='x']` block; no component changes are needed either way, since every component
already reads tokens by name rather than hardcoding colors.
