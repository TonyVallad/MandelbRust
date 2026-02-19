//! Directory where the executable lives. Used for preferences, bookmarks, and color profiles
//! so that data is stored next to the app when run as a standalone exe.

use std::path::PathBuf;

/// Directory containing the running executable. Falls back to current directory if unavailable.
pub fn exe_directory() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}
