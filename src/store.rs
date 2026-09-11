//! Portable file-backed storage — replaces `DatabaseManager` (SQLite) and
//! `PresetStore` with plain JSON files so the app stays fully self-contained.
//!
//! Layout (next to the executable, or in dev next to the project):
//!
//! ```text
//! <app_dir>/
//! ├── data/
//! │   ├── sounds.json      — sound library
//! │   ├── categories.json  — categories
//! │   └── settings.json    — app settings
//! └── sounds/              — audio files copied in from wherever they were dropped
//! ```

use crate::models::{AppSettings, SoundCategory, SoundFile, now_ms};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub struct Store {
    /// Root directory (next to the exe in release, project root in dev).
    app_dir: PathBuf,
    pub sounds: Vec<SoundFile>,
    pub categories: Vec<SoundCategory>,
    pub settings: AppSettings,
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    Ok(Some(serde_json::from_str(&text).with_context(|| {
        format!("parsing {}", path.display())
    })?))
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)?;
    // Write via temp file + rename for atomicity.
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

impl Store {
    /// Open (or create) the store. Seeds default categories on first run.
    pub fn init() -> Result<Self> {
        let app_dir = Self::resolve_app_dir();
        fs::create_dir_all(app_dir.join("data"))?;
        fs::create_dir_all(app_dir.join("sounds"))?;

        let sounds: Vec<SoundFile> =
            read_json(&app_dir.join("data/sounds.json"))?.unwrap_or_default();
        let categories: Vec<SoundCategory> =
            read_json(&app_dir.join("data/categories.json"))?.unwrap_or_default();
        let settings: AppSettings =
            read_json(&app_dir.join("data/settings.json"))?.unwrap_or_default();

        let mut store = Self {
            app_dir,
            sounds,
            categories,
            settings,
        };

        if store.categories.is_empty() {
            store.categories = default_categories();
            store.save_categories()?;
        }
        Ok(store)
    }

