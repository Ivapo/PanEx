use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::{App, AppMode, PaneView};
use crate::layout::{LayoutNode, SplitDirection};
use crate::sort::SortField;

/// Amber accent — folder icons and the footer brand.
const ACCENT: Color = Color::Rgb(255, 191, 0);
/// Padded so a status line clipped at the column edge can't butt up against it.
const BRAND: &str = "  panex ";
/// Scroll thumb glyph. Half-width so it reads lighter than the full block
/// while staying distinct from the `│` border it is drawn over.
const SCROLL_THUMB: &str = "▐";

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Reserve bottom row for status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    app.pane_views.clear();
    render_layout_node(frame, app, &app.layout_root.clone(), chunks[0]);
    render_status_bar(frame, app, chunks[1]);

    // Render dialog overlays
    match &app.mode {
        AppMode::Help => {
            render_help_dialog(frame, area);
        }
        AppMode::Confirm { title, message, selected, .. } => {
            render_confirm_dialog(frame, area, title, message, *selected);
        }
        AppMode::Prompt {
            title,
            input,
            cursor,
            ..
        } => {
            render_prompt_dialog(frame, area, title, input, *cursor);
        }
        AppMode::Rename { input, cursor, .. } => {
            render_prompt_dialog(frame, area, "Rename", input, *cursor);
        }
        AppMode::PathEdit { input, cursor, completions, completion_index, .. } => {
            let title = if let Some(idx) = completion_index {
                format!("Go to path ({}/{})", idx + 1, completions.len())
            } else {
                "Go to path".to_string()
            };
            render_prompt_dialog(frame, area, &title, input, *cursor);
        }
        AppMode::FavoritesList { selected, .. } => {
            render_favorites_dialog(frame, area, &app.config.favorites.paths, *selected);
        }
        _ => {}
    }
}

fn render_layout_node(frame: &mut Frame, app: &mut App, node: &LayoutNode, area: Rect) {
    match node {
        LayoutNode::Leaf { pane_id } => {
            render_pane(frame, app, pane_id, area);
        }
        LayoutNode::Split {
            direction,
            first,
            second,
            ratio,
        } => {
            let dir = match direction {
                SplitDirection::Vertical => Direction::Horizontal,
                SplitDirection::Horizontal => Direction::Vertical,
            };
            let pct = (*ratio * 100.0) as u16;
            let chunks = Layout::default()
                .direction(dir)
                .constraints([
                    Constraint::Percentage(pct),
                    Constraint::Percentage(100 - pct),
                ])
                .split(area);
            render_layout_node(frame, app, first, chunks[0]);
            render_layout_node(frame, app, second, chunks[1]);
        }
    }
}

