/// Where a spawned Session's Claude Code hooks (`UserPromptSubmit`/`Stop`)
/// should POST status events back to. Implemented by the infrastructure
/// layer's local HTTP callback server (see docs/adr/0010) — kept as a port
/// so `CreateSessionUseCase` never depends on a concrete HTTP server type.
pub trait HookCallbackPort: Send + Sync {
    /// e.g. `http://127.0.0.1:54213/hook`.
    fn callback_url(&self) -> String;
}
