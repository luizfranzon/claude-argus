use std::path::PathBuf;

use argus_application::ports::PtyPort;
use argus_application::use_cases::{CloseDecision, ExitSink, OutputSink};
use argus_domain::shell::ShellLayout;
use argus_domain::{Workspace, WorkspaceId};
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::fs_commands::{start_watch, stop_watch};
use crate::events::{
    StartupPathResolvedEvent, WorkspaceClosedEvent, WorkspaceCloseReason,
    EVENT_STARTUP_PATH_RESOLVED, EVENT_WORKSPACE_CLOSED, EVENT_WORKSPACE_CREATED,
};
use crate::state::AppState;

pub type WorkspaceDto = Workspace;
pub type ShellLayoutDto = ShellLayout;

/// Builds the pair of callbacks every workspace-creating command passes into
/// the use case: raw PTY bytes go straight to the per-workspace `Channel`
/// (no JSON envelope), while a process ending on its own is translated into
/// the low-frequency `workspace-closed` JSON event.
fn output_and_exit_sinks(app: AppHandle, channel: Channel<Vec<u8>>) -> (OutputSink, ExitSink) {
    let on_output: OutputSink = Box::new(move |data| {
        let _ = channel.send(data);
    });

    let on_exit: ExitSink = Box::new(move |workspace_id, _reason| {
        stop_watch(&app.state::<AppState>(), workspace_id);
        let _ = app.emit(
            EVENT_WORKSPACE_CLOSED,
            WorkspaceClosedEvent {
                id: workspace_id,
                reason: WorkspaceCloseReason::ProcessExited,
            },
        );
    });

    (on_output, on_exit)
}

#[tauri::command]
pub async fn resolve_startup_path(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let result = state.resolve_startup_path.execute().await;
    let event = match &result {
        Ok(()) => StartupPathResolvedEvent {
            ok: true,
            error: None,
        },
        Err(e) => StartupPathResolvedEvent {
            ok: false,
            error: Some(e.to_string()),
        },
    };
    let _ = app.emit(EVENT_STARTUP_PATH_RESOLVED, event);
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_initial_directory(state: State<'_, AppState>) -> Option<String> {
    state
        .initial_directory
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn create_workspace_via_picker(
    app: AppHandle,
    state: State<'_, AppState>,
    channel: Channel<Vec<u8>>,
) -> Result<Option<WorkspaceDto>, String> {
    let (on_output, on_exit) = output_and_exit_sinks(app.clone(), channel);
    let workspace = state
        .create_workspace
        .create_via_picker(on_output, on_exit)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(workspace) = &workspace {
        start_watch(&app, &state, workspace.id, workspace.directory.clone());
        let _ = app.emit(EVENT_WORKSPACE_CREATED, workspace);
    }
    Ok(workspace)
}

#[tauri::command]
pub async fn create_workspace_with_directory(
    app: AppHandle,
    state: State<'_, AppState>,
    channel: Channel<Vec<u8>>,
    directory: String,
) -> Result<WorkspaceDto, String> {
    let (on_output, on_exit) = output_and_exit_sinks(app.clone(), channel);
    let workspace = state
        .create_workspace
        .create_with_directory(PathBuf::from(directory), on_output, on_exit)
        .await
        .map_err(|e| e.to_string())?;
    start_watch(&app, &state, workspace.id, workspace.directory.clone());
    let _ = app.emit(EVENT_WORKSPACE_CREATED, &workspace);
    Ok(workspace)
}

#[tauri::command]
pub async fn duplicate_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    channel: Channel<Vec<u8>>,
    source_id: WorkspaceId,
) -> Result<Option<WorkspaceDto>, String> {
    let (on_output, on_exit) = output_and_exit_sinks(app.clone(), channel);
    let workspace = state
        .create_workspace
        .duplicate(source_id, on_output, on_exit)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(workspace) = &workspace {
        start_watch(&app, &state, workspace.id, workspace.directory.clone());
        let _ = app.emit(EVENT_WORKSPACE_CREATED, workspace);
    }
    Ok(workspace)
}

#[derive(serde::Serialize)]
pub enum CloseDecisionDto {
    RequiresConfirmation,
    AlreadyClosed,
}

impl From<CloseDecision> for CloseDecisionDto {
    fn from(decision: CloseDecision) -> Self {
        match decision {
            CloseDecision::RequiresConfirmation => CloseDecisionDto::RequiresConfirmation,
            CloseDecision::AlreadyClosed => CloseDecisionDto::AlreadyClosed,
        }
    }
}

#[tauri::command]
pub fn request_close_workspace(
    state: State<'_, AppState>,
    id: WorkspaceId,
) -> Result<CloseDecisionDto, String> {
    Ok(state.request_close_workspace.execute(id).into())
}

#[tauri::command]
pub fn confirm_close_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    id: WorkspaceId,
) -> Result<(), String> {
    state
        .confirm_close_workspace
        .execute(id)
        .map_err(|e| e.to_string())?;
    stop_watch(&state, id);
    let _ = app.emit(
        EVENT_WORKSPACE_CLOSED,
        WorkspaceClosedEvent {
            id,
            reason: WorkspaceCloseReason::UserConfirmed,
        },
    );
    Ok(())
}

#[tauri::command]
pub fn write_to_pty(state: State<'_, AppState>, id: WorkspaceId, data: Vec<u8>) -> Result<(), String> {
    let handle = state
        .manager
        .lock()
        .unwrap()
        .pty_handle_for(id)
        .ok_or_else(|| "unknown workspace".to_string())?;
    state.pty.write(handle, &data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resize_pty(state: State<'_, AppState>, id: WorkspaceId, cols: u16, rows: u16) -> Result<(), String> {
    let handle = state
        .manager
        .lock()
        .unwrap()
        .pty_handle_for(id)
        .ok_or_else(|| "unknown workspace".to_string())?;
    state.pty.resize(handle, cols, rows).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_workspaces(state: State<'_, AppState>) -> Vec<WorkspaceDto> {
    state
        .manager
        .lock()
        .unwrap()
        .list()
        .into_iter()
        .cloned()
        .collect()
}

#[tauri::command]
pub fn get_shell_layout(state: State<'_, AppState>) -> ShellLayoutDto {
    state.manager.lock().unwrap().layout().clone()
}

/// Ensures a Workspace's Editor panel exists (idempotent) — called the first
/// time a file is opened for that Workspace — and returns the updated layout
/// so the frontend can re-render the Grid split without a second round trip.
#[tauri::command]
pub fn open_editor_panel(state: State<'_, AppState>, id: WorkspaceId) -> ShellLayoutDto {
    let mut manager = state.manager.lock().unwrap();
    manager.open_editor(id);
    manager.layout().clone()
}
