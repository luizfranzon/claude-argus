pub mod shell;
pub mod workspace;

pub use workspace::{close_requires_confirmation, Workspace, WorkspaceId, WorkspaceStatus};
