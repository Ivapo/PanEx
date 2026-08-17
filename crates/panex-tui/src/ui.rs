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
/// `working` in the card view. Named rather than `Color::Cyan` because the
/// distinction that matters is from `ready`'s green, and terminal palettes
/// render their own cyan anywhere from blue to mint.
const TEAL: Color = Color::Rgb(0, 178, 172);
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
            render_help_dialog(frame, area, app.oko_available);
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

    if app.oko_pane_id.as_deref() == Some(pane_id) {
        render_oko_pane(frame, app, pane_id, area, border_style);
        return;
    }

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

/// Boxed card: two borders and two lines of content — the path, and either a
/// status with its age or the foreground job.
const CARD_HEIGHT: u16 = 4;
/// Stacked card: a rule, the name line, and two indented under it.
const CARD_HEIGHT_STACKED: u16 = 4;
/// The stacked separator. Dashed rather than solid so it reads as a gap
/// between cards and not as the edge of a box — the boxes are what one
/// column is trying to get away from.
const CARD_RULE: &str = "┈";
/// Below this a card holds no readable path, so the grid stays one column.
const CARD_MIN_WIDTH: u16 = 22;

/// The card view. What each field means is oko's, in its
/// `rules/follow-stream.md`; how it is laid out is deliberately ours — its
/// spec says nothing about layout.
fn render_oko_pane(
    frame: &mut Frame,
    app: &mut App,
    pane_id: &str,
    area: Rect,
    border_style: Style,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            " Oko ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Registered like any other pane so clicking it makes it active. Its list
    // area is empty, so a click selects no row and a double click opens
    // nothing — there is nothing here to open.
    app.pane_views.insert(
        pane_id.to_string(),
        PaneView { area, list_area: Rect::default() },
    );

    if inner.height < 2 {
        return;
    }

    let header = Rect { height: 1, ..inner };
    frame.render_widget(
        Paragraph::new(Span::styled("Tabs", Style::default().fg(Color::DarkGray))),
        header,
    );
    let body = Rect {
        y: inner.y + 1,
        height: inner.height - 1,
        ..inner
    };

    match &app.oko_view {
        crate::oko::View::Connecting => {
            render_oko_note(frame, body, "connecting to oko…", Color::DarkGray)
        }
        crate::oko::View::Lost(message) => render_oko_note(frame, body, message, Color::Red),
        crate::oko::View::Rows(rows) if rows.is_empty() => {
            render_oko_note(frame, body, "no tabs to show", Color::DarkGray)
        }
        crate::oko::View::Rows(rows) => render_oko_cards(
            frame,
            body,
            rows,
            &app.home_path,
            app.oko_selected.as_deref(),
        ),
    }
}

fn render_oko_note(frame: &mut Frame, area: Rect, message: &str, color: Color) {
    let text = Paragraph::new(format!(" {}", message)).style(Style::default().fg(color));
    frame.render_widget(text, Rect { height: 1, ..area });
}

fn render_oko_cards(
    frame: &mut Frame,
    area: Rect,
    rows: &[crate::oko::Row],
    home: &str,
    selected: Option<&str>,
) {
    let columns = (area.width / CARD_MIN_WIDTH).max(1);
    // In one column the side rules enclose nothing — they only spend two
    // columns of a pane that is already the narrow one. A rule above each
    // card separates them just as well, and buys back a row as well.
    // No scrolling here yet: a window with more tabs than fit simply shows the
    // ones that do, rather than drawing a card over the pane border.
    let fits = |y: u16, h: u16| y + h <= area.y + area.height;

    if columns == 1 {
        let mut y = area.y;
        for (i, row) in rows.iter().enumerate() {
            // The rule goes *between* cards, so the first one does without:
            // the header above it is already the separator it would be.
            let ruled = i > 0;
            let height = if ruled {
                CARD_HEIGHT_STACKED
            } else {
                CARD_HEIGHT_STACKED - 1
            };
            if !fits(y, height) {
                break;
            }
            render_oko_card_stacked(
                frame,
                Rect { y, height, ..area },
                row,
                home,
                selected == Some(row.session_id.as_str()),
                ruled,
            );
            y += height;
        }
        return;
    }

    let card_width = area.width / columns;
    for (i, row) in rows.iter().enumerate() {
        let i = i as u16;
        let y = area.y + (i / columns) * CARD_HEIGHT;
        if !fits(y, CARD_HEIGHT) {
            break;
        }
        render_oko_card_boxed(
            frame,
            Rect {
                x: area.x + (i % columns) * card_width,
                y,
                width: card_width,
                height: CARD_HEIGHT,
            },
            row,
            home,
            selected == Some(row.session_id.as_str()),
        );
    }
}