fn render_pane(frame: &mut Frame, app: &mut App, pane_id: &str, area: Rect) {
    let is_active = app.active_pane_id == pane_id;
    let border_style = if is_active {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let (current_path, search_mode_query) = app
        .pane_map
        .get(pane_id)
        .map(|p| {
            let sq = if let AppMode::Search { pane_id: sid } = &app.mode {
                if sid == pane_id {
                    Some(p.search_query.clone())
                } else {
                    None
                }
            } else {
                None
            };
            (p.current_path.clone(), sq)
        })
        .unwrap_or_default();

    // Build title with path (truncated from left if too long).
    // Truncate by chars, not bytes — byte slicing panics on multi-byte
    // characters and when the pane is narrower than the path suffix.
    let max_title_len = area.width.saturating_sub(4) as usize;
    let path_chars: Vec<char> = current_path.chars().collect();
    let display_path = if path_chars.len() > max_title_len {
        let keep = max_title_len.saturating_sub(1);
        let tail: String = path_chars[path_chars.len() - keep..].iter().collect();
        format!("…{}", tail)
    } else {
        current_path.clone()
    };

    let fav_indicator = if app.config.is_favorite(&current_path) { "★ " } else { "" };
    let title = if let Some(ref query) = search_mode_query {
        format!(" 🔍 {} ", query)
    } else {
        format!(" {}{} ", fav_indicator, display_path)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 {
        app.pane_views.insert(
            pane_id.to_string(),
            PaneView { area, list_area: Rect::default() },
        );
        return;
    }

    // Column header row
    let header_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    let list_area = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height - 1,
    };

    app.pane_views.insert(pane_id.to_string(), PaneView { area, list_area });

    render_column_header(frame, app, header_area);
    render_file_list(frame, app, pane_id, list_area);
}

fn render_column_header(frame: &mut Frame, app: &App, area: Rect) {
    let fields = [
        (SortField::Name, "Name"),
        (SortField::Extension, "Ext"),
        (SortField::Size, "Size"),
        (SortField::Modified, "Modified"),
    ];

    let spans: Vec<Span> = fields
        .iter()
        .enumerate()
        .flat_map(|(i, (field, label))| {
            let mut parts = Vec::new();
            if i > 0 {
                parts.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
            }
            if *field == app.sort_field {
                parts.push(Span::styled(
                    format!("{} {}", label, app.sort_direction.indicator()),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                parts.push(Span::styled(
                    label.to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            parts
        })
        .collect();

    let header_line = Line::from(spans);
    let header = Paragraph::new(header_line);
    frame.render_widget(header, area);
}

fn render_file_list(frame: &mut Frame, app: &mut App, pane_id: &str, area: Rect) {
    let is_active = app.active_pane_id == pane_id;

    let pane = match app.pane_map.get_mut(pane_id) {
        Some(p) => p,
        None => return,
    };

    if pane.entries.is_empty() {
        let empty = Paragraph::new("  (empty)")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, area);
        return;
    }

    let rows: Vec<Row> = pane
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let icon = if entry.is_dir { "\u{f07b}" } else { "\u{f016}" };
            let icon_color = if entry.is_dir { ACCENT } else { Color::DarkGray };

            let ext = if entry.is_dir {
                String::new()
            } else {
                entry
                    .name
                    .rsplit_once('.')
                    .map(|(_, e)| e.to_string())
                    .unwrap_or_default()
            };

            let size = if entry.is_dir {
                String::from("—")
            } else {
                format_size(entry.size)
            };

            let modified = format_date(entry.modified);

            let is_selected = pane.selected_paths.contains(&entry.path);
            let is_focused = is_active && pane.focus_index == i as i32;

            let (row_bg, name_fg) = if is_focused && is_selected {
                (Some(Color::Blue), Color::White)
            } else if is_focused {
                (Some(Color::DarkGray), Color::White)
            } else if is_selected {
                (Some(Color::Blue), Color::White)
            } else if entry.is_dir {
                (None, Color::Blue)
            } else {
                (None, Color::Reset)
            };

            let mut icon_style = Style::default().fg(icon_color);
            let mut name_style = Style::default().fg(name_fg);
            if entry.is_dir && !is_focused && !is_selected {
                name_style = name_style.add_modifier(Modifier::BOLD);
            }
            if is_focused && is_selected {
                name_style = name_style.add_modifier(Modifier::BOLD);
            }
            if let Some(bg) = row_bg {
                icon_style = icon_style.bg(bg);
                name_style = name_style.bg(bg);
            }

            let name_cell = Cell::from(Line::from(vec![
                Span::styled(format!("{} ", icon), icon_style),
                Span::styled(entry.name.clone(), name_style),
            ]));

            let row_style = match row_bg {
                Some(bg) => Style::default().bg(bg).fg(name_fg),
                None => Style::default().fg(name_fg),
            };

            Row::new(vec![
                name_cell,
                Cell::from(ext),
                Cell::from(size),
                Cell::from(modified),
            ])
            .style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Min(10),
        Constraint::Length(6),
        Constraint::Length(8),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths).column_spacing(1);

    let len = pane.entries.len();
    let viewport = area.height as usize;
    frame.render_stateful_widget(table, area, &mut pane.table_state);

    // Scroll indicator on the pane's right border. Read the offset *after*
    // rendering — the table adjusts it to keep the selected row visible.
    render_scroll_thumb(
        frame,
        Rect {
            x: area.x + area.width,
            y: area.y,
            width: 1,
            height: area.height,
        },
        len,
        viewport,
        pane.table_state.offset(),
    );
}

/// Draws a scroll thumb over `area` (one column, spanning the list rows).
///
/// Hand-rolled rather than `ratatui::Scrollbar`, which rounds the thumb's
/// start and end independently and so visibly changes the thumb's length by a
/// cell as you scroll. Here the length is computed once and only the position
/// moves, so the thumb sits flush at both ends and never resizes mid-scroll.
fn render_scroll_thumb(
    frame: &mut Frame,
    area: Rect,
    len: usize,
    viewport: usize,
    offset: usize,
) {
    let track = area.height as usize;
    // Nothing to indicate when everything already fits.
    if track == 0 || viewport == 0 || len <= viewport {
        return;
    }

    // Length is proportional to the visible fraction, fixed for a given list.
    let thumb = (track * viewport / len).clamp(1, track);
    let travel = track - thumb;
    let max_offset = len - viewport;
    // Round to nearest so the thumb lands flush at the top and the bottom.
    let start = (offset.min(max_offset) * travel + max_offset / 2) / max_offset;

    let style = Style::default().fg(Color::DarkGray);
    let buf = frame.buffer_mut();
    for i in start..start + thumb {
        let y = area.y + i as u16;
        if y >= area.y + area.height {
            break;
        }
        // `cell_mut` returns None off-buffer — a pane squeezed to the screen
        // edge can put this column out of range, and indexing would panic.
        if let Some(cell) = buf.cell_mut((area.x, y)) {
            cell.set_symbol(SCROLL_THUMB).set_style(style);
        }
    }
}


fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let pane = app.pane_map.get(&app.active_pane_id);
    let item_count = pane.map(|p| p.entries.len()).unwrap_or(0);
    let sel_count = pane.map(|p| p.selected_paths.len()).unwrap_or(0);

    let mut parts = vec![
        format!(" {} items", item_count),
        format!("Sort: {} {}", app.sort_field.label(), app.sort_direction.indicator()),
    ];

    if sel_count > 0 {
        parts.insert(1, format!("{} selected", sel_count));
    }

    if !app.show_hidden {
        parts.push("Hidden: off".to_string());
    } else {
        parts.push("Hidden: on".to_string());
    }

    let left = parts.join(" │ ");

    let mode_hint = match &app.mode {
        AppMode::Normal => "?:help q:quit |_:split +-:size W:close /:search f:fav",
        AppMode::Help => "Esc/q/?:close",
        AppMode::Search { .. } => "Esc:cancel  Enter:confirm",
        AppMode::Rename { .. } => "Esc:cancel  Enter:rename",
        AppMode::Confirm { .. } => "←→:select  Enter:confirm  y/n  Esc:cancel",
        AppMode::Prompt { .. } => "Esc:cancel  Enter:create",
        AppMode::PathEdit { .. } => "Tab:complete  Bksp:up dir  Enter:go  Esc:cancel",
        AppMode::FavoritesList { .. } => "↑↓:select  Enter:go  e:edit path  d:remove  Esc:cancel",
    };

    let line = match &app.status_message {
        Some(msg) => Line::from(vec![
            Span::styled(msg.clone(), Style::default().fg(Color::Yellow)),
            Span::raw("  "),
            Span::styled(mode_hint, Style::default().fg(Color::DarkGray)),
        ]),
        None => Line::from(vec![
            Span::styled(left, Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled(mode_hint, Style::default().fg(Color::DarkGray)),
        ]),
    };

    // Split the row so a long status line is clipped rather than running
    // under the brand.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(BRAND.chars().count() as u16),
        ])
        .split(area);

    frame.render_widget(Paragraph::new(line), cols[0]);
    frame.render_widget(
        Paragraph::new(BRAND).style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        cols[1],
    );
}

fn render_confirm_dialog(frame: &mut Frame, area: Rect, title: &str, message: &str, selected: usize) {
    let dialog = centered_rect(50, 7, area);
    frame.render_widget(Clear, dialog);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(" {} ", title));
    let inner = block.inner(dialog);
    frame.render_widget(block, dialog);

    let yes_style = if selected == 0 {
        Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let no_style = if selected == 1 {
        Style::default().fg(Color::Black).bg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let text = Paragraph::new(vec![
        Line::from(""),
        Line::from(message.to_string()),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(" Yes ", yes_style),
            Span::raw("   "),
            Span::styled(" No ", no_style),
        ]),
    ]);
    frame.render_widget(text, inner);
}

fn render_prompt_dialog(frame: &mut Frame, area: Rect, title: &str, input: &str, cursor: usize) {
    let dialog = centered_rect(60, 7, area);
    frame.render_widget(Clear, dialog);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(format!(" {} ", title));
    let inner = block.inner(dialog);
    frame.render_widget(block, dialog);

    // Show input with cursor
    let (before, after) = input.split_at(cursor.min(input.len()));
    let line = Line::from(vec![
        Span::raw("  > "),
        Span::raw(before),
        Span::styled(
            if after.is_empty() { " " } else { &after[..1] },
            Style::default().bg(Color::White).fg(Color::Black),
        ),
        Span::raw(if after.len() > 1 { &after[1..] } else { "" }),
    ]);

    let text = Paragraph::new(vec![Line::from(""), line]);
    frame.render_widget(text, inner);
}

fn render_favorites_dialog(frame: &mut Frame, area: Rect, favorites: &[String], selected: usize) {
    let height = (favorites.len() as u16 + 4).min(area.height.saturating_sub(4));
    let dialog = centered_rect(60, height, area);
    frame.render_widget(Clear, dialog);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" ★ Favorites (e:edit path) ");
    let inner = block.inner(dialog);
    frame.render_widget(block, dialog);

    let mut lines = Vec::new();
    for (i, path) in favorites.iter().enumerate() {
        let style = if i == selected {
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(Span::styled(format!("  {}  ", path), style)));
    }

    let text = Paragraph::new(lines);
    frame.render_widget(text, inner);
}

