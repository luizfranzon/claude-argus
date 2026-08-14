use std::fmt;
use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PtyHandleId(Uuid);

impl PtyHandleId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PtyHandleId {
    fn default() -> Self {
        Self::new()
    }
}

/// Why a PTY-backed process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    Normal,
    Crashed,
}

/// Everything needed to spawn a program in a PTY. `on_output`/`on_exit` are
/// callbacks rather than a channel type so `PtyPort` stays free of any
/// particular async runtime choice, and so fakes in tests can invoke them
/// synchronously without needing a running executor.
pub struct SpawnSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// PATH resolved once at startup by `ResolveStartupPathUseCase`, injected
    /// explicitly here because the host process (launched from a GUI icon) may
    /// not have inherited the user's full shell PATH itself.
    pub env_path: String,
    pub on_output: Box<dyn Fn(Vec<u8>) + Send + Sync>,
    pub on_exit: Box<dyn Fn(ExitReason) + Send + Sync>,
}

impl fmt::Debug for SpawnSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpawnSpec")
            .field("program", &self.program)
            .field("args", &self.args)
            .field("cwd", &self.cwd)
            .field("env_path", &self.env_path)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PtyError {
    #[error("failed to spawn process: {0}")]
    SpawnFailed(String),
    #[error("unknown pty handle")]
    UnknownHandle,
    #[error("io error: {0}")]
    Io(String),
}

#[async_trait]
pub trait PtyPort: Send + Sync {
    async fn spawn(&self, spec: SpawnSpec) -> Result<PtyHandleId, PtyError>;
    fn write(&self, handle: PtyHandleId, data: &[u8]) -> Result<(), PtyError>;
    fn resize(&self, handle: PtyHandleId, cols: u16, rows: u16) -> Result<(), PtyError>;
    fn kill(&self, handle: PtyHandleId) -> Result<(), PtyError>;
}
