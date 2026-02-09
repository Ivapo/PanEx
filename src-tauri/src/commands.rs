use crate::fs_ops::{self, FileEntry};

#[tauri::command]
pub fn read_dir(path: String) -> Result<Vec<FileEntry>, String> {
    fs_ops::read_directory(&path)
}

#[tauri::command]
pub fn get_home_dir() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Could not determine home directory".to_string())
}

#[tauri::command]
pub fn get_parent_dir(path: String) -> Result<String, String> {
    std::path::Path::new(&path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "No parent directory".to_string())
}
