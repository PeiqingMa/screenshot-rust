use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Hotkey binding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyBinding {
    /// Whether Ctrl modifier is required
    pub ctrl: bool,
    /// Whether Shift modifier is required
    pub shift: bool,
    /// Whether Alt modifier is required
    pub alt: bool,
    /// The virtual key code (e.g., 'S' = 0x53)
    pub key: u8,
    /// Human-readable description
    pub description: String,
}

impl Default for HotkeyBinding {
    fn default() -> Self {
        Self {
            ctrl: true,
            shift: true,
            alt: false,
            key: 0x53, // 'S' key
            description: "Ctrl+Shift+S".to_string(),
        }
    }
}

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Global hotkey binding for triggering capture
    pub hotkey: HotkeyBinding,
    /// Default save directory (None = ask every time)
    pub save_directory: Option<String>,
    /// Default image format for saving
    pub save_format: String,
    /// Whether to copy to clipboard after capture
    pub auto_copy: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: HotkeyBinding::default(),
            save_directory: None,
            save_format: "png".to_string(),
            auto_copy: false,
        }
    }
}

impl Config {
    /// Get the configuration file path (in %APPDATA%\RustShot\)
    fn config_path() -> PathBuf {
        let base = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let mut path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
                path.pop();
                path
            });
        let dir = base.join("RustShot");
        // Ensure directory exists (ignore error if it already does)
        let _ = fs::create_dir_all(&dir);
        dir.join("config.json")
    }

    /// Load configuration from disk, or return defaults if file doesn't exist
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => {
                        eprintln!("Failed to parse config file: {}", e);
                    }
                },
                Err(e) => {
                    eprintln!("Failed to read config file: {}", e);
                }
            }
        }
        Config::default()
    }

    /// Save current configuration to disk
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}
