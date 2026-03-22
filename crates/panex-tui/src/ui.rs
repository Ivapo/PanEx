use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::{App, AppMode};
use crate::layout::{LayoutNode, SplitDirection};
use crate::sort::SortField;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Reserve bottom row for status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    render_layout_node(frame, app, &app.layout_root.clone(), chunks[0]);
    render_status_bar(frame, app, chunks[1]);

    // Render dialog overlays
    match &app.mode {
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

    // Build title with path (truncated from left if too long)
    let max_title_len = area.width.saturating_sub(4) as usize;
    let display_path = if current_path.len() > max_title_len {
        format!("…{}", &current_path[current_path.len() - max_title_len + 1..])
    } else {
        current_path.clone()
    };

    let title = if let Some(ref query) = search_mode_query {
        format!(" 🔍 {} ", query)
    } else {
        format!(" {} ", display_path)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 2 {
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
            let icon_color = if entry.is_dir { Color::Rgb(255, 191, 0) } else { Color::DarkGray };

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

    frame.render_stateful_widget(table, area, &mut pane.table_state);
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
        AppMode::Normal => "q:quit │:split W:close Tab:pane /:search",
        AppMode::Search { .. } => "Esc:cancel  Enter:confirm",
        AppMode::Rename { .. } => "Esc:cancel  Enter:rename",
        AppMode::Confirm { .. } => "←→:select  Enter:confirm  y/n  Esc:cancel",
        AppMode::Prompt { .. } => "Esc:cancel  Enter:create",
        AppMode::PathEdit { .. } => "Tab:complete  Bksp:up dir  Enter:go  Esc:cancel",
    };

    if let Some(msg) = &app.status_message {
        let line = Line::from(vec![
            Span::styled(msg.as_str(), Style::default().fg(Color::Yellow)),
            Span::raw("  "),
            Span::styled(mode_hint, Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    } else {
        let line = Line::from(vec![
            Span::styled(left, Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled(mode_hint, Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }
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
    // Simple date formatting: MM/DD/YY
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
    format!("{:02}/{:02}/{:02}", m + 1, d, y % 100)
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
