use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// One independently-stageable/committable repository shown in the GitPanel
/// — the Workspace's own repo (if it is one) plus one entry per initialized
/// submodule. See "Git Repository" in CONTEXT.md.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRepository {
    pub name: String,
    pub path: PathBuf,
    pub is_submodule: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileStatusKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStatusEntry {
    pub path: String,
    pub staged: bool,
    pub kind: FileStatusKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffContent {
    pub old: String,
    pub new: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitEntry {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub date: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub ahead: u32,
    pub behind: u32,
    pub has_upstream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum GitError {
    #[error("git is not installed")]
    NotInstalled,
    #[error("not a git repository")]
    NotARepository,
    #[error("git command failed: {0}")]
    CommandFailed(String),
}

/// Everything the GitPanel needs, backed (per ADR-0009) by shelling out to
/// the user's own `git` — never a bundled libgit2. Whole-file staging only;
/// no amend. `repo_path` is always one entry from `list_repositories` (the
/// Workspace root or one of its submodules), never an arbitrary subpath.
#[async_trait]
pub trait GitPort: Send + Sync {
    async fn is_git_available(&self) -> bool;
    async fn list_repositories(&self, workspace_root: PathBuf) -> Vec<GitRepository>;
    async fn status(&self, repo_path: PathBuf) -> Result<Vec<FileStatusEntry>, GitError>;
    async fn diff(&self, repo_path: PathBuf, file: String, staged: bool) -> Result<DiffContent, GitError>;
    async fn stage(&self, repo_path: PathBuf, files: Vec<String>) -> Result<(), GitError>;
    async fn unstage(&self, repo_path: PathBuf, files: Vec<String>) -> Result<(), GitError>;
    async fn commit(&self, repo_path: PathBuf, message: String) -> Result<(), GitError>;
    async fn log(&self, repo_path: PathBuf, skip: u32, limit: u32) -> Result<Vec<CommitEntry>, GitError>;
    async fn current_branch(&self, repo_path: PathBuf) -> Result<Option<String>, GitError>;
    async fn list_branches(&self, repo_path: PathBuf) -> Result<Vec<BranchInfo>, GitError>;
    async fn switch_branch(&self, repo_path: PathBuf, name: String) -> Result<(), GitError>;
    async fn sync_status(&self, repo_path: PathBuf) -> Result<SyncStatus, GitError>;
    async fn push(&self, repo_path: PathBuf) -> Result<(), GitError>;
    async fn pull(&self, repo_path: PathBuf) -> Result<(), GitError>;
    async fn fetch(&self, repo_path: PathBuf) -> Result<(), GitError>;
}
