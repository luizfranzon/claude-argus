use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;

use argus_application::ports::{FileStatus, GitStatusPort};
use async_trait::async_trait;
use tokio::process::Command;

fn command(root: &std::path::Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(root).args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        // Suppress the console window Windows would otherwise flash for
        // every invocation of a spawned console subprocess from a GUI app.
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

fn status_kind(code: char) -> Option<FileStatus> {
    match code {
        'M' => Some(FileStatus::Modified),
        'A' => Some(FileStatus::Added),
        'D' => Some(FileStatus::Deleted),
        'R' | 'C' => Some(FileStatus::Renamed),
        _ => None,
    }
}

/// Parses `git status --porcelain -uall` output (paths relative to whatever
/// directory `-C` pointed at — confirmed by the original `GitCliAdapter`,
/// see ADR-0009) into a `root`-joined absolute-path map.
fn parse(porcelain: &str, root: &std::path::Path) -> HashMap<PathBuf, FileStatus> {
    let mut out = HashMap::new();
    for line in porcelain.lines() {
        if line.len() < 3 {
            continue;
        }
        let mut chars = line.chars();
        let x = chars.next().unwrap();
        let y = chars.next().unwrap();
        let rest = &line[3..];
        let path = rest.rsplit_once(" -> ").map_or(rest, |(_, to)| to);
        if path.is_empty() {
            continue;
        }

        let conflicted = matches!((x, y), ('U', _) | (_, 'U') | ('A', 'A') | ('D', 'D'));
        let status = if conflicted {
            FileStatus::Conflicted
        } else if x == '?' && y == '?' {
            FileStatus::Untracked
        } else if let Some(s) = status_kind(y).or_else(|| status_kind(x)) {
            s
        } else {
            continue;
        };
        out.insert(root.join(path), status);
    }
    out
}

/// `GitStatusPort` backed by shelling out to the system `git` binary — same
/// tradeoff as the removed `GitPanel`'s `GitCliAdapter` (ADR-0009), just with
/// a single read-only method instead of the full stage/commit/diff surface.
pub struct GitStatusCliAdapter;

impl GitStatusCliAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitStatusCliAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GitStatusPort for GitStatusCliAdapter {
    async fn status(&self, root: PathBuf) -> HashMap<PathBuf, FileStatus> {
        let Ok(output) = command(&root, &["status", "--porcelain", "-uall"]).output().await else {
            return HashMap::new();
        };
        if !output.status.success() {
            return HashMap::new();
        }
        parse(&String::from_utf8_lossy(&output.stdout), &root)
    }

    async fn branch(&self, root: PathBuf) -> Option<String> {
        let output = command(&root, &["branch", "--show-current"]).output().await.ok()?;
        if !output.status.success() {
            return None;
        }
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }
}
