/// Scroll offset that keeps `selected` centered within a `height`-row
/// viewport whenever the list is taller than the viewport, clamped so the
/// window never runs past either end of the list. Lists in this app render
/// their own rows directly (no `ratatui::widgets::ListState`), since several
/// views build each row from more than plain text (icons, status dots) —
/// this is the manual equivalent of `ListState`'s auto-scroll, just centered
/// instead of "minimally keep in view".
pub fn centered_offset(len: usize, selected: usize, height: usize) -> usize {
    if height == 0 || len <= height {
        return 0;
    }
    let half = height / 2;
    let max_offset = len - height;
    selected.saturating_sub(half).min(max_offset)
}

/// Slices `rows` down to the centered viewport and returns `(visible_rows,
/// offset, selected_index_within_visible_rows)`. `offset` is handed back so
/// callers building click hit-regions can recover each visible row's
/// absolute index (`offset + row_position`) without re-deriving it.
pub fn window<'a, T>(rows: &'a [T], selected: usize, height: usize) -> (&'a [T], usize, usize) {
    let offset = centered_offset(rows.len(), selected, height);
    let end = (offset + height).min(rows.len());
    (&rows[offset..end], offset, selected.saturating_sub(offset))
}
