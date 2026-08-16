use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{AppState, RuntimeStatus, WorkspaceEntry};
use crate::ui::blink;
use crate::ui::hitmap::HitMap;
use crate::ui::layout::equal_columns;

/// Alternation period for the "unread" workspace-tab indicator — matches the
/// sidebar dot (`sidebar::agents::UNREAD_BLINK_MS`) so both blink in sync.
const UNREAD_BLINK_MS: u128 = 500;

/// Highest-priority status across a workspace's sessions, for the topbar's
/// single aggregate glyph: a session waiting on the user outranks one that
/// merely finished unseen, which outranks nothing to report.
fn aggregate_indicator(app: &AppState, entry: &WorkspaceEntry) -> Option<(&'static str, Color)> {
    let mut any_unread = false;
    for session_id in &entry.sessions {
        let Some(session_entry) = app.sessions.get(session_id) else { continue };
        if session_entry.status == Some(RuntimeStatus::Waiting) {
            return Some(("◆", Color::Magenta));
        }
        any_unread |= session_entry.unread;
    }
    if any_unread {
        if blink::on(UNREAD_BLINK_MS) {
            Some(("○", Color::Green))
        } else {
            Some((" ", Color::Green))
        }
    } else {
        None
    }
}

pub fn draw(f: &mut Frame, area: Rect, app: &AppState, hitmap: &mut HitMap) {
    if app.workspaces.is_empty() {
        f.render_widget(
            Paragraph::new(" argus-tui — sem workspaces ").style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    let columns = equal_columns(area, app.workspaces.len());
    for (col, workspace_id) in columns.iter().zip(app.workspaces.iter()) {
        let Some(entry) = app.workspace_entries.get(workspace_id) else { continue };
        let name = entry
            .workspace
            .directory
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| entry.workspace.directory.display().to_string());
        let title = match &entry.branch {
            Some(branch) => format!("{name} @ {branch}"),
            None => name,
        };
        let active = app.active_workspace == Some(*workspace_id);
        let style = if active {
            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let label = match aggregate_indicator(app, entry) {
            Some((glyph, color)) => {
                let indicator_style = if active { style.fg(color) } else { Style::default().fg(color) };
                Line::from(vec![
                    Span::raw(" "),
                    Span::styled(glyph, indicator_style),
                    Span::styled(format!(" {title} "), style),
                ])
            }
            None => Line::from(format!(" {title} ")).style(style),
        };
        f.render_widget(Paragraph::new(label), *col);
        hitmap.topbar_tabs.push((*col, *workspace_id));
    }
}
