use argus_domain::SessionId;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, State};

use crate::commands::workspace_commands::SessionDto;
use crate::commands::workspace_commands::{output_and_exit_sinks, CloseDecisionDto};
use crate::events::{SessionCloseReason, SessionClosedEvent, EVENT_SESSION_CLOSED, EVENT_SESSION_CREATED};
use crate::state::AppState;
use argus_domain::WorkspaceId;

/// Spawns an additional Session inside an already-open Workspace. Unlike the
/// Workspace-creating commands, this never touches the folder picker or
/// starts a new file watcher — the Workspace's directory is already known
/// and already watched.
#[tauri::command]
pub async fn create_session(
    app: AppHandle,
    state: State<'_, AppState>,
    channel: Channel<Vec<u8>>,
    workspace_id: WorkspaceId,
    name: Option<String>,
) -> Result<SessionDto, String> {
    let (on_output, on_exit) = output_and_exit_sinks(app.clone(), channel);
    let session = state
        .create_session
        .execute(workspace_id, name, on_output, on_exit)
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit(EVENT_SESSION_CREATED, &session);
    Ok(session)
}

#[tauri::command]
pub fn request_close_session(state: State<'_, AppState>, id: SessionId) -> Result<CloseDecisionDto, String> {
    Ok(state.request_close_session.execute(id).into())
}

#[tauri::command]
pub fn confirm_close_session(app: AppHandle, state: State<'_, AppState>, id: SessionId) -> Result<(), String> {
    state
        .confirm_close_session
        .execute(id)
        .map_err(|e| e.to_string())?;
    let _ = app.emit(
        EVENT_SESSION_CLOSED,
        SessionClosedEvent {
            id,
            reason: SessionCloseReason::UserConfirmed,
        },
    );
    Ok(())
}

#[tauri::command]
pub fn rename_session(state: State<'_, AppState>, id: SessionId, name: String) {
    state.manager.lock().unwrap().rename_session(id, name);
}

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Vec<SessionDto> {
    state
        .manager
        .lock()
        .unwrap()
        .list_sessions()
        .into_iter()
        .cloned()
        .collect()
}
