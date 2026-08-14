---
status: accepted
---

# GitPanel shells out to the system `git` CLI, not git2-rs

`GitPanel` needs status, diff, staging, commit, history, submodule, and push/pull/fetch/branch
data. We chose to shell out to the user's own `git` binary and parse its output, rather than
add `git2`/`libgit2` as a Rust dependency. `git2` avoids process-spawn overhead and works
without `git` on PATH, but historically causes native-linking pain on Windows (OpenSSL/libssh2)
and can silently diverge from the user's actual git behavior (credential helpers, GPG signing,
hooks, submodule config) since it reimplements git rather than running it. Shelling out
inherits the user's real git configuration for free and matches what a terminal-based git
workflow already assumes is installed — argus already expects users to run `claude` inside git
repos.

Two scope decisions made alongside this:

- **Staging is whole-file only**, not hunk-level. Partial-file staging needs diff-hunk parsing
  and `git apply --cached`; deferred as a possible increment rather than shipped in v2.
- **Each initialized git submodule is its own Git Repository** in the panel (own stage/commit/
  history/diff), not just a status badge on the parent — see **Git Repository** in CONTEXT.md.
  Commands for a submodule's repository run with that submodule's directory as cwd.
- **Push/pull/fetch and branch switching are in scope**; amending a previous commit's message is
  not — only composing the next commit's message.
