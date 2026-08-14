use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::AppState;
use crate::ui::hitmap::HitMap;
use crate::ui::layout::equal_columns;

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
        let active = app.active_workspace == Some(*workspace_id);
        let style = if active {
            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        f.render_widget(Paragraph::new(Line::from(format!(" {name} "))).style(style), *col);
        hitmap.topbar_tabs.push((*col, *workspace_id));
    }
}
