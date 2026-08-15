use std::path::PathBuf;

use super::HomeDirResolver;

/// `$HOME` is reliably set for any interactive login session on Linux and
/// macOS alike (Finder-launched or Terminal-launched) — no divergence
/// between the two worth a separate adapter.
pub struct UnixHomeDirResolver;

impl HomeDirResolver for UnixHomeDirResolver {
    fn home_dir(&self) -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_from_home_env_var() {
        std::env::set_var("HOME", "/home/argus-test");
        assert_eq!(
            UnixHomeDirResolver.home_dir(),
            Some(PathBuf::from("/home/argus-test"))
        );
        std::env::remove_var("HOME");
    }

    #[test]
    fn none_when_home_unset() {
        std::env::remove_var("HOME");
        assert_eq!(UnixHomeDirResolver.home_dir(), None);
    }
}
