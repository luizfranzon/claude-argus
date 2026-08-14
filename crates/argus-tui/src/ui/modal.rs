use ratatui::layout::{Constraint, Direction, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::Modal;

/// Dark-navy fill for the modal body — deliberately a shade darker than
/// whatever the terminal's own background is, so the popup reads as a
/// distinct surface rather than a transparent box outline.
const MODAL_BG: Color = Color::Rgb(13, 17, 28);
/// Slightly lighter band behind the title row, same idea as the reference
/// TUI's title bar.
const TITLE_BG: Color = Color::Rgb(30, 41, 59);
const BORDER: Color = Color::Cyan;
const KEY: Color = Color::Cyan;
const HINT: Color = Color::DarkGray;

/// How much of each cell's original brightness survives the dim — lower is
/// darker. Applied to everything already drawn this frame (topbar, sidebar,
/// terminal, statusbar) outside `popup`, so the modal reads as the one thing
/// in focus. Ratatui cells have no real alpha channel, so this fakes it by
/// scaling each cell's actual RGB toward black in place.
const DIM_FACTOR: f32 = 0.35;

fn dim_backdrop(f: &mut Frame, area: Rect, popup: Rect) {
    let buf = f.buffer_mut();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            let inside_popup = x >= popup.x
                && x < popup.x.saturating_add(popup.width)
                && y >= popup.y
                && y < popup.y.saturating_add(popup.height);
            if inside_popup {
                continue;
            }
            let Some(cell) = buf.cell_mut((x, y)) else { continue };
            cell.fg = dim(cell.fg);
            cell.bg = dim(cell.bg);
        }
    }
}

fn dim(color: Color) -> Color {
    let (r, g, b) = to_rgb(color);
    Color::Rgb(
        (f32::from(r) * DIM_FACTOR) as u8,
        (f32::from(g) * DIM_FACTOR) as u8,
        (f32::from(b) * DIM_FACTOR) as u8,
    )
}

/// Approximates every named/indexed `Color` variant as RGB so it can be
/// scaled uniformly — this app's own styles are almost all `Color::Rgb(..)`
/// already, but named ANSI colors (`Cyan`, `DarkGray`, …) still show up here
/// and there and need *some* real triplet to dim consistently.
fn to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (205, 0, 0),
        Color::Green => (0, 205, 0),
        Color::Yellow => (205, 205, 0),
        Color::Blue => (0, 0, 238),
        Color::Magenta => (205, 0, 205),
        Color::Cyan => (0, 205, 205),
        Color::Gray => (229, 229, 229),
        Color::DarkGray => (127, 127, 127),
        Color::LightRed => (255, 0, 0),
        Color::LightGreen => (0, 255, 0),
        Color::LightYellow => (255, 255, 0),
        Color::LightBlue => (92, 92, 255),
        Color::LightMagenta => (255, 0, 255),
        Color::LightCyan => (0, 255, 255),
        Color::White => (255, 255, 255),
        _ => (40, 40, 40),
    }
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width)]).flex(Flex::Center).areas(area);
    let [area] = Layout::vertical([Constraint::Length(height)]).flex(Flex::Center).areas(area);
    area
}

pub fn draw(f: &mut Frame, area: Rect, modal: &Modal) {
    let (title, body, is_prompt) = describe(modal);
    let width = (area.width.saturating_sub(4)).clamp(30, 70);
    let popup = centered(area, width, 8);

    dim_backdrop(f, area, popup);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(MODAL_BG));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title band
            Constraint::Length(1), // divider
            Constraint::Length(2), // body
            Constraint::Length(1), // divider
            Constraint::Length(1), // footer hints
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {title}"),
            Style::default().fg(Color::White).bg(TITLE_BG).add_modifier(Modifier::BOLD),
        )))
        .style(Style::default().bg(TITLE_BG)),
        rows[0],
    );
    f.render_widget(divider(rows[1].width), rows[1]);
    f.render_widget(Paragraph::new(body).style(Style::default().bg(MODAL_BG)), rows[2]);
    f.render_widget(divider(rows[3].width), rows[3]);
    f.render_widget(footer(is_prompt), rows[4]);
}

fn divider(width: u16) -> Paragraph<'static> {
    Paragraph::new("─".repeat(width as usize)).style(Style::default().fg(BORDER).bg(MODAL_BG))
}

fn footer(is_prompt: bool) -> Paragraph<'static> {
    let key_style = Style::default().fg(KEY).add_modifier(Modifier::BOLD);
    let hint_style = Style::default().fg(HINT);
    let spans = if is_prompt {
        vec![
            Span::raw(" "),
            Span::styled("Enter", key_style),
            Span::styled(" confirma  ", hint_style),
            Span::styled("Esc", key_style),
            Span::styled(" cancela", hint_style),
        ]
    } else {
        vec![
            Span::raw(" "),
            Span::styled("y", key_style),
            Span::styled(" confirma  ", hint_style),
            Span::styled("outra tecla", key_style),
            Span::styled(" cancela", hint_style),
        ]
    };
    Paragraph::new(Line::from(spans)).style(Style::default().bg(MODAL_BG))
}

fn describe(modal: &Modal) -> (String, String, bool) {
    match modal {
        Modal::NewWorkspacePath { input } => {
            ("Novo workspace".into(), format!("Caminho do diretório:\n{input}_"), true)
        }
        Modal::RenameSession { input, .. } => {
            ("Renomear sessão".into(), format!("Novo nome:\n{input}_"), true)
        }
        Modal::NewFile { dir, input, .. } => (
            "Novo arquivo".into(),
            format!("Nome em {}:\n{input}_", dir.display()),
            true,
        ),
        Modal::NewDir { dir, input, .. } => (
            "Nova pasta".into(),
            format!("Nome em {}:\n{input}_", dir.display()),
            true,
        ),
        Modal::RenamePath { from, input, .. } => (
            "Renomear".into(),
            format!("Novo nome para {}:\n{input}_", from.display()),
            true,
        ),
        Modal::CommitMessage { input, .. } => {
            ("Commit".into(), format!("Mensagem:\n{input}_"), true)
        }
        Modal::ConfirmCloseSession { .. } => {
            ("Fechar sessão".into(), "Encerrar esta sessão e seu processo claude?".into(), false)
        }
        Modal::ConfirmCloseWorkspace { .. } => (
            "Fechar workspace".into(),
            "Encerrar o workspace e todas as suas sessões?".into(),
            false,
        ),
        Modal::ConfirmDeletePath { path, .. } => (
            "Excluir".into(),
            format!("Enviar para a lixeira: {}?", path.display()),
            false,
        ),
    }
}
