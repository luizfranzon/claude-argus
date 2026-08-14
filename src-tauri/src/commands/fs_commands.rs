use std::path::PathBuf;

use argus_application::ports::{FileEntry, FileSystemPort, FileWatcherPort};
use argus_domain::WorkspaceId;
use tauri::{AppHandle, Emitter, State};

use crate::events::{FsChangedEvent, EVENT_FS_CHANGED};
use crate::state::AppState;

pub type FileEntryDto = FileEntry;

/// Starts watching a newly-registered Workspace's directory, storing the
/// resulting handle so `stop_watch` can tear it down later. Called right
/// after every workspace-creating command succeeds.
pub(crate) fn start_watch(app: &AppHandle, state: &State<'_, AppState>, id: WorkspaceId, directory: PathBuf) {
    let app = app.clone();
    let handle = state.watcher.watch(
        directory,
        Box::new(move || {
            let _ = app.emit(EVENT_FS_CHANGED, FsChangedEvent { workspace_id: id });
        }),
    );
    if let Ok(handle) = handle {
        state.manager.lock().unwrap().set_watch_handle(id, handle);
    }
}

/// Stops watching a Workspace being removed. A no-op if it was never
/// watching (e.g. removal raced with a failed `start_watch`).
pub(crate) fn stop_watch(state: &State<'_, AppState>, id: WorkspaceId) {
    let handle = state.manager.lock().unwrap().take_watch_handle(id);
    if let Some(handle) = handle {
        state.watcher.unwatch(handle);
    }
}

#[tauri::command]
pub async fn list_dir(state: State<'_, AppState>, path: String) -> Result<Vec<FileEntryDto>, String> {
    state.fs.list_dir(PathBuf::from(path)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn read_file(state: State<'_, AppState>, path: String) -> Result<String, String> {
    state.fs.read_file(PathBuf::from(path)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn write_file(state: State<'_, AppState>, path: String, contents: String) -> Result<(), String> {
    state
        .fs
        .write_file(PathBuf::from(path), contents)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_file(state: State<'_, AppState>, path: String) -> Result<(), String> {
    state.fs.create_file(PathBuf::from(path)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_dir(state: State<'_, AppState>, path: String) -> Result<(), String> {
    state.fs.create_dir(PathBuf::from(path)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rename_path(state: State<'_, AppState>, from: String, to: String) -> Result<(), String> {
    state
        .fs
        .rename(PathBuf::from(from), PathBuf::from(to))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_path(state: State<'_, AppState>, path: String) -> Result<(), String> {
    state.fs.delete(PathBuf::from(path)).await.map_err(|e| e.to_string())
}