fn help_lines(sections: &[(&str, &[(&str, &str)])]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (i, (title, items)) in sections.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(
            format!(" {}", title),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));
        for (key, desc) in items.iter() {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<12}", key), Style::default().fg(Color::Cyan)),
                Span::styled((*desc).to_string(), Style::default().fg(Color::White)),
            ]));
        }
    }
    lines
}

fn render_help_dialog(frame: &mut Frame, area: Rect) {
    let left = help_lines(&[
        (
            "Navigation",
            &[
                ("j/k ↑/↓", "move focus"),
                ("Enter", "open / enter folder"),
                ("Bksp", "up one directory"),
                ("~ / Home", "go to home"),
                ("g / G", "jump to top / bottom"),
                ("PgUp/PgDn", "page up / down"),
                ("Tab", "next pane"),
                ("e", "edit path / favorites"),
                ("f", "toggle favorite"),
            ],
        ),
        (
            "Panes",
            &[
                ("|", "split vertical"),
                ("_", "split horizontal"),
                ("+ / =", "grow pane 25%"),
                ("-", "shrink pane 25%"),
                ("W", "close pane"),
            ],
        ),
        (
            "Selection",
            &[
                ("Shift+j/k", "extend selection"),
                ("Ctrl+a", "select all"),
                ("Esc", "clear selection"),
            ],
        ),
    ]);

    let right = help_lines(&[
        (
            "Files",
            &[
                ("y / x", "copy / cut"),
                ("p, Ctrl+v", "paste"),
                ("r / F2", "rename"),
                ("d / Del", "delete (trash)"),
                ("n / N", "new file / folder"),
                ("o", "open in default app"),
                ("t", "open in terminal"),
            ],
        ),
        (
            "View",
            &[
                ("/, Ctrl+f", "search"),
                ("s / S", "sort field / direction"),
                (".", "show hidden files"),
                ("F5", "refresh"),
            ],
        ),
        (
            "Mouse",
            &[
                ("wheel", "scroll pane at cursor"),
                ("click", "focus pane / select row"),
            ],
        ),
        ("Other", &[("?", "toggle this help"), ("q", "quit")]),
    ]);

    let content_height = left.len().max(right.len()) as u16 + 2;
    let height = content_height.min(area.height.saturating_sub(2));
    let width = 74u16.min(area.width.saturating_sub(4));
    let dialog = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, dialog);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Keyboard Shortcuts ");
    let inner = block.inner(dialog);
    frame.render_widget(block, dialog);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);
    frame.render_widget(Paragraph::new(left), cols[0]);
    frame.render_widget(Paragraph::new(right), cols[1]);
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_y = area.height.saturating_sub(height) / 2;
    let popup_width = area.width * percent_x / 100;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    Rect::new(popup_x + area.x, popup_y + area.y, popup_width, height)
}

fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < units.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{} B", bytes)
    } else {
        format!("{:.1} {}", size, units[unit_idx])
    }
}

fn format_date(timestamp: u64) -> String {
    if timestamp == 0 {
        return "—".to_string();
    }
    // Simple date formatting: DD/MM/YY
    let secs = timestamp as i64;
    let days = secs / 86400;
    // Approximate: days since epoch to date
    // Using a simple algorithm
    let mut y = 1970i32;
    let mut remaining = days;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let month_days = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 0usize;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md as i64 {
            m = i;
            break;
        }
        remaining -= md as i64;
    }
    let d = remaining + 1;
    format!("{:02}/{:02}/{:02}", d, m + 1, y % 100)
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::resize_axis;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render(app: &mut App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn footer(screen: &str) -> String {
        screen.lines().last().unwrap().to_string()
    }

    #[test]
    fn brand_sits_at_the_right_edge_of_the_footer() {
        let mut app = App::new().unwrap();
        let screen = render(&mut app, 80, 20);
        let last = footer(&screen);
        assert!(
            last.trim_end().ends_with("panex"),
            "footer should end with the brand, got: {last:?}"
        );
    }

    /// A status message long enough to reach the brand must be clipped by the
    /// layout column instead of overwriting it.
    #[test]
    fn long_status_does_not_run_under_the_brand() {
        let mut app = App::new().unwrap();
        app.set_status("x".repeat(200));
        let last = footer(&render(&mut app, 80, 20));
        assert!(
            last.trim_end().ends_with("panex"),
            "brand should survive a long status, got: {last:?}"
        );
        assert_eq!(last.len(), 80);
    }

    /// End-to-end: growing the left pane moves the divider right on screen.
    #[test]
    fn resize_widens_the_pane_on_screen() {
        let mut app = App::new().unwrap();
        let second = app.next_pane_id();
        app.layout_root = crate::layout::split_pane(
            &app.layout_root,
            &app.active_pane_id.clone(),
            &second,
            SplitDirection::Vertical,
        );
        app.pane_map
            .insert(second.clone(), crate::app::PaneState::new("/"));

        let divider_x = |screen: &str| {
            // Row 0 is the top border of both panes: "┌──…┐┌──…┐". The second
            // '┌' marks where the right pane starts. Count chars, not bytes —
            // box-drawing glyphs are 3 bytes each.
            screen
                .lines()
                .next()
                .unwrap()
                .chars()
                .enumerate()
                .filter(|(_, c)| *c == '┌')
                .nth(1)
                .map(|(i, _)| i)
        };

        let before = divider_x(&render(&mut app, 80, 20)).unwrap();

        resize_axis(&mut app.layout_root, &app.active_pane_id.clone(), SplitDirection::Vertical, 1)
            .unwrap();
        let after = divider_x(&render(&mut app, 80, 20)).unwrap();

        assert!(after > before, "divider should move right: {before} -> {after}");
        assert_eq!(before, 40);
        assert_eq!(after, 50, "0.625 of 80 columns");
    }
}

