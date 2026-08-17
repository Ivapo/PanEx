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

    // The card pane has its own small keyboard. Beyond that only the keys
    // that act on panes get through — the rest would act on the directory it
    // carries for splitting, which it never draws, so the effect is invisible.
    if app.oko_pane_id.as_deref() == Some(app.active_pane_id.as_str())
        && (handle_oko_keys(app, key.code) || !acts_on_panes(key.code))
    {
        return;
    }

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
            move_focus(app, -1, Ends::Stop);
            toggle_selection_at_focus(app);
        }
        KeyCode::Down | KeyCode::Char('j') if shift => {
            move_focus(app, 1, Ends::Stop);
            toggle_selection_at_focus(app);
        }
        KeyCode::Up | KeyCode::Char('k') => move_focus(app, -1, Ends::Wrap),
        KeyCode::Down | KeyCode::Char('j') => move_focus(app, 1, Ends::Wrap),
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

        // Card view. Bound only when a usable oko is on PATH — otherwise this
        // falls through unhandled, which is the point: no binding, no entry
        // in the help overlay, nothing that looks broken when pressed.
        KeyCode::Char('O') if app.oko_available => toggle_oko_pane(app),

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

/// The card pane's own keys. Returns true when the key was one of them.
///
/// Deliberately the same shapes the file list uses — `j`/`k` to move, `Enter`
/// to act on what is under the cursor, `r` to rename — so the pane is not a
/// second keyboard to learn.
fn handle_oko_keys(app: &mut App, key: KeyCode) -> bool {
    match key {
        KeyCode::Up | KeyCode::Char('k') => move_oko_selection(app, -1),
        KeyCode::Down | KeyCode::Char('j') => move_oko_selection(app, 1),
        KeyCode::Enter => jump_to_selected_tab(app),
        KeyCode::Char('r') => start_tab_rename(app),
        _ => return false,
    }
    true
}

fn selected_row(app: &App) -> Option<&crate::oko::Row> {
    let crate::oko::View::Rows(rows) = &app.oko_view else {
        return None;
    };
    let id = app.oko_selected.as_deref()?;
    rows.iter().find(|row| row.session_id == id)
}

fn move_oko_selection(app: &mut App, delta: i32) {
    let crate::oko::View::Rows(rows) = &app.oko_view else {
        return;
    };
    if rows.is_empty() {
        return;
    }
    // A selection whose tab has closed is no position at all, so movement
    // restarts from the top rather than from wherever it used to be.
    let current = selected_index(app);
    let crate::oko::View::Rows(rows) = &app.oko_view else {
        return;
    };
    // Wraps, as the file list does: a short list of tabs is a ring, and
    // stopping at the last card only means pressing `k` six times to reach it.
    let next = match current {
        Some(i) => (i as i32 + delta).rem_euclid(rows.len() as i32) as usize,
        None => 0,
    };
    app.oko_selected = Some(rows[next].session_id.clone());
    scroll_selection_into_view(app);
}

/// Scroll the card view by whole cards, and drag the selection along only if
/// it would otherwise scroll out of sight — the same bargain the file list
/// strikes with its wheel.
fn scroll_cards(app: &mut App, delta: i32) -> bool {
    let crate::oko::View::Rows(rows) = &app.oko_view else {
        return false;
    };
    if app.oko_capacity == 0 {
        return false;
    }
    let max = rows.len().saturating_sub(app.oko_capacity) as i32;
    let next = (app.oko_offset as i32 + delta).clamp(0, max) as usize;
    let moved = next != app.oko_offset;
    app.oko_offset = next;
    drag_selection_into_view(app) || moved
}

/// Pull the selection back inside the visible cards. Nothing to do while it is
/// already there, which is every scroll that has not passed it by.
fn drag_selection_into_view(app: &mut App) -> bool {
    let crate::oko::View::Rows(rows) = &app.oko_view else {
        return false;
    };
    if rows.is_empty() || app.oko_capacity == 0 {
        return false;
    }
    let Some(current) = selected_index(app) else {
        return false;
    };
    let last = (app.oko_offset + app.oko_capacity - 1).min(rows.len() - 1);
    let clamped = current.clamp(app.oko_offset, last);
    if clamped == current {
        return false;
    }
    let session_id = rows[clamped].session_id.clone();
    app.oko_selected = Some(session_id);
    true
}

