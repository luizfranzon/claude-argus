use std::path::PathBuf;

use super::HomeDirResolver;

/// `HOME` is unset for a normally-launched Windows process — it's only
/// present when launched from a Unix-like shell (Git Bash, WSL interop).
/// `USERPROFILE` is what Winlogon actually sets for every interactive user
/// session, and what the Node-based Claude Code CLI itself resolves `~`
/// against — so it's checked first for shells that carry a real `HOME`,
/// then falls back to it. No deeper fallback (`HOMEDRIVE`+`HOMEPATH`,
/// `SHGetKnownFolderPath`) is wired up: those matter for service accounts
/// or redirected profiles, which don't apply to Argus's desktop app
/// launched by a logged-in interactive user.
pub struct WindowsHomeDirResolver;

impl HomeDirResolver for WindowsHomeDirResolver {
    fn home_dir(&self) -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_home_when_set() {
        std::env::set_var("HOME", r"C:\home-style\argus-test");
        std::env::set_var("USERPROFILE", r"C:\Users\argus-test");
        assert_eq!(
            WindowsHomeDirResolver.home_dir(),
            Some(PathBuf::from(r"C:\home-style\argus-test"))
        );
        std::env::remove_var("HOME");
        std::env::remove_var("USERPROFILE");
    }

    #[test]
    fn falls_back_to_userprofile_when_home_unset() {
        std::env::remove_var("HOME");
        std::env::set_var("USERPROFILE", r"C:\Users\argus-test");
        assert_eq!(
            WindowsHomeDirResolver.home_dir(),
            Some(PathBuf::from(r"C:\Users\argus-test"))
        );
        std::env::remove_var("USERPROFILE");
    }

    #[test]
    fn none_when_neither_is_set() {
        std::env::remove_var("HOME");
        std::env::remove_var("USERPROFILE");
        assert_eq!(WindowsHomeDirResolver.home_dir(), None);
    }
}
