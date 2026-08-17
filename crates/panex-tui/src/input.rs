use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::{App, AppMode, ClipMode, ConfirmAction, FileClipboard, PromptAction};
use crate::layout::{self, SplitDirection, collect_leaf_ids, count_leaves};
use crate::sort::apply_sort_and_filter;

/// How long after a click a second one on the same row still counts as a
/// double click. Matches the macOS default; slower than this reads as two
/// deliberate clicks, and a much longer window makes stray pairs open files.
const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(400);

pub fn handle_key_event(app: &mut App, key: KeyEvent) {
    // Clear status message on any keypress
    app.status_message = None;
    app.status_message_at = None;

    match &app.mode {
        AppMode::Normal => handle_normal(app, key),
        AppMode::Help => handle_help(app, key),
        AppMode::Search { .. } => handle_search(app, key),
        AppMode::Rename { .. } => handle_rename(app, key),
        AppMode::Confirm { .. } => handle_confirm(app, key),
        AppMode::Prompt { .. } => handle_prompt(app, key),
        AppMode::PathEdit { .. } => handle_path_edit(app, key),
        AppMode::FavoritesList { .. } => handle_favorites_list(app, key),
    }
}

fn handle_normal(app: &mut App, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    match key.code {
        // Quit
        KeyCode::Char('q') if !ctrl => {
            app.should_quit = true;
        }
        KeyCode::Char('c') if ctrl => {
            // Ctrl+C = copy to clipboard (not quit)
            copy_to_clipboard(app, ClipMode::Copy);
        }

        // Navigation
        KeyCode::Up | KeyCode::Char('k') if shift => {
            move_focus(app, -1);
            toggle_selection_at_focus(app);
        }
        KeyCode::Down | KeyCode::Char('j') if shift => {
            move_focus(app, 1);
            toggle_selection_at_focus(app);
        }
        KeyCode::Up | KeyCode::Char('k') => move_focus(app, -1),
        KeyCode::Down | KeyCode::Char('j') => move_focus(app, 1),
        KeyCode::PageUp => page_move(app, -1),
        KeyCode::PageDown => page_move(app, 1),
        KeyCode::Char('g') => focus_to(app, 0),
        KeyCode::Char('G') => focus_to(app, i32::MAX),
        KeyCode::Enter => open_focused(app),
        KeyCode::Backspace => navigate_up(app),
        KeyCode::Home | KeyCode::Char('~') => {
            let home = app.home_path.clone();
            let pane_id = app.active_pane_id.clone();
            app.navigate_to(&pane_id, &home);
        }

        // Refresh active pane
        KeyCode::F(5) => {
            let pane_id = app.active_pane_id.clone();
            app.refresh_pane(&pane_id);
        }

        // Pane management
        KeyCode::Char('|') => split_active_pane(app, SplitDirection::Vertical),
        KeyCode::Char('_') => split_active_pane(app, SplitDirection::Horizontal),
        KeyCode::Char('W') => close_active_pane(app),
        KeyCode::Tab => cycle_pane(app),

        // Pane size — '=' is the unshifted twin of '+'
        KeyCode::Char('+') | KeyCode::Char('=') if !ctrl => resize_active_pane(app, 1),
        KeyCode::Char('-') if !ctrl => resize_active_pane(app, -1),

        // File operations
        KeyCode::Char('y') => copy_to_clipboard(app, ClipMode::Copy),
        KeyCode::Char('x') => copy_to_clipboard(app, ClipMode::Cut),
        KeyCode::Char('p') => paste_clipboard(app),
        KeyCode::Char('v') if ctrl => paste_clipboard(app),
        KeyCode::Char('r') | KeyCode::F(2) => start_rename(app),
        KeyCode::Char('d') | KeyCode::Delete => start_delete(app),
        KeyCode::Char('o') => open_in_default_app(app),
        KeyCode::Char('t') => open_in_terminal(app),
        KeyCode::Char('n') => start_new_file(app),
        KeyCode::Char('N') => start_new_folder(app),
        KeyCode::Char('a') if ctrl => select_all(app),
        KeyCode::Esc => deselect_all(app),

        // Help overlay
        KeyCode::Char('?') => {
            app.mode = AppMode::Help;
        }

        // Search
        KeyCode::Char('/') => {
            let pane_id = app.active_pane_id.clone();
            app.mode = AppMode::Search { pane_id };
        }
        KeyCode::Char('f') if ctrl => {
            let pane_id = app.active_pane_id.clone();
            app.mode = AppMode::Search { pane_id };
        }

        // Sort
        KeyCode::Char('s') if !shift => {
            app.sort_field = app.sort_field.cycle();
            refilter_all_panes(app);
        }
        KeyCode::Char('S') => {
            app.sort_direction = app.sort_direction.toggle();
            refilter_all_panes(app);
        }

        // Hidden files
        KeyCode::Char('.') => {
            app.show_hidden = !app.show_hidden;
            refilter_all_panes(app);
        }

        // Path edit — show favorites list first if any exist
        KeyCode::Char('e') => {
            let pane_id = app.active_pane_id.clone();
            if !app.config.favorites.paths.is_empty() {
                app.mode = AppMode::FavoritesList {
                    pane_id,
                    selected: 0,
                };
            } else {
                let path = app
                    .pane_map
                    .get(&pane_id)
                    .map(|p| p.current_path.clone())
                    .unwrap_or_default();
                app.mode = AppMode::PathEdit {
                    pane_id,
                    input: path.clone(),
                    cursor: path.len(),
                    completions: Vec::new(),
                    completion_index: None,
                    completion_prefix: String::new(),
                };
            }
        }

        // Toggle current path as favorite
        KeyCode::Char('f') if !ctrl => {
            let current_path = app
                .pane_map
                .get(&app.active_pane_id)
                .map(|p| p.current_path.clone())
                .unwrap_or_default();
            match app.config.toggle_favorite(&current_path) {
                Ok(true) => app.set_status("★ Added to favorites".to_string()),
                Ok(false) => app.set_status("☆ Removed from favorites".to_string()),
                Err(e) => app.set_status(format!("Favorite error: {}", e)),
            }
        }

        _ => {}
    }
}

