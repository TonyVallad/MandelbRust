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

/// How escaped pixels are mapped to palette colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColoringMode {
    #[default]
    Standard,
    Histogram,
    DistanceEstimation,
}

/// How interior (non-escaping) pixels are colored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteriorMode {
    #[default]
    Black,
    StripeAverage,
}

/// Full display/color configuration: palette choice, cycle mode, start-from
/// black/white, smooth (log-log) toggle. Used by the app, profiles, and bookmarks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayColorSettings {
    /// Index into the built-in palettes list (used when `custom_palette_name` is `None`).
    pub palette_index: usize,
    /// When set, a user-defined palette with this name is used instead of `palette_index`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_palette_name: Option<String>,
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
    /// How escaped pixels are colored.
    #[serde(default)]
    pub coloring_mode: ColoringMode,
    /// How interior pixels are colored.
    #[serde(default)]
    pub interior_mode: InteriorMode,
    /// Stripe density for interior stripe-average coloring.
    #[serde(default = "default_stripe_density")]
    pub stripe_density: f64,
}

fn default_low_threshold_start() -> u32 {
    10
}
fn default_low_threshold_end() -> u32 {
    30
}
fn default_stripe_density() -> f64 {
    1.0
}

impl Default for DisplayColorSettings {
    fn default() -> Self {
        Self {
            palette_index: 0,
            custom_palette_name: None,
            palette_mode: PaletteMode::default(),
            start_from: StartFrom::default(),
            low_threshold_start: default_low_threshold_start(),
            low_threshold_end: default_low_threshold_end(),
            smooth_coloring: true,
            coloring_mode: ColoringMode::default(),
            interior_mode: InteriorMode::default(),
            stripe_density: default_stripe_density(),
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
