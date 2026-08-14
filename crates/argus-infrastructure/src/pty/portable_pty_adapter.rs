use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use argus_application::ports::{ExitReason, PtyError, PtyHandleId, PtyPort, SpawnSpec};
use async_trait::async_trait;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

struct PtySession {
    writer: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
}

/// Real `PtyPort` backed by `portable-pty`. Each spawn opens a native PTY,
/// starts the target process attached to its slave side, and dedicates one OS
/// thread to pumping master-side output into `on_output` until EOF, at which
/// point it reaps the child and invokes `on_exit`.
///
/// Sessions are kept behind `Arc` so `write`/`resize`/`kill` only hold the
/// map lock long enough to clone the handle out — the (potentially
/// blocking) I/O against one session's PTY never blocks lookups or I/O on
/// any other session.
#[derive(Default)]
pub struct PortablePtyAdapter {
    sessions: RwLock<HashMap<PtyHandleId, Arc<PtySession>>>,
}

impl PortablePtyAdapter {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PtyPort for PortablePtyAdapter {
    async fn spawn(&self, spec: SpawnSpec) -> Result<PtyHandleId, PtyError> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::SpawnFailed(e.to_string()))?;

        let mut cmd = CommandBuilder::new(&spec.program);
        cmd.args(&spec.args);
        cmd.cwd(&spec.cwd);
        cmd.env("PATH", &spec.env_path);
        // Without an explicit TERM, CLI tools (chalk/ink-based ones like
        // `claude` included) can't detect color support and fall back to
        // plain, unstyled output even though they're attached to a real PTY —
        // this is what real terminal emulators set, so we match that here.
        // `FORCE_COLOR` additionally short-circuits color-detection libraries
        // (chalk, picocolors, ansi-colors, ...) that don't trust TERM/isTTY
        // heuristics through a ConPTY-backed process on Windows.
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("FORCE_COLOR", "3");

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| PtyError::SpawnFailed(e.to_string()))?;
        // Drop our copy of the slave fd/handle: the child owns the real one,
        // and keeping this open would prevent the master's reader from ever
        // observing EOF once the process exits.
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::Io(e.to_string()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| PtyError::Io(e.to_string()))?;

        let handle = PtyHandleId::new();
        let child = Arc::new(Mutex::new(child));

        spawn_output_pump(reader, child.clone(), spec.on_output, spec.on_exit);

        self.sessions.write().unwrap().insert(
            handle,
            Arc::new(PtySession {
                writer: Mutex::new(writer),
                master: Mutex::new(pair.master),
                child,
            }),
        );

        Ok(handle)
    }

    fn write(&self, handle: PtyHandleId, data: &[u8]) -> Result<(), PtyError> {
        let session = self.session(handle)?;
        let result = session.writer.lock().unwrap().write_all(data).map_err(|e| PtyError::Io(e.to_string()));
        result
    }

    fn resize(&self, handle: PtyHandleId, cols: u16, rows: u16) -> Result<(), PtyError> {
        let session = self.session(handle)?;
        let result = session
            .master
            .lock()
            .unwrap()
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::Io(e.to_string()));
        result
    }

    fn kill(&self, handle: PtyHandleId) -> Result<(), PtyError> {
        let session = self.session(handle)?;
        let result = session.child.lock().unwrap().kill().map_err(|e| PtyError::Io(e.to_string()));
        result
    }
}

impl PortablePtyAdapter {
    fn session(&self, handle: PtyHandleId) -> Result<Arc<PtySession>, PtyError> {
        self.sessions
            .read()
            .unwrap()
            .get(&handle)
            .cloned()
            .ok_or(PtyError::UnknownHandle)
    }
}

fn spawn_output_pump(
    mut reader: Box<dyn Read + Send>,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    on_output: Box<dyn Fn(Vec<u8>) + Send + Sync>,
    on_exit: Box<dyn Fn(ExitReason) + Send + Sync>,
) {
    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => on_output(buf[..n].to_vec()),
                Err(_) => break,
            }
        }

        // By the time the master side hits EOF the child has exited (its
        // slave fd/handle closed), so this reap returns immediately rather
        // than blocking indefinitely.
        let reason = match child.lock().unwrap().wait() {
            Ok(status) if status.success() => ExitReason::Normal,
            _ => ExitReason::Crashed,
        };
        on_exit(reason);
    });
}
