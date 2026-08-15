use std::path::PathBuf;

use serde::Deserialize;
use uuid::Uuid;

use crate::env::{HomeDirResolver, PlatformHomeDirResolver};

/// One `claude` CLI process's live state, as Claude Code itself maintains it
/// at `~/.claude/sessions/<pid>.json` — one JSON object per file, rewritten
/// in place on every status/name change (including `/rename`). Only the
/// fields Argus cares about are parsed; everything else is ignored.
#[derive(Debug, Deserialize)]
struct ClaudeSessionFile {
    #[serde(rename = "sessionId")]
    session_id: Uuid,
    name: String,
}

/// `~/.claude/sessions`, Argus's window into Claude Code's own session
/// state. `None` when the home directory can't be resolved (see
/// `PlatformHomeDirResolver`).
pub fn claude_sessions_dir() -> Option<PathBuf> {
    PlatformHomeDirResolver.home_dir().map(|home| home.join(".claude/sessions"))
}

/// Reads every `~/.claude/sessions/*.json`, returning `(sessionId, name)`
/// for each one that parses. Missing directory or unreadable/malformed
/// files are skipped rather than failing the whole read — this is a
/// best-effort sync against another process's state, not a critical path.
pub fn read_claude_session_names(dir: &PathBuf) -> Vec<(Uuid, String)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
        .filter_map(|contents| serde_json::from_str::<ClaudeSessionFile>(&contents).ok())
        .map(|file| (file.session_id, file.name))
        .collect()
}
