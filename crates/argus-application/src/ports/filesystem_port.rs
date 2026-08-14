use std::path::{Path, PathBuf};

use async_trait::async_trait;

/// Native OS folder-picker. `None` means the user cancelled the dialog.
#[async_trait]
pub trait DirectoryPicker: Send + Sync {
    async fn pick_folder(&self, starting_dir: Option<&Path>) -> Option<PathBuf>;
}
