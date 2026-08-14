use std::path::PathBuf;

use argus_application::ports::{
    BranchInfo, CommitEntry, ExitReason, FileEntry, FileStatusEntry, FsError, GitError,
    GitRepository, SyncStatus,
};
use argus_application::use_cases::{CreateSessionError, CreateWorkspaceError, CreatedWorkspace};
use argus_domain::{Session, SessionId, WorkspaceId};
use argus_infrastructure::HookEventKind;
use uuid::Uuid;

/// Everything that can happen off the main loop's thread and needs to be
/// folded back into `AppState`. Keyboard input is handled separately, straight
/// out of `crossterm`'s event stream — this enum is only for
/// backend-originated events (PTY output, async use case results, file
/// watcher/git/fs results, hook callbacks).
#[derive(Debug)]
pub enum AppEvent {
    /// Raw PTY bytes tagged with a locally-generated `stream_id` rather than a
    /// `SessionId`, since `OutputSink` fires before `CreateSessionUseCase`
    /// resolves with the real `Session` — buffered under `stream_id` until
    /// the Session it belongs to is known.
    PtyOutput(Uuid, Vec<u8>),
    SessionSpawned {
        stream_id: Uuid,
        workspace_id: WorkspaceId,
        result: Result<Session, CreateSessionError>,
    },
    SessionExited(SessionId, ExitReason),
    WorkspaceSpawned {
        stream_id: Uuid,
        result: Result<CreatedWorkspace, CreateWorkspaceError>,
    },
    FsChanged(WorkspaceId),
    HookStatus(SessionId, HookEventKind),
    /// A Claude Code session (`~/.claude/sessions/<pid>.json`, see
    /// `argus_infrastructure::read_claude_session_names`) picked up a new
    /// `name` — e.g. via that session's own `/rename` — that differs from
    /// what this Session is currently called. `SessionId` matches because
    /// it *is* Claude Code's own session id (`--session-id` at spawn).
    ClaudeSessionRenamed(SessionId, String),
    DirLoaded(WorkspaceId, PathBuf, Result<Vec<FileEntry>, FsError>),
    FsOpDone(WorkspaceId, PathBuf, Result<(), FsError>),
    GitAvailable(bool),
    GitReposLoaded(WorkspaceId, Vec<GitRepository>),
    GitRefreshed {
        workspace_id: WorkspaceId,
        repo: PathBuf,
        status: Result<Vec<FileStatusEntry>, GitError>,
        branch: Result<Option<String>, GitError>,
        branches: Result<Vec<BranchInfo>, GitError>,
        sync: Result<SyncStatus, GitError>,
    },
    GitLogLoaded {
        workspace_id: WorkspaceId,
        repo: PathBuf,
        skip: u32,
        entries: Result<Vec<CommitEntry>, GitError>,
    },
    GitActionDone {
        workspace_id: WorkspaceId,
        repo: PathBuf,
        action: &'static str,
        result: Result<(), GitError>,
    },
}
