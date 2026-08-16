use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;

/// A file's working-tree state relative to its Git Repository. Drives the
/// File Explorer's per-file decoration (a single colored letter, mirroring
/// VS Code) and, propagated upward, the same decoration on every ancestor
/// directory up to the Workspace root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

impl FileStatus {
    /// Priority when combining multiple files' statuses into their shared
    /// ancestor directory's badge — lower wins. Ordered "most alarming
    /// first" so, e.g., a conflict anywhere under a folder is never hidden
    /// behind a plain modification shown elsewhere in that folder.
    fn priority(self) -> u8 {
        match self {
            FileStatus::Conflicted => 0,
            FileStatus::Added => 1,
            FileStatus::Untracked => 2,
            FileStatus::Modified => 3,
            FileStatus::Renamed => 4,
            FileStatus::Deleted => 5,
        }
    }

    /// Picks whichever of `self`/`other` should represent a directory that
    /// contains files with both statuses.
    pub fn combine(self, other: FileStatus) -> FileStatus {
        if other.priority() < self.priority() { other } else { self }
    }

    /// Single-letter badge, matching VS Code's own File Explorer decorations.
    pub fn letter(self) -> char {
        match self {
            FileStatus::Modified => 'M',
            FileStatus::Added => 'A',
            FileStatus::Deleted => 'D',
            FileStatus::Renamed => 'R',
            FileStatus::Untracked => 'U',
            FileStatus::Conflicted => 'C',
        }
    }
}

/// Per-file `git status`, scoped to exactly what the File Explorer needs to
/// decorate its tree — not the full stage/commit/diff/history surface a
/// GitPanel would need (see ADR-0009, superseded: that panel was removed,
/// but its git-CLI-vs-git2 tradeoff still applies here). Read-only, single
/// method, backed by shelling out to the user's own `git`.
#[async_trait]
pub trait GitStatusPort: Send + Sync {
    /// Every changed file at or under `root`, keyed by its absolute path,
    /// mapped to its `FileStatus`. Empty (not an error) when `root` isn't
    /// inside a git working tree, or the `git` binary isn't installed.
    async fn status(&self, root: PathBuf) -> HashMap<PathBuf, FileStatus>;

    /// The current branch name for the git working tree rooted at `root`, for
    /// the topbar's " <workspace> @ <branch> " tab label. `None` (not an
    /// error) when `root` isn't inside a git working tree, the `git` binary
    /// isn't installed, or HEAD is detached.
    async fn branch(&self, root: PathBuf) -> Option<String>;
}
