pub mod close_policy;
pub mod session;
pub mod workspace;

pub use session::{
    close_requires_confirmation as session_close_requires_confirmation, Session, SessionId,
    SessionStatus,
};
pub use workspace::{close_requires_confirmation, Workspace, WorkspaceId, WorkspaceStatus};