#[cfg(test)]
mod scrollbar_tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn app_with_entries(count: usize) -> (App, String) {
        let mut app = App::new().unwrap();
        let id = app.active_pane_id.clone();
        let entries = (0..count)
            .map(|i| panex_core::FileEntry {
                name: format!("file-{i:02}.txt"),
                path: format!("/x/file-{i:02}.txt"),
                is_dir: false,
                size: 1024,
                modified: 1_700_000_000,
            })
            .collect();
        app.pane_map.get_mut(&id).unwrap().entries = entries;
        (app, id)
    }

    /// Rows of the pane's right border column, top to bottom.
    fn right_border(app: &mut App, w: u16, h: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        (0..buf.area.height)
            .map(|y| buf[(w - 1, y)].symbol().to_string())
            .collect()
    }

    fn thumb_rows(border: &[String]) -> Vec<usize> {
        border
            .iter()
            .enumerate()
            .filter(|(_, s)| s.as_str() == SCROLL_THUMB)
            .map(|(i, _)| i)
            .collect()
    }

    /// A list that fits needs no indicator — the border stays unbroken.
    #[test]
    fn no_scrollbar_when_list_fits() {
        let (mut app, _) = app_with_entries(3);
        let border = right_border(&mut app, 46, 14);
        assert!(
            thumb_rows(&border).is_empty(),
            "expected no thumb, got {border:?}"
        );
    }

    /// The thumb must keep one length for the whole scroll — `ratatui`'s own
    /// Scrollbar rounds start and end separately and visibly jitters by a cell.
    #[test]
    fn thumb_length_is_constant_at_every_offset() {
        for (count, w, h) in [(40usize, 46u16, 14u16), (13, 46, 14), (500, 46, 24), (41, 60, 40)] {
            let (mut app, id) = app_with_entries(count);
            let viewport = (h - 4) as usize; // borders + header + status row
            let max_offset = count - viewport;

            let lengths: Vec<usize> = (0..=max_offset)
                .map(|offset| {
                    let pane = app.pane_map.get_mut(&id).unwrap();
                    pane.focus_index = offset as i32;
                    pane.table_state.select(Some(offset));
                    *pane.table_state.offset_mut() = offset;
                    thumb_rows(&right_border(&mut app, w, h)).len()
                })
                .collect();

            let first = lengths[0];
            assert!(
                lengths.iter().all(|&l| l == first),
                "thumb resized while scrolling {count} items in a {viewport}-row viewport: {lengths:?}"
            );
            assert!(first >= 1);
        }
    }

    /// Flush at the top when at the top, flush at the bottom when at the bottom.
    #[test]
    fn thumb_reaches_both_ends_of_the_track() {
        let (mut app, id) = app_with_entries(40);
        let viewport = 10; // 14 rows - 2 borders - 1 header - 1 status
        let set = |app: &mut App, offset: usize| {
            let pane = app.pane_map.get_mut(&id).unwrap();
            pane.focus_index = offset as i32;
            pane.table_state.select(Some(offset));
            *pane.table_state.offset_mut() = offset;
            thumb_rows(&right_border(app, 46, 14))
        };

        // Row 0 is the top border; list rows start at row 2 and run 10 deep.
        let top = set(&mut app, 0);
        assert_eq!(*top.first().unwrap(), 2, "thumb should start at the first list row");

        let bottom = set(&mut app, 40 - viewport);
        assert_eq!(*bottom.last().unwrap(), 11, "thumb should end at the last list row");
    }

    /// The thumb tracks the scroll offset down the pane.
    #[test]
    fn thumb_follows_scroll_offset() {
        let (mut app, id) = app_with_entries(40);

        let at_offset = |app: &mut App, offset: usize| {
            let pane = app.pane_map.get_mut(&id).unwrap();
            pane.focus_index = offset as i32;
            pane.table_state.select(Some(offset));
            *pane.table_state.offset_mut() = offset;
            thumb_rows(&right_border(app, 46, 14))
        };

        let top = at_offset(&mut app, 0);
        let middle = at_offset(&mut app, 15);
        let bottom = at_offset(&mut app, 30);

        assert!(!top.is_empty() && !middle.is_empty() && !bottom.is_empty());
        assert!(top[0] < middle[0], "thumb should descend: {top:?} {middle:?}");
        assert!(middle[0] < bottom[0], "thumb should descend: {middle:?} {bottom:?}");

        // Never drawn over the pane's corners.
        let border = right_border(&mut app, 46, 14);
        assert_eq!(border[0], "┐", "top corner intact");
        assert_eq!(border[12], "┘", "bottom corner intact");
    }
}

