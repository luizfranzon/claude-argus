mod app;
mod event;
mod icons;
mod runtime;
mod text_input;
mod ui;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

use app::AppState;
use runtime::Runtime;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    install_panic_hook();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), DisableMouseCapture, LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let rt = Runtime::new(tx)?;
    rt.resolve_startup_path().await;

    let size = terminal.size()?;
    let full = ratatui::layout::Rect::new(0, 0, size.width, size.height);
    let regions = ui::layout::compute(full, ui::layout::DEFAULT_SIDEBAR_WIDTH);
    let content_size = ui::layout::pty_content_size(regions.terminal);

    let mut state = AppState::new(rt, content_size);
    state.spawn_initial_workspace(initial_directory());

    let mut events = EventStream::new();
    let mut hitmap = ui::HitMap::default();

    loop {
        terminal.draw(|f| {
            hitmap = ui::draw(f, &state);
        })?;

        if state.should_quit {
            return Ok(());
        }

        tokio::select! {
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
