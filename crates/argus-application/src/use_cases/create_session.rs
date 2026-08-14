use std::sync::{Arc, Mutex};

use argus_domain::{Session, SessionId, SessionStatus, WorkspaceId};
use thiserror::Error;

use crate::ports::{ExitReason, HookCallbackPort, PtyError, PtyPort, SpawnSpec};
use crate::use_cases::handle_process_exit::HandleSessionProcessExitUseCase;
use crate::workspace_manager::WorkspaceManager;

pub type OutputSink = Box<dyn Fn(Vec<u8>) + Send + Sync>;
pub type SessionExitSink = Box<dyn Fn(SessionId, ExitReason) + Send + Sync>;

#[derive(Debug, Error)]
pub enum CreateSessionError {
    #[error("unknown workspace")]
    WorkspaceNotFound,
    #[error("failed to spawn claude process: {0}")]
    PtySpawnFailed(PtyError),
}

/// Spawns a new Session (its own `claude` process attached to its own PTY)
/// inside an existing Workspace. A Workspace can host many concurrently
/// running Sessions (see ADR-0010) — this is the seam that creates each one,
/// whether it's a Workspace's auto-created first Session or an additional
/// one the user spawns later.
///
/// Every spawned `claude` process is passed `--session-id` (so Claude
/// Code's own session id *is* this Session's id — no separate mapping
/// needed) and a `--settings` JSON string wiring `UserPromptSubmit`/`Stop`
/// hooks to GET the `HookCallbackPort`'s URL with `sessionId`/`event` query
/// params, letting the frontend show a "thinking"/"idle" status per Session
/// without parsing PTY output (see docs/adr's status-capture note).
pub struct CreateSessionUseCase<Pty: PtyPort, Hooks: HookCallbackPort> {
    manager: Arc<Mutex<WorkspaceManager>>,
    pty: Arc<Pty>,
    hooks: Arc<Hooks>,
    process_exit: Arc<HandleSessionProcessExitUseCase>,
}

impl<Pty: PtyPort, Hooks: HookCallbackPort> CreateSessionUseCase<Pty, Hooks> {
    pub fn new(
        manager: Arc<Mutex<WorkspaceManager>>,
        pty: Arc<Pty>,
        hooks: Arc<Hooks>,
        process_exit: Arc<HandleSessionProcessExitUseCase>,
    ) -> Self {
        Self {
            manager,
            pty,
            hooks,
            process_exit,
        }
    }

    /// `name` defaults to "Session {N}" (N = 1 + however many Sessions this
    /// Workspace already has) when not given. The count is read while still
    /// holding the manager lock, before spawning, to avoid a race between two
    /// concurrent creates picking the same number.
    pub async fn execute(
        &self,
        workspace_id: WorkspaceId,
        name: Option<String>,
        on_output: OutputSink,
        on_exit: SessionExitSink,
    ) -> Result<Session, CreateSessionError> {
        let (directory, env_path, resolved_name) = {
            let manager = self.manager.lock().unwrap();
            let workspace = manager
                .get(workspace_id)
                .ok_or(CreateSessionError::WorkspaceNotFound)?;
            let directory = workspace.directory.clone();
            let env_path = manager.resolved_path().unwrap_or_default().to_string();
            let resolved_name = name.unwrap_or_else(|| {
                format!(
                    "Session {}",
                    manager.sessions_for_workspace(workspace_id).len() + 1
                )
            });
            (directory, env_path, resolved_name)
        };

        let session_id = SessionId::new();
        let process_exit = Arc::clone(&self.process_exit);

        let spec = SpawnSpec {
            program: "claude".to_string(),
            args: hook_args(session_id, self.hooks.callback_url()),
            cwd: directory,
            env_path,
            on_output,
            on_exit: Box::new(move |reason| {
                process_exit.execute(session_id);
                on_exit(session_id, reason);
            }),
        };

        let pty_handle = self
            .pty
            .spawn(spec)
            .await
            .map_err(CreateSessionError::PtySpawnFailed)?;

        let mut session = Session::new(session_id, workspace_id, resolved_name);
        session.status = SessionStatus::Running;
        self.manager
            .lock()
            .unwrap()
            .register_session(session.clone(), pty_handle);

        Ok(session)
    }
}

/// A `curl` hook command with no shell-quoting hazards: the URL carries
/// `sessionId`/`event` as query params (both plain UUID/ASCII, safe
/// unescaped) rather than a JSON POST body, which would need different
/// escaping on `cmd.exe` vs POSIX shells for the same `--settings` string.
fn hook_command(callback_url: &str, session_id: SessionId, event: &str) -> String {
    format!("curl -s \"{callback_url}?sessionId={session_id}&event={event}\"")
}

