use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

// ---------------------------------------------------------------------------
// Last-view snapshot
// ---------------------------------------------------------------------------

/// Minimal state captured so the app can restore its previous view on startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastView {
    pub mode: String,
    pub center_re: f64,
    pub center_im: f64,
    pub scale: f64,
    pub max_iterations: u32,
    pub escape_radius: f64,
    pub palette_index: usize,
    pub smooth_coloring: bool,
    pub aa_level: u32,
    pub julia_c_re: f64,
    pub julia_c_im: f64,
}

// ---------------------------------------------------------------------------
// Application preferences
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPreferences {
    #[serde(default = "default_window_width")]
    pub window_width: f32,
    #[serde(default = "default_window_height")]
    pub window_height: f32,
    #[serde(default = "default_max_iterations")]
    pub default_max_iterations: u32,
    #[serde(default)]
    pub default_palette_index: usize,
    #[serde(default = "default_true")]
    pub restore_last_view: bool,
    #[serde(default)]
    pub last_view: Option<LastView>,
    /// Custom bookmarks directory. When empty, the default OS config path is used.
    #[serde(default)]
    pub bookmarks_dir: String,
}

fn default_window_width() -> f32 {
    1280.0
}
fn default_window_height() -> f32 {
    720.0
}
fn default_max_iterations() -> u32 {
    256
}
fn default_true() -> bool {
    true
}

impl Default for AppPreferences {
    fn default() -> Self {
        Self {
            window_width: default_window_width(),
            window_height: default_window_height(),
            default_max_iterations: default_max_iterations(),
            default_palette_index: 0,
            restore_last_view: true,
            last_view: None,
            bookmarks_dir: String::new(),
        }
    }
}

impl AppPreferences {
    /// Load preferences from the OS config directory, falling back to defaults.
    pub fn load() -> Self {
        let path = config_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(json) => match serde_json::from_str::<AppPreferences>(&json) {
                    Ok(prefs) => {
                        info!("Loaded preferences from {}", path.display());
                        return prefs;
                    }
                    Err(e) => {
                        error!("Failed to parse preferences: {e}");
                    }
                },
                Err(e) => {
                    error!("Failed to read preferences file: {e}");
                }
            }
        } else {
            debug!("No preferences file at {}", path.display());
        }
        Self::default()
    }

    /// Persist preferences to disk.
    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                error!("Failed to create config directory: {e}");
                return;
            }
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = fs::write(&path, &json) {
                    error!("Failed to write preferences: {e}");
                } else {
                    debug!("Saved preferences");
                }
            }
            Err(e) => error!("Failed to serialize preferences: {e}"),
        }
    }
}

fn config_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "MandelbRust")
        .map(|d| d.config_dir().join("preferences.json"))
        .unwrap_or_else(|| PathBuf::from("preferences.json"))
}
