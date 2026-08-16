---
status: accepted
---

# Fuzzy finder's file search uses the `ignore` crate, not `GitPort`

The fuzzy finder (Ctrl+F) needs to walk and re-walk a Workspace's file tree
live, once per keystroke's worth of debounce (`FileSearchPort::walk_files`,
`grep_content`) — fast enough that a fast typist never sees a stale result
list. At the time of this decision, `File Status` (ADR-0009, since removed
along with the Git sidebar tab) already computed a `.gitignore`-aware view of
"is this path ignored" by shelling out to the user's own `git`, and it would
have been tempting to route the finder's own `include_ignored` toggle through
that same `GitPort` so there was exactly one implementation of "ignored" in
the codebase.

`FileSearchPort` (`argus-application::ports::file_search_port`, backed by
`RipgrepSearchAdapter` in `argus-infrastructure::search`) deliberately does
not do this — it uses the `ignore` crate's own gitignore parser instead,
walking the filesystem directly rather than spawning a `git status
--ignored` process per keystroke. Spawning a process on every debounced
keystroke (`GREP_DEBOUNCE`, 90ms) would add process-spawn latency to every
character typed into the finder; the `ignore` crate parses `.gitignore`
files in-process as part of the same walk that lists files, which is what
lets Files-mode search feel instant even on a large repository.

The tradeoff at the time: `FileSearchPort` and `GitPort`/`File Status` were two
independent implementations of "what counts as ignored," and could
disagree at the edges — nested `.gitignore` precedence and the user's global
gitignore are exactly the cases `ignore` and `git status --ignored` have
historically diverged on in edge cases upstream. Now that `GitPort`/`File
Status` has been removed, `FileSearchPort`'s `ignore`-crate-backed walk is the
only implementation of "ignored" in the codebase; the fuzzy finder's
`include_ignored` toggle (Ctrl+G) remains a best-effort "mostly matches
`.gitignore`" filter scoped only to search results.

## Considered Options

Routing `FileSearchPort::walk_files` through `GitPort` (e.g. `git ls-files`
plus `git status --ignored` to reconstruct the ignored set) was considered
and rejected on latency grounds — see above. It remains an option to revisit
if the two implementations are ever found to disagree in a way that
confuses users, rather than merely in git-internals edge cases.
