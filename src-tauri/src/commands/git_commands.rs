use std::path::PathBuf;

use argus_application::ports::{
    BranchInfo, CommitEntry, DiffContent, FileStatusEntry, GitPort, GitRepository, SyncStatus,
};
use tauri::State;

use crate::state::AppState;

pub type GitRepositoryDto = GitRepository;
pub type FileStatusEntryDto = FileStatusEntry;
pub type DiffContentDto = DiffContent;
pub type CommitEntryDto = CommitEntry;
pub type BranchInfoDto = BranchInfo;
pub type SyncStatusDto = SyncStatus;

#[tauri::command]
pub async fn git_available(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.git.is_git_available().await)
}

#[tauri::command]
pub async fn git_list_repositories(
    state: State<'_, AppState>,
    workspace_root: String,
) -> Result<Vec<GitRepositoryDto>, String> {
    Ok(state.git.list_repositories(PathBuf::from(workspace_root)).await)
}

#[tauri::command]
pub async fn git_status(state: State<'_, AppState>, repo_path: String) -> Result<Vec<FileStatusEntryDto>, String> {
    state.git.status(PathBuf::from(repo_path)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_diff(
    state: State<'_, AppState>,
    repo_path: String,
    file: String,
    staged: bool,
) -> Result<DiffContentDto, String> {
    state
        .git
        .diff(PathBuf::from(repo_path), file, staged)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_stage(state: State<'_, AppState>, repo_path: String, files: Vec<String>) -> Result<(), String> {
    state.git.stage(PathBuf::from(repo_path), files).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_unstage(state: State<'_, AppState>, repo_path: String, files: Vec<String>) -> Result<(), String> {
    state.git.unstage(PathBuf::from(repo_path), files).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_commit(state: State<'_, AppState>, repo_path: String, message: String) -> Result<(), String> {
    state.git.commit(PathBuf::from(repo_path), message).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_log(
    state: State<'_, AppState>,
    repo_path: String,
    skip: u32,
    limit: u32,
) -> Result<Vec<CommitEntryDto>, String> {
    state.git.log(PathBuf::from(repo_path), skip, limit).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_current_branch(state: State<'_, AppState>, repo_path: String) -> Result<Option<String>, String> {
    state.git.current_branch(PathBuf::from(repo_path)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_list_branches(state: State<'_, AppState>, repo_path: String) -> Result<Vec<BranchInfoDto>, String> {
    state.git.list_branches(PathBuf::from(repo_path)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_switch_branch(state: State<'_, AppState>, repo_path: String, name: String) -> Result<(), String> {
    state.git.switch_branch(PathBuf::from(repo_path), name).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_sync_status(state: State<'_, AppState>, repo_path: String) -> Result<SyncStatusDto, String> {
    state.git.sync_status(PathBuf::from(repo_path)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_push(state: State<'_, AppState>, repo_path: String) -> Result<(), String> {
    state.git.push(PathBuf::from(repo_path)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_pull(state: State<'_, AppState>, repo_path: String) -> Result<(), String> {
    state.git.pull(PathBuf::from(repo_path)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn git_fetch(state: State<'_, AppState>, repo_path: String) -> Result<(), String> {
    state.git.fetch(PathBuf::from(repo_path)).await.map_err(|e| e.to_string())
}
