use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, AppMode, ClipMode, ConfirmAction, FileClipboard, PromptAction};
use crate::layout::{self, SplitDirection, collect_leaf_ids, count_leaves};
use crate::sort::apply_sort_and_filter;

pub fn handle_key_event(app: &mut App, key: KeyEvent) {
    match &app.mode {
        AppMode::Normal => handle_normal(app, key),
        AppMode::Search { .. } => handle_search(app, key),
        AppMode::Rename { .. } => handle_rename(app, key),
        AppMode::Confirm { .. } => handle_confirm(app, key),
        AppMode::Prompt { .. } => handle_prompt(app, key),
        AppMode::PathEdit { .. } => handle_path_edit(app, key),
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
        KeyCode::Enter => open_focused(app),
        KeyCode::Backspace => navigate_up(app),
        KeyCode::Home | KeyCode::Char('~') => {
            let home = app.home_path.clone();
            let pane_id = app.active_pane_id.clone();
            app.navigate_to(&pane_id, &home);
        }

        // Pane management
        KeyCode::Char('|') => split_active_pane(app, SplitDirection::Vertical),
        KeyCode::Char('-') if !ctrl => split_active_pane(app, SplitDirection::Horizontal),
        KeyCode::Char('W') => close_active_pane(app),
        KeyCode::Tab => cycle_pane(app),

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

        // Path edit
        KeyCode::Char('e') => {
            let pane_id = app.active_pane_id.clone();
            let path = app
                .pane_map
                .get(&pane_id)
                .map(|p| p.current_path.clone())
                .unwrap_or_default();
            app.mode = AppMode::PathEdit {
                pane_id,
                input: path.clone(),
                cursor: path.len(),
            };
        }

        // Type-ahead search: any printable char starts search
        KeyCode::Char(c) if !ctrl && c.is_alphanumeric() => {
            let pane_id = app.active_pane_id.clone();
            if let Some(pane) = app.pane_map.get_mut(&pane_id) {
                pane.search_query = c.to_string();
            }
            app.mode = AppMode::Search {
                pane_id: pane_id.clone(),
            };
            app.refilter_pane(&pane_id);
        }

        _ => {}
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
                        app.status_message = Some(format!("Renamed to {}", input));
                        app.refresh_pane(&pane_id);
                    }
                    Err(e) => {
                        app.status_message = Some(format!("Rename failed: {}", e));
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
            if cursor > 0 {
                cursor -= 1;
            }
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

fn handle_confirm(app: &mut App, key: KeyEvent) {
    let action = if let AppMode::Confirm { action, .. } = &app.mode {
        // We need to clone data out
        match action {
            ConfirmAction::Delete(paths) => ConfirmAction::Delete(paths.clone()),
        }
    } else {
        return;
    };

    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => {
            match action {
                ConfirmAction::Delete(paths) => {
                    let mut errors = Vec::new();
                    for p in &paths {
                        if let Err(e) = panex_core::delete_entry(p, false) {
                            errors.push(e);
                        }
                    }
                    if errors.is_empty() {
                        app.status_message =
                            Some(format!("Deleted {} item(s)", paths.len()));
                    } else {
                        app.status_message = Some(format!("Delete errors: {}", errors.join(", ")));
                    }
                    // Refresh all panes
                    let pane_ids: Vec<String> = app.pane_map.keys().cloned().collect();
                    for pid in pane_ids {
                        app.refresh_pane(&pid);
                    }
                }
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
                        app.status_message = Some(format!("Created {}", input));
                        let pane_id = app.active_pane_id.clone();
                        app.refresh_pane(&pane_id);
                    }
                    Err(e) => {
                        app.status_message = Some(format!("Create failed: {}", e));
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
            if cursor > 0 {
                cursor -= 1;
            }
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

fn handle_path_edit(app: &mut App, key: KeyEvent) {
    let (pane_id, mut input, mut cursor) =
        if let AppMode::PathEdit {
            pane_id,
            input,
            cursor,
        } = &app.mode
        {
            (pane_id.clone(), input.clone(), *cursor)
        } else {
            return;
        };

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
            if cursor > 0 {
                input.remove(cursor - 1);
                cursor -= 1;
            }
            app.mode = AppMode::PathEdit {
                pane_id,
                input,
                cursor,
            };
        }
        KeyCode::Left => {
            if cursor > 0 {
                cursor -= 1;
            }
            app.mode = AppMode::PathEdit {
                pane_id,
                input,
                cursor,
            };
        }
        KeyCode::Right => {
            if cursor < input.len() {
                cursor += 1;
            }
            app.mode = AppMode::PathEdit {
                pane_id,
                input,
                cursor,
            };
        }
        KeyCode::Char(c) => {
            input.insert(cursor, c);
            cursor += 1;
            app.mode = AppMode::PathEdit {
                pane_id,
                input,
                cursor,
            };
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
        if let Err(e) = panex_core::open_entry(&entry.path) {
            app.status_message = Some(format!("Open failed: {}", e));
        }
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
            app.status_message = Some(format!("Error: {}", e));
        }
    }
    app.pane_map.insert(new_id, new_pane);
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
    app.status_message = Some(format!("{} {} item(s)", label, count));
}

fn paste_clipboard(app: &mut App) {
    let clipboard = match &app.file_clipboard {
        Some(c) => c,
        None => {
            app.status_message = Some("Nothing to paste".to_string());
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
        app.status_message = Some(format!("Pasted {} item(s)", entries.len()));
    } else {
        app.status_message = Some(format!("Paste errors: {}", errors.join(", ")));
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
    };
}

fn open_in_default_app(app: &mut App) {
    let pane_id = app.active_pane_id.clone();
    if let Some(pane) = app.pane_map.get(&pane_id) {
        if pane.focus_index >= 0 && (pane.focus_index as usize) < pane.entries.len() {
            let path = pane.entries[pane.focus_index as usize].path.clone();
            if let Err(e) = panex_core::open_entry(&path) {
                app.status_message = Some(format!("Open failed: {}", e));
            }
        }
    }
}

fn open_in_terminal(app: &mut App) {
    let pane_id = app.active_pane_id.clone();
    if let Some(pane) = app.pane_map.get(&pane_id) {
        if let Err(e) = panex_core::open_in_terminal(&pane.current_path) {
            app.status_message = Some(format!("Terminal failed: {}", e));
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
