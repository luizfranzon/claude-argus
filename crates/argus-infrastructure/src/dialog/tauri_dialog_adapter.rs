use std::path::{Path, PathBuf};

use argus_application::ports::DirectoryPicker;
use async_trait::async_trait;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
use tokio::sync::oneshot;

/// Native OS folder picker via `tauri-plugin-dialog`. The plugin's API is
/// callback-based, so this wraps it in a `oneshot` channel to present the
/// `async fn` shape `DirectoryPicker` expects.
pub struct TauriDialogAdapter {
    app_handle: AppHandle,
}

impl TauriDialogAdapter {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

#[async_trait]
impl DirectoryPicker for TauriDialogAdapter {
    async fn pick_folder(&self, _starting_dir: Option<&Path>) -> Option<PathBuf> {
        let (tx, rx) = oneshot::channel();

        self.app_handle.dialog().file().pick_folder(move |result| {
            let path = result.and_then(|p| p.into_path().ok());
            let _ = tx.send(path);
        });

        rx.await.ok().flatten()
    }
}