fn handle_help(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('?') => {
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
}

/// Returns true if the event changed anything visible (caller redraws only then,
/// so hover/move events don't trigger redraw storms).
pub fn handle_mouse_event(app: &mut App, mouse: MouseEvent) -> bool {
    let scroll_ok = matches!(app.mode, AppMode::Normal | AppMode::Search { .. });
    match mouse.kind {
        MouseEventKind::ScrollUp if scroll_ok => {
            scroll_pane_at(app, mouse.column, mouse.row, -3)
        }
        MouseEventKind::ScrollDown if scroll_ok => {
            scroll_pane_at(app, mouse.column, mouse.row, 3)
        }
        MouseEventKind::Down(MouseButton::Left) if app.mode == AppMode::Normal => {
            click_at(app, mouse.column, mouse.row)
        }
        _ => false,
    }
}

fn pane_at(app: &App, x: u16, y: u16) -> Option<String> {
    app.pane_views.iter().find_map(|(id, view)| {
        let a = view.area;
        if x >= a.x && x < a.x + a.width && y >= a.y && y < a.y + a.height {
            Some(id.clone())
        } else {
            None
        }
    })
}

/// Scroll the viewport of the pane under the cursor. Focus only moves when it
/// would otherwise leave the visible window (so ratatui doesn't snap the
/// offset back to the selected row on the next render).
fn scroll_pane_at(app: &mut App, x: u16, y: u16, delta: i32) -> bool {
    let pane_id = match pane_at(app, x, y) {
        Some(id) => id,
        None => return false,
    };
    let view_height = app
        .pane_views
        .get(&pane_id)
        .map(|v| v.list_area.height as usize)
        .unwrap_or(0);
    let pane = match app.pane_map.get_mut(&pane_id) {
        Some(p) => p,
        None => return false,
    };

    let len = pane.entries.len();
    if len == 0 || view_height == 0 {
        return false;
    }

    let max_offset = len.saturating_sub(view_height) as i32;
    let old_offset = pane.table_state.offset();
    let new_offset = (old_offset as i32 + delta).clamp(0, max_offset) as usize;
    *pane.table_state.offset_mut() = new_offset;

    let mut focus_moved = false;
    if pane.focus_index >= 0 {
        let lo = new_offset as i32;
        let hi = (new_offset + view_height - 1).min(len - 1) as i32;
        let clamped = pane.focus_index.clamp(lo, hi);
        if clamped != pane.focus_index {
            pane.focus_index = clamped;
            pane.table_state.select(Some(clamped as usize));
            focus_moved = true;
        }
    }

    new_offset != old_offset || focus_moved
}

fn click_at(app: &mut App, x: u16, y: u16) -> bool {
    let pane_id = match pane_at(app, x, y) {
        Some(id) => id,
        None => return false,
    };
    let mut changed = false;
    if app.active_pane_id != pane_id {
        app.active_pane_id = pane_id.clone();
        changed = true;
    }

    let list = match app.pane_views.get(&pane_id) {
        Some(v) => v.list_area,
        None => return changed,
    };
    if y < list.y || y >= list.y + list.height {
        return changed;
    }

    let idx = match app.pane_map.get_mut(&pane_id) {
        Some(pane) => {
            let idx = pane.table_state.offset() + (y - list.y) as usize;
            if idx >= pane.entries.len() {
                return changed;
            }
            if pane.focus_index != idx as i32 {
                pane.focus_index = idx as i32;
                pane.table_state.select(Some(idx));
                changed = true;
            }
            idx
        }
        None => return changed,
    };

    // A second click on the same row within the window opens it, exactly as
    // Enter does. Acting on that second click rather than waiting the window
    // out keeps the single click instant — it only moves focus, so there is
    // nothing to take back if the pair never completes.
    let now = Instant::now();
    if completes_double_click(app.last_click.as_ref(), &pane_id, idx, now) {
        // Forget the pair before opening: the row under the cursor now belongs
        // to a different directory, and a third click should start afresh
        // rather than open whatever has scrolled into that position.
        app.last_click = None;
        open_focused(app);
        return true;
    }

    app.last_click = Some((pane_id, idx, now));
    changed
}

/// Whether this click is the second half of a double click. All three have to
/// hold: same pane, same row, inside the window. Two clicks on neighbouring
/// rows are two clicks, however fast — the user re-aimed between them.
fn completes_double_click(
    last: Option<&(String, usize, Instant)>,
    pane_id: &str,
    idx: usize,
    now: Instant,
) -> bool {
    match last {
        Some((last_pane, last_idx, at)) => {
            last_pane == pane_id
                && *last_idx == idx
                && now.duration_since(*at) < DOUBLE_CLICK_WINDOW
        }
        None => false,
    }
}

fn handle_search(app: &mut App, key: KeyEvent) {
    let pane_id = if let AppMode::Search { pane_id } = &app.mode {
        pane_id.clone()
    } else {
        return;
    };

    match key.code {
        KeyCode::Esc => {
            if let Some(pane) = app.pane_map.get_mut(&pane_id) {
                pane.search_query.clear();
            }
            app.refilter_pane(&pane_id);
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter | KeyCode::Down => {
            // Exit search, focus first result
            app.mode = AppMode::Normal;
        }
        KeyCode::Backspace => {
            if let Some(pane) = app.pane_map.get_mut(&pane_id) {
                pane.search_query.pop();
            }
            app.refilter_pane(&pane_id);
        }
        KeyCode::Char(c) => {
            if let Some(pane) = app.pane_map.get_mut(&pane_id) {
                pane.search_query.push(c);
            }
            app.refilter_pane(&pane_id);
        }
        _ => {}
    }
}

fn handle_rename(app: &mut App, key: KeyEvent) {
    let (pane_id, path, mut input, mut cursor) =
        if let AppMode::Rename {
            pane_id,
            path,
            input,
            cursor,
        } = &app.mode
        {
            (pane_id.clone(), path.clone(), input.clone(), *cursor)
        } else {
            return;
        };

    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            if !input.is_empty() {
                match panex_core::rename_entry(&path, &input) {
                    Ok(()) => {
                        app.set_status(format!("Renamed to {}", input));
                        app.refresh_pane(&pane_id);
                    }
                    Err(e) => {
                        app.set_status(format!("Rename failed: {}", e));
                    }
                }
            }
            app.mode = AppMode::Normal;
        }
        KeyCode::Backspace => {
            if cursor > 0 {
                input.remove(cursor - 1);
                cursor -= 1;
            }
            app.mode = AppMode::Rename {
                pane_id,
                path,
                input,
                cursor,
            };
        }
        KeyCode::Left => {
            cursor = cursor.saturating_sub(1);
            app.mode = AppMode::Rename {
                pane_id,
                path,
                input,
                cursor,
            };
        }
        KeyCode::Right => {
            if cursor < input.len() {
                cursor += 1;
            }
            app.mode = AppMode::Rename {
                pane_id,
                path,
                input,
                cursor,
            };
        }
        KeyCode::Char(c) => {
            input.insert(cursor, c);
            cursor += 1;
            app.mode = AppMode::Rename {
                pane_id,
                path,
                input,
                cursor,
            };
        }
        _ => {}
    }
}

