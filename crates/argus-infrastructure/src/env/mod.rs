#[cfg(unix)]
pub mod unix_shell_path;
#[cfg(windows)]
pub mod windows_env_path;

#[cfg(unix)]
pub use unix_shell_path::UnixLoginShellPathResolver as PlatformPathResolver;
#[cfg(windows)]
pub use windows_env_path::WindowsRegistryPathResolver as PlatformPathResolver;
