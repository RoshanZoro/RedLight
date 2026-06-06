//! Persistent settings stored at `%APPDATA%\RedLight\config.json`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    /// Turn the red filter on automatically when the app launches.
    pub auto_on_start: bool,
    /// Whether the app is registered to start with Windows (mirrors the
    /// registry Run key; the registry is the source of truth).
    pub start_on_boot: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_on_start: false,
            start_on_boot: false,
        }
    }
}

impl Config {
    fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("RedLight").join("config.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, text);
        }
    }
}
