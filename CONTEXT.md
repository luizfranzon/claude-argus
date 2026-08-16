# argus

Desktop app that orchestrates Claude Code agent sessions. A user opens a directory as a
**Workspace**; argus keeps one running `claude` process per Workspace and renders its
terminal output.

## Language

**Workspace**:
A directory opened in argus. The unit represented by a tab in the top bar. Hosts one or more
concurrently running `Session`s; closing a Workspace closes all of its Sessions.
_Avoid_: Tab, Project — "Tab" was rejected because it's a UI term, not a domain one.

**Session**:
One running `claude` process attached to its own PTY, living inside a Workspace. A Workspace
can host multiple Sessions in parallel — each starts, runs, and closes independently, and each
renders as its own `Terminal` panel in the `Grid`. Auto-named ("Session 1", "Session 2", ...)
on creation, renameable afterward. Subject to the same `Close confirmation` rule as a Workspace.
Not persisted across restarts (see ADR-0006, extended by ADR-0010).
_Avoid_: Agent, Terminal — "Terminal" is the panel that renders a Session, not the process
itself.

**Session Runtime Status**:
Whether a Session's `claude` process is actively working on a prompt (`Thinking`), sitting
idle between prompts (`Idle`), or blocked on a user decision (`Waiting` — a tool permission
prompt or an option picker), shown as a dot/diamond on its card/cell. Derived from Claude
Code's own `UserPromptSubmit`/`Stop`/`Notification` hooks calling back into argus, not by
parsing PTY output. The `Notification` hook is matcher-scoped to only the notification types
that genuinely block on a decision (`permission_prompt`, `elicitation_dialog`,
`elicitation_url_dialog`, `agent_needs_input`) — Claude Code's other notification types, most
importantly `idle_prompt` (a ~60s-idle reminder with nothing pending), are deliberately left
unmatched so a merely-idle Session never gets misreported as `Waiting` (see ADR-0012). Distinct
from a Session's `Close confirmation` status (Starting/Running/.../Terminating) — this is about
what the process is *doing*, not whether it's alive.
_Avoid_: Session Status (collides with the close-confirmation status).

**Feature Group**:
A user-defined bucket (name + color) for organizing a Workspace's Sessions, created via a
modal. Scoped to one Workspace — never shared across Workspaces, so grouping never mixes
Sessions from different directories. A Session belongs to exactly one Feature Group at a time,
or to the built-in `Ungrouped` bucket. Deleting a Feature Group moves its Sessions to
`Ungrouped` rather than closing them. Not persisted across restarts, same as `Session`.
_Avoid_: Group (bare), Tag, Category — "Tag" was rejected because a Session can only belong to
one Feature Group at a time, unlike a tag.

**Session Filter**:
The Feature Group currently selected for a Workspace — either a specific Feature Group, the
built-in `Ungrouped` bucket, or the built-in `All` view — which determines which Sessions
render in that Workspace's `Grid`. Remembered per Workspace, so switching the active Workspace
in the top bar and back restores its last Session Filter.
_Avoid_: Active group, filter (bare).

**Region**:
A named, extensible area of the app shell that can host Panels: `SidebarLeft`, `Grid`,
`TopBar`, `BottomBar`. All four exist from app startup, even when empty — this is what lets
a future panel kind (e.g. a community widget) be placed anywhere without changing the shell's
shape. As of v3, `Grid` can hold multiple `Terminal` panels at once — one per `Session` of the
active Workspace that passes its `Session Filter` — tiled as a freely resizable (tmux-style)
split tree the user can also rearrange by dragging one Session's cell onto another to swap
positions.

**Panel**:
A single thing displayed inside a Region. Identified by a `PanelKind`.

**PanelKind**:
What a Panel displays. Deliberately an open-ended enum — v1 shipped only `Terminal(WorkspaceId)`;
v2 adds `Editor(WorkspaceId)` and `FileExplorer(WorkspaceId)`; v3
changes `Terminal` to carry a `SessionId` instead of a `WorkspaceId`, since the PTY it renders
now belongs to a Session, not directly to a Workspace (see ADR-0010). This is the seam a future
community widget system extends, not a fixed set.

