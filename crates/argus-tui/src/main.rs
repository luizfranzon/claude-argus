mod app;
mod event;
mod icons;
mod runtime;
mod text_input;
mod ui;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use base64::Engine;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

use app::AppState;
use runtime::Runtime;

/// Pushes text to the system clipboard via an OSC 52 escape sequence
/// written straight to the terminal. Kept alongside `spawn_local_clipboard_copy`
/// as a best-effort second path for terminals that do honor OSC 52 (kitty,
/// alacritty, wezterm, foot, and anything reached over SSH) — GNOME
/// Terminal's VTE deliberately does not implement OSC 52 clipboard-set, so
/// on that terminal this write is a no-op and the local tool call is what
/// actually gets the text into the clipboard.
fn copy_to_system_clipboard(out: &mut impl io::Write, text: &str) -> anyhow::Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    write!(out, "\x1b]52;c;{encoded}\x07")?;
    out.flush()?;
    Ok(())
}

/// Fires off a best-effort, fire-and-forget push of `text` to the local
/// system clipboard by shelling out to whichever clipboard tool is on
/// `PATH` — `wl-copy` under Wayland, `xsel`/`xclip` under X11. This is what
/// actually works on GNOME Terminal, since its VTE backend ignores OSC 52
/// clipboard-set entirely. Failures (no tool installed, remote/SSH session
/// with no local X/Wayland to talk to) are swallowed — `copy_to_system_clipboard`
/// covers the terminals where OSC 52 does work instead.
fn spawn_local_clipboard_copy(text: String) {
    tokio::spawn(async move {
        let _ = copy_to_local_clipboard(&text).await;
    });
}

async fn copy_to_local_clipboard(text: &str) -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;
    use tokio::process::Command;

    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let candidates: &[(&str, &[&str])] = if wayland {
        &[("wl-copy", &[]), ("xsel", &["--clipboard", "--input"]), ("xclip", &["-selection", "clipboard"])]
    } else {
        &[("xsel", &["--clipboard", "--input"]), ("xclip", &["-selection", "clipboard"]), ("wl-copy", &[])]
    };

    let mut last_err = anyhow::anyhow!("no clipboard tool available (install xsel, xclip, or wl-clipboard)");
    for (cmd, args) in candidates {
        let spawned = Command::new(cmd)
            .args(*args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            Err(e) => {
                last_err = e.into();
                continue;
            }
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes()).await;
        }
        match child.wait().await {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => last_err = anyhow::anyhow!("{cmd} exited with {status}"),
            Err(e) => last_err = e.into(),
        }
    }
    Err(last_err)
}

fn initial_directory() -> PathBuf {
    std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        original(info);
    }));
}

/// `cargo run` spawns the built binary in its own new process group (to
/// manage Ctrl+C forwarding itself) but keeps reasserting its own process
/// group as the terminal's foreground group — one-shot `tcsetpgrp` at
/// startup wins the race briefly and then loses it again, so this must be
/// re-claimable, not a single call. `SIGTTOU` has to be ignored around the
/// call: `tcsetpgrp` from a background group would otherwise immediately
/// stop us with the same signal we're trying to route around.
#[cfg(unix)]
fn claim_terminal_foreground() {
    unsafe {
        let previous = libc::signal(libc::SIGTTOU, libc::SIG_IGN);
        libc::tcsetpgrp(libc::STDIN_FILENO, libc::getpgrp());
        libc::signal(libc::SIGTTOU, previous);
    }
}