    /// Executable dir in release; current dir when run via `cargo run`.
    fn resolve_app_dir() -> PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            // In dev builds (target/debug|release) fall back to the project root.
            if exe.parent().map_or(false, |p| {
                p.file_name().map_or(false, |n| {
                    n == "debug" || n == "release" || n == "deps"
                })
            }) {
                if let Ok(cwd) = std::env::current_dir() {
                    return cwd;
                }
            }
            if let Some(parent) = exe.parent() {
                return parent.to_path_buf();
            }
        }
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    pub fn sounds_dir(&self) -> PathBuf {
        self.app_dir.join("sounds")
    }

    // ── Sounds ──

    pub fn save_sounds(&self) -> Result<()> {
        write_json(&self.app_dir.join("data/sounds.json"), &self.sounds)
    }

    pub fn add_sound(&mut self, sound: SoundFile) -> Result<()> {
        self.sounds.push(sound);
        self.save_sounds()
    }

    pub fn update_sound(&mut self, updated: SoundFile) -> Result<()> {
        if let Some(s) = self.sounds.iter_mut().find(|s| s.id == updated.id) {
            *s = updated;
        }
        self.save_sounds()
    }

    pub fn remove_sound(&mut self, sound_id: &str) -> Result<()> {
        self.sounds.retain(|s| s.id != sound_id);
        self.save_sounds()
    }

    pub fn get_sound(&self, sound_id: &str) -> Option<&SoundFile> {
        self.sounds.iter().find(|s| s.id == sound_id)
    }

    // ── Categories ──

    pub fn save_categories(&self) -> Result<()> {
        write_json(&self.app_dir.join("data/categories.json"), &self.categories)
    }

    pub fn add_category(&mut self, category: SoundCategory) -> Result<()> {
        self.categories.push(category);
        self.save_categories()
    }

    pub fn remove_category(&mut self, category_id: &str) -> Result<()> {
        self.categories.retain(|c| c.id != category_id);
        // Move sounds from the deleted category to "default".
        for s in &mut self.sounds {
            if s.category_id == category_id {
                s.category_id = "default".to_string();
            }
        }
        self.save_categories()?;
        self.save_sounds()
    }

    // ── Settings ──

    pub fn save_settings(&self) -> Result<()> {
        write_json(&self.app_dir.join("data/settings.json"), &self.settings)
    }

    // ── Presets (portable JSON files in ~/.soundpad-rs/presets) ──

    pub fn presets_dir(&self) -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".soundpad")
            .join("presets")
    }

    pub fn list_presets(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.presets_dir())
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map_or(false, |x| x == "json"))
                    .filter_map(|e| {
                        e.file_name().to_string_lossy().strip_suffix(".json").map(String::from)
                    })
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    pub fn save_preset(&self, name: &str, description: &str) -> Result<()> {
        let preset = crate::models::Preset {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: description.to_string(),
            sounds: self.sounds.clone(),
            categories: self.categories.clone(),
            created_at: now_ms(),
            updated_at: now_ms(),
        };
        let path = self.presets_dir().join(format!("{}.json", sanitize(name)));
        write_json(&path, &preset)?;
        Ok(())
    }

    pub fn load_preset(&mut self, name: &str) -> Result<()> {
        let path = self.presets_dir().join(format!("{}.json", sanitize(name)));
        let preset: crate::models::Preset =
            read_json(&path)?.context("preset not found")?;
        self.sounds = preset.sounds;
        if !preset.categories.is_empty() {
            self.categories = preset.categories;
        }
        self.save_sounds()?;
        self.save_categories()?;
        Ok(())
    }

    pub fn delete_preset(&self, name: &str) -> Result<()> {
        let path = self.presets_dir().join(format!("{}.json", sanitize(name)));
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Copy an audio file into the portable `sounds/` directory and return the new path.
    pub fn copy_sound_portable(&self, source: &Path) -> Result<PathBuf> {
        let dir = self.sounds_dir();
        fs::create_dir_all(&dir)?;
        let file_name = source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("sound_{}", now_ms()));
        let target = dir.join(format!("{}_{}", now_ms(), file_name));
        fs::copy(source, &target)
            .with_context(|| format!("copying {} -> {}", source.display(), target.display()))?;
        Ok(target)
    }
}

/// `name.json` → `name_json`-style safe file name.
fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .to_lowercase()
}

/// The default category set (mirrors the original app).
pub fn default_categories() -> Vec<SoundCategory> {
    vec![
        SoundCategory::new("default", "All Sounds", "folder", 0),
        SoundCategory::new("memes", "Memes", "emoji", 1),
        SoundCategory::new("alerts", "Alerts", "notifications", 2),
        SoundCategory::new("music", "Music", "music", 3),
        SoundCategory::new("voice", "Voice", "star", 4),
        SoundCategory::new("sfx", "Sound Effects", "sfx", 5),
        SoundCategory::new("custom", "Custom", "star", 6),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_special_chars() {
        assert_eq!(sanitize("My Preset! v2"), "my_preset__v2");
    }

    #[test]
    fn sound_file_extension_uppercases() {
        let s = SoundFile::new("test", "foo/bar.mp3", "default");
        assert_eq!(s.extension(), "MP3");
    }

    #[test]
    fn store_roundtrip_settings() {
        let dir = std::env::temp_dir().join(format!("soundpad_test_{}", std::process::id()));
        let data = dir.join("data");
        fs::create_dir_all(&data).unwrap();
        let settings_path = data.join("settings.json");
        let s = AppSettings {
            master_volume: 0.55,
            ..Default::default()
        };
        write_json(&settings_path, &s).unwrap();
        let back: AppSettings = read_json(&settings_path).unwrap().unwrap();
        assert!((back.master_volume - 0.55).abs() < f32::EPSILON);
        fs::remove_dir_all(&dir).ok();
    }
}
