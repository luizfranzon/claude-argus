use std::path::PathBuf;
use std::process::Stdio;

use argus_application::ports::{
    BranchInfo, CommitEntry, DiffContent, FileStatusEntry, FileStatusKind, GitError, GitPort,
    GitRepository, SyncStatus,
};
use async_trait::async_trait;
use tokio::process::Command;

const RS: char = '\u{1e}'; // record separator
const US: char = '\u{1f}'; // unit separator

fn command(repo_path: &std::path::Path, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo_path).args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        // Suppress the console window Windows would otherwise flash for
        // every invocation of a spawned console subprocess from a GUI app.
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

async fn run(repo_path: &std::path::Path, args: &[&str]) -> Result<String, GitError> {
    let output = command(repo_path, args)
        .output()
        .await
        .map_err(|_| GitError::NotInstalled)?;
    if !output.status.success() {
        return Err(GitError::CommandFailed(String::from_utf8_lossy(&output.stderr).trim().to_string()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// `Ok("")` (rather than an error) on failure — used where "doesn't exist
/// yet" (e.g. a file untracked at HEAD) is a normal, expected outcome.
async fn run_or_empty(repo_path: &std::path::Path, args: &[&str]) -> String {
    run(repo_path, args).await.unwrap_or_default()
}

fn status_kind(code: char) -> FileStatusKind {
    match code {
        'A' => FileStatusKind::Added,
        'D' => FileStatusKind::Deleted,
        'R' | 'C' => FileStatusKind::Renamed,
        '?' => FileStatusKind::Untracked,
        _ => FileStatusKind::Modified,
    }
}

fn parse_status(porcelain: &str) -> Vec<FileStatusEntry> {
    let mut entries = Vec::new();
    for line in porcelain.lines() {
        if line.len() < 3 {
            continue;
        }
        let mut chars = line.chars();
        let x = chars.next().unwrap();
        let y = chars.next().unwrap();
        let rest = &line[3..];
        let path = rest.rsplit_once(" -> ").map_or(rest, |(_, to)| to).to_string();

        let conflicted = matches!(
            (x, y),
            ('U', _) | (_, 'U') | ('A', 'A') | ('D', 'D')
        );
        if conflicted {
            entries.push(FileStatusEntry { path, staged: false, kind: FileStatusKind::Conflicted });
            continue;
        }
        if x == '?' && y == '?' {
            entries.push(FileStatusEntry { path, staged: false, kind: FileStatusKind::Untracked });
            continue;
        }
        if x != ' ' {
            entries.push(FileStatusEntry { path: path.clone(), staged: true, kind: status_kind(x) });
        }
        if y != ' ' {
            entries.push(FileStatusEntry { path, staged: false, kind: status_kind(y) });
        }
    }
    entries
}

/// `GitPort` backed by shelling out to the system `git` binary — see
/// ADR-0009 for why (not `git2`/libgit2).
pub struct GitCliAdapter;

impl GitCliAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitCliAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GitPort for GitCliAdapter {
    async fn is_git_available(&self) -> bool {
        let mut cmd = Command::new("git");
        cmd.arg("--version").stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(windows)]
        {
            cmd.creation_flags(0x0800_0000);
        }
        cmd.status().await.map(|s| s.success()).unwrap_or(false)
    }

    async fn list_repositories(&self, workspace_root: PathBuf) -> Vec<GitRepository> {
        let Ok(top_level) = run(&workspace_root, &["rev-parse", "--show-toplevel"]).await else {
            return Vec::new();
        };
        let root_path = PathBuf::from(top_level.trim());
        let root_name = root_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root_path.display().to_string());

        let mut repos = vec![GitRepository { name: root_name, path: root_path.clone(), is_submodule: false }];

        // Skip the spawn entirely for the common case of a repo with no
        // submodules — `git submodule status` still walks the whole tree.
        if !tokio::fs::try_exists(root_path.join(".gitmodules")).await.unwrap_or(false) {
            return repos;
        }

        let submodule_status = run_or_empty(&root_path, &["submodule", "status"]).await;
        for line in submodule_status.lines() {
            // ` <sha> <path> (<describe>)`; a leading `-` means uninitialized.
            let trimmed = line.trim_start_matches(['+', 'U']);
            if trimmed.starts_with('-') {
                continue;
            }
            let mut parts = trimmed.trim_start().split_whitespace();
            let _sha = parts.next();
            let Some(relative_path) = parts.next() else { continue };
            repos.push(GitRepository {
                name: relative_path.to_string(),
                path: root_path.join(relative_path),
                is_submodule: true,
            });
        }
        repos
    }

    async fn status(&self, repo_path: PathBuf) -> Result<Vec<FileStatusEntry>, GitError> {
        // `-uall`: without it, git collapses an entire new untracked directory
        // into one `?? dirname/` entry instead of listing the files inside —
        // the File Status list must only ever show files, never a folder.
        let out = run(&repo_path, &["status", "--porcelain", "-uall"]).await?;
        Ok(parse_status(&out))
    }

    async fn diff(&self, repo_path: PathBuf, file: String, staged: bool) -> Result<DiffContent, GitError> {
        let old = run_or_empty(&repo_path, &["show", &format!("HEAD:{file}")]).await;
        let new = if staged {
            run_or_empty(&repo_path, &["show", &format!(":{file}")]).await
        } else {
            tokio::fs::read_to_string(repo_path.join(&file)).await.unwrap_or_default()
        };
        Ok(DiffContent { old, new })
    }

    async fn stage(&self, repo_path: PathBuf, files: Vec<String>) -> Result<(), GitError> {
        let mut args = vec!["add", "--"];
        args.extend(files.iter().map(String::as_str));
        run(&repo_path, &args).await.map(|_| ())
    }

    async fn unstage(&self, repo_path: PathBuf, files: Vec<String>) -> Result<(), GitError> {
        // `reset --` rather than `restore --staged --`: it's been in git since
        // the beginning and, unlike `restore` (2.23+, 2019), also unstages
        // cleanly on a brand-new repo with no commits yet (an unborn HEAD).
        let mut args = vec!["reset", "--"];
        args.extend(files.iter().map(String::as_str));
        run(&repo_path, &args).await.map(|_| ())
    }

    async fn commit(&self, repo_path: PathBuf, message: String) -> Result<(), GitError> {
        run(&repo_path, &["commit", "-m", &message]).await.map(|_| ())
    }

    async fn log(&self, repo_path: PathBuf, skip: u32, limit: u32) -> Result<Vec<CommitEntry>, GitError> {
        let format = format!("--pretty=format:%H{US}%h{US}%an{US}%aI{US}%s{RS}");
        let skip_arg = format!("--skip={skip}");
        let limit_arg = format!("-n{limit}");
        let out = run(&repo_path, &["log", &skip_arg, &limit_arg, &format]).await;
        let out = match out {
            Ok(out) => out,
            // An empty repo (no commits yet) makes `git log` fail — that's a
            // normal, expected state, not an error the caller should see.
            Err(_) => return Ok(Vec::new()),
        };
        let mut commits = Vec::new();
        for record in out.split(RS) {
            let record = record.trim();
            if record.is_empty() {
                continue;
            }
            let mut fields = record.split(US);
            let (Some(hash), Some(short_hash), Some(author), Some(date), Some(summary)) =
                (fields.next(), fields.next(), fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            commits.push(CommitEntry {
                hash: hash.to_string(),
                short_hash: short_hash.to_string(),
                author: author.to_string(),
                date: date.to_string(),
                summary: summary.to_string(),
            });
        }
        Ok(commits)
    }

    async fn current_branch(&self, repo_path: PathBuf) -> Result<Option<String>, GitError> {
        // `symbolic-ref` resolves even before the first commit (an "unborn"
        // branch, where `rev-parse --abbrev-ref HEAD` fails with "ambiguous
        // argument HEAD"); it only fails on a genuinely detached HEAD, which
        // is the "no current branch" case this returns `None` for.
        let Ok(name) = run(&repo_path, &["symbolic-ref", "--short", "HEAD"]).await else {
            return Ok(None);
        };
        let name = name.trim();
        Ok(if name.is_empty() { None } else { Some(name.to_string()) })
    }

    async fn list_branches(&self, repo_path: PathBuf) -> Result<Vec<BranchInfo>, GitError> {
        let format = format!("--format=%(refname:short){US}%(HEAD)");
        let out = run(&repo_path, &["branch", &format]).await?;
        Ok(out
            .lines()
            .filter_map(|line| {
                let mut parts = line.split(US);
                let name = parts.next()?.to_string();
                let is_current = parts.next() == Some("*");
                Some(BranchInfo { name, is_current })
            })
            .collect())
    }

    async fn switch_branch(&self, repo_path: PathBuf, name: String) -> Result<(), GitError> {
        run(&repo_path, &["checkout", &name]).await.map(|_| ())
    }

    async fn sync_status(&self, repo_path: PathBuf) -> Result<SyncStatus, GitError> {
        let Ok(_upstream) = run(&repo_path, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]).await
        else {
            return Ok(SyncStatus { ahead: 0, behind: 0, has_upstream: false });
        };
        let counts = run(&repo_path, &["rev-list", "--left-right", "--count", "HEAD...@{u}"]).await?;
        let mut parts = counts.split_whitespace();
        let ahead = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let behind = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        Ok(SyncStatus { ahead, behind, has_upstream: true })
    }

    async fn push(&self, repo_path: PathBuf) -> Result<(), GitError> {
        run(&repo_path, &["push"]).await.map(|_| ())
    }

    async fn pull(&self, repo_path: PathBuf) -> Result<(), GitError> {
        run(&repo_path, &["pull"]).await.map(|_| ())
    }

    async fn fetch(&self, repo_path: PathBuf) -> Result<(), GitError> {
        run(&repo_path, &["fetch"]).await.map(|_| ())
    }
}
