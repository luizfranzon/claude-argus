use argus_application::ports::{EnvResolutionError, ShellEnvironmentResolver};
use async_trait::async_trait;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

/// Resolves PATH the way Windows itself composes it at logon: user PATH
/// (`HKCU\Environment`) followed by system PATH
/// (`HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment`).
/// Reading straight from the registry (rather than trusting this process's own
/// inherited `PATH`) matters because a double-clicked shortcut can carry a
/// stale environment if PATH was changed since the last logon.
pub struct WindowsRegistryPathResolver;

#[async_trait]
impl ShellEnvironmentResolver for WindowsRegistryPathResolver {
    async fn resolve_path(&self) -> Result<String, EnvResolutionError> {
        tokio::task::spawn_blocking(resolve_path_sync)
            .await
            .map_err(|e| EnvResolutionError::ResolutionFailed(e.to_string()))?
    }
}

fn resolve_path_sync() -> Result<String, EnvResolutionError> {
    let user_path = read_path(RegKey::predef(HKEY_CURRENT_USER), "Environment").unwrap_or_default();
    let system_path = read_path(
        RegKey::predef(HKEY_LOCAL_MACHINE),
        r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
    )
    .unwrap_or_default();

    let combined = [system_path, user_path]
        .into_iter()
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join(";");

    if combined.is_empty() {
        return Err(EnvResolutionError::ResolutionFailed(
            "could not read PATH from either HKCU or HKLM Environment keys".to_string(),
        ));
    }

    Ok(expand_env_vars(&combined))
}

fn read_path(hive: RegKey, subkey: &str) -> Option<String> {
    let key = hive.open_subkey(subkey).ok()?;
    key.get_value::<String, _>("Path").ok()
}

/// Expands `%VAR%` references (e.g. `%SystemRoot%\system32`) commonly found in
/// the raw HKLM system PATH value, using this process's own environment.
fn expand_env_vars(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '%' {
            result.push(c);
            continue;
        }
        let mut name = String::new();
        let mut closed = false;
        for next in chars.by_ref() {
            if next == '%' {
                closed = true;
                break;
            }
            name.push(next);
        }
        if closed {
            match std::env::var(&name) {
                Ok(value) => result.push_str(&value),
                Err(_) => {
                    result.push('%');
                    result.push_str(&name);
                    result.push('%');
                }
            }
        } else {
            result.push('%');
            result.push_str(&name);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_known_variable() {
        std::env::set_var("ARGUS_TEST_VAR", "C:\\Windows");
        assert_eq!(
            expand_env_vars("%ARGUS_TEST_VAR%\\system32"),
            "C:\\Windows\\system32"
        );
    }

    #[test]
    fn leaves_unknown_variable_untouched() {
        assert_eq!(expand_env_vars("%NOT_A_REAL_VAR%\\x"), "%NOT_A_REAL_VAR%\\x");
    }

    #[test]
    fn passes_through_strings_without_percent_signs() {
        assert_eq!(expand_env_vars("C:\\bin;D:\\tools"), "C:\\bin;D:\\tools");
    }
}
