//! Color profile I/O: list, load, and save DisplayColorSettings as JSON files in `color_profiles/`.

use std::fs;
use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

use crate::display_color::DisplayColorSettings;

const PROFILES_DIR_NAME: &str = "color_profiles";
const DEFAULT_PROFILE_NAME: &str = "Default";

/// Directory next to the executable where profile JSON files are stored.
pub fn color_profiles_dir() -> PathBuf {
    crate::app_dir::exe_directory().join(PROFILES_DIR_NAME)
}

/// Sanitize a profile name for use as a filename (replace invalid chars with `_`).
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect()
}

/// Ensure the color_profiles directory exists and contains at least Default.json if empty.
pub fn ensure_default_profile() {
    let dir = color_profiles_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        warn!("Could not create color_profiles dir {}: {}", dir.display(), e);
        return;
    }
    let entries: Vec<_> = match fs::read_dir(&dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(e) => {
            warn!("Could not read color_profiles dir {}: {}", dir.display(), e);
            return;
        }
    };
    let has_any_json = entries.iter().any(|e| {
        e.path()
            .extension()
            .map(|ext| ext == "json")
            .unwrap_or(false)
    });
    if !has_any_json {
        let default = DisplayColorSettings::default();
        if save_profile_inner(&dir, DEFAULT_PROFILE_NAME, &default).is_ok() {
            info!("Created default color profile at {}/Default.json", dir.display());
        }
    }
}

/// List display names of existing profiles (filename without .json).
pub fn list_profiles() -> Vec<String> {
    let dir = color_profiles_dir();
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

/// Load a profile by display name. Returns default settings on error.
pub fn load_profile(name: &str) -> DisplayColorSettings {
    let dir = color_profiles_dir();
    let path = dir.join(sanitize_name(name)).with_extension("json");
    match fs::read_to_string(&path) {
        Ok(json) => match serde_json::from_str::<DisplayColorSettings>(&json) {
            Ok(s) => {
                debug!("Loaded color profile from {}", path.display());
                s
            }
            Err(e) => {
                warn!("Invalid color profile {}: {}", path.display(), e);
                DisplayColorSettings::default()
            }
        },
        Err(e) => {
            warn!("Could not read color profile {}: {}", path.display(), e);
            DisplayColorSettings::default()
        }
    }
}

fn save_profile_inner(
    dir: &Path,
    name: &str,
    settings: &DisplayColorSettings,
) -> std::io::Result<()> {
    let path = dir.join(sanitize_name(name)).with_extension("json");
    let json = serde_json::to_string_pretty(settings).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    })?;
    fs::write(path, json)
}

/// Save current settings as a profile. Overwrites if exists. Name is sanitized for the filename.
pub fn save_profile(name: &str, settings: &DisplayColorSettings) -> Result<(), String> {
    let dir = color_profiles_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    save_profile_inner(&dir, name, settings).map_err(|e| e.to_string())
}
