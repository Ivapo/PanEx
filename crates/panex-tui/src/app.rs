use panex_core::FileEntry;
use ratatui::widgets::TableState;
use std::collections::{HashMap, HashSet};

use crate::layout::LayoutNode;
use crate::sort::{apply_sort_and_filter, SortDirection, SortField};

pub struct PaneState {
    pub current_path: String,
    pub entries: Vec<FileEntry>,
    pub selected_paths: HashSet<String>,
    pub focus_index: i32,
    pub search_query: String,
    pub table_state: TableState,
}

impl PaneState {
    pub fn new(path: &str) -> Self {
        Self {
            current_path: path.to_string(),
            entries: Vec::new(),
            selected_paths: HashSet::new(),
            focus_index: 0,
            search_query: String::new(),
            table_state: TableState::default().with_selected(Some(0)),
        }
    }
}

#[derive(PartialEq)]
pub enum AppMode {
    Normal,
    Search { pane_id: String },
    Rename {
        pane_id: String,
        path: String,
        input: String,
        cursor: usize,
    },
    Confirm {
        title: String,
        message: String,
        action: ConfirmAction,
    },
    Prompt {
        title: String,
        input: String,
        cursor: usize,
        action: PromptAction,
    },
    PathEdit {
        pane_id: String,
        input: String,
        cursor: usize,
    },
}

#[derive(PartialEq)]
pub enum ConfirmAction {
    Delete(Vec<String>),
}

#[derive(PartialEq)]
pub enum PromptAction {
    NewFile(String),
    NewFolder(String),
}

pub struct FileClipboard {
    pub entries: Vec<FileEntry>,
    pub mode: ClipMode,
}

#[derive(PartialEq)]
pub enum ClipMode {
    Copy,
    Cut,
}

pub struct App {
    pub layout_root: LayoutNode,
    pub pane_map: HashMap<String, PaneState>,
    pub active_pane_id: String,
    pub home_path: String,
    pub pane_counter: u32,
    pub show_hidden: bool,
    pub sort_field: SortField,
    pub sort_direction: SortDirection,
    pub file_clipboard: Option<FileClipboard>,
    pub raw_entries_map: HashMap<String, Vec<FileEntry>>,
    pub mode: AppMode,
    pub status_message: Option<String>,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Result<Self, String> {
        let home_path = panex_core::get_home_dir()?;
        let start_path = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| home_path.clone());
        let pane_id = "pane-1".to_string();

        let mut pane = PaneState::new(&start_path);

        let raw_entries = panex_core::read_directory(&start_path)?;
        let filtered = apply_sort_and_filter(&raw_entries, false, "", SortField::Name, SortDirection::Asc);
        pane.entries = filtered;

        let mut pane_map = HashMap::new();
        let mut raw_entries_map = HashMap::new();
        raw_entries_map.insert(pane_id.clone(), raw_entries);
        pane_map.insert(pane_id.clone(), pane);

        Ok(Self {
            layout_root: LayoutNode::Leaf {
                pane_id: pane_id.clone(),
            },
            pane_map,
            active_pane_id: pane_id,
            home_path,
            pane_counter: 1,
            show_hidden: false,
            sort_field: SortField::Name,
            sort_direction: SortDirection::Asc,
            file_clipboard: None,
            raw_entries_map,
            mode: AppMode::Normal,
            status_message: None,
            should_quit: false,
        })
    }

    pub fn navigate_to(&mut self, pane_id: &str, path: &str) {
        match panex_core::read_directory(path) {
            Ok(raw_entries) => {
                let filtered = apply_sort_and_filter(
                    &raw_entries,
                    self.show_hidden,
                    "",
                    self.sort_field,
                    self.sort_direction,
                );
                if let Some(pane) = self.pane_map.get_mut(pane_id) {
                    pane.current_path = path.to_string();
                    pane.entries = filtered;
                    pane.focus_index = if pane.entries.is_empty() { -1 } else { 0 };
                    pane.selected_paths.clear();
                    pane.search_query.clear();
                    pane.table_state.select(if pane.entries.is_empty() {
                        None
                    } else {
                        Some(0)
                    });
                }
                self.raw_entries_map.insert(pane_id.to_string(), raw_entries);
                self.status_message = None;
            }
            Err(e) => {
                self.status_message = Some(format!("Error: {}", e));
            }
        }
    }

    pub fn refresh_pane(&mut self, pane_id: &str) {
        let path = if let Some(pane) = self.pane_map.get(pane_id) {
            pane.current_path.clone()
        } else {
            return;
        };
        let search_query = self
            .pane_map
            .get(pane_id)
            .map(|p| p.search_query.clone())
            .unwrap_or_default();

        match panex_core::read_directory(&path) {
            Ok(raw_entries) => {
                let filtered = apply_sort_and_filter(
                    &raw_entries,
                    self.show_hidden,
                    &search_query,
                    self.sort_field,
                    self.sort_direction,
                );
                if let Some(pane) = self.pane_map.get_mut(pane_id) {
                    pane.entries = filtered;
                    // Clamp focus
                    if pane.entries.is_empty() {
                        pane.focus_index = -1;
                        pane.table_state.select(None);
                    } else if pane.focus_index >= pane.entries.len() as i32 {
                        pane.focus_index = pane.entries.len() as i32 - 1;
                        pane.table_state.select(Some(pane.focus_index as usize));
                    }
                }
                self.raw_entries_map.insert(pane_id.to_string(), raw_entries);
            }
            Err(e) => {
                self.status_message = Some(format!("Error: {}", e));
            }
        }
    }

    pub fn refilter_pane(&mut self, pane_id: &str) {
        let raw = self.raw_entries_map.get(pane_id).cloned().unwrap_or_default();
        let search_query = self
            .pane_map
            .get(pane_id)
            .map(|p| p.search_query.clone())
            .unwrap_or_default();

        let filtered = apply_sort_and_filter(
            &raw,
            self.show_hidden,
            &search_query,
            self.sort_field,
            self.sort_direction,
        );
        if let Some(pane) = self.pane_map.get_mut(pane_id) {
            pane.entries = filtered;
            if pane.entries.is_empty() {
                pane.focus_index = -1;
                pane.table_state.select(None);
            } else {
                pane.focus_index = 0;
                pane.table_state.select(Some(0));
            }
        }
    }

    pub fn next_pane_id(&mut self) -> String {
        self.pane_counter += 1;
        format!("pane-{}", self.pane_counter)
    }
}
