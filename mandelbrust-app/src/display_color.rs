//! Display and color settings: a single, serializable model used everywhere
//! coloring or display is decided (main view, export, profiles, bookmarks).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Palette mode
// ---------------------------------------------------------------------------

/// How the palette repeats over the iteration range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum PaletteMode {
    /// Palette repeats `n` times over [0, max_iterations). Cycle length = max_iterations / n.
    ByCycles { n: u32 },
    /// Palette repeats every `len` iterations. Position = iteration % len, normalized to [0, 1).
    ByCycleLength { len: u32 },
}

impl Default for PaletteMode {
    fn default() -> Self {
        PaletteMode::ByCycles { n: 1 }
    }
}

// ---------------------------------------------------------------------------
// Start-from (low-iteration fade)
// ---------------------------------------------------------------------------

/// Fade the first few iterations from solid black or white into the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartFrom {
    #[default]
    None,
    Black,
    White,
}

// ---------------------------------------------------------------------------
// DisplayColorSettings
// ---------------------------------------------------------------------------

/// Full display/color configuration: palette choice, cycle mode, start-from
/// black/white, smooth (log-log) toggle. Used by the app, profiles, and bookmarks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayColorSettings {
    /// Index into the built-in palettes list.
    pub palette_index: usize,
    /// How the palette repeats over the iteration range.
    #[serde(default)]
    pub palette_mode: PaletteMode,
    /// Fade the first few iterations from black or white (MSZP-style).
    #[serde(default)]
    pub start_from: StartFrom,
    /// Iteration threshold: below this, pixel is solid black or white (when start_from != None).
    #[serde(default = "default_low_threshold_start")]
    pub low_threshold_start: u32,
    /// Iteration threshold: between start and end, blend to palette (when start_from != None).
    #[serde(default = "default_low_threshold_end")]
    pub low_threshold_end: u32,
    /// Use continuous (log-log) iteration for coloring; when false, use raw integer count.
    pub smooth_coloring: bool,
}

fn default_low_threshold_start() -> u32 {
    10
}
fn default_low_threshold_end() -> u32 {
    30
}

impl Default for DisplayColorSettings {
    fn default() -> Self {
        Self {
            palette_index: 0,
            palette_mode: PaletteMode::default(),
            start_from: StartFrom::default(),
            low_threshold_start: default_low_threshold_start(),
            low_threshold_end: default_low_threshold_end(),
            smooth_coloring: true,
        }
    }
}

impl DisplayColorSettings {
    /// Effective cycle length in iterations (for ByCycles, depends on max_iterations).
    pub fn cycle_length(&self, max_iterations: u32) -> u32 {
        match self.palette_mode {
            PaletteMode::ByCycles { n } => {
                if n == 0 {
                    max_iterations
                } else {
                    max_iterations / n
                }
            }
            PaletteMode::ByCycleLength { len } => len,
        }
    }
}