/// Three lines and no rule: the name with its tab, then the directory and
/// what is happening in it, indented beneath. The indent is what groups them,
/// so a separator would only spend a row saying the same thing.
fn render_oko_card_stacked(
    frame: &mut Frame,
    area: Rect,
    row: &crate::oko::Row,
    home: &str,
    selected: bool,
    ruled: bool,
) {
    if area.width < 4 {
        return;
    }
    // Above the name rather than around the card: it separates this one from
    // the one before it, which is all the boxes were doing here.
    let area = if ruled {
        frame.render_widget(
            Paragraph::new(Span::styled(
                CARD_RULE.repeat(area.width as usize),
                Style::default().fg(Color::DarkGray),
            )),
            Rect { height: 1, ..area },
        );
        if area.height < 2 {
            return;
        }
        Rect {
            y: area.y + 1,
            height: area.height - 1,
            ..area
        }
    } else {
        area
    };

    // The name line carries the selection the way the file list marks the row
    // under the cursor — same colours, so moving through cards and moving
    // through files are visibly the same gesture.
    let (fg, dim) = if selected {
        (Color::White, Color::Gray)
    } else {
        (Color::Reset, Color::DarkGray)
    };
    let mut line_style = Style::default().fg(fg);
    if selected {
        line_style = line_style.bg(Color::DarkGray);
    }

    let tab = format!("⌘ {}", row.tab);
    let width = area.width as usize;
    let name_room = width.saturating_sub(tab.chars().count() + 3);
    let name = truncate_right(row.name.as_deref().unwrap_or("—"), name_room);
    let gap = width.saturating_sub(name.chars().count() + tab.chars().count() + 2);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(name, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" ".repeat(gap)),
            Span::styled(tab, Style::default().fg(dim)),
            Span::raw(" "),
        ]))
        .style(line_style),
        Rect { height: 1, ..area },
    );

    // The two detail lines are indented under the name rather than aligned
    // with it, which is the whole separator this layout has.
    let indent = Rect {
        x: area.x + 3,
        width: area.width.saturating_sub(3),
        y: area.y + 1,
        height: 1,
    };
    if area.height < 2 {
        return;
    }
    let inner_width = indent.width as usize;
    let path = row.path.as_deref().map(|p| abbreviate(p, home)).unwrap_or_default();
    frame.render_widget(
        Paragraph::new(Span::styled(
            truncate_left(&path, inner_width),
            Style::default().fg(Color::DarkGray),
        )),
        indent,
    );

    if area.height < 3 {
        return;
    }
    frame.render_widget(
        Paragraph::new(activity_line(row, inner_width, Color::DarkGray)),
        Rect { y: indent.y + 1, ..indent },
    );
}

fn render_oko_card_boxed(
    frame: &mut Frame,
    area: Rect,
    row: &crate::oko::Row,
    home: &str,
    selected: bool,
) {
    let name = row.name.as_deref().unwrap_or("—");
    // Side by side there is no full-width line to highlight without it
    // reading as a filled box, so the selection lands on the border instead.
    let (border, title) = if selected {
        (
            Style::default().fg(ACCENT),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )
    } else {
        (
            Style::default().fg(Color::DarkGray),
            Style::default().fg(Color::White),
        )
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border)
        .title(Span::styled(format!(" {} {} ", row.tab, name), title));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let width = inner.width as usize;
    let path = row.path.as_deref().map(|p| abbreviate(p, home)).unwrap_or_default();
    frame.render_widget(
        Paragraph::new(Span::styled(
            truncate_left(&path, width),
            Style::default().fg(Color::DarkGray),
        )),
        Rect { height: 1, ..inner },
    );

    if inner.height < 2 {
        return;
    }
    frame.render_widget(
        Paragraph::new(activity_line(row, width, Color::DarkGray)),
        Rect { y: inner.y + 1, height: 1, ..inner },
    );
}

/// What is happening in that directory, and how long it has said so.
///
/// A row carries a status or a job and never both: a status means a Claude
/// tab, and oko withholds `jobName` there because it was measured unstable
/// within one session.
fn activity_line(row: &crate::oko::Row, width: usize, dim: Color) -> Line<'static> {
    let (text, color) = match (row.status.as_deref(), row.job.as_deref()) {
        (Some(status), _) => {
            let (glyph, color) = status_style(status);
            (format!("{} {}", glyph, status), color)
        }
        (None, Some(job)) => (truncate_left(job, width), dim),
        (None, None) => (String::new(), dim),
    };
    let age = row.age.clone().unwrap_or_default();
    let gap = width.saturating_sub(text.chars().count() + age.chars().count());
    Line::from(vec![
        Span::styled(text, Style::default().fg(color)),
        Span::raw(" ".repeat(gap)),
        Span::styled(age, Style::default().fg(dim)),
    ])
}

