use std::path::PathBuf;

use argus_application::ports::{FileEntry, FileSystemPort, FsError};
use async_trait::async_trait;

fn io_err(err: std::io::Error, path: &std::path::Path) -> FsError {
    match err.kind() {
        std::io::ErrorKind::NotFound => FsError::NotFound(path.display().to_string()),
        std::io::ErrorKind::AlreadyExists => FsError::AlreadyExists(path.display().to_string()),
        _ => FsError::Io(err.to_string()),
    }
}

/// `FileSystemPort` backed by `tokio::fs`. Directories-first, then
/// alphabetical, matching the ordering every mainstream file explorer uses.
pub struct StdFsAdapter;

impl StdFsAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StdFsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FileSystemPort for StdFsAdapter {
    async fn list_dir(&self, path: PathBuf) -> Result<Vec<FileEntry>, FsError> {
        let mut read_dir = tokio::fs::read_dir(&path).await.map_err(|e| io_err(e, &path))?;
        let mut entries = Vec::new();
        while let Some(entry) = read_dir.next_entry().await.map_err(|e| io_err(e, &path))? {
            let file_type = entry.file_type().await.map_err(|e| io_err(e, &path))?;
            entries.push(FileEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: entry.path(),
                is_dir: file_type.is_dir(),
            });
        }
        let mut keyed: Vec<(String, FileEntry)> =
            entries.into_iter().map(|e| (e.name.to_lowercase(), e)).collect();
        keyed.sort_by(|(a_key, a), (b_key, b)| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a_key.cmp(b_key),
        });
        let entries = keyed.into_iter().map(|(_, e)| e).collect();
        Ok(entries)
    }

    async fn read_file(&self, path: PathBuf) -> Result<String, FsError> {
        tokio::fs::read_to_string(&path).await.map_err(|e| io_err(e, &path))
    }

    async fn write_file(&self, path: PathBuf, contents: String) -> Result<(), FsError> {
        tokio::fs::write(&path, contents).await.map_err(|e| io_err(e, &path))
    }

    async fn create_file(&self, path: PathBuf) -> Result<(), FsError> {
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Err(FsError::AlreadyExists(path.display().to_string()));
        }
        tokio::fs::write(&path, b"").await.map_err(|e| io_err(e, &path))
    }

    async fn create_dir(&self, path: PathBuf) -> Result<(), FsError> {
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Err(FsError::AlreadyExists(path.display().to_string()));
        }
        tokio::fs::create_dir_all(&path).await.map_err(|e| io_err(e, &path))
    }

    async fn rename(&self, from: PathBuf, to: PathBuf) -> Result<(), FsError> {
        tokio::fs::rename(&from, &to).await.map_err(|e| io_err(e, &from))
    }

    async fn delete(&self, path: PathBuf) -> Result<(), FsError> {
        tokio::task::spawn_blocking(move || {
            trash::delete(&path).map_err(|e| FsError::Io(e.to_string()))
        })
        .await
        .map_err(|e| FsError::Io(e.to_string()))?
    }
}