fn confirm_execute(app: &mut App, action: ConfirmAction) {
    match action {
        ConfirmAction::Delete(paths) => {
            let mut errors = Vec::new();
            for p in &paths {
                if let Err(e) = panex_core::delete_entry(p, false) {
                    errors.push(e);
                }
            }
            if errors.is_empty() {
                app.set_status(format!("Deleted {} item(s)", paths.len()));
            } else {
                app.set_status(format!("Delete errors: {}", errors.join(", ")));
            }
            // Refresh all panes
            let pane_ids: Vec<String> = app.pane_map.keys().cloned().collect();
            for pid in pane_ids {
                app.refresh_pane(&pid);
            }
        }
    }
}

fn handle_confirm(app: &mut App, key: KeyEvent) {
    let (action, selected) = if let AppMode::Confirm { action, selected, .. } = &app.mode {
        let a = match action {
            ConfirmAction::Delete(paths) => ConfirmAction::Delete(paths.clone()),
        };
        (a, *selected)
    } else {
        return;
    };

    match key.code {
        KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l') => {
            // Toggle between Yes (0) and No (1)
            if let AppMode::Confirm { selected, .. } = &mut app.mode {
                *selected = if *selected == 0 { 1 } else { 0 };
            }
        }
        KeyCode::Char('y') => {
            confirm_execute(app, action);
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            if selected == 0 {
                confirm_execute(app, action);
            }
            app.mode = AppMode::Normal;
        }
        KeyCode::Char('n') | KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        _ => {}
    }
}