#[cfg(test)]
mod panic_probe {
    use super::*;
    use crate::layout::split_pane;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn no_panic_at_any_size() {
        let mut fails: Vec<(u16, u16)> = Vec::new();
        for w in 1..=60u16 {
            for h in 1..=25u16 {
                let mut app = App::new().unwrap();
                let id = app.active_pane_id.clone();
                let entries: Vec<panex_core::FileEntry> = (0..60)
                    .map(|i| panex_core::FileEntry {
                        name: format!("file-{i:02}.txt"),
                        path: format!("/x/{i}"),
                        is_dir: false,
                        size: 1,
                        modified: 1_700_000_000,
                    })
                    .collect();
                app.pane_map.get_mut(&id).unwrap().entries = entries.clone();
                for (n, dir) in [
                    (1u32, SplitDirection::Vertical),
                    (2, SplitDirection::Horizontal),
                    (3, SplitDirection::Vertical),
                ] {
                    let nid = format!("p{n}");
                    app.layout_root = split_pane(&app.layout_root, &id, &nid, dir);
                    let mut st = crate::app::PaneState::new("/x");
                    st.entries = entries.clone();
                    app.pane_map.insert(nid, st);
                }
                let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    t.draw(|f| draw(f, &mut app)).unwrap();
                }));
                if r.is_err() {
                    fails.push((w, h));
                }
            }
        }
        println!("FAILCOUNT={} sizes={:?}", fails.len(), &fails[..fails.len().min(30)]);
        assert!(fails.is_empty(), "panics at {} terminal sizes", fails.len());
    }
}
