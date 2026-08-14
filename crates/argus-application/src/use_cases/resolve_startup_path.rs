use std::sync::{Arc, Mutex};

use crate::ports::{EnvResolutionError, ShellEnvironmentResolver};
use crate::workspace_manager::WorkspaceManager;

/// Resolves and caches the user's full PATH once per app process lifetime.
/// Every subsequent workspace spawn reuses the cached value instead of
/// re-invoking the (potentially slow) shell probe.
pub struct ResolveStartupPathUseCase<Resolver: ShellEnvironmentResolver> {
    manager: Arc<Mutex<WorkspaceManager>>,
    resolver: Arc<Resolver>,
}

impl<Resolver: ShellEnvironmentResolver> ResolveStartupPathUseCase<Resolver> {
    pub fn new(manager: Arc<Mutex<WorkspaceManager>>, resolver: Arc<Resolver>) -> Self {
        Self { manager, resolver }
    }

    pub async fn execute(&self) -> Result<(), EnvResolutionError> {
        if self.manager.lock().unwrap().resolved_path().is_some() {
            return Ok(());
        }
        let path = self.resolver.resolve_path().await?;
        self.manager.lock().unwrap().set_resolved_path(path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeShellEnvironmentResolver;

    #[tokio::test]
    async fn caches_the_resolved_path() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let resolver = Arc::new(FakeShellEnvironmentResolver::returning(Ok(
            "/usr/bin:/bin".to_string()
        )));
        let use_case = ResolveStartupPathUseCase::new(Arc::clone(&manager), Arc::clone(&resolver));

        use_case.execute().await.unwrap();

        assert_eq!(
            manager.lock().unwrap().resolved_path(),
            Some("/usr/bin:/bin")
        );
    }

    #[tokio::test]
    async fn resolver_is_only_called_once_across_multiple_executes() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let resolver = Arc::new(FakeShellEnvironmentResolver::returning(Ok(
            "/usr/bin".to_string()
        )));
        let use_case = ResolveStartupPathUseCase::new(Arc::clone(&manager), Arc::clone(&resolver));

        use_case.execute().await.unwrap();
        use_case.execute().await.unwrap();

        assert_eq!(resolver.call_count(), 1);
    }

    #[tokio::test]
    async fn propagates_resolution_failure() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let resolver = Arc::new(FakeShellEnvironmentResolver::returning(Err(
            EnvResolutionError::ResolutionFailed("boom".to_string()),
        )));
        let use_case = ResolveStartupPathUseCase::new(manager, resolver);

        assert!(use_case.execute().await.is_err());
    }
}