fn handle_prompt(app: &mut App, key: KeyEvent) {
    let (title, mut input, mut cursor, action) =
        if let AppMode::Prompt {
            title,
            input,
            cursor,
            action,
        } = &app.mode
        {
            (title.clone(), input.clone(), *cursor, match action {
                PromptAction::NewFile(dir) => PromptAction::NewFile(dir.clone()),
                PromptAction::NewFolder(dir) => PromptAction::NewFolder(dir.clone()),
            })
        } else {
            return;
        };

    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            if !input.is_empty() {
                let result = match &action {
                    PromptAction::NewFile(dir) => panex_core::create_file(dir, &input),
                    PromptAction::NewFolder(dir) => panex_core::create_folder(dir, &input),
                };
                match result {
                    Ok(()) => {
                        app.set_status(format!("Created {}", input));
                        let pane_id = app.active_pane_id.clone();
                        app.refresh_pane(&pane_id);
                    }
                    Err(e) => {
                        app.set_status(format!("Create failed: {}", e));
                    }
                }
            }
            app.mode = AppMode::Normal;
        }
        KeyCode::Backspace => {
            if cursor > 0 {
                input.remove(cursor - 1);
                cursor -= 1;
            }
            app.mode = AppMode::Prompt {
                title,
                input,
                cursor,
                action,
            };
        }
        KeyCode::Left => {
            cursor = cursor.saturating_sub(1);
            app.mode = AppMode::Prompt {
                title,
                input,
                cursor,
                action,
            };
        }
        KeyCode::Right => {
            if cursor < input.len() {
                cursor += 1;
            }
            app.mode = AppMode::Prompt {
                title,
                input,
                cursor,
                action,
            };
        }
        KeyCode::Char(c) => {
            input.insert(cursor, c);
            cursor += 1;
            app.mode = AppMode::Prompt {
                title,
                input,
                cursor,
                action,
            };
        }
        _ => {}
    }
}

fn path_edit_set(app: &mut App, pane_id: String, input: String, cursor: usize) {
    app.mode = AppMode::PathEdit {
        pane_id,
        input,
        cursor,
        completions: Vec::new(),
        completion_index: None,
        completion_prefix: String::new(),
    };
}

fn compute_completions(input: &str, home_path: &str) -> (String, Vec<String>) {
    let expanded = if input.starts_with('~') {
        input.replacen('~', home_path, 1)
    } else {
        input.to_string()
    };
    let path = std::path::Path::new(&expanded);

    // Split into parent dir and the prefix being typed
    let (dir, prefix) = if expanded.ends_with('/') {
        (expanded.as_str().to_string(), String::new())
    } else {
        let parent = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let file_part = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        (parent, file_part)
    };

    let prefix_lower = prefix.to_lowercase();
    let mut matches = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.to_lowercase().starts_with(&prefix_lower) {
                let full = if dir.ends_with('/') {
                    format!("{}{}", dir, name)
                } else {
                    format!("{}/{}", dir, name)
                };
                // Add trailing slash for directories
                let full = if entry.path().is_dir() {
                    format!("{}/", full)
                } else {
                    full
                };
                // Convert back to ~ if original used it
                let full = if input.starts_with('~') {
                    full.replacen(home_path, "~", 1)
                } else {
                    full
                };
                matches.push(full);
            }
        }
    }
    matches.sort_by_key(|a| a.to_lowercase());
    (prefix, matches)
}

fn handle_path_edit(app: &mut App, key: KeyEvent) {
    let (pane_id, mut input, mut cursor, completions, completion_index, completion_prefix) =
        if let AppMode::PathEdit {
            pane_id,
            input,
            cursor,
            completions,
            completion_index,
            completion_prefix,
        } = &app.mode
        {
            (pane_id.clone(), input.clone(), *cursor, completions.clone(), *completion_index, completion_prefix.clone())
        } else {
            return;
        };

    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            let target = if input.starts_with('~') {
                input.replacen('~', &app.home_path, 1)
            } else {
                input.clone()
            };
            app.navigate_to(&pane_id, &target);
            app.mode = AppMode::Normal;
        }
        KeyCode::Backspace => {
            // If cursor is at end, remove last path segment
            if cursor == input.len() && input.len() > 1 {
                // Strip trailing slash if present
                let trimmed = if input.ends_with('/') {
                    &input[..input.len() - 1]
                } else {
                    &input
                };
                // Find the last slash and truncate after it
                if let Some(pos) = trimmed.rfind('/') {
                    input.truncate(pos + 1);
                    cursor = input.len();
                }
            } else if cursor > 0 {
                input.remove(cursor - 1);
                cursor -= 1;
            }
            path_edit_set(app, pane_id, input, cursor);
        }
        KeyCode::Tab | KeyCode::BackTab => {
            let backward = shift || key.code == KeyCode::BackTab;

            if completions.is_empty() || completion_index.is_none() {
                // First Tab press: compute completions
                let (prefix, matches) = compute_completions(&input, &app.home_path);
                if matches.is_empty() {
                    return;
                }
                let idx = 0;
                let new_input = matches[idx].clone();
                let new_cursor = new_input.len();
                app.mode = AppMode::PathEdit {
                    pane_id,
                    input: new_input,
                    cursor: new_cursor,
                    completions: matches,
                    completion_index: Some(idx),
                    completion_prefix: prefix,
                };
            } else {
                // Cycle through existing completions
                let len = completions.len();
                let cur = completion_index.unwrap_or(0);
                let next = if backward {
                    if cur == 0 { len - 1 } else { cur - 1 }
                } else {
                    (cur + 1) % len
                };
                let new_input = completions[next].clone();
                let new_cursor = new_input.len();
                app.mode = AppMode::PathEdit {
                    pane_id,
                    input: new_input,
                    cursor: new_cursor,
                    completions,
                    completion_index: Some(next),
                    completion_prefix,
                };
            }
        }
        KeyCode::Left => {
            cursor = cursor.saturating_sub(1);
            path_edit_set(app, pane_id, input, cursor);
        }
        KeyCode::Right => {
            if cursor < input.len() {
                cursor += 1;
            }
            path_edit_set(app, pane_id, input, cursor);
        }
        KeyCode::Char(c) => {
            input.insert(cursor, c);
            cursor += 1;
            // Reset completions when typing
            path_edit_set(app, pane_id, input, cursor);
        }
        _ => {}
    }
}

