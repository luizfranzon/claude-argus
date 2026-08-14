use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EnvResolutionError {
    #[error("failed to resolve PATH from login shell/environment: {0}")]
    ResolutionFailed(String),
}

/// Reconstructs the user's full PATH (as their login shell would see it),
/// since a process launched from a GUI icon may not inherit it. Resolved once
/// at startup and cached — see `ResolveStartupPathUseCase`.
#[async_trait]
pub trait ShellEnvironmentResolver: Send + Sync {
    async fn resolve_path(&self) -> Result<String, EnvResolutionError>;
}
