use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Serialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
}

pub fn read_directory(path: &str) -> Result<Vec<FileEntry>, String> {
    let dir_path = Path::new(path);
    if !dir_path.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }

    let mut entries = Vec::new();

    let read_dir = fs::read_dir(dir_path).map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in read_dir {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let metadata = entry
            .metadata()
            .map_err(|e| format!("Failed to read metadata: {}", e))?;

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        entries.push(FileEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            path: entry.path().to_string_lossy().to_string(),
            is_dir: metadata.is_dir(),
            size: metadata.len(),
            modified,
        });
    }

    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

pub fn rename_entry(path: &str, new_name: &str) -> Result<(), String> {
    let source = PathBuf::from(path);
    if !source.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    let parent = source
        .parent()
        .ok_or_else(|| "Cannot determine parent directory".to_string())?;
    let dest = parent.join(new_name);

    if dest.exists() {
        return Err(format!("A file named '{}' already exists", new_name));
    }

    fs::rename(&source, &dest).map_err(|e| format!("Failed to rename: {}", e))
}

pub fn open_entry(path: &str) -> Result<(), String> {
    let target = std::path::PathBuf::from(path);
    if !target.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", path])
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("Failed to open: {}", e))?;
    }

    Ok(())
}

pub fn delete_entry(path: &str) -> Result<(), String> {
    let target = PathBuf::from(path);
    if !target.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    trash::delete(&target).map_err(|e| format!("Failed to move to trash: {}", e))
}
