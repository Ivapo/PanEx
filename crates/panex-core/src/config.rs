use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PanexConfig {
    #[serde(default)]
    pub favorites: FavoritesConfig,
    #[serde(default)]
    pub open: OpenConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FavoritesConfig {
    #[serde(default)]
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct OpenConfig {
    #[serde(default)]
    pub gui: HashMap<String, String>,
    #[serde(default)]
    pub tui: HashMap<String, String>,
}

impl PanexConfig {
    /// Returns ~/.panex/config.toml
    pub fn config_path() -> Result<PathBuf, String> {
        let home = dirs::home_dir().ok_or("Could not determine home directory")?;
        Ok(home.join(".panex").join("config.toml"))
    }

    /// Load config from disk, returning defaults if file doesn't exist.
    pub fn load() -> Self {
        let path = match Self::config_path() {
            Ok(p) => p,
            Err(_) => return Self::default(),
        };
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        toml::from_str(&content).unwrap_or_default()
    }

    /// Save config to disk, creating ~/.panex/ if needed.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }
        let content =
            toml::to_string_pretty(self).map_err(|e| format!("Failed to serialize config: {}", e))?;
        fs::write(&path, content).map_err(|e| format!("Failed to write config: {}", e))
    }

    pub fn is_favorite(&self, path: &str) -> bool {
        let normalized = normalize_path(path);
        self.favorites.paths.iter().any(|p| normalize_path(p) == normalized)
    }

    pub fn add_favorite(&mut self, path: &str) -> Result<(), String> {
        let normalized = normalize_path(path);
        if !self.is_favorite(&normalized) {
            self.favorites.paths.push(normalized);
            self.save()?;
        }
        Ok(())
    }

    pub fn remove_favorite(&mut self, path: &str) -> Result<(), String> {
        let normalized = normalize_path(path);
        self.favorites.paths.retain(|p| normalize_path(p) != normalized);
        self.save()
    }

    pub fn toggle_favorite(&mut self, path: &str) -> Result<bool, String> {
        if self.is_favorite(path) {
            self.remove_favorite(path)?;
            Ok(false)
        } else {
            self.add_favorite(path)?;
            Ok(true)
        }
    }

    /// Get the custom app for a file extension in GUI mode.
    pub fn get_gui_app(&self, ext: &str) -> Option<&String> {
        let key = if ext.starts_with('.') {
            ext.to_string()
        } else {
            format!(".{}", ext)
        };
        self.open.gui.get(&key)
    }

    /// Get the custom command for a file extension in TUI mode.
    pub fn get_tui_app(&self, ext: &str) -> Option<&String> {
        let key = if ext.starts_with('.') {
            ext.to_string()
        } else {
            format!(".{}", ext)
        };
        self.open.tui.get(&key)
    }
}

/// Expand ~ to home dir for comparison, and strip trailing slashes.
fn normalize_path(path: &str) -> String {
    let expanded = if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            path.replacen('~', &home.to_string_lossy(), 1)
        } else {
            path.to_string()
        }
    } else {
        path.to_string()
    };
    expanded.trim_end_matches('/').to_string()
}
