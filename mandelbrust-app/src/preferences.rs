use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

use crate::display_color::DisplayColorSettings;

// ---------------------------------------------------------------------------
// Last-view snapshot
// ---------------------------------------------------------------------------

/// Minimal state captured so the app can restore its previous view on startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastView {
    pub mode: String,
    pub center_re: f64,
    pub center_im: f64,
    /// Low-order bits for double-double center precision (~31 digits total).
    #[serde(default)]
    pub center_re_lo: f64,
    /// Low-order bits for double-double center precision (~31 digits total).
    #[serde(default)]
    pub center_im_lo: f64,
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
    /// Full display/color settings from last session. Restored on startup so palette mode, start-from, etc. persist.
    #[serde(default)]
    pub last_display_color: Option<DisplayColorSettings>,
    /// Custom bookmarks directory. When empty, a `bookmarks/` folder next to the executable is used.
    #[serde(default)]
    pub bookmarks_dir: String,

    // Phase 9: Minimap
    #[serde(default = "default_true")]
    pub show_minimap: bool,
    /// Minimap side length: Small=128, Medium=256, Large=384.
    #[serde(default)]
    pub minimap_size: MinimapSize,
    /// Half-extent of complex-plane range (e.g. 2.0 → -2..2). Configurable zoom.
    #[serde(default = "default_minimap_zoom")]
    pub minimap_zoom_half_extent: f64,
    #[serde(default = "default_minimap_iterations")]
    pub minimap_iterations: u32,
    /// Minimap panel opacity 0.0..=1.0 (default 0.75). Applied to background and image.
    #[serde(default = "default_minimap_opacity")]
    pub minimap_opacity: f32,
    /// Crosshair lines opacity 0.0..=1.0 (default 0.5).
    #[serde(default = "default_crosshair_opacity")]
    pub crosshair_opacity: f32,
    /// HUD panel background opacity 0.0..=1.0 (default 0.65). Excludes toolbar.
    #[serde(default = "default_hud_panel_opacity")]
    pub hud_panel_opacity: f32,

    // Phase 10: Julia C Explorer (cols/rows derived from viewport to fill it)
    #[serde(default = "default_julia_explorer_max_iterations")]
    pub julia_explorer_max_iterations: u32,
    /// C extent half: grid shows [-0.75-L, -0.75+L]×[-L,L] in C. Smaller = zoom in.
    #[serde(default = "default_julia_explorer_extent_half")]
    pub julia_explorer_extent_half: f64,
    /// Side length of each grid cell in pixels (cols/rows = viewport / this).
    #[serde(default = "default_julia_explorer_cell_size_px")]
    pub julia_explorer_cell_size_px: u32,

    // Phase 10.5: J preview panel
    #[serde(default)]
    pub show_j_preview: bool,
    #[serde(default = "default_julia_preview_iterations")]
    pub julia_preview_iterations: u32,
}

/// Minimap widget size (side length in pixels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MinimapSize {
    #[default]
    Small,
    Medium,
    Large,
}

impl MinimapSize {
    pub fn side_pixels(self) -> u32 {
        match self {
            MinimapSize::Small => 128,
            MinimapSize::Medium => 256,
            MinimapSize::Large => 384,
        }
    }
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
fn default_minimap_zoom() -> f64 {
    2.0
}
fn default_minimap_iterations() -> u32 {
    500
}
fn default_minimap_opacity() -> f32 {
    0.75
}
fn default_crosshair_opacity() -> f32 {
    0.5
}
fn default_hud_panel_opacity() -> f32 {
    0.65
}
fn default_julia_explorer_max_iterations() -> u32 {
    200
}
fn default_julia_explorer_extent_half() -> f64 {
    2.0
}
fn default_julia_explorer_cell_size_px() -> u32 {
    64
}
fn default_julia_preview_iterations() -> u32 {
    250
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
            last_display_color: None,
            bookmarks_dir: String::new(),
            show_minimap: true,
            minimap_size: MinimapSize::default(),
            minimap_zoom_half_extent: default_minimap_zoom(),
            minimap_iterations: default_minimap_iterations(),
            minimap_opacity: default_minimap_opacity(),
            crosshair_opacity: default_crosshair_opacity(),
            hud_panel_opacity: default_hud_panel_opacity(),
            julia_explorer_max_iterations: default_julia_explorer_max_iterations(),
            julia_explorer_extent_half: default_julia_explorer_extent_half(),
            julia_explorer_cell_size_px: default_julia_explorer_cell_size_px(),
            show_j_preview: false,
            julia_preview_iterations: default_julia_preview_iterations(),
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
                    Ok(mut prefs) => {
                        info!("Loaded preferences from {}", path.display());
                        // One-time migration: widen C explorer extent if it was the old narrow default.
                        if prefs.julia_explorer_extent_half < 1.5 {
                            prefs.julia_explorer_extent_half = 2.0;
                            prefs.save();
                        }
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
    crate::app_dir::exe_directory().join("preferences.json")
}
