//! Core data models — Rust port of `model/Models.kt`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Serde default for id fields: random UUID string.
fn gen_id() -> String {
    Uuid::new_v4().to_string()
}

/// Timestamp in milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// A single sound file in the soundpad.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundFile {
    #[serde(default = "gen_id")]
    pub id: String,
    pub name: String,
    pub file_path: PathBuf,
    /// Duration in seconds.
    #[serde(default)]
    pub duration: f32,
    /// Per-sound volume 0.0..=1.0.
    #[serde(default = "default_volume")]
    pub volume: f32,
    /// e.g. "F1", "Ctrl+Shift+1".
    #[serde(default)]
    pub hotkey: Option<String>,
    #[serde(default = "default_category")]
    pub category_id: String,
    #[serde(default)]
    pub looped: bool,
    #[serde(default = "now_ms")]
    pub created_at: i64,
    #[serde(default)]
    pub hotkey_enabled: bool,
}

fn default_volume() -> f32 {
    1.0
}

fn default_category() -> String {
    "default".to_string()
}

impl SoundFile {
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>, category_id: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            file_path: path.into(),
            duration: 0.0,
            volume: 1.0,
            hotkey: None,
            category_id: category_id.to_string(),
            looped: false,
            created_at: now_ms(),
            hotkey_enabled: false,
        }
    }

    /// Uppercase file extension, e.g. "MP3".
    pub fn extension(&self) -> String {
        self.file_path
            .extension()
            .map(|e| e.to_string_lossy().to_uppercase())
            .unwrap_or_default()
    }
}

/// Category for organizing sounds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundCategory {
    #[serde(default = "gen_id")]
    pub id: String,
    pub name: String,
    /// Built-in icon name ("folder", "emoji", "notifications", "music", "star", ...).
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default = "default_color")]
    pub color: u32,
    #[serde(default)]
    pub order: i32,
    /// Path to a custom icon image.
    #[serde(default)]
    pub custom_icon_path: Option<PathBuf>,
}

fn default_icon() -> String {
    "folder".to_string()
}

fn default_color() -> u32 {
    0xFF6750A4
}

impl SoundCategory {
    pub fn new(id: impl Into<String>, name: impl Into<String>, icon: &str, order: i32) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            icon: icon.to_string(),
            color: 0xFF6750A4,
            order,
            custom_icon_path: None,
        }
    }
}

/// A saved collection of sounds and categories, stored as a JSON file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    #[serde(default = "gen_id")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub sounds: Vec<SoundFile>,
    #[serde(default)]
    pub categories: Vec<SoundCategory>,
    #[serde(default = "now_ms")]
    pub created_at: i64,
    #[serde(default = "now_ms")]
    pub updated_at: i64,
}

/// Application settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// "default" or an output device name.
    pub output_device: String,
    pub master_volume: f32,
    pub dark_theme: bool,
    pub hotkeys_enabled: bool,
    pub minimize_to_tray: bool,
    pub auto_start: bool,
    pub virtual_cable_enabled: bool,
    pub grid_columns: usize,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            output_device: "default".to_string(),
            master_volume: 0.8,
            dark_theme: true,
            hotkeys_enabled: true,
            minimize_to_tray: false,
            auto_start: false,
            virtual_cable_enabled: false,
            grid_columns: 4,
        }
    }
}

/// Playback state of a sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}
