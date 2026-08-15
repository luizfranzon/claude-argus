use std::path::PathBuf;

#[cfg(unix)]
pub mod unix_home_dir;
#[cfg(unix)]
pub mod unix_shell_path;
#[cfg(windows)]
pub mod windows_env_path;
#[cfg(windows)]
pub mod windows_home_dir;

#[cfg(unix)]
pub use unix_shell_path::UnixLoginShellPathResolver as PlatformPathResolver;
#[cfg(windows)]
pub use windows_env_path::WindowsRegistryPathResolver as PlatformPathResolver;

#[cfg(unix)]
pub use unix_home_dir::UnixHomeDirResolver as PlatformHomeDirResolver;
#[cfg(windows)]
pub use windows_home_dir::WindowsHomeDirResolver as PlatformHomeDirResolver;

/// Resolves the current user's home directory. An infra-local seam, not an
/// `argus-application` port — nothing above `argus-infrastructure` injects
/// or mocks this, it just isolates the per-platform quirk of *how* home
/// gets resolved (see `WindowsHomeDirResolver`) behind one call, the same
/// way `ShellEnvironmentResolver` isolates PATH resolution. `Option`, not
/// `Result`: every caller today treats "can't resolve" as a silent no-op
/// (see `claude_sessions_dir`/`log_watch_failure`), so there's no error
/// detail worth carrying.
pub trait HomeDirResolver {
    fn home_dir(&self) -> Option<PathBuf>;
}
