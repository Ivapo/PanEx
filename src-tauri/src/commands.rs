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

#[tauri::command]
pub fn open_entry(path: String) -> Result<(), String> {
    fs_ops::open_entry(&path)
}

#[tauri::command]
pub fn rename_entry(path: String, new_name: String) -> Result<(), String> {
    fs_ops::rename_entry(&path, &new_name)
}

#[tauri::command]
pub fn delete_entry(path: String) -> Result<(), String> {
    fs_ops::delete_entry(&path)
}

#[tauri::command]
pub fn copy_entry(source: String, dest_dir: String) -> Result<String, String> {
    fs_ops::copy_entry(&source, &dest_dir)
}

#[tauri::command]
pub fn move_entry(source: String, dest_dir: String) -> Result<String, String> {
    fs_ops::move_entry(&source, &dest_dir)
}

#[tauri::command]
pub fn calculate_dir_size(path: String) -> Result<u64, String> {
    fs_ops::calculate_directory_size(&path)
}
