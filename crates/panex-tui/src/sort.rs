use panex_core::FileEntry;
use std::cmp::Ordering;

#[derive(Clone, Copy, PartialEq)]
pub enum SortField {
    Name,
    Extension,
    Size,
    Modified,
}

impl SortField {
    pub fn label(self) -> &'static str {
        match self {
            SortField::Name => "Name",
            SortField::Extension => "Ext",
            SortField::Size => "Size",
            SortField::Modified => "Modified",
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            SortField::Name => SortField::Extension,
            SortField::Extension => SortField::Size,
            SortField::Size => SortField::Modified,
            SortField::Modified => SortField::Name,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    pub fn toggle(self) -> Self {
        match self {
            SortDirection::Asc => SortDirection::Desc,
            SortDirection::Desc => SortDirection::Asc,
        }
    }

    pub fn indicator(self) -> &'static str {
        match self {
            SortDirection::Asc => "▲",
            SortDirection::Desc => "▼",
        }
    }
}

fn extension_of(name: &str) -> &str {
    name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("")
}

pub fn sort_entries(entries: &mut [FileEntry], field: SortField, direction: SortDirection) {
    entries.sort_by(|a, b| {
        // Dirs always before files
        if a.is_dir != b.is_dir {
            return if a.is_dir {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }

        let cmp = match field {
            SortField::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortField::Extension => {
                let ea = extension_of(&a.name).to_lowercase();
                let eb = extension_of(&b.name).to_lowercase();
                ea.cmp(&eb).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            }
            SortField::Size => a.size.cmp(&b.size),
            SortField::Modified => a.modified.cmp(&b.modified),
        };

        match direction {
            SortDirection::Asc => cmp,
            SortDirection::Desc => cmp.reverse(),
        }
    });
}

pub fn filter_hidden(entries: &[FileEntry], show_hidden: bool) -> Vec<FileEntry> {
    if show_hidden {
        entries.to_vec()
    } else {
        entries
            .iter()
            .filter(|e| !e.name.starts_with('.'))
            .cloned()
            .collect()
    }
}

pub fn filter_search(entries: &[FileEntry], query: &str) -> Vec<FileEntry> {
    if query.is_empty() {
        entries.to_vec()
    } else {
        let q = query.to_lowercase();
        entries
            .iter()
            .filter(|e| e.name.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }
}

pub fn apply_sort_and_filter(
    raw: &[FileEntry],
    show_hidden: bool,
    search_query: &str,
    field: SortField,
    direction: SortDirection,
) -> Vec<FileEntry> {
    let after_hidden = filter_hidden(raw, show_hidden);
    let after_search = filter_search(&after_hidden, search_query);
    let mut result = after_search;
    sort_entries(&mut result, field, direction);
    result
}
