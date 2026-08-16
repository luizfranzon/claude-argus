use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};
use ratatui::Frame;

use argus_application::ports::FileStatus;

use crate::app::{AppState, WorkspaceEntry};
use crate::i18n::t;
use crate::icons;
use crate::ui::hitmap::HitMap;
use crate::ui::scroll;

/// Color for a File Explorer row's status badge, matching VS Code's own
/// File Explorer decoration palette.
fn status_color(status: FileStatus) -> Color {
    match status {
        FileStatus::Modified => Color::Rgb(224, 175, 104),
        FileStatus::Added | FileStatus::Untracked => Color::Rgb(115, 201, 145),
        FileStatus::Deleted => Color::Rgb(224, 108, 117),
        FileStatus::Renamed => Color::Rgb(115, 201, 145),
        FileStatus::Conflicted => Color::Rgb(224, 108, 117),
    }
}

pub fn draw(f: &mut Frame, area: Rect, _app: &AppState, entry: &WorkspaceEntry, hitmap: &mut HitMap) {
    let root = entry.workspace.directory.clone();
    let rows = entry.explorer.flatten(&root);
    let (visible, offset, visible_selected) =
        scroll::window(&rows, entry.explorer.selected, area.height as usize);

    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(i, (path, depth, is_dir))| {
            let selected = i == visible_selected;
            let row = Rect { x: area.x, y: area.y + i as u16, width: area.width, height: 1 };
            hitmap.explorer_rows.push((row, offset + i, path.clone(), *is_dir));
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string());
            let expanded = entry.explorer.expanded.contains(path);
            let indent = "  ".repeat(*depth);

            let name_style = if selected {
                Style::default().add_modifier(Modifier::BOLD).bg(Color::DarkGray)
            } else if *is_dir {
                Style::default().fg(Color::Rgb(122, 162, 247))
            } else {
                Style::default()
            };

            let prefix = format!("{}{indent}", if selected { "> " } else { "  " });
            let mut width = prefix.chars().count();
            let mut spans = vec![Span::raw(prefix)];
            if *is_dir {
                let (icon, color) = icons::folder(expanded);
                let arrow = format!("{} ", icons::arrow(expanded));
                let folder_icon = format!("{icon} ");
                width += arrow.chars().count() + folder_icon.chars().count();
                spans.push(Span::styled(arrow, Style::default().fg(Color::DarkGray)));
                spans.push(Span::styled(folder_icon, Style::default().fg(color)));
            } else {
                let (icon, color) = icons::for_file(&name);
                let file_icon = format!("{icon} ");
                width += 2 + file_icon.chars().count();
                spans.push(Span::raw("  "));
                spans.push(Span::styled(file_icon, Style::default().fg(color)));
            }
            let status = entry.git_status_for(path, *is_dir);
            let name_style = match status {
                Some(status) if !selected => name_style.fg(status_color(status)),
                _ => name_style,
            };
            width += name.chars().count();
            spans.push(Span::styled(name, name_style));
            if let Some(status) = status {
                // Right-align the badge, leaving at least one column of
                // breathing room from the sidebar's edge (and from the name
                // if the row's too narrow to fit both).
                let pad = (area.width as usize).saturating_sub(width + 2).max(1);
                spans.push(Span::raw(" ".repeat(pad)));
                spans.push(Span::styled(
                    status.letter().to_string(),
                    Style::default().fg(status_color(status)).add_modifier(Modifier::BOLD),
                ));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = if items.is_empty() {
        List::new(vec![ListItem::new(t("sidebar.explorer.loading", &[]))])
    } else {
        List::new(items)
    };
    f.render_widget(list, area);
}
