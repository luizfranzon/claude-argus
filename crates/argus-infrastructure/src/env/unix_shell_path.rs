use std::process::Command;

use argus_application::ports::{EnvResolutionError, ShellEnvironmentResolver};
use async_trait::async_trait;

/// Resolves PATH by running the user's login shell and reading back its
/// exported environment (`$SHELL -lic env`), which picks up whatever
/// nvm/asdf/etc. rc-file mutations the user's real terminal sessions would
/// see — unlike the PATH this process itself inherited when launched from a
/// GUI icon.
///
/// Deliberately reads `env`'s output rather than `echo -n $PATH`: fish
/// stores `$PATH` as a list and *displays* it space-separated when
/// interpolated directly (`echo $PATH` → `/a /b /c`), even though it still
/// exports the real process environment variable colon-separated like every
/// other shell — `env`'s `PATH=/a:/b:/c` line reads that real exported form
/// regardless of which shell produced it.
pub struct UnixLoginShellPathResolver;

#[async_trait]
impl ShellEnvironmentResolver for UnixLoginShellPathResolver {
    async fn resolve_path(&self) -> Result<String, EnvResolutionError> {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

        let output = tokio::task::spawn_blocking(move || {
            Command::new(&shell).args(["-lic", "env"]).output()
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

        let env_output = String::from_utf8(output.stdout)
            .map_err(|e| EnvResolutionError::ResolutionFailed(e.to_string()))?;
        let path = env_output
            .lines()
            .find_map(|line| line.strip_prefix("PATH="))
            .map(str::to_string);

        match path {
            Some(path) if !path.is_empty() => Ok(path),
            _ => Err(EnvResolutionError::ResolutionFailed(
                "login shell produced an empty PATH".to_string(),
            )),
        }
    }
}
