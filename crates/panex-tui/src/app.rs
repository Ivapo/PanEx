use panex_core::FileEntry;
use panex_core::config::PanexConfig;
use ratatui::layout::Rect;
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
    /// Size level on each axis: -1 smaller, 0 default, +1 larger. Tracked per
    /// axis because a sibling can reclaim one axis without touching the other.
    pub width_level: i8,
    pub height_level: i8,
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
            width_level: 0,
            height_level: 0,
        }
    }
}

/// Screen regions of a pane from the last render, used for mouse hit-testing
/// and page-size calculations.
#[derive(Clone, Copy, Default)]
pub struct PaneView {
    pub area: Rect,
    pub list_area: Rect,
}

#[derive(PartialEq)]
pub enum AppMode {
    Normal,
    Help,
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
        selected: usize, // 0 = Yes, 1 = No
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
        completions: Vec<String>,
        completion_index: Option<usize>,
        completion_prefix: String,
    },
    FavoritesList {
        pane_id: String,
        selected: usize,
    },
}

#[derive(PartialEq)]
pub enum ConfirmAction {
    Delete(Vec<String>),
}

#[derive(PartialEq, Clone, Debug)]
pub enum PromptAction {
    NewFile(String),
    NewFolder(String),
    /// Name the iTerm2 session behind a card. Carries the session id, so a
    /// tab closing while the prompt is open cannot land the name on a
    /// neighbour — it fails instead.
    RenameTab(String),
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
    pub pane_views: HashMap<String, PaneView>,
    pub mode: AppMode,
    pub status_message: Option<String>,
    pub status_message_at: Option<std::time::Instant>,
    pub should_quit: bool,
    pub config: PanexConfig,
    /// Pane, row and time of the last left click, so the next one can be
    /// recognised as the second half of a double click. Crossterm reports
    /// button presses only — pairing them is ours to do.
    pub last_click: Option<(String, usize, std::time::Instant)>,
    /// Whether a usable oko is on PATH, decided once at startup. False means
    /// the shortcut is not bound and the help overlay does not list it — a
    /// key that does nothing reads as a broken keyboard.
    pub oko_available: bool,
    /// The one leaf currently drawing cards, if any. At most one: the view
    /// reports on the whole window, so a second would draw the same thing
    /// twice and hold a second iTerm2 connection to do it.
    pub oko_pane_id: Option<String>,
    pub oko_stream: Option<crate::oko::Stream>,
    pub oko_view: crate::oko::View,
    /// Which card is selected, held as a session id rather than a position.
    /// Tabs open and close under the cursor, and a position would quietly
    /// re-point Enter or a rename at whichever row slid into that slot.
    pub oko_selected: Option<String>,
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

        let config = PanexConfig::load();

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
            pane_views: HashMap::new(),
            mode: AppMode::Normal,
            status_message: None,
            status_message_at: None,
            should_quit: false,
            config,
            last_click: None,
            oko_available: crate::oko::is_available(),
            oko_pane_id: None,
            oko_stream: None,
            oko_view: crate::oko::View::Connecting,
            oko_selected: None,
        })
    }

    /// Take whatever the oko reader has queued. Returns true if the cards
    /// changed, so the caller redraws only then — the stream is already quiet
    /// by design, and an identical snapshot should not cost a frame.
    pub fn pump_oko(&mut self) -> bool {
        let events = match &self.oko_stream {
            Some(stream) => stream.drain(),
            None => return false,
        };

        let mut changed = false;
        for event in events {
            match event {
                crate::oko::Event::Rows(rows) => {
                    let same = matches!(&self.oko_view, crate::oko::View::Rows(shown) if *shown == rows);
                    if !same {
                        // Keep the selection on the session it was on. It only
                        // moves when that tab has gone, and then to the first
                        // row rather than to whatever took its place.
                        let still_there = self
                            .oko_selected
                            .as_deref()
                            .is_some_and(|id| rows.iter().any(|row| row.session_id == id));
                        if !still_there {
                            self.oko_selected = rows.first().map(|row| row.session_id.clone());
                        }
                        self.oko_view = crate::oko::View::Rows(rows);
                        changed = true;
                    }
                }
                crate::oko::Event::Lost(message) => {
                    // Terminal: nothing more is coming down this stream, so
                    // drop it rather than polling a dead channel forever.
                    self.oko_view = crate::oko::View::Lost(message);
                    self.oko_stream = None;
                    changed = true;
                }
            }
        }
        changed
    }

    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some(msg);
        self.status_message_at = Some(std::time::Instant::now());
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
                self.set_status(format!("Error: {}", e));
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
                self.set_status(format!("Error: {}", e));
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
