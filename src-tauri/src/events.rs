use argus_domain::WorkspaceId;
use serde::Serialize;

/// Why a workspace was removed — distinct from the PTY-level `ExitReason`
/// (Normal/Crashed): this is about *who* triggered the removal.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum WorkspaceCloseReason {
    UserConfirmed,
    ProcessExited,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceClosedEvent {
    pub id: WorkspaceId,
    pub reason: WorkspaceCloseReason,
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

pub const EVENT_WORKSPACE_CREATED: &str = "workspace-created";
pub const EVENT_WORKSPACE_CLOSED: &str = "workspace-closed";
pub const EVENT_STARTUP_PATH_RESOLVED: &str = "startup-path-resolved";
pub const EVENT_FS_CHANGED: &str = "fs-changed";