fn handle_favorites_list(app: &mut App, key: KeyEvent) {
    let (pane_id, selected) = if let AppMode::FavoritesList { pane_id, selected } = &app.mode {
        (pane_id.clone(), *selected)
    } else {
        return;
    };

    let count = app.config.favorites.paths.len();
    if count == 0 {
        app.mode = AppMode::Normal;
        return;
    }

    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let new_sel = if selected == 0 { count - 1 } else { selected - 1 };
            app.mode = AppMode::FavoritesList {
                pane_id,
                selected: new_sel,
            };
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let new_sel = (selected + 1) % count;
            app.mode = AppMode::FavoritesList {
                pane_id,
                selected: new_sel,
            };
        }
        KeyCode::Enter => {
            let fav_path = app.config.favorites.paths[selected].clone();
            // Expand ~ to home
            let target = if fav_path.starts_with('~') {
                fav_path.replacen('~', &app.home_path, 1)
            } else {
                fav_path
            };
            app.navigate_to(&pane_id, &target);
            app.mode = AppMode::Normal;
        }
        // 'e' again or '/' switches to path edit mode (type a path manually)
        KeyCode::Char('e') | KeyCode::Char('/') => {
            let path = app
                .pane_map
                .get(&pane_id)
                .map(|p| p.current_path.clone())
                .unwrap_or_default();
            app.mode = AppMode::PathEdit {
                pane_id,
                input: path.clone(),
                cursor: path.len(),
                completions: Vec::new(),
                completion_index: None,
                completion_prefix: String::new(),
            };
        }
        // 'd' deletes the selected favorite
        KeyCode::Char('d') => {
            let fav_path = app.config.favorites.paths[selected].clone();
            match app.config.remove_favorite(&fav_path) {
                Ok(()) => {
                    app.set_status(format!("Removed favorite: {}", fav_path));
                    if app.config.favorites.paths.is_empty() {
                        app.mode = AppMode::Normal;
                    } else {
                        let new_sel = selected.min(app.config.favorites.paths.len() - 1);
                        app.mode = AppMode::FavoritesList {
                            pane_id,
                            selected: new_sel,
                        };
                    }
                }
                Err(e) => app.set_status(format!("Error: {}", e)),
            }
        }
        _ => {}
    }
}

// --- Helper functions ---

fn move_focus(app: &mut App, delta: i32) {
    let pane_id = app.active_pane_id.clone();
    if let Some(pane) = app.pane_map.get_mut(&pane_id) {
        if pane.entries.is_empty() {
            return;
        }
        let new_idx = (pane.focus_index + delta).clamp(0, pane.entries.len() as i32 - 1);
        pane.focus_index = new_idx;
        pane.table_state.select(Some(new_idx as usize));
    }
}

fn focus_to(app: &mut App, idx: i32) {
    let pane_id = app.active_pane_id.clone();
    if let Some(pane) = app.pane_map.get_mut(&pane_id) {
        if pane.entries.is_empty() {
            return;
        }
        let new_idx = idx.clamp(0, pane.entries.len() as i32 - 1);
        pane.focus_index = new_idx;
        pane.table_state.select(Some(new_idx as usize));
    }
}

fn page_move(app: &mut App, dir: i32) {
    let page = app
        .pane_views
        .get(&app.active_pane_id)
        .map(|v| v.list_area.height as i32)
        .filter(|h| *h > 0)
        .unwrap_or(10);
    move_focus(app, dir * page);
}

fn toggle_selection_at_focus(app: &mut App) {
    let pane_id = app.active_pane_id.clone();
    if let Some(pane) = app.pane_map.get_mut(&pane_id) {
        if pane.focus_index >= 0 && (pane.focus_index as usize) < pane.entries.len() {
            let path = pane.entries[pane.focus_index as usize].path.clone();
            if pane.selected_paths.contains(&path) {
                pane.selected_paths.remove(&path);
            } else {
                pane.selected_paths.insert(path);
            }
        }
    }
}