fn hook_args(session_id: SessionId, callback_url: String) -> Vec<String> {
    let settings = serde_json::json!({
        "hooks": {
            "UserPromptSubmit": [{
                "hooks": [{
                    "type": "command",
                    "command": hook_command(&callback_url, session_id, "prompt_submitted"),
                }],
            }],
            "Stop": [{
                "hooks": [{
                    "type": "command",
                    "command": hook_command(&callback_url, session_id, "stop"),
                }],
            }],
        },
    });

    vec![
        "--session-id".to_string(),
        session_id.to_string(),
        "--settings".to_string(),
        settings.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::testing::{FakeHookCallbackPort, FakePtyPort};
    use argus_domain::Workspace;

    fn noop_sinks() -> (OutputSink, SessionExitSink) {
        (Box::new(|_| {}), Box::new(|_, _| {}))
    }

    fn use_case(
        manager: Arc<Mutex<WorkspaceManager>>,
        pty: Arc<FakePtyPort>,
    ) -> CreateSessionUseCase<FakePtyPort, FakeHookCallbackPort> {
        let process_exit = Arc::new(HandleSessionProcessExitUseCase::new(Arc::clone(&manager)));
        let hooks = Arc::new(FakeHookCallbackPort::new("http://127.0.0.1:9999/hook"));
        CreateSessionUseCase::new(manager, pty, hooks, process_exit)
    }

    #[tokio::test]
    async fn spawns_and_registers_a_running_session() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let workspace = Workspace::new(WorkspaceId::new(), PathBuf::from("/tmp/project"));
        let workspace_id = workspace.id;
        manager.lock().unwrap().register(workspace);
        let pty = Arc::new(FakePtyPort::new());
        let use_case = use_case(Arc::clone(&manager), Arc::clone(&pty));

        let (on_output, on_exit) = noop_sinks();
        let session = use_case
            .execute(workspace_id, None, on_output, on_exit)
            .await
            .unwrap();

        assert_eq!(session.name, "Session 1");
        assert_eq!(session.status, SessionStatus::Running);
        assert!(manager.lock().unwrap().get_session(session.id).is_some());
    }

    #[tokio::test]
    async fn passes_session_id_and_hook_settings_to_the_spawned_process() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let workspace = Workspace::new(WorkspaceId::new(), PathBuf::from("/tmp/project"));
        let workspace_id = workspace.id;
        manager.lock().unwrap().register(workspace);
        let pty = Arc::new(FakePtyPort::new());
        let use_case = use_case(Arc::clone(&manager), Arc::clone(&pty));

        let (on_output, on_exit) = noop_sinks();
        let session = use_case
            .execute(workspace_id, None, on_output, on_exit)
            .await
            .unwrap();

        let args = pty.last_args().expect("a spawn was recorded");
        assert_eq!(args[0], "--session-id");
        assert_eq!(args[1], session.id.to_string());
        assert_eq!(args[2], "--settings");
        assert!(args[3].contains("UserPromptSubmit"));
        assert!(args[3].contains("Stop"));
        assert!(args[3].contains("http://127.0.0.1:9999/hook"));
    }

    #[tokio::test]
    async fn auto_numbers_subsequent_sessions() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let workspace = Workspace::new(WorkspaceId::new(), PathBuf::from("/tmp/project"));
        let workspace_id = workspace.id;
        manager.lock().unwrap().register(workspace);
        let pty = Arc::new(FakePtyPort::new());
        let use_case = use_case(Arc::clone(&manager), Arc::clone(&pty));

        let (on_output, on_exit) = noop_sinks();
        use_case.execute(workspace_id, None, on_output, on_exit).await.unwrap();
        let (on_output, on_exit) = noop_sinks();
        let second = use_case.execute(workspace_id, None, on_output, on_exit).await.unwrap();

        assert_eq!(second.name, "Session 2");
    }

    #[tokio::test]
    async fn unknown_workspace_fails() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let pty = Arc::new(FakePtyPort::new());
        let use_case = use_case(manager, pty);

        let (on_output, on_exit) = noop_sinks();
        let result = use_case.execute(WorkspaceId::new(), None, on_output, on_exit).await;

        assert!(matches!(result, Err(CreateSessionError::WorkspaceNotFound)));
    }

    #[tokio::test]
    async fn pty_exit_removes_only_that_session() {
        let manager = Arc::new(Mutex::new(WorkspaceManager::new()));
        let workspace = Workspace::new(WorkspaceId::new(), PathBuf::from("/tmp/project"));
        let workspace_id = workspace.id;
        manager.lock().unwrap().register(workspace);
        let pty = Arc::new(FakePtyPort::new());
        let use_case = use_case(Arc::clone(&manager), Arc::clone(&pty));

        let (on_output, on_exit) = noop_sinks();
        let session = use_case.execute(workspace_id, None, on_output, on_exit).await.unwrap();
        let handle = manager.lock().unwrap().pty_handle_for_session(session.id).unwrap();

        pty.trigger_exit(handle, ExitReason::Crashed);

        assert!(manager.lock().unwrap().get_session(session.id).is_none());
        assert!(manager.lock().unwrap().get(workspace_id).is_some());
    }
}