**ShellLayout**:
The whole app shell: every Region and every Panel placed into one. Owned by the
`WorkspaceManager` and mutated whenever a Workspace is created or removed.

**Close confirmation**:
The rule deciding whether closing a Workspace or a Session must first ask the user to confirm.
In v1 this is always required while a Workspace is `Starting`/`Running`/`AwaitingCloseConfirmation` —
modeled as its own function (`close_requires_confirmation`) so a future "only confirm if busy"
rule touches one place, not every call site. v3 applies the same rule to closing an individual
`Session`; closing a Workspace still confirms once and, if accepted, closes all of its Sessions.

**Split**:
The layout state of a Workspace's `Grid` cell once its `Editor` panel opens: `Terminal` and
`Editor` render side by side, resizable, rather than one replacing the other. A Workspace with
no open file has no Split — the cell holds only the `Terminal` panel.

**File Explorer**:
The `FileExplorer` panel: a read/write tree view of a Workspace's directory, decorated with
each file's `File Status`, with CRUD actions (create, rename, delete, move). Ignored paths (per
`.gitignore`) still appear, dimmed. Scoped to one Workspace, like every sidebar panel.
_Avoid_: File tree, Explorer (bare) — always qualify with "File" to not collide with other
panel kinds.

**File Status**:
A file's working-tree state relative to its git repository: modified, added, deleted, renamed,
untracked, or conflicted. Drives the File Explorer's per-file badge — a single colored letter,
mirroring VS Code — computed by shelling out to the user's own `git` (`GitStatusPort`, same
git-CLI-vs-git2 tradeoff as ADR-0009, though that ADR's own `GitPanel`/`GitPort` were removed).
A directory's badge is the combined status of every changed file at or beneath it, propagated
recursively up to the Workspace root — so a single modified file deep in the tree marks every
ancestor folder above it, not just its immediate parent.
_Avoid_: Git status (bare) — always say "File Status" to keep this a File Explorer concept, not
a claim that argus has a Git panel again.

**Editor**:
The `Editor` panel: a Monaco-backed code editor opened by clicking a file in the File Explorer.
Supports multiple open files as tabs, each independently dirty/saved. Reloading on an
**External edit conflict** (below) is per-file.
_Avoid_: Monaco (that's the library, not the domain concept).

**External edit conflict**:
The situation where a file open in the Editor with unsaved changes is also modified on disk by
something outside argus — almost always the `claude` process running in that Workspace's own
Terminal. Detected via file watcher. Never resolved silently: the user is always asked to keep
the Editor's version or reload the on-disk version. A clean (non-dirty) open file reloads
automatically instead, since there's nothing to lose.

**Path reference**:
The text a file/folder drag from the File Explorer inserts into a Workspace's Terminal: the
path relative to that Workspace's root, quoted only if it contains spaces or shell-special
characters. Inserted at the cursor (`term.paste`), never executed — the user still presses
Enter themselves.

**Locale**:
The language argus's own UI (key hints, modal copy, status/notification text) renders in —
distinct from any language the user's `claude` process or shell happens to use, which argus
never touches. Resolved once at startup: detected from the OS, overridable by the user,
falling back to Portuguese if neither names a language argus ships (see ADR-0015).
_Avoid_: Language (bare) — reserve "Locale" for argus's own UI; a Workspace's programming
language or a Session's spoken-language output is not this.

**Translation Key**:
The stable identifier argus's UI code looks up to render a piece of UI text in the active
Locale — named for the domain action it represents (e.g. "creating a Session", "confirming a
Workspace close"), not for the screen or widget currently displaying it, so reorganizing the
UI doesn't require renaming keys. Every Translation Key must resolve in every Locale argus
ships; there is no such thing as a partially-translated Locale (see ADR-0015).