fn open_focused(app: &mut App) {
    let pane_id = app.active_pane_id.clone();
    let entry = {
        let pane = match app.pane_map.get(&pane_id) {
            Some(p) => p,
            None => return,
        };
        if pane.focus_index < 0 || pane.focus_index as usize >= pane.entries.len() {
            return;
        }
        pane.entries[pane.focus_index as usize].clone()
    };

    if entry.is_dir {
        app.navigate_to(&pane_id, &entry.path);
    } else {
        open_file_with_config(app, &entry.path);
    }
}

fn navigate_up(app: &mut App) {
    let pane_id = app.active_pane_id.clone();
    let current = app
        .pane_map
        .get(&pane_id)
        .map(|p| p.current_path.clone())
        .unwrap_or_default();

    if let Some(parent) = std::path::Path::new(&current).parent() {
        let parent_str = parent.to_string_lossy().to_string();
        app.navigate_to(&pane_id, &parent_str);
    }
}

fn split_active_pane(app: &mut App, direction: SplitDirection) {
    let new_id = app.next_pane_id();
    let current_path = app
        .pane_map
        .get(&app.active_pane_id)
        .map(|p| p.current_path.clone())
        .unwrap_or_else(|| app.home_path.clone());

    app.layout_root = layout::split_pane(
        &app.layout_root,
        &app.active_pane_id,
        &new_id,
        direction,
    );

    let mut new_pane = crate::app::PaneState::new(&current_path);
    match panex_core::read_directory(&current_path) {
        Ok(raw) => {
            let filtered = apply_sort_and_filter(
                &raw,
                app.show_hidden,
                "",
                app.sort_field,
                app.sort_direction,
            );
            new_pane.entries = filtered;
            app.raw_entries_map.insert(new_id.clone(), raw);
        }
        Err(e) => {
            app.set_status(format!("Error: {}", e));
        }
    }
    app.pane_map.insert(new_id, new_pane);
}

/// Grow (`delta` = 1) or shrink (`delta` = -1) the active pane by 25% on both
/// axes, clamped to one step either side of the default. Each axis moves its
/// nearest ancestor split, so an axis with no split simply doesn't move —
/// a pane in a left/right split only changes width.
fn resize_active_pane(app: &mut App, delta: i8) {
    let pane_id = app.active_pane_id.clone();
    let Some(pane) = app.pane_map.get(&pane_id) else {
        return;
    };
    let (old_w, old_h) = (pane.width_level, pane.height_level);
    let new_w = (old_w + delta).clamp(-1, 1);
    let new_h = (old_h + delta).clamp(-1, 1);

    let axes = [
        (SplitDirection::Vertical, new_w, old_w),
        (SplitDirection::Horizontal, new_h, old_h),
    ];

    let mut resized_any = false;
    let mut moved_any = false;

    for (direction, new_level, old_level) in axes {
        let is_width = direction == SplitDirection::Vertical;
        let Some(affected) =
            layout::resize_axis(&mut app.layout_root, &pane_id, direction, new_level)
        else {
            continue;
        };
        resized_any = true;
        moved_any |= new_level != old_level;

        // Everyone sharing the boundary we just moved loses their claim on it.
        for id in affected {
            if let Some(p) = app.pane_map.get_mut(&id) {
                if is_width {
                    p.width_level = 0;
                } else {
                    p.height_level = 0;
                }
            }
        }
        if let Some(p) = app.pane_map.get_mut(&pane_id) {
            if is_width {
                p.width_level = new_level;
            } else {
                p.height_level = new_level;
            }
        }
    }

    if !resized_any {
        app.set_status("No split to resize — split first with | or _".to_string());
    } else if !moved_any {
        let edge = if delta > 0 { "largest" } else { "smallest" };
        app.set_status(format!("Pane already at its {} size", edge));
    }
}

fn close_active_pane(app: &mut App) {
    if count_leaves(&app.layout_root) <= 1 {
        return;
    }

    let pane_id = app.active_pane_id.clone();
    if let Some(new_root) = layout::remove_pane(&app.layout_root, &pane_id) {
        app.layout_root = new_root;
        app.pane_map.remove(&pane_id);
        app.raw_entries_map.remove(&pane_id);

        // Activate first remaining pane
        let leaf_ids = collect_leaf_ids(&app.layout_root);
        if let Some(first) = leaf_ids.first() {
            app.active_pane_id = first.clone();
        }
    }
}

fn cycle_pane(app: &mut App) {
    let ids = collect_leaf_ids(&app.layout_root);
    if ids.len() <= 1 {
        return;
    }
    let current_idx = ids.iter().position(|id| id == &app.active_pane_id);
    let next_idx = match current_idx {
        Some(i) => (i + 1) % ids.len(),
        None => 0,
    };
    app.active_pane_id = ids[next_idx].clone();
}

fn copy_to_clipboard(app: &mut App, mode: ClipMode) {
    let pane_id = app.active_pane_id.clone();
    let entries: Vec<_> = if let Some(pane) = app.pane_map.get(&pane_id) {
        if pane.selected_paths.is_empty() {
            // Copy focused item
            if pane.focus_index >= 0 && (pane.focus_index as usize) < pane.entries.len() {
                vec![pane.entries[pane.focus_index as usize].clone()]
            } else {
                return;
            }
        } else {
            pane.entries
                .iter()
                .filter(|e| pane.selected_paths.contains(&e.path))
                .cloned()
                .collect()
        }
    } else {
        return;
    };

    let count = entries.len();
    let label = match mode {
        ClipMode::Copy => "Copied",
        ClipMode::Cut => "Cut",
    };
    app.file_clipboard = Some(FileClipboard { entries, mode });
    app.set_status(format!("{} {} item(s)", label, count));
}

