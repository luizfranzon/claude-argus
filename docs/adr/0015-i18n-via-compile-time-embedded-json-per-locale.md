---
status: accepted
---

# UI text is externalized into per-locale JSON files, embedded at compile time

Argus's TUI has always had its ~230 user-facing strings (key hints, modal
copy, status/notification messages) hardcoded in Portuguese across
`crates/argus-tui/src`. To let the community translate the app without
needing a maintainer to touch every string, we're introducing a top-level
`languages/` folder with one flat JSON file per locale — `pt_BR.json`,
`en_US.json`, `es_ES.json` — and a new `argus-tui/src/i18n.rs` module that
loads them.

The files are pulled in via `include_str!` and baked into the binary at
compile time, not read from disk at runtime. This was a deliberate trade
against a "drop a JSON next to the binary" runtime-loader design: contributing
a translation means editing a file and opening a PR against the argus repo,
same as any other change, and the app can never end up in a state where a
malformed or missing on-disk file breaks startup. The cost is that a new or
corrected translation requires a new release to reach users — accepted
because release cadence for this project is fast enough that this isn't a
real burden, and PR-based contribution was the explicit goal.

Keys are flat strings (not nested JSON objects) using a
`domain.action.type` convention — e.g. `session.create.error`,
`workspace.rename.confirm_title` — chosen over a per-screen or
per-source-file naming scheme so the key stays stable if the UI is
reorganized, since it names the domain action, not the widget that happens
to display it today. Values interpolate named placeholders (`{error}`,
`{name}`) via a small `t(key, &[(name, value)])` helper — no external
templating crate.

## Missing-translation policy has no runtime middle ground

Because only `pt_BR.json` starts complete, we needed a policy for what
happens when `en_US`/`es_ES` haven't caught up on a given key. We rejected
silent runtime fallback to `pt_BR` for incomplete languages: a `cargo test`
enforces that `en_US.json` and `es_ES.json` contain exactly the same key set
as `pt_BR.json`, and the build fails otherwise. In other words, "partially
translated" is not a state the app can ship in — every language file is
complete or the CI is red. A separate test scans the source for `t("...")`
call sites and confirms each literal key exists in `pt_BR.json`, catching
typos before merge. The one runtime fallback that remains is a last resort:
if a key somehow doesn't exist anywhere (a bug slipping past both tests), `t`
returns the raw key string instead of panicking, so a translation bug
degrades to ugly text rather than a crashed session.

## Language resolution

Active locale is resolved via the `sys-locale` crate reading the OS locale,
overridable with the `ARGUS_LANG` environment variable, falling back to
`pt_BR` if the detected/requested locale isn't one Argus ships. `sys-locale`
was added specifically because Windows has no `LANG`/`LC_ALL` environment
convention — a maintainer or contributor testing on Windows would otherwise
never see auto-detection do anything.

## Where the module lives

`i18n.rs` lives inside `argus-tui`, not `argus-infrastructure`. Only the TUI
crate consumes translated text today; there is no second consumer to justify
placing it in the shared infrastructure layer.

## Considered options

**Runtime-loaded translation files** (read from a directory beside the
binary or in a user config dir) was the alternative to compile-time
embedding. Rejected: it would let users swap translations without a
rebuild, but the project has no existing config/settings-loading mechanism
to hook into, and it reopens the "app must handle a missing/corrupt
on-disk file" failure mode for no benefit given the community's contribution
path is a PR, not a drop-in file.

**Nested JSON objects mirroring the dotted key hierarchy** was considered
and rejected in favor of flat string keys — with ~230 keys, a flat map is a
trivial `HashMap<String, String>` via `serde_json`, avoids indentation drift
between the three files in a diff, and is easier for a first-time contributor
to edit without understanding nested structure.
