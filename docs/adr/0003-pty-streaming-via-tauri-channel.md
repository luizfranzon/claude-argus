---
status: accepted
---

# Stream PTY output via a per-Workspace Tauri Channel, not `emit` events

`claude`'s terminal output is high-volume and continuous. Tauri's regular `emit`/`listen`
event system serializes every payload as JSON, which adds real overhead per chunk at
terminal-output volume. Instead, each Workspace gets its own `tauri::ipc::Channel<Vec<u8>>`,
created on the frontend and passed into the create-workspace command, so raw bytes go
straight from the PTY reader thread to xterm.js with no JSON envelope.

Regular `emit` events are still used for low-frequency lifecycle signals
(`workspace-created`, `workspace-closed`, `startup-path-resolved`) where JSON overhead
doesn't matter. Picking Channels for output only, not universally, was a deliberate
trade-off — it means two different plumbing mechanisms exist in the Tauri command surface,
which is worth knowing before adding a third kind of frontend↔backend signal.
