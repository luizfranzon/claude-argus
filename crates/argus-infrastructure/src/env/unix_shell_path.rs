use std::process::Command;

use argus_application::ports::{EnvResolutionError, ShellEnvironmentResolver};
use async_trait::async_trait;

/// Resolves PATH by running the user's login shell (`$SHELL -lic 'echo -n $PATH'`),
/// which picks up whatever nvm/asdf/etc. rc-file mutations the user's real
/// terminal sessions would see — unlike the PATH this process itself inherited
/// when launched from a GUI icon.
pub struct UnixLoginShellPathResolver;

#[async_trait]
impl ShellEnvironmentResolver for UnixLoginShellPathResolver {
    async fn resolve_path(&self) -> Result<String, EnvResolutionError> {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

        let output = tokio::task::spawn_blocking(move || {
            Command::new(&shell)
                .args(["-lic", "echo -n $PATH"])
                .output()
        })
        .await
        .map_err(|e| EnvResolutionError::ResolutionFailed(e.to_string()))?
        .map_err(|e| EnvResolutionError::ResolutionFailed(e.to_string()))?;

        if !output.status.success() {
            return Err(EnvResolutionError::ResolutionFailed(format!(
                "login shell exited with {}",
                output.status
            )));
        }

        let path = String::from_utf8(output.stdout)
            .map_err(|e| EnvResolutionError::ResolutionFailed(e.to_string()))?;
        let path = path.trim().to_string();

        if path.is_empty() {
            return Err(EnvResolutionError::ResolutionFailed(
                "login shell produced an empty PATH".to_string(),
            ));
        }

        Ok(path)
    }
}
