use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// One entry returned by a single-level directory listing. Listing is
/// lazy/one-level (not a recursive tree) so the File Explorer can expand
/// large directories (e.g. a monorepo root) without walking everything
/// up front — matching how VS Code's own explorer works.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum FsError {
    #[error("path not found: {0}")]
    NotFound(String),
    #[error("path already exists: {0}")]
    AlreadyExists(String),
    #[error("io error: {0}")]
    Io(String),
}

/// CRUD + read/write over a Workspace's directory tree. Every path passed in
/// is an absolute path already resolved by the caller (the frontend derives
/// it from the Workspace root plus the relative path the user is acting on).
#[async_trait]
pub trait FileSystemPort: Send + Sync {
    async fn list_dir(&self, path: PathBuf) -> Result<Vec<FileEntry>, FsError>;
    async fn read_file(&self, path: PathBuf) -> Result<String, FsError>;
    async fn write_file(&self, path: PathBuf, contents: String) -> Result<(), FsError>;
    async fn create_file(&self, path: PathBuf) -> Result<(), FsError>;
    async fn create_dir(&self, path: PathBuf) -> Result<(), FsError>;
    async fn rename(&self, from: PathBuf, to: PathBuf) -> Result<(), FsError>;
    /// Deletes a file or directory (recursively). Sent to the OS trash/recycle
    /// bin when the platform supports it, per the confirmed CRUD scope — not
    /// a permanent unlink.
    async fn delete(&self, path: PathBuf) -> Result<(), FsError>;
}
