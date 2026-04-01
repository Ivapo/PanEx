use panex_core::FileEntry;
use panex_core::config::PanexConfig;

#[tauri::command]
pub fn read_dir(path: String) -> Result<Vec<FileEntry>, String> {
    panex_core::read_directory(&path)
}

#[tauri::command]
pub fn get_home_dir() -> Result<String, String> {
    panex_core::get_home_dir()
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
    let config = PanexConfig::load();
    if let Some(ext) = panex_core::get_extension(&path) {
        if let Some(app) = config.get_gui_app(&ext) {
            return panex_core::open_entry_with_app(&path, Some(app));
        }
    }
    panex_core::open_entry(&path)
}

#[tauri::command]
pub fn rename_entry(path: String, new_name: String) -> Result<(), String> {
    panex_core::rename_entry(&path, &new_name)
}

#[tauri::command]
pub fn delete_entry(path: String, permanent: Option<bool>) -> Result<(), String> {
    panex_core::delete_entry(&path, permanent.unwrap_or(false))
}

#[tauri::command]
pub fn copy_entry(source: String, dest_dir: String) -> Result<String, String> {
    panex_core::copy_entry(&source, &dest_dir)
}

#[tauri::command]
pub fn move_entry(source: String, dest_dir: String) -> Result<String, String> {
    panex_core::move_entry(&source, &dest_dir)
}

#[tauri::command]
pub fn calculate_dir_size(path: String) -> Result<u64, String> {
    panex_core::calculate_directory_size(&path)
}

#[tauri::command]
pub fn create_file(dir: String, name: String) -> Result<(), String> {
    panex_core::create_file(&dir, &name)
}

#[tauri::command]
pub fn create_folder(dir: String, name: String) -> Result<(), String> {
    panex_core::create_folder(&dir, &name)
}

#[tauri::command]
pub fn open_in_terminal(path: String) -> Result<(), String> {
    panex_core::open_in_terminal(&path)
}

#[tauri::command]
pub fn get_favorites() -> Vec<String> {
    PanexConfig::load().favorites.paths
}

#[tauri::command]
pub fn is_favorite(path: String) -> bool {
    PanexConfig::load().is_favorite(&path)
}

#[tauri::command]
pub fn toggle_favorite(path: String) -> Result<bool, String> {
    let mut config = PanexConfig::load();
    config.toggle_favorite(&path)
}
