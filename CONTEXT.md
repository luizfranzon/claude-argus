# argus

Desktop app that orchestrates Claude Code agent sessions. A user opens a directory as a
**Workspace**; argus keeps one running `claude` process per Workspace and renders its
terminal output.

## Language

**Workspace**:
A directory opened in argus, backed by exactly one running `claude` process attached to a
PTY. The unit represented by a tab in the top bar.
_Avoid_: Session, Tab, Project — "Session" was rejected because it undersells the directory
binding; "Tab" was rejected because it's a UI term, not a domain one.

**Region**:
A named, extensible area of the app shell that can host Panels: `SidebarLeft`, `Grid`,
`TopBar`, `BottomBar`. All four exist from app startup, even when empty — this is what lets
a future panel kind (e.g. a community widget) be placed anywhere without changing the shell's
shape.

**Panel**:
A single thing displayed inside a Region. Identified by a `PanelKind`. In v1 the only kind is
`Terminal`, which carries the `WorkspaceId` it renders.

**PanelKind**:
What a Panel displays. Deliberately an open-ended enum — v1 shipped only `Terminal(WorkspaceId)`;
v2 adds `Editor(WorkspaceId)`, `FileExplorer(WorkspaceId)`, and `GitPanel(WorkspaceId)`. This is
the seam a future community widget system extends, not a fixed set.

**ShellLayout**:
The whole app shell: every Region and every Panel placed into one. Owned by the
`WorkspaceManager` and mutated whenever a Workspace is created or removed.

**Close confirmation**:
The rule deciding whether closing a Workspace must first ask the user to confirm. In v1 this
is always required while a Workspace is `Starting`/`Running`/`AwaitingCloseConfirmation` —
modeled as its own function (`close_requires_confirmation`) so a future "only confirm if busy"
rule touches one place, not every call site.

**Split**:
The layout state of a Workspace's `Grid` cell once its `Editor` panel opens: `Terminal` and
`Editor` render side by side, resizable, rather than one replacing the other. A Workspace with
no open file has no Split — the cell holds only the `Terminal` panel.

**File Explorer**:
The `FileExplorer` panel: a read/write tree view of a Workspace's directory, decorated with
each file's `File Status` and CRUD actions (create, rename, delete, move). Ignored paths (per
`.gitignore`) still appear, dimmed. Scoped to one Workspace, like every sidebar panel.
_Avoid_: File tree, Explorer (bare) — always qualify with "File" to not collide with the Git
panel's own tree-like UI.

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

**Git Repository** (in the Git Panel sense):
One independently-stageable/committable git repository as shown in the `GitPanel`. A Workspace
whose directory is a git repo shows exactly one; each initialized git submodule beneath it adds
another, since submodules commit independently of their parent repo. Not the same as
`Workspace` — a Workspace with no `.git` shows zero Git Repositories and the GitPanel explains
that instead of showing controls.
_Avoid_: Repo, submodule (submodule is a *kind* of Git Repository once initialized, not a
separate concept the UI treats differently).

**File Status**:
A file's working-tree state relative to its Git Repository: untracked, modified (unstaged),
staged, or conflicted. Drives both the File Explorer's per-file decoration and the GitPanel's
changed-files list. Computed by shelling out to the user's own `git` — see ADR-0009.

**Path reference**:
The text a file/folder drag from the File Explorer inserts into a Workspace's Terminal: the
path relative to that Workspace's root, quoted only if it contains spaces or shell-special
characters. Inserted at the cursor (`term.paste`), never executed — the user still presses
Enter themselves.