fn paste_clipboard(app: &mut App) {
    let clipboard = match &app.file_clipboard {
        Some(c) => c,
        None => {
            app.set_status("Nothing to paste".to_string());
            return;
        }
    };

    let pane_id = app.active_pane_id.clone();
    let dest_dir = app
        .pane_map
        .get(&pane_id)
        .map(|p| p.current_path.clone())
        .unwrap_or_default();

    let is_cut = clipboard.mode == ClipMode::Cut;
    let entries: Vec<_> = clipboard.entries.clone();
    let mut errors = Vec::new();

    for entry in &entries {
        let result = if is_cut {
            panex_core::move_entry(&entry.path, &dest_dir)
        } else {
            panex_core::copy_entry(&entry.path, &dest_dir)
        };
        if let Err(e) = result {
            errors.push(e);
        }
    }

    if is_cut {
        app.file_clipboard = None;
    }

    if errors.is_empty() {
        app.set_status(format!("Pasted {} item(s)", entries.len()));
    } else {
        app.set_status(format!("Paste errors: {}", errors.join(", ")));
    }

    // Refresh all panes
    let pane_ids: Vec<String> = app.pane_map.keys().cloned().collect();
    for pid in pane_ids {
        app.refresh_pane(&pid);
    }
}

fn start_rename(app: &mut App) {
    let pane_id = app.active_pane_id.clone();
    if let Some(pane) = app.pane_map.get(&pane_id) {
        if pane.focus_index >= 0 && (pane.focus_index as usize) < pane.entries.len() {
            let entry = &pane.entries[pane.focus_index as usize];
            let name = entry.name.clone();
            let path = entry.path.clone();
            app.mode = AppMode::Rename {
                pane_id,
                path,
                input: name.clone(),
                cursor: name.len(),
            };
        }
    }
}

fn start_delete(app: &mut App) {
    let pane_id = app.active_pane_id.clone();
    let paths: Vec<String> = if let Some(pane) = app.pane_map.get(&pane_id) {
        if pane.selected_paths.is_empty() {
            if pane.focus_index >= 0 && (pane.focus_index as usize) < pane.entries.len() {
                vec![pane.entries[pane.focus_index as usize].path.clone()]
            } else {
                return;
            }
        } else {
            pane.selected_paths.iter().cloned().collect()
        }
    } else {
        return;
    };

    let message = if paths.len() == 1 {
        let name = std::path::Path::new(&paths[0])
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        format!("Move \"{}\" to trash?", name)
    } else {
        format!("Move {} items to trash?", paths.len())
    };

    app.mode = AppMode::Confirm {
        title: "Delete".to_string(),
        message,
        action: ConfirmAction::Delete(paths),
        selected: 0, // default to Yes
    };
}

fn open_in_default_app(app: &mut App) {
    let pane_id = app.active_pane_id.clone();
    if let Some(pane) = app.pane_map.get(&pane_id) {
        if pane.focus_index >= 0 && (pane.focus_index as usize) < pane.entries.len() {
            let path = pane.entries[pane.focus_index as usize].path.clone();
            open_file_with_config(app, &path);
        }
    }
}

/// Open a file using TUI config. Terminal commands (hx, nvim, etc.) open in a new terminal tab.
fn open_file_with_config(app: &mut App, path: &str) {
    let custom_app = panex_core::get_extension(path)
        .and_then(|ext| app.config.get_tui_app(&ext).cloned());

    if let Some(cmd) = custom_app {
        // Open in a new terminal tab
        if let Err(e) = panex_core::open_in_terminal_with_command(&cmd, &[path]) {
            app.set_status(format!("Open failed: {}", e));
        }
    } else {
        // Fall back to OS default
        if let Err(e) = panex_core::open_entry(path) {
            app.set_status(format!("Open failed: {}", e));
        }
    }
}

fn open_in_terminal(app: &mut App) {
    let pane_id = app.active_pane_id.clone();
    if let Some(pane) = app.pane_map.get(&pane_id) {
        if let Err(e) = panex_core::open_in_terminal(&pane.current_path) {
            app.set_status(format!("Terminal failed: {}", e));
        }
    }
}

fn start_new_file(app: &mut App) {
    let dir = app
        .pane_map
        .get(&app.active_pane_id)
        .map(|p| p.current_path.clone())
        .unwrap_or_default();
    let default = "untitled.txt".to_string();
    app.mode = AppMode::Prompt {
        title: "New File".to_string(),
        input: default.clone(),
        cursor: default.len(),
        action: PromptAction::NewFile(dir),
    };
}