/// `install_panic_hook` and the cleanup after `run()` only cover exit paths;
/// none of them fire when the process is job-control-stopped.
///
/// Ctrl+Z (SIGTSTP) is a real, user-intended suspend: restore the terminal
/// before actually stopping, and re-enter our TUI state on SIGCONT.
///
/// SIGTTIN/SIGTTOU are different — for this app, being backgrounded is
/// never intentional (see `claim_terminal_foreground`; `cargo run` fights us
/// for the terminal for the whole session, not just at startup), so treat
/// the signal as "reclaim and keep going" rather than "stop": every stdin
/// read or terminal-affecting write that finds us backgrounded re-wins
/// foreground instead of actually suspending.
#[cfg(unix)]
fn spawn_suspend_handler(resumed_tx: mpsc::UnboundedSender<()>) -> anyhow::Result<()> {
    use signal_hook::consts::signal::{SIGCONT, SIGTSTP, SIGTTIN, SIGTTOU};
    use signal_hook::iterator::Signals;

    let mut signals = Signals::new([SIGTSTP, SIGTTIN, SIGTTOU, SIGCONT])?;
    std::thread::spawn(move || {
        for signal in signals.forever() {
            match signal {
                SIGTSTP => {
                    let _ = disable_raw_mode();
                    let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
                    signal_hook::low_level::emulate_default_handler(SIGTSTP).ok();
                }
                SIGTTIN | SIGTTOU => {
                    claim_terminal_foreground();
                }
                SIGCONT => {
                    let _ = enable_raw_mode();
                    let _ = execute!(io::stdout(), EnterAlternateScreen);
                    let _ = resumed_tx.send(());
                }
                _ => {}
            }
        }
    });
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    install_panic_hook();

    #[cfg(unix)]
    claim_terminal_foreground();

    let (resumed_tx, resumed_rx) = mpsc::unbounded_channel();
    #[cfg(unix)]
    spawn_suspend_handler(resumed_tx)?;
    #[cfg(not(unix))]
    drop(resumed_tx);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, resumed_rx).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), DisableMouseCapture, LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut resumed_rx: mpsc::UnboundedReceiver<()>,
) -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let rt = Runtime::new(tx)?;
    rt.resolve_startup_path().await;
    rt.watch_claude_sessions();

    let size = terminal.size()?;
    let full = ratatui::layout::Rect::new(0, 0, size.width, size.height);
    let regions = ui::layout::compute(full, ui::layout::DEFAULT_SIDEBAR_WIDTH);
    let content_size = ui::layout::pty_content_size(regions.terminal);

    let mut state = AppState::new(rt, content_size);
    state.spawn_initial_workspace(initial_directory());

    if state.mouse_capture_enabled {
        execute!(terminal.backend_mut(), EnableMouseCapture)?;
    }

    let mut events = EventStream::new();
    let mut hitmap = ui::HitMap::default();
    let mut mouse_capture_enabled = state.mouse_capture_enabled;

    loop {
        terminal.draw(|f| {
            hitmap = ui::draw(f, &state);
        })?;

        if state.should_quit {
            return Ok(());
        }

        if state.mouse_capture_enabled != mouse_capture_enabled {
            mouse_capture_enabled = state.mouse_capture_enabled;
            if mouse_capture_enabled {
                execute!(terminal.backend_mut(), EnableMouseCapture)?;
            } else {
                execute!(terminal.backend_mut(), DisableMouseCapture)?;
            }
        }

        if let Some(text) = state.clipboard_copy_requested.take() {
            copy_to_system_clipboard(terminal.backend_mut(), &text)?;
            spawn_local_clipboard_copy(text);
        }

        tokio::select! {
            Some(()) = resumed_rx.recv() => {
                terminal.clear()?;
                // SIGTSTP unconditionally disabled mouse capture on the way
                // out; re-apply whatever the app's flag actually wants and
                // resync the shadow var so the top-of-loop diff check above
                // doesn't skip it next iteration.
                if state.mouse_capture_enabled {
                    execute!(terminal.backend_mut(), EnableMouseCapture)?;
                }
                mouse_capture_enabled = state.mouse_capture_enabled;
            }
            Some(app_event) = rx.recv() => {
                state.handle_app_event(app_event);
            }
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        if key.kind != KeyEventKind::Release {
                            state.on_key(key);
                        }
                    }
                    Some(Ok(Event::Mouse(mouse))) => {
                        state.on_mouse(mouse, &hitmap);
                    }
                    Some(Ok(Event::Resize(cols, rows))) => {
                        let full = ratatui::layout::Rect::new(0, 0, cols, rows);
                        let regions = ui::layout::compute(full, state.sidebar_width);
                        let content_size = ui::layout::pty_content_size(regions.terminal);
                        state.set_terminal_size(content_size.0, content_size.1);
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => return Err(e.into()),
                    None => return Ok(()),
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(250)) => {
                // periodic redraw tick so hook-status/spinner-like state never
                // feels stuck between real events, and so a stale status-bar
                // message gets cleared even with no other input arriving
                state.tick();
            }
        }
    }
}