fn status_style(status: &str) -> (&'static str, Color) {
    match status {
        "ready" => ("●", Color::Green),
        "working" => ("◐", TEAL),
        "waiting" => ("▲", ACCENT),
        "stale" => ("○", Color::DarkGray),
        // A status this build does not know is still worth showing as text —
        // the schema check upstream is what guards against drawing nonsense.
        _ => ("·", Color::DarkGray),
    }
}

/// oko publishes paths unabbreviated on purpose — `~` is a decoration, and
/// which one to use is the drawing program's business.
fn abbreviate(path: &str, home: &str) -> String {
    match path.strip_prefix(home) {
        Some("") if !home.is_empty() => "~".to_string(),
        Some(rest) if rest.starts_with('/') => format!("~{}", rest),
        _ => path.to_string(),
    }
}

/// Names read from the start, so a long one loses its tail.
fn truncate_right(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max || max == 0 {
        return s.to_string();
    }
    let keep = max.saturating_sub(1);
    format!("{}…", chars[..keep].iter().collect::<String>())
}

/// Paths and job names are recognised by their tails, so drop from the left.
fn truncate_left(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max || max == 0 {
        return s.to_string();
    }
    let keep = max.saturating_sub(1);
    format!("…{}", chars[chars.len() - keep..].iter().collect::<String>())
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
    // The card view has no items, no sort and no hidden files. Reporting on a
    // file list that isn't there would describe the wrong pane.
    let left = if app.oko_pane_id.as_deref() == Some(app.active_pane_id.as_str()) {
        match &app.oko_view {
            crate::oko::View::Rows(rows) => format!(" {} tabs", rows.len()),
            crate::oko::View::Connecting => " connecting".to_string(),
            crate::oko::View::Lost(_) => " no stream".to_string(),
        }
    } else {
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

        parts.join(" │ ")
    };

    let cards_active = app.oko_pane_id.as_deref() == Some(app.active_pane_id.as_str());
    let mode_hint = match &app.mode {
        AppMode::Normal if cards_active => "↑↓/jk:select  ↵:jump  r:rename  O:close  Tab:panes",
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

fn render_help_dialog(frame: &mut Frame, area: Rect, oko_available: bool) {
    // Listed only when there is a usable oko to open. An entry for a key that
    // is not bound is worse than no entry at all.
    let mut panes: Vec<(&str, &str)> = vec![
        ("|", "split vertical"),
        ("_", "split horizontal"),
        ("+ / =", "grow pane 25%"),
        ("-", "shrink pane 25%"),
        ("W", "close pane"),
    ];
    if oko_available {
        panes.push(("O", "tab cards (oko)"));
    }

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
        ("Panes", &panes),
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
                ("double-click", "open (same as Enter)"),
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

#[cfg(test)]
mod oko_tests {
    use super::*;
    use crate::app::App;
    use crate::oko::{Row, View};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn screen(app: &mut App, width: u16, height: u16) -> String {
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

    fn row(tab: u32, name: &str, path: &str, status: Option<&str>, age: Option<&str>, job: Option<&str>) -> Row {
        Row {
            session_id: format!("session-{tab}"),
            tab,
            name: Some(name.to_string()),
            path: Some(path.to_string()),
            status: status.map(String::from),
            age: age.map(String::from),
            job: job.map(String::from),
        }
    }

    fn showing(view: View) -> App {
        let mut app = App::new().unwrap();
        app.oko_pane_id = Some(app.active_pane_id.clone());
        app.oko_view = view;
        app
    }

    fn four_tabs() -> View {
        View::Rows(vec![
            row(1, "ivapo", "/Users/me", None, None, Some("panex")),
            row(2, "oko", "/Users/me/dev/main/oko", Some("ready"), Some(">30m"), None),
            row(3, "tikray", "/Users/me/dev/main/tikray", Some("working"), None, None),
            row(4, "PanEx", "/Users/me/dev/main/PanEx", Some("waiting"), Some(">5m"), None),
        ])
    }

    #[test]
    fn a_card_carries_tab_name_path_status_and_age() {
        let mut app = showing(four_tabs());
        let s = screen(&mut app, 60, 14);
        for expected in ["2 oko", "/Users/me/dev/main/oko", "ready", ">30m"] {
            assert!(s.contains(expected), "missing {expected:?} in:\n{s}");
        }
    }

    /// The pane is Oko's, and the thing it lists is tabs. Both are named.
    #[test]
    fn the_pane_says_what_it_is() {
        let mut app = showing(four_tabs());
        let s = screen(&mut app, 60, 14);
        assert!(s.contains("Oko"), "pane title missing in:\n{s}");
        assert!(s.contains("Tabs"), "content header missing in:\n{s}");
    }

    /// A row with a status is a Claude tab and publishes no job; a row without
    /// one shows its foreground process instead.
    #[test]
    fn a_plain_tab_shows_its_job_where_a_status_would_go() {
        let mut app = showing(four_tabs());
        let s = screen(&mut app, 60, 14);
        assert!(s.contains("panex"), "job missing in:\n{s}");
    }

    /// oko publishes paths unabbreviated on purpose — the `~` is ours to draw.
    #[test]
    fn home_is_abbreviated_when_drawing() {
        let home = crate::app::App::new().unwrap().home_path;
        let mut app = showing(View::Rows(vec![row(
            1,
            "here",
            &format!("{home}/dev/main/oko"),
            Some("working"),
            None,
            None,
        )]));
        let s = screen(&mut app, 60, 10);
        assert!(s.contains("~/dev/main/oko"), "not abbreviated in:\n{s}");
    }

    /// A card that cannot be drawn whole would spill over the pane border.
    #[test]
    fn cards_that_do_not_fit_are_left_out_rather_than_clipped() {
        let mut app = showing(four_tabs());
        let s = screen(&mut app, 30, 8);
        for line in s.lines() {
            assert_eq!(
                line.chars().count(),
                30,
                "row is not the pane width in:\n{s}"
            );
        }
    }

    /// The card view has no items, no sort and no hidden files to report.
    #[test]
    fn the_status_bar_describes_tabs_not_a_file_list() {
        let mut app = showing(four_tabs());
        let s = screen(&mut app, 60, 14);
        let footer = s.lines().last().unwrap();
        assert!(footer.contains("4 tabs"), "got: {footer:?}");
        assert!(!footer.contains("Sort:"), "got: {footer:?}");
    }

    /// Whether any cell on the line containing `needle` is drawn in the accent.
    fn accented_line(app: &mut App, needle: &str) -> bool {
        let mut terminal = Terminal::new(TestBackend::new(60, 14)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        for y in 0..buffer.area.height {
            let line: String = (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect();
            if line.contains(needle) {
                return (0..buffer.area.width).any(|x| buffer[(x, y)].fg == ACCENT);
            }
        }
        panic!("no line containing {needle:?}");
    }

    /// The selected card is what Enter jumps to, so it has to be visibly the
    /// one — and the unselected ones have to be visibly not.
    #[test]
    fn the_selected_card_is_marked_and_the_others_are_not() {
        let mut app = showing(four_tabs());
        app.oko_selected = Some("session-3".to_string());

        assert!(accented_line(&mut app, "3 tikray"), "selected card unmarked");
        assert!(!accented_line(&mut app, "2 oko"), "unselected card marked");
    }

    #[test]
    fn a_lost_stream_says_so_instead_of_drawing_cards() {
        let mut app = showing(View::Lost("oko: the API is off".to_string()));
        let s = screen(&mut app, 60, 10);
        assert!(s.contains("the API is off"), "in:\n{s}");
    }
}

#[cfg(test)]
mod oko_layout_tests {
    use super::*;
    use crate::app::App;
    use crate::oko::{Row, View};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn rows() -> Vec<Row> {
        [
            (1u32, "ivapo", "/Users/me", None, None, Some("panex")),
            (2, "oko", "/Users/me/dev/oko", Some("ready"), Some(">30m"), None),
            (3, "tikray", "/Users/me/dev/tikray", Some("working"), None, None),
        ]
        .into_iter()
        .map(|(tab, name, path, status, age, job)| Row {
            session_id: format!("session-{tab}"),
            tab,
            name: Some(name.to_string()),
            path: Some(path.to_string()),
            status: status.map(String::from),
            age: age.map(String::from),
            job: job.map(String::from),
        })
        .collect()
    }

    fn screen(width: u16, height: u16) -> String {
        let mut app = App::new().unwrap();
        app.oko_pane_id = Some(app.active_pane_id.clone());
        app.oko_view = View::Rows(rows());
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame, &mut app)).unwrap();
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

    /// The interior of a rendered pane, with its own borders stripped.
    fn interior(screen: &str) -> Vec<String> {
        screen
            .lines()
            .skip(1)
            .take_while(|l| !l.starts_with('└'))
            .map(|l| {
                let chars: Vec<char> = l.chars().collect();
                chars[1..chars.len() - 1].iter().collect()
            })
            .collect()
    }

    /// One column of cards has nothing to enclose, so the side rules go and
    /// the row they cost comes back.
    /// The screen, plus which lines carry a highlight background.
    fn screen_with_highlights(app: &mut App, w: u16, h: u16) -> Vec<(String, bool)> {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area.height)
            .map(|y| {
                let text = (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>();
                let lit = (0..buffer.area.width).any(|x| buffer[(x, y)].bg == Color::DarkGray);
                (text, lit)
            })
            .collect()
    }

    fn stacked(selected: &str) -> Vec<(String, bool)> {
        let mut app = App::new().unwrap();
        app.oko_pane_id = Some(app.active_pane_id.clone());
        app.oko_view = View::Rows(rows());
        app.oko_selected = Some(selected.to_string());
        screen_with_highlights(&mut app, 34, 16)
    }

    /// A separator separates: it goes between cards, and the header above the
    /// first is already doing that job for it.
    #[test]
    fn the_rule_goes_between_cards_and_not_above_the_first() {
        let lines: Vec<String> = stacked("session-1").into_iter().map(|(t, _)| t).collect();
        let ruled: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.contains("┈"))
            .map(|(i, _)| i)
            .collect();
        let first_card = lines.iter().position(|l| l.contains("ivapo")).unwrap();

        assert!(
            !ruled.contains(&(first_card - 1)),
            "rule above the first card:\n{}",
            lines.join("\n")
        );
        let second_card = lines.iter().position(|l| l.contains("oko")).unwrap();
        assert!(
            ruled.contains(&(second_card - 1)),
            "no rule before the second card:\n{}",
            lines.join("\n")
        );
    }

    /// Name and tab on one line, directory and activity indented under it.
    #[test]
    fn a_stacked_card_is_a_name_line_and_two_indented_under_it() {
        let lines: Vec<String> = stacked("session-2").into_iter().map(|(t, _)| t).collect();
        let at = lines.iter().position(|l| l.contains("oko")).unwrap();
        assert!(lines[at].contains("⌘ 2"), "tab not on the name line: {:?}", lines[at]);
        assert!(lines[at + 1].starts_with("│   /"), "path not indented: {:?}", lines[at + 1]);
        assert!(lines[at + 2].starts_with("│   ●"), "activity not indented: {:?}", lines[at + 2]);
    }

    /// The highlight marks the name line and only the name line, the same as
    /// the row under the cursor in a file pane.
    #[test]
    fn the_highlight_lands_on_the_name_line() {
        let lines = stacked("session-2");
        let lit: Vec<&String> = lines.iter().filter(|(_, l)| *l).map(|(t, _)| t).collect();
        assert_eq!(lit.len(), 1, "expected one highlighted line, got {lit:?}");
        assert!(lit[0].contains("oko") && lit[0].contains("⌘ 2"), "wrong line: {:?}", lit[0]);
    }

    #[test]
    fn the_highlight_moves_with_the_selection() {
        let lit = |sel| {
            stacked(sel)
                .into_iter()
                .find(|(_, l)| *l)
                .map(|(t, _)| t)
                .unwrap()
        };
        assert!(lit("session-1").contains("ivapo"));
        assert!(lit("session-3").contains("tikray"));
    }

    #[test]
    fn a_single_column_draws_no_card_sides() {
        // Tall enough for three stacked cards and the two rules between them.
        let s = screen(34, 16);
        for line in interior(&s) {
            assert!(
                !line.contains('│'),
                "vertical rule inside a one-column view:\n{s}"
            );
        }
        // Boxed cards are 4 rows, stacked ones 3, so all three fit here.
        assert!(s.contains("tikray"), "third card should fit:\n{s}");
    }

    /// Side by side, the box is what tells one card from its neighbour.
    #[test]
    fn two_columns_keep_the_boxes() {
        let s = screen(62, 14);
        assert!(
            interior(&s).iter().any(|l| l.contains('│')),
            "cards should be boxed when they sit side by side:\n{s}"
        );
        assert!(s.contains("1 ivapo") && s.contains("2 oko"));
    }

    /// Whichever style, no card may spill past the pane it lives in.
    #[test]
    fn cards_stay_inside_the_pane() {
        for (w, h) in [(34, 14), (62, 14), (24, 8), (80, 6)] {
            let s = screen(w, h);
            for line in s.lines() {
                assert_eq!(line.chars().count(), w as usize, "at {w}x{h}:\n{s}");
            }
        }
    }
}