/// Scroll the view to the selected card, moving as few cards as will do it.
/// The keyboard drives the viewport here, where the wheel drives the selection.
fn scroll_selection_into_view(app: &mut App) {
    let Some(current) = selected_index(app) else {
        return;
    };
    if app.oko_capacity == 0 {
        return;
    }
    if current < app.oko_offset {
        app.oko_offset = current;
    } else if current >= app.oko_offset + app.oko_capacity {
        app.oko_offset = current + 1 - app.oko_capacity;
    }
}

fn selected_index(app: &App) -> Option<usize> {
    let crate::oko::View::Rows(rows) = &app.oko_view else {
        return None;
    };
    let id = app.oko_selected.as_deref()?;
    rows.iter().position(|row| row.session_id == id)
}

fn jump_to_selected_tab(app: &mut App) {
    let Some(session_id) = app.oko_selected.clone() else {
        return;
    };
    if let Err(e) = crate::oko::activate(&session_id) {
        app.set_status(format!("Jump failed: {}", e));
    }
}

fn start_tab_rename(app: &mut App) {
    let Some(row) = selected_row(app) else {
        return;
    };
    // Prefilled with what the card shows, so clearing it back to the derived
    // name is a visible delete rather than a thing you have to know about.
    let session_id = row.session_id.clone();
    let current = row.name.clone().unwrap_or_default();
    app.mode = AppMode::Prompt {
        title: "Rename Tab".to_string(),
        input: current.clone(),
        cursor: current.len(),
        action: PromptAction::RenameTab(session_id),
    };
}

fn report_creation(app: &mut App, created: Result<(), String>, name: &str) {
    match created {
        Ok(()) => {
            app.set_status(format!("Created {}", name));
            let pane_id = app.active_pane_id.clone();
            app.refresh_pane(&pane_id);
        }
        Err(e) => app.set_status(format!("Create failed: {}", e)),
    }
}

