use std::path::PathBuf;

use argus_application::ports::PtyPort;
use argus_application::use_cases::{CloseDecision, CreatedWorkspace, OutputSink, SessionExitSink};
use argus_domain::shell::ShellLayout;
use argus_domain::{Session, Workspace, WorkspaceId};
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, State};

use crate::commands::fs_commands::{start_watch, stop_watch};
use crate::events::{
    SessionCloseReason, SessionClosedEvent, StartupPathResolvedEvent, WorkspaceClosedEvent,
    WorkspaceCloseReason, EVENT_SESSION_CLOSED, EVENT_SESSION_CREATED, EVENT_STARTUP_PATH_RESOLVED,
    EVENT_WORKSPACE_CLOSED, EVENT_WORKSPACE_CREATED,
};
use crate::state::AppState;

pub type WorkspaceDto = Workspace;
pub type SessionDto = Session;
pub type ShellLayoutDto = ShellLayout;

/// What every Workspace-creating command returns: the Workspace itself plus
/// the first Session auto-spawned inside it (see ADR-0010 — a Workspace is
/// never created "empty"), so the frontend never needs a second round trip
/// to discover the Session it should render a Terminal for.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceResponse {
    pub workspace: WorkspaceDto,
    pub session: SessionDto,
}

impl From<CreatedWorkspace> for CreateWorkspaceResponse {
    fn from(created: CreatedWorkspace) -> Self {
        Self {
            workspace: created.workspace,
            session: created.first_session,
        }
    }
}

/// Builds the pair of callbacks every Session-creating command passes into
/// the use case: raw PTY bytes go straight to the per-Session `Channel` (no
/// JSON envelope), while a process ending on its own is translated into the
/// low-frequency `session-closed` JSON event.
pub(crate) fn output_and_exit_sinks(
    app: AppHandle,
    channel: Channel<Vec<u8>>,
) -> (OutputSink, SessionExitSink) {
    let on_output: OutputSink = Box::new(move |data| {
        let _ = channel.send(data);
    });

    let on_exit: SessionExitSink = Box::new(move |session_id, _reason| {
        let _ = app.emit(
            EVENT_SESSION_CLOSED,
            SessionClosedEvent {
                id: session_id,
                reason: SessionCloseReason::ProcessExited,
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
) -> Result<Option<CreateWorkspaceResponse>, String> {
    let (on_output, on_exit) = output_and_exit_sinks(app.clone(), channel);
    let created = state
        .create_workspace
        .create_via_picker(on_output, on_exit)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(created) = &created {
        start_watch(&app, &state, created.workspace.id, created.workspace.directory.clone());
        let _ = app.emit(EVENT_WORKSPACE_CREATED, &created.workspace);
        let _ = app.emit(EVENT_SESSION_CREATED, &created.first_session);
    }
    Ok(created.map(CreateWorkspaceResponse::from))
}

#[tauri::command]
pub async fn create_workspace_with_directory(
    app: AppHandle,
    state: State<'_, AppState>,
    channel: Channel<Vec<u8>>,
    directory: String,
) -> Result<CreateWorkspaceResponse, String> {
    let (on_output, on_exit) = output_and_exit_sinks(app.clone(), channel);
    let created = state
        .create_workspace
        .create_with_directory(PathBuf::from(directory), on_output, on_exit)
        .await
        .map_err(|e| e.to_string())?;
    start_watch(&app, &state, created.workspace.id, created.workspace.directory.clone());
    let _ = app.emit(EVENT_WORKSPACE_CREATED, &created.workspace);
    let _ = app.emit(EVENT_SESSION_CREATED, &created.first_session);
    Ok(created.into())
}

#[tauri::command]
pub async fn duplicate_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
    channel: Channel<Vec<u8>>,
    source_id: WorkspaceId,
) -> Result<Option<CreateWorkspaceResponse>, String> {
    let (on_output, on_exit) = output_and_exit_sinks(app.clone(), channel);
    let created = state
        .create_workspace
        .duplicate(source_id, on_output, on_exit)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(created) = &created {
        start_watch(&app, &state, created.workspace.id, created.workspace.directory.clone());
        let _ = app.emit(EVENT_WORKSPACE_CREATED, &created.workspace);
        let _ = app.emit(EVENT_SESSION_CREATED, &created.first_session);
    }
    Ok(created.map(CreateWorkspaceResponse::from))
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
pub fn write_to_pty(
    state: State<'_, AppState>,
    id: argus_domain::SessionId,
    data: Vec<u8>,
) -> Result<(), String> {
    let handle = state
        .manager
        .lock()
        .unwrap()
        .pty_handle_for_session(id)
        .ok_or_else(|| "unknown session".to_string())?;
    state.pty.write(handle, &data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resize_pty(
    state: State<'_, AppState>,
    id: argus_domain::SessionId,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let handle = state
        .manager
        .lock()
        .unwrap()
        .pty_handle_for_session(id)
        .ok_or_else(|| "unknown session".to_string())?;
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
