use ratatui::layout::{Constraint, Direction, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

use crate::app::Modal;
use crate::i18n::t;
use crate::ui::border::Border;
use crate::ui::overlay::{dim_backdrop, BORDER, HINT, KEY, SURFACE_BG as MODAL_BG, TITLE_BG};

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
    let inner = Border::solid(BORDER).bg(MODAL_BG).render(f, popup);

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
            Span::styled(format!(" {}  ", t("modal.footer.confirm", &[])), hint_style),
            Span::styled("Esc", key_style),
            Span::styled(format!(" {}", t("modal.footer.cancel", &[])), hint_style),
        ]
    } else {
        vec![
            Span::raw(" "),
            Span::styled("y", key_style),
            Span::styled(format!(" {}  ", t("modal.footer.confirm", &[])), hint_style),
            Span::styled(t("modal.footer.other_key", &[]), key_style),
            Span::styled(format!(" {}", t("modal.footer.cancel", &[])), hint_style),
        ]
    };
    Paragraph::new(Line::from(spans)).style(Style::default().bg(MODAL_BG))
}

fn describe(modal: &Modal) -> (String, String, bool) {
    match modal {
        Modal::NewWorkspacePath { input } => (
            t("modal.new_workspace.title", &[]),
            t("modal.new_workspace.body", &[("input", input)]),
            true,
        ),
        Modal::RenameSession { input, .. } => (
            t("modal.rename_session.title", &[]),
            t("modal.rename_session.body", &[("input", input)]),
            true,
        ),
        Modal::NewFile { dir, input, .. } => (
            t("modal.new_file.title", &[]),
            t("modal.new_file.body", &[("dir", &dir.display().to_string()), ("input", input)]),
            true,
        ),
        Modal::NewDir { dir, input, .. } => (
            t("modal.new_dir.title", &[]),
            t("modal.new_dir.body", &[("dir", &dir.display().to_string()), ("input", input)]),
            true,
        ),
        Modal::RenamePath { from, input, .. } => (
            t("modal.rename_path.title", &[]),
            t("modal.rename_path.body", &[("from", &from.display().to_string()), ("input", input)]),
            true,
        ),
        Modal::ConfirmCloseSession { .. } => {
            (t("modal.confirm_close_session.title", &[]), t("modal.confirm_close_session.body", &[]), false)
        }
        Modal::ConfirmCloseWorkspace { .. } => {
            (t("modal.confirm_close_workspace.title", &[]), t("modal.confirm_close_workspace.body", &[]), false)
        }
        Modal::ConfirmDeletePath { path, .. } => (
            t("modal.confirm_delete_path.title", &[]),
            t("modal.confirm_delete_path.body", &[("path", &path.display().to_string())]),
            false,
        ),
    }
}
