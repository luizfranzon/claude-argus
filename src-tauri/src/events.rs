use argus_domain::{SessionId, WorkspaceId};
use serde::Serialize;

/// Why a workspace was removed. Always `UserConfirmed` now that a Workspace
/// no longer owns a PTY directly (see ADR-0010) — an unprompted process exit
/// only ever affects one Session, never the whole Workspace, so there is no
/// `ProcessExited` variant here (compare `SessionCloseReason`, which has one).
#[derive(Debug, Clone, Copy, Serialize)]
pub enum WorkspaceCloseReason {
    UserConfirmed,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceClosedEvent {
    pub id: WorkspaceId,
    pub reason: WorkspaceCloseReason,
}

/// Why a Session was removed — same shape as `WorkspaceCloseReason`, kept as
/// its own type since Sessions and Workspaces are closed independently.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum SessionCloseReason {
    UserConfirmed,
    ProcessExited,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionClosedEvent {
    pub id: SessionId,
    pub reason: SessionCloseReason,
}

#[derive(Debug, Clone, Serialize)]
pub struct StartupPathResolvedEvent {
    pub ok: bool,
    pub error: Option<String>,
}

/// Fired (debounced) whenever anything changes under a Workspace's directory
/// tree, whether from `claude`, an external editor, or argus's own File
/// Explorer CRUD — the frontend reacts by re-listing the affected directory
/// and refreshing that Workspace's git status.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsChangedEvent {
    pub workspace_id: WorkspaceId,
}

/// Whether a Session's `claude` process is actively working on a prompt —
/// derived from its `UserPromptSubmit`/`Stop` hooks (see `HookServer`), not
/// from parsing PTY output.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionRuntimeStatus {
    Thinking,
    Idle,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatusChangedEvent {
    pub session_id: SessionId,
    pub status: SessionRuntimeStatus,
}

pub const EVENT_WORKSPACE_CREATED: &str = "workspace-created";
pub const EVENT_WORKSPACE_CLOSED: &str = "workspace-closed";
pub const EVENT_SESSION_CREATED: &str = "session-created";
pub const EVENT_SESSION_CLOSED: &str = "session-closed";
pub const EVENT_SESSION_STATUS_CHANGED: &str = "session-status-changed";
pub const EVENT_STARTUP_PATH_RESOLVED: &str = "startup-path-resolved";
pub const EVENT_FS_CHANGED: &str = "fs-changed";
