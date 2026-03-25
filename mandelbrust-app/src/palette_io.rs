//! Palette file I/O: list, load, save, rename, and delete user-defined palettes
//! stored as individual JSON files in the `palettes/` directory.
//!
//! The UI for palette management (Task 16.5) will consume these functions.
use std::fs;
use std::path::PathBuf;

use tracing::{debug, info, warn};

use mandelbrust_core::palette_data::PaletteDefinition;

const PALETTES_DIR_NAME: &str = "palettes";

/// Directory next to the executable where palette JSON files are stored.
pub fn palettes_dir() -> PathBuf {
    crate::app_dir::exe_directory().join(PALETTES_DIR_NAME)
}

/// Sanitize a palette name for use as a filename.
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// Ensure the palettes directory exists.
pub fn ensure_palettes_dir() {
    let dir = palettes_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        warn!("Could not create palettes dir {}: {}", dir.display(), e);
    }
}

/// List display names of existing palettes (filename without .json).
pub fn list_palettes() -> Vec<String> {
    let dir = palettes_dir();
    let rd = match fs::read_dir(&dir) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            if p.extension().map(|ext| ext == "json").unwrap_or(false) {
                p.file_stem().map(|s| s.to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names
}

/// Load a palette by name. Returns `None` on error.
pub fn load_palette(name: &str) -> Option<PaletteDefinition> {
    let dir = palettes_dir();
    let path = dir.join(sanitize_name(name)).with_extension("json");
    match fs::read_to_string(&path) {
        Ok(json) => match serde_json::from_str::<PaletteDefinition>(&json) {
            Ok(pal) => {
                debug!("Loaded palette from {}", path.display());
                Some(pal)
            }
            Err(e) => {
                warn!("Invalid palette {}: {}", path.display(), e);
                None
            }
        },
        Err(e) => {
            warn!("Could not read palette {}: {}", path.display(), e);
            None
        }
    }
}

/// Save a palette. Overwrites if exists.
pub fn save_palette(palette: &PaletteDefinition) -> Result<(), String> {
    let dir = palettes_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir
        .join(sanitize_name(&palette.name))
        .with_extension("json");
    let json = serde_json::to_string_pretty(palette).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    info!("Saved palette to {}", path.display());
    Ok(())
}

/// Rename a palette file on disk.
pub fn rename_palette(old_name: &str, new_name: &str) -> Result<(), String> {
    let dir = palettes_dir();
    let old_path = dir.join(sanitize_name(old_name)).with_extension("json");
    let new_path = dir.join(sanitize_name(new_name)).with_extension("json");
    fs::rename(&old_path, &new_path).map_err(|e| e.to_string())
}

/// Delete a palette file on disk.
pub fn delete_palette(name: &str) -> Result<(), String> {
    let dir = palettes_dir();
    let path = dir.join(sanitize_name(name)).with_extension("json");
    fs::remove_file(&path).map_err(|e| e.to_string())
}

/// Load all palettes from the directory.
pub fn load_all_palettes() -> Vec<PaletteDefinition> {
    list_palettes()
        .iter()
        .filter_map(|name| load_palette(name))
        .collect()
}