fn start_new_folder(app: &mut App) {
    let dir = app
        .pane_map
        .get(&app.active_pane_id)
        .map(|p| p.current_path.clone())
        .unwrap_or_default();
    let default = "New Folder".to_string();
    app.mode = AppMode::Prompt {
        title: "New Folder".to_string(),
        input: default.clone(),
        cursor: default.len(),
        action: PromptAction::NewFolder(dir),
    };
}

fn select_all(app: &mut App) {
    let pane_id = app.active_pane_id.clone();
    if let Some(pane) = app.pane_map.get_mut(&pane_id) {
        pane.selected_paths = pane.entries.iter().map(|e| e.path.clone()).collect();
    }
}

fn deselect_all(app: &mut App) {
    let pane_id = app.active_pane_id.clone();
    if let Some(pane) = app.pane_map.get_mut(&pane_id) {
        pane.selected_paths.clear();
    }
}

fn refilter_all_panes(app: &mut App) {
    let pane_ids: Vec<String> = app.pane_map.keys().cloned().collect();
    for pid in pane_ids {
        app.refilter_pane(&pid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ago(ms: u64) -> Instant {
        Instant::now() - Duration::from_millis(ms)
    }

    #[test]
    fn pairs_two_quick_clicks_on_the_same_row() {
        let last = ("pane-1".to_string(), 4, ago(100));
        assert!(completes_double_click(
            Some(&last),
            "pane-1",
            4,
            Instant::now()
        ));
    }

    #[test]
    fn does_not_pair_across_the_window() {
        let last = ("pane-1".to_string(), 4, ago(900));
        assert!(!completes_double_click(
            Some(&last),
            "pane-1",
            4,
            Instant::now()
        ));
    }

    /// Re-aiming at another row means the user meant two clicks, not one open.
    #[test]
    fn does_not_pair_across_rows() {
        let last = ("pane-1".to_string(), 4, ago(50));
        assert!(!completes_double_click(
            Some(&last),
            "pane-1",
            5,
            Instant::now()
        ));
    }

    /// Nor across panes, where the first click was the one that focused it.
    #[test]
    fn does_not_pair_across_panes() {
        let last = ("pane-1".to_string(), 4, ago(50));
        assert!(!completes_double_click(
            Some(&last),
            "pane-2",
            4,
            Instant::now()
        ));
    }

    #[test]
    fn first_click_of_the_session_is_never_a_pair() {
        assert!(!completes_double_click(None, "pane-1", 0, Instant::now()));
    }
}

/// Drives real mouse events through the whole path — hit-test, focus, open —
/// rather than just the pairing predicate above.
#[cfg(test)]
mod click_tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::PathBuf;

    /// A directory under the system temp dir, removed when the test ends.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!("panex-{}-{}", tag, std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A pane holding `tmp`, plus the screen position of its first row. The
    /// render is what populates `pane_views`, which mouse hit-testing reads.
    fn pane_showing(tmp: &TempDir) -> (App, String, u16, u16) {
        let mut app = App::new().unwrap();
        let pane_id = app.active_pane_id.clone();
        app.navigate_to(&pane_id, &tmp.0.to_string_lossy());

        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| crate::ui::draw(frame, &mut app)).unwrap();
        let view = app.pane_views[&pane_id];
        let (x, y) = (view.list_area.x, view.list_area.y);
        (app, pane_id, x, y)
    }

    fn click(app: &mut App, x: u16, y: u16) {
        handle_mouse_event(
            app,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: x,
                row: y,
                modifiers: KeyModifiers::NONE,
            },
        );
    }

    #[test]
    fn double_click_enters_the_folder_under_the_cursor() {
        let tmp = TempDir::new("dbl");
        std::fs::create_dir(tmp.0.join("sub")).unwrap();
        let (mut app, pane_id, x, y) = pane_showing(&tmp);

        click(&mut app, x, y);
        click(&mut app, x, y);

        assert_eq!(
            app.pane_map[&pane_id].current_path,
            tmp.0.join("sub").to_string_lossy()
        );
    }

    #[test]
    fn a_lone_click_only_moves_focus() {
        let tmp = TempDir::new("single");
        std::fs::create_dir(tmp.0.join("sub")).unwrap();
        let (mut app, pane_id, x, y) = pane_showing(&tmp);

        click(&mut app, x, y);

        assert_eq!(app.pane_map[&pane_id].current_path, tmp.0.to_string_lossy());
        assert_eq!(app.pane_map[&pane_id].focus_index, 0);
    }

    /// The window has to expire, or a click resting on a row from minutes ago
    /// would open it.
    #[test]
    fn a_stale_first_click_does_not_pair() {
        let tmp = TempDir::new("stale");
        std::fs::create_dir(tmp.0.join("sub")).unwrap();
        let (mut app, pane_id, x, y) = pane_showing(&tmp);

        click(&mut app, x, y);
        // Age the first click past the window without sleeping through it.
        if let Some((_, _, at)) = app.last_click.as_mut() {
            *at -= DOUBLE_CLICK_WINDOW * 2;
        }
        click(&mut app, x, y);

        assert_eq!(app.pane_map[&pane_id].current_path, tmp.0.to_string_lossy());
    }
}
