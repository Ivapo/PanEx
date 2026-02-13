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
            size: disk_size(&metadata),
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

pub fn delete_entry(path: &str, permanent: bool) -> Result<(), String> {
    let target = PathBuf::from(path);
    if !target.exists() {
        return Err(format!("Path does not exist: {}", path));
    }

    if permanent {
        if target.is_dir() {
            fs::remove_dir_all(&target).map_err(|e| format!("Failed to delete: {}", e))
        } else {
            fs::remove_file(&target).map_err(|e| format!("Failed to delete: {}", e))
        }
    } else {
        trash::delete(&target).map_err(|e| format!("Failed to move to trash: {}", e))
    }
}

pub fn copy_entry(source: &str, dest_dir: &str) -> Result<String, String> {
    let src = PathBuf::from(source);
    if !src.exists() {
        return Err(format!("Source does not exist: {}", source));
    }
    let dest = PathBuf::from(dest_dir);
    if !dest.is_dir() {
        return Err(format!("Destination is not a directory: {}", dest_dir));
    }

    let file_name = src
        .file_name()
        .ok_or_else(|| "Cannot determine file name".to_string())?;
    let dest_path = dest.join(file_name);

    if src.is_dir() {
        copy_dir_recursive(&src, &dest_path)?;
    } else {
        fs::copy(&src, &dest_path).map_err(|e| format!("Failed to copy file: {}", e))?;
    }

    Ok(dest_path.to_string_lossy().to_string())
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("Failed to create directory: {}", e))?;

    let entries =
        fs::read_dir(src).map_err(|e| format!("Failed to read source directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let entry_dest = dest.join(entry.file_name());

        if entry.path().is_dir() {
            copy_dir_recursive(&entry.path(), &entry_dest)?;
        } else {
            fs::copy(entry.path(), &entry_dest)
                .map_err(|e| format!("Failed to copy file: {}", e))?;
        }
    }

    Ok(())
}

pub fn calculate_directory_size(path: &str) -> Result<u64, String> {
    let dir_path = Path::new(path);
    if !dir_path.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }

    fn walk(dir: &Path) -> u64 {
        let mut total: u64 = 0;
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Ok(meta) = entry.metadata() {
                        if meta.is_dir() {
                            total += walk(&entry.path());
                        } else {
                            total += disk_size(&meta);
                        }
                    }
                }
            }
        }
        total
    }

    Ok(walk(dir_path))
}

/// Returns actual disk usage (blocks * 512) on Unix, logical size on Windows.
#[cfg(unix)]
fn disk_size(meta: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.blocks() * 512
}

#[cfg(not(unix))]
fn disk_size(meta: &fs::Metadata) -> u64 {
    meta.len()
}

pub fn create_file(dir: &str, name: &str) -> Result<(), String> {
    let path = Path::new(dir).join(name);
    if path.exists() {
        return Err(format!("A file named '{}' already exists", name));
    }
    fs::File::create(&path).map_err(|e| format!("Failed to create file: {}", e))?;
    Ok(())
}

pub fn create_folder(dir: &str, name: &str) -> Result<(), String> {
    let path = Path::new(dir).join(name);
    if path.exists() {
        return Err(format!("A folder named '{}' already exists", name));
    }
    fs::create_dir(&path).map_err(|e| format!("Failed to create folder: {}", e))?;
    Ok(())
}

pub fn open_in_terminal(path: &str) -> Result<(), String> {
    let dir = Path::new(path);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }

    #[cfg(target_os = "macos")]
    {
        // Prefer iTerm2 if installed, fall back to Terminal.app
        let app = if Path::new("/Applications/iTerm.app").exists() {
            "iTerm"
        } else {
            "Terminal"
        };
        std::process::Command::new("open")
            .args(["-a", app, path])
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "cmd", "/K", &format!("cd /d {}", path)])
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Try common terminal emulators in order
        let terminals = ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"];
        let mut launched = false;
        for term in &terminals {
            let result = if *term == "gnome-terminal" {
                std::process::Command::new(term)
                    .arg("--working-directory")
                    .arg(path)
                    .spawn()
            } else {
                std::process::Command::new(term)
                    .current_dir(path)
                    .spawn()
            };
            if result.is_ok() {
                launched = true;
                break;
            }
        }
        if !launched {
            return Err("No supported terminal emulator found".to_string());
        }
    }

    Ok(())
}

pub fn move_entry(source: &str, dest_dir: &str) -> Result<String, String> {
    let src = PathBuf::from(source);
    if !src.exists() {
        return Err(format!("Source does not exist: {}", source));
    }
    let dest = PathBuf::from(dest_dir);
    if !dest.is_dir() {
        return Err(format!("Destination is not a directory: {}", dest_dir));
    }

    let file_name = src
        .file_name()
        .ok_or_else(|| "Cannot determine file name".to_string())?;
    let dest_path = dest.join(file_name);

    // Try fast rename first (works on same volume)
    match fs::rename(&src, &dest_path) {
        Ok(()) => return Ok(dest_path.to_string_lossy().to_string()),
        Err(_) => {
            // Cross-volume: copy then delete
            copy_entry(source, dest_dir)?;
            if src.is_dir() {
                fs::remove_dir_all(&src)
                    .map_err(|e| format!("Copied but failed to remove source: {}", e))?;
            } else {
                fs::remove_file(&src)
                    .map_err(|e| format!("Copied but failed to remove source: {}", e))?;
            }
            Ok(dest_path.to_string_lossy().to_string())
        }
    }
}
