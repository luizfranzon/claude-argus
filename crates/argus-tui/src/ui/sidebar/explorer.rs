use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};
use ratatui::Frame;

use crate::app::{AppState, WorkspaceEntry};
use crate::i18n::t;
use crate::icons;
use crate::ui::hitmap::HitMap;
use crate::ui::scroll;

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

            let mut spans = vec![
                Span::raw(if selected { "> " } else { "  " }),
                Span::raw(indent),
            ];
            if *is_dir {
                let (icon, color) = icons::folder(expanded);
                spans.push(Span::styled(format!("{} ", icons::arrow(expanded)), Style::default().fg(Color::DarkGray)));
                spans.push(Span::styled(format!("{icon} "), Style::default().fg(color)));
            } else {
                let (icon, color) = icons::for_file(&name);
                spans.push(Span::raw("  "));
                spans.push(Span::styled(format!("{icon} "), Style::default().fg(color)));
            }
            spans.push(Span::styled(name, name_style));

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