/// Keys that mean the same thing whatever a pane is showing.
fn acts_on_panes(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::Char('q')
            | KeyCode::Char('O')
            | KeyCode::Char('W')
            | KeyCode::Char('|')
            | KeyCode::Char('_')
            | KeyCode::Char('+')
            | KeyCode::Char('=')
            | KeyCode::Char('-')
            | KeyCode::Char('?')
            | KeyCode::Tab
    )
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
    // The card pane keeps its own offset: its list is of cards, each several
    // rows tall, so a row count would scroll it by fractions of a card. One
    // card a tick covers about the three rows a file pane moves by.
    if app.oko_pane_id.as_deref() == Some(pane_id.as_str()) {
        return scroll_cards(app, delta.signum());
    }
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

    // The card pane draws no rows, so its clicks are hit-tested against the
    // cards themselves rather than against a list area it does not have.
    if app.oko_pane_id.as_deref() == Some(pane_id.as_str()) {
        return click_card_at(app, &pane_id, x, y) || changed;
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

/// A click inside the card pane: it selects the card under the cursor, and a
/// second one on the same card jumps to that tab — the same pair of gestures a
/// file row answers to, so the pane is not a second mouse to learn.
///
/// Pairing is by card position, as it is for file rows, but the jump goes to
/// whatever is selected *now*: if a tab closed between the two clicks and the
/// cards shuffled, the second click has already re-aimed the selection at the
/// card actually under the cursor.
fn click_card_at(app: &mut App, pane_id: &str, x: u16, y: u16) -> bool {
    let hit = app
        .oko_cards
        .iter()
        .position(|(card, _)| {
            x >= card.x && x < card.x + card.width && y >= card.y && y < card.y + card.height
        })
        .map(|i| (i, app.oko_cards[i].1.clone()));

    // The gaps around the cards are not a card. Clicking one is a deliberate
    // miss, and should not leave half a pair waiting for the next click.
    let Some((idx, session_id)) = hit else {
        app.last_click = None;
        return false;
    };

    let mut changed = false;
    if app.oko_selected.as_deref() != Some(session_id.as_str()) {
        app.oko_selected = Some(session_id);
        changed = true;
    }

    let now = Instant::now();
    if completes_double_click(app.last_click.as_ref(), pane_id, idx, now) {
        app.last_click = None;
        jump_to_selected_tab(app);
        return true;
    }

    app.last_click = Some((pane_id.to_string(), idx, now));
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
            (title.clone(), input.clone(), *cursor, action.clone())
        } else {
            return;
        };

    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            match &action {
                // An empty name clears it, which is the only way back to the
                // name oko derives — so this action alone acts on an empty
                // input, where the others treat it as a cancel.
                PromptAction::RenameTab(session_id) => {
                    let name = (!input.is_empty()).then(|| input.clone());
                    let outcome = crate::oko::set_name(session_id, name.as_deref());
                    app.set_status(match (outcome, &name) {
                        (Ok(()), Some(name)) => format!("Renamed tab to {}", name),
                        (Ok(()), None) => "Tab name cleared".to_string(),
                        (Err(e), _) => format!("Rename failed: {}", e),
                    });
                }
                PromptAction::NewFile(dir) if !input.is_empty() => {
                    let created = panex_core::create_file(dir, &input);
                    report_creation(app, created, &input);
                }
                PromptAction::NewFolder(dir) if !input.is_empty() => {
                    let created = panex_core::create_folder(dir, &input);
                    report_creation(app, created, &input);
                }
                _ => {}
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

/// What the first and last row do to a step that would leave the list.
#[derive(Clone, Copy, PartialEq)]
enum Ends {
    /// Stop there. What a page jump and an extended selection want: both are
    /// aimed at a distance, and a wrap would land them somewhere else entirely.
    Stop,
    /// Past the last row is the first, and before the first is the last.
    Wrap,
}

fn move_focus(app: &mut App, delta: i32, ends: Ends) {
    let pane_id = app.active_pane_id.clone();
    if let Some(pane) = app.pane_map.get_mut(&pane_id) {
        if pane.entries.is_empty() {
            return;
        }
        let len = pane.entries.len() as i32;
        // Clamped first, because a focus that is out of range has no step to
        // wrap from — a modulo of it would land anywhere.
        let from = pane.focus_index.clamp(0, len - 1);
        let new_idx = match ends {
            Ends::Wrap => (from + delta).rem_euclid(len),
            Ends::Stop => (from + delta).clamp(0, len - 1),
        };
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
    move_focus(app, dir * page, Ends::Stop);
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
    let pane_id = app.active_pane_id.clone();
    close_pane(app, &pane_id);
}

fn close_pane(app: &mut App, pane_id: &str) {
    if count_leaves(&app.layout_root) <= 1 {
        // Removing the only leaf would leave no pane at all. If it is the
        // card view, it stops being one and goes back to showing files —
        // closing the view should never be a dead end.
        if app.oko_pane_id.as_deref() == Some(pane_id) {
            detach_oko(app);
            let home = app.home_path.clone();
            app.navigate_to(pane_id, &home);
        }
        return;
    }

    if let Some(new_root) = layout::remove_pane(&app.layout_root, pane_id) {
        app.layout_root = new_root;
        app.pane_map.remove(pane_id);
        app.raw_entries_map.remove(pane_id);
        if app.oko_pane_id.as_deref() == Some(pane_id) {
            detach_oko(app);
        }

        // Activate first remaining pane
        let leaf_ids = collect_leaf_ids(&app.layout_root);
        if let Some(first) = leaf_ids.first() {
            app.active_pane_id = first.clone();
        }
    }
}

/// Opens the card view as a pane, or closes the one already open.
fn toggle_oko_pane(app: &mut App) {
    if let Some(existing) = app.oko_pane_id.clone() {
        close_pane(app, &existing);
        return;
    }

    open_oko_pane(app);
    match crate::oko::Stream::start() {
        Ok(stream) => {
            app.oko_stream = Some(stream);
            app.oko_view = crate::oko::View::Connecting;
        }
        Err(e) => app.oko_view = crate::oko::View::Lost(e),
    }
}

/// Carves out the pane and returns its id. Separate from starting the stream
/// so the layout can be exercised without spawning anything.
fn open_oko_pane(app: &mut App) -> String {
    let new_id = app.next_pane_id();
    // On the left: the cards are a sidebar you glance at, and the file pane
    // you were working in keeps the position your eye is already on.
    app.layout_root = layout::split_pane_on(
        &app.layout_root,
        &app.active_pane_id,
        &new_id,
        SplitDirection::Vertical,
        layout::Side::Before,
    );
    // Its entry list stays empty — nothing here is a file — but it carries the
    // directory it was opened from, so splitting *this* pane produces a file
    // pane showing somewhere real rather than one with no path at all.
    let origin = app
        .pane_map
        .get(&app.active_pane_id)
        .map(|p| p.current_path.clone())
        .unwrap_or_else(|| app.home_path.clone());
    app.pane_map
        .insert(new_id.clone(), crate::app::PaneState::new(&origin));
    app.oko_pane_id = Some(new_id.clone());
    new_id
}

/// Forget the stream. Dropping it kills the child — oko exits on its own when
/// stdout closes, but a closed view should not leave that to chance.
fn detach_oko(app: &mut App) {
    app.oko_pane_id = None;
    app.oko_stream = None;
    app.oko_view = crate::oko::View::Connecting;
    app.oko_cards.clear();
    app.oko_offset = 0;
    app.oko_capacity = 0;
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

    /// `j` off the bottom lands on the first row, `k` off the top on the last.
    /// The wrap is for the plain keys only — see the page/extend tests below.
    #[test]
    fn j_and_k_wrap_around_the_file_list() {
        let tmp = TempDir::new("wrap");
        for name in ["a", "b", "c"] {
            std::fs::create_dir(tmp.0.join(name)).unwrap();
        }
        let (mut app, pane_id, _, _) = pane_showing(&tmp);
        let focus = |app: &App| app.pane_map[&pane_id].focus_index;

        press(&mut app, KeyCode::Char('k'));
        assert_eq!(focus(&app), 2, "up from the first row is the last");
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(focus(&app), 0, "down from the last row is the first");
        press(&mut app, KeyCode::Down);
        assert_eq!(focus(&app), 1, "arrows wrap the same way");
    }

    /// An extended selection is aimed at a distance, and wrapping it would
    /// sweep the whole list into the selection on one keypress.
    #[test]
    fn shift_j_stops_at_the_end_instead_of_wrapping() {
        let tmp = TempDir::new("nowrap");
        for name in ["a", "b"] {
            std::fs::create_dir(tmp.0.join(name)).unwrap();
        }
        let (mut app, pane_id, _, _) = pane_showing(&tmp);

        for _ in 0..4 {
            handle_key_event(
                &mut app,
                KeyEvent::new(KeyCode::Char('j'), KeyModifiers::SHIFT),
            );
        }

        assert_eq!(app.pane_map[&pane_id].focus_index, 1, "should have stopped");
    }

    /// A page jump is aimed at a distance too — off the end it stops there.
    #[test]
    fn page_down_stops_at_the_last_row() {
        let tmp = TempDir::new("page");
        for name in ["a", "b", "c"] {
            std::fs::create_dir(tmp.0.join(name)).unwrap();
        }
        let (mut app, pane_id, _, _) = pane_showing(&tmp);

        press(&mut app, KeyCode::PageDown);

        assert_eq!(app.pane_map[&pane_id].focus_index, 2);
    }

    fn press(app: &mut App, code: KeyCode) {
        handle_key_event(app, KeyEvent::from(code));
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

/// The card pane's place in the layout, and what the rest of the keyboard
/// does while it is the active one.
#[cfg(test)]
mod oko_pane_tests {
    use super::*;
    use crate::app::App;

    fn press(app: &mut App, code: KeyCode) {
        handle_key_event(app, KeyEvent::from(code));
    }

    #[test]
    fn the_card_pane_opens_on_the_left() {
        let mut app = App::new().unwrap();
        let files = app.active_pane_id.clone();
        let cards = open_oko_pane(&mut app);
        assert_eq!(collect_leaf_ids(&app.layout_root), vec![cards, files]);
    }

    /// The bug this fixes: the card pane carried no directory, so splitting it
    /// produced a pane with nothing to list and nowhere to navigate from.
    #[test]
    fn splitting_the_card_pane_gives_a_pane_that_lists_files() {
        let mut app = App::new().unwrap();
        let files = app.active_pane_id.clone();
        let cards = open_oko_pane(&mut app);
        app.active_pane_id = cards.clone();

        press(&mut app, KeyCode::Char('|'));

        let fresh = collect_leaf_ids(&app.layout_root)
            .into_iter()
            .find(|id| *id != cards && *id != files)
            .expect("a third pane should exist");
        let pane = &app.pane_map[&fresh];
        assert!(!pane.current_path.is_empty(), "no directory to navigate from");
        assert!(!pane.entries.is_empty(), "listed nothing");
    }

    /// Every file key would otherwise act on the directory the card pane
    /// carries for splitting — one it never draws, so the effect is invisible.
    #[test]
    fn file_keys_do_nothing_while_the_cards_are_active() {
        let mut app = App::new().unwrap();
        let cards = open_oko_pane(&mut app);
        app.active_pane_id = cards;

        for code in [KeyCode::Char('n'), KeyCode::Char('N'), KeyCode::Char('r'), KeyCode::Char('/')] {
            press(&mut app, code);
            assert!(
                app.mode == AppMode::Normal,
                "{code:?} opened a mode over the cards"
            );
        }
    }

    /// Pane keys still have to work, or the pane is a trap.
    #[test]
    fn pane_keys_still_work_while_the_cards_are_active() {
        let mut app = App::new().unwrap();
        let files = app.active_pane_id.clone();
        let cards = open_oko_pane(&mut app);
        app.active_pane_id = cards;

        press(&mut app, KeyCode::Tab);
        assert_eq!(app.active_pane_id, files, "Tab should leave the cards");
    }

    fn card(session: &str, tab: u32, name: &str) -> crate::oko::Row {
        crate::oko::Row {
            session_id: session.to_string(),
            tab,
            name: Some(name.to_string()),
            path: Some("/tmp".to_string()),
            status: Some("working".to_string()),
            age: None,
            job: None,
        }
    }

    /// An app showing three cards, with the card pane active.
    fn showing_cards() -> App {
        let mut app = App::new().unwrap();
        let cards = open_oko_pane(&mut app);
        app.active_pane_id = cards;
        app.oko_view = crate::oko::View::Rows(vec![
            card("a", 1, "one"),
            card("b", 2, "two"),
            card("c", 3, "three"),
        ]);
        app.oko_selected = Some("a".to_string());
        app
    }

    /// A handful of tabs is a ring: stopping at the last card only means
    /// pressing `k` back through all of them to reach the first.
    #[test]
    fn j_and_k_move_the_selection_and_wrap_at_the_ends() {
        let mut app = showing_cards();

        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.oko_selected.as_deref(), Some("b"));
        press(&mut app, KeyCode::Down);
        assert_eq!(app.oko_selected.as_deref(), Some("c"));
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.oko_selected.as_deref(), Some("a"), "past the last is the first");

        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.oko_selected.as_deref(), Some("c"), "before the first is the last");
    }

    /// One card is its own neighbour in both directions, and neither key may
    /// divide by the length to find that out.
    #[test]
    fn a_single_card_stays_put() {
        let mut app = showing_cards();
        app.oko_view = crate::oko::View::Rows(vec![card("a", 1, "one")]);

        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('k'));

        assert_eq!(app.oko_selected.as_deref(), Some("a"));
    }

    /// Selection is a session id, not a position, so a tab closing above the
    /// selected one must not slide the selection onto a different card.
    #[test]
    fn the_selection_follows_the_session_not_the_position() {
        let mut app = showing_cards();
        app.oko_selected = Some("c".to_string());

        app.oko_view = crate::oko::View::Rows(vec![card("c", 1, "three")]);
        press(&mut app, KeyCode::Char('k'));

        assert_eq!(app.oko_selected.as_deref(), Some("c"));
    }

    /// `r` prefills with what the card shows, and carries the session id so a
    /// tab closing while the prompt is open cannot land the name elsewhere.
    #[test]
    fn r_opens_a_rename_prefilled_with_the_current_name() {
        let mut app = showing_cards();
        app.oko_selected = Some("b".to_string());

        press(&mut app, KeyCode::Char('r'));

        match &app.mode {
            AppMode::Prompt { input, action, .. } => {
                assert_eq!(input, "two");
                assert_eq!(*action, PromptAction::RenameTab("b".to_string()));
            }
            _ => panic!("no rename prompt opened"),
        }
    }

    /// Nothing selected is nothing to rename — not a prompt aimed at nobody.
    #[test]
    fn r_does_nothing_without_a_selection() {
        let mut app = showing_cards();
        app.oko_selected = None;
        press(&mut app, KeyCode::Char('r'));
        assert!(app.mode == AppMode::Normal);
    }

    /// A window of tabs can outrun the pane. Scrolling is by whole cards,
    /// since a card is what the view is a list of.
    fn many_cards(count: u32) -> App {
        let mut app = App::new().unwrap();
        let cards = open_oko_pane(&mut app);
        app.active_pane_id = cards;
        app.oko_view = crate::oko::View::Rows(
            (1..=count)
                .map(|i| card(&format!("s{i}"), i, &format!("tab {i}")))
                .collect(),
        );
        app.oko_selected = Some("s1".to_string());
        // What a render leaves behind, without one: a pane holding four cards.
        app.oko_capacity = 4;
        app
    }

    fn wheel(app: &mut App, down: bool) {
        let area = app.pane_views[app.oko_pane_id.as_ref().unwrap()].area;
        handle_mouse_event(
            app,
            MouseEvent {
                kind: if down {
                    MouseEventKind::ScrollDown
                } else {
                    MouseEventKind::ScrollUp
                },
                column: area.x + 1,
                row: area.y + 1,
                modifiers: KeyModifiers::NONE,
            },
        );
    }

    #[test]
    fn the_wheel_scrolls_the_cards_and_stops_at_the_last_page() {
        let mut app = many_cards(10);
        drawn(&mut app); // populates pane_views, which hit-testing reads
        app.oko_capacity = 4;

        wheel(&mut app, true);
        assert_eq!(app.oko_offset, 1, "a tick should move one card");

        for _ in 0..20 {
            wheel(&mut app, true);
        }
        assert_eq!(app.oko_offset, 6, "10 cards, 4 visible — the last page");

        for _ in 0..20 {
            wheel(&mut app, false);
        }
        assert_eq!(app.oko_offset, 0);
    }

    /// The wheel drags the selection only when it would otherwise scroll out
    /// of sight — the bargain the file list already strikes.
    #[test]
    fn scrolling_past_the_selection_takes_it_along() {
        let mut app = many_cards(10);
        drawn(&mut app);
        app.oko_capacity = 4;

        wheel(&mut app, true);
        assert_eq!(app.oko_selected.as_deref(), Some("s2"), "left behind");

        wheel(&mut app, false);
        assert_eq!(
            app.oko_selected.as_deref(),
            Some("s2"),
            "still in view, so it should not have moved"
        );
    }

    /// Moving the selection past the last visible card scrolls the view rather
    /// than selecting something the pane is not showing.
    #[test]
    fn the_view_follows_the_selection_off_the_bottom() {
        let mut app = many_cards(10);

        for _ in 0..4 {
            press(&mut app, KeyCode::Char('j'));
        }

        assert_eq!(app.oko_selected.as_deref(), Some("s5"));
        assert_eq!(app.oko_offset, 1, "should have scrolled by one card");
    }

    /// Wrapping from the last card to the first has to bring the view back
    /// with it, or `j` lands on a card that is scrolled off the top.
    #[test]
    fn wrapping_to_the_first_card_scrolls_back_to_the_top() {
        let mut app = many_cards(10);
        app.oko_selected = Some("s10".to_string());
        app.oko_offset = 6;

        press(&mut app, KeyCode::Char('j'));

        assert_eq!(app.oko_selected.as_deref(), Some("s1"));
        assert_eq!(app.oko_offset, 0);
    }

    /// Closing the view is never a dead end, even as the only pane left.
    #[test]
    fn closing_the_last_card_pane_leaves_a_file_pane() {
        let mut app = App::new().unwrap();
        let files = app.active_pane_id.clone();
        let cards = open_oko_pane(&mut app);
        close_pane(&mut app, &files);
        app.active_pane_id = cards.clone();

        close_pane(&mut app, &cards);

        assert!(app.oko_pane_id.is_none(), "still marked as the card pane");
        assert_eq!(collect_leaf_ids(&app.layout_root), vec![cards.clone()]);
        assert_eq!(app.pane_map[&cards].current_path, app.home_path);
    }

    /// Renders once so the cards have screen positions, and returns the middle
    /// of each one — hit-testing reads what the last frame drew.
    fn drawn(app: &mut App) -> Vec<(u16, u16)> {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| crate::ui::draw(frame, app)).unwrap();
        app.oko_cards
            .iter()
            .map(|(card, _)| (card.x + card.width / 2, card.y + card.height / 2))
            .collect()
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

    /// A click on a card selects it, exactly as a click on a file row moves
    /// the cursor there — and it takes the pane with it.
    #[test]
    fn clicking_a_card_selects_it() {
        let mut app = showing_cards();
        let files = collect_leaf_ids(&app.layout_root)
            .into_iter()
            .find(|id| Some(id.as_str()) != app.oko_pane_id.as_deref())
            .unwrap();
        let cards = drawn(&mut app);
        app.active_pane_id = files;

        let (x, y) = cards[2];
        click(&mut app, x, y);

        assert_eq!(app.oko_selected.as_deref(), Some("c"));
        assert_eq!(app.active_pane_id, app.oko_pane_id.clone().unwrap());
    }

    /// The second click is the one that jumps, so it must be recognised as the
    /// other half of the pair rather than banked as a fresh first click.
    #[test]
    fn a_second_click_on_the_same_card_is_a_pair() {
        let mut app = showing_cards();
        let cards = drawn(&mut app);
        let (x, y) = cards[1];

        click(&mut app, x, y);
        assert!(app.last_click.is_some(), "first click was not banked");

        click(&mut app, x, y);
        assert!(
            app.last_click.is_none(),
            "the pair should have been spent on a jump, not re-banked"
        );
        assert_eq!(app.oko_selected.as_deref(), Some("b"));
    }

    /// Two clicks on different cards are two clicks, however fast.
    #[test]
    fn clicks_on_two_cards_do_not_pair() {
        let mut app = showing_cards();
        let cards = drawn(&mut app);

        click(&mut app, cards[0].0, cards[0].1);
        click(&mut app, cards[1].0, cards[1].1);

        assert!(app.last_click.is_some(), "the second click should stand alone");
        assert_eq!(app.oko_selected.as_deref(), Some("b"));
    }

    /// A click on the gap between cards is a deliberate miss: it changes no
    /// selection, and leaves no half-pair for the next click to complete.
    #[test]
    fn clicking_past_the_last_card_selects_nothing() {
        let mut app = showing_cards();
        let cards = drawn(&mut app);
        let below = cards.last().unwrap().1 + 4;

        click(&mut app, cards[0].0, cards[0].1);
        click(&mut app, cards[0].0, below);

        assert_eq!(app.oko_selected.as_deref(), Some("a"), "selection moved");
        assert!(app.last_click.is_none(), "a miss should not bank a click");
    }
}
