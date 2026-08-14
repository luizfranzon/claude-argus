use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WatchHandle(Uuid);

impl WatchHandle {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for WatchHandle {
    fn default() -> Self {
        Self::new()
    }
}

pub type WatchCallback = Box<dyn Fn() + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WatchError {
    #[error("failed to watch path: {0}")]
    Failed(String),
}

/// Watches a Workspace root recursively, invoking `on_change` (debounced by
/// the adapter) whenever anything under it is created, modified, removed, or
/// renamed. One `WatchHandle` per Workspace, started on registration and
/// stopped on removal — see `WorkspaceManager`.
pub trait FileWatcherPort: Send + Sync {
    fn watch(&self, root: PathBuf, on_change: WatchCallback) -> Result<WatchHandle, WatchError>;
    fn unwatch(&self, handle: WatchHandle);
}
