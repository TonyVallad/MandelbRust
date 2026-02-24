use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::display_color::DisplayColorSettings;

// ---------------------------------------------------------------------------
// Bookmark
// ---------------------------------------------------------------------------

/// A saved exploration state with an embedded preview image.
///
/// Each bookmark is stored as an individual `.json` file inside the
/// `bookmarks/` directory, making them trivially shareable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub name: String,
    /// `"Mandelbrot"` or `"Julia"`.
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
    /// Full display/color snapshot (Phase 8). When absent, infer from palette_index/smooth_coloring.
    #[serde(default)]
    pub display_color: Option<DisplayColorSettings>,
    pub aa_level: u32,
    pub julia_c_re: f64,
    pub julia_c_im: f64,
    /// Hierarchical labels using `/` as separator (e.g. "Spirals/Double").
    #[serde(default, alias = "tags")]
    pub labels: Vec<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub created_at: String,
    /// Base64-encoded PNG thumbnail embedded in the file.
    #[serde(default)]
    pub thumbnail_png: String,

    /// Legacy field — kept for backwards-compatible deserialization, then migrated.
    /// Not written to new files.
    #[serde(default, skip_serializing)]
    pub thumbnail_file: String,
}

impl Bookmark {
    /// Human-readable summary for list views.
    pub fn summary(&self) -> String {
        let zoom = 1.0 / self.scale;
        format!("{} — zoom {zoom:.2e}", self.mode)
    }

    /// Whether this bookmark was loaded from the legacy format and has a
    /// separate thumbnail file that should be migrated.
    pub fn has_legacy_thumbnail(&self) -> bool {
        !self.thumbnail_file.is_empty() && self.thumbnail_png.is_empty()
    }
}

/// Suggest smart default labels for a bookmark based on its state.
pub fn suggest_default_labels(mode: &str, scale: f64, max_iterations: u32) -> Vec<String> {
    let mut labels = vec![mode.to_lowercase()];
    let zoom = 1.0 / scale;
    if zoom > 1e10 {
        labels.push("Deep zoom".to_string());
    } else if zoom > 1e4 {
        labels.push("Medium zoom".to_string());
    } else {
        labels.push("Overview".to_string());
    }
    if max_iterations >= 2000 {
        labels.push("High detail".to_string());
    }
    labels
}

// ---------------------------------------------------------------------------
// Label tree
// ---------------------------------------------------------------------------

/// A node in the hierarchical label tree, built from flat label strings.
#[derive(Debug)]
pub struct LabelNode {
    /// Display name of this node (leaf segment).
    pub name: String,
    /// Full path from root (e.g. "Spirals/Double").
    pub full_path: String,
    pub children: Vec<LabelNode>,
}

/// Build a tree of labels from a flat list of `/`-separated paths.
pub fn build_label_tree(labels: &[String]) -> Vec<LabelNode> {
    let mut roots: Vec<LabelNode> = Vec::new();
    for label in labels {
        let parts: Vec<&str> = label.split('/').collect();
        insert_into_tree(&mut roots, &parts, 0);
    }
    roots
}

fn insert_into_tree(nodes: &mut Vec<LabelNode>, parts: &[&str], depth: usize) {
    if depth >= parts.len() {
        return;
    }
    let name = parts[depth];
    let full_path = parts[..=depth].join("/");

    let pos = nodes.iter().position(|n| n.name == name);
    let idx = if let Some(pos) = pos {
        pos
    } else {
        nodes.push(LabelNode {
            name: name.to_string(),
            full_path,
            children: Vec::new(),
        });
        nodes.len() - 1
    };
    insert_into_tree(&mut nodes[idx].children, parts, depth + 1);
}

/// Collect every unique label (including parent segments) from bookmarks.
pub fn collect_all_labels(bookmarks: &[Bookmark]) -> Vec<String> {
    let mut set = HashSet::new();
    set.insert("Favorites".to_string()); // Always present.
    for bm in bookmarks {
        for label in &bm.labels {
            let parts: Vec<&str> = label.split('/').collect();
            for i in 0..parts.len() {
                set.insert(parts[..=i].join("/"));
            }
        }
    }
    let mut labels: Vec<String> = set.into_iter().collect();
    // "Favorites" first, then alphabetical.
    labels.sort_by(|a, b| {
        let fa = a == "Favorites";
        let fb = b == "Favorites";
        fb.cmp(&fa)
            .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
    });
    labels
}

/// Collect only the leaf labels (the labels exactly as stored on bookmarks).
pub fn collect_leaf_labels(bookmarks: &[Bookmark]) -> Vec<String> {
    let mut set = HashSet::new();
    set.insert("Favorites".to_string()); // Always present.
    for bm in bookmarks {
        for label in &bm.labels {
            set.insert(label.clone());
        }
    }
    let mut labels: Vec<String> = set.into_iter().collect();
    // "Favorites" first, then alphabetical.
    labels.sort_by(|a, b| {
        let fa = a == "Favorites";
        let fb = b == "Favorites";
        fb.cmp(&fa)
            .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
    });
    labels
}

// ---------------------------------------------------------------------------
// Bookmark store  (one file per bookmark)
// ---------------------------------------------------------------------------

/// Manages a collection of bookmarks.
///
/// Each bookmark lives as an individual `.json` file inside the bookmarks
/// directory (by default a `bookmarks/` folder next to the executable).
/// The store reloads from disk every time the bookmark explorer is opened.
pub struct BookmarkStore {
    bookmarks: Vec<Bookmark>,
    /// Parallel vector: the on-disk filename for each bookmark (without path).
    filenames: Vec<String>,
    dir: PathBuf,
}

impl BookmarkStore {
    /// Load all bookmarks from the bookmarks directory.
    /// If `custom_dir` is non-empty, use that path; otherwise use the default
    /// OS config path. Legacy migration only runs for the default directory.
    pub fn load(custom_dir: &str) -> Self {
        let dir = if custom_dir.is_empty() {
            bookmarks_dir()
        } else {
            PathBuf::from(custom_dir)
        };

        if let Err(e) = fs::create_dir_all(&dir) {
            error!("Failed to create bookmarks directory: {e}");
        }

        // One-time migration from legacy single-file format (default dir only).
        if custom_dir.is_empty() {
            migrate_legacy(&dir);
        }

        let mut store = Self {
            bookmarks: Vec::new(),
            filenames: Vec::new(),
            dir,
        };
        store.reload();
        store
    }

    /// Change the bookmarks directory and reload.
    pub fn set_directory(&mut self, new_dir: &str) {
        let dir = if new_dir.is_empty() {
            bookmarks_dir()
        } else {
            PathBuf::from(new_dir)
        };
        if let Err(e) = fs::create_dir_all(&dir) {
            error!("Failed to create bookmarks directory: {e}");
        }
        info!("Bookmarks directory changed to {}", dir.display());
        self.dir = dir;
        self.reload();
    }

    /// Re-scan the bookmarks directory and reload all `.json` files.
    pub fn reload(&mut self) {
        self.bookmarks.clear();
        self.filenames.clear();

        let entries = match fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(e) => {
                error!("Failed to read bookmarks directory: {e}");
                return;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match fs::read_to_string(&path) {
                Ok(json) => match serde_json::from_str::<Bookmark>(&json) {
                    Ok(bm) => {
                        let fname = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        self.filenames.push(fname);
                        self.bookmarks.push(bm);
                    }
                    Err(e) => {
                        warn!("Skipping invalid bookmark file {}: {e}", path.display());
                    }
                },
                Err(e) => {
                    warn!("Failed to read {}: {e}", path.display());
                }
            }
        }

        info!(
            "Loaded {} bookmarks from {}",
            self.bookmarks.len(),
            self.dir.display()
        );
    }

    pub fn bookmarks(&self) -> &[Bookmark] {
        &self.bookmarks
    }

    /// Path to the bookmarks directory (for display / sharing).
    pub fn directory(&self) -> &std::path::Path {
        &self.dir
    }

    pub fn add(&mut self, bookmark: Bookmark) {
        info!("Adding bookmark: {}", bookmark.name);
        let filename = self.unique_filename(&bookmark.name);
        write_bookmark_file(&self.dir, &filename, &bookmark);
        self.filenames.push(filename);
        self.bookmarks.push(bookmark);
    }

    pub fn remove(&mut self, index: usize) {
        if index >= self.bookmarks.len() {
            return;
        }
        info!("Removing bookmark: {}", self.bookmarks[index].name);
        delete_bookmark_file(&self.dir, &self.filenames[index]);
        self.bookmarks.remove(index);
        self.filenames.remove(index);
    }

    pub fn rename(&mut self, index: usize, new_name: String) {
        if index >= self.bookmarks.len() {
            return;
        }
        debug!(
            "Renaming bookmark {} -> {}",
            self.bookmarks[index].name, new_name
        );
        self.bookmarks[index].name = new_name.clone();

        // Re-derive filename so it matches the new name.
        let old_filename = self.filenames[index].clone();
        let new_filename = self.unique_filename(&new_name);
        delete_bookmark_file(&self.dir, &old_filename);
        write_bookmark_file(&self.dir, &new_filename, &self.bookmarks[index]);
        self.filenames[index] = new_filename;
    }

    /// Toggle a label on a bookmark. Returns `true` if the label was added.
    pub fn toggle_label(&mut self, index: usize, label: &str) -> bool {
        if index >= self.bookmarks.len() {
            return false;
        }
        let bm = &mut self.bookmarks[index];
        let added = if let Some(pos) = bm.labels.iter().position(|l| l == label) {
            bm.labels.remove(pos);
            false
        } else {
            bm.labels.push(label.to_string());
            true
        };
        self.persist(index);
        added
    }

    /// Update a bookmark's fields via a closure, then persist to disk.
    pub fn update_viewport(&mut self, index: usize, updater: impl FnOnce(&mut Bookmark)) {
        if index >= self.bookmarks.len() {
            return;
        }
        info!("Updating bookmark: {}", self.bookmarks[index].name);
        updater(&mut self.bookmarks[index]);
        self.persist(index);
    }

    pub fn sort_by_name(&mut self) {
        let mut indices: Vec<usize> = (0..self.bookmarks.len()).collect();
        indices.sort_by(|&a, &b| {
            self.bookmarks[a]
                .name
                .to_lowercase()
                .cmp(&self.bookmarks[b].name.to_lowercase())
        });
        self.apply_order(&indices);
    }

    pub fn sort_by_date(&mut self) {
        let mut indices: Vec<usize> = (0..self.bookmarks.len()).collect();
        indices.sort_by(|&a, &b| {
            self.bookmarks[b]
                .created_at
                .cmp(&self.bookmarks[a].created_at)
        });
        self.apply_order(&indices);
    }

    /// Generate an auto-name like "Mandelbrot_000021" for a given fractal mode.
    pub fn next_auto_name(&self, mode: &str) -> String {
        let prefix = format!("{mode}_");
        let max_num = self
            .bookmarks
            .iter()
            .filter_map(|bm| {
                bm.name
                    .strip_prefix(&prefix)
                    .and_then(|s| s.parse::<u32>().ok())
            })
            .max()
            .unwrap_or(0);
        format!("{prefix}{:06}", max_num + 1)
    }

    /// No-op save — kept for API compatibility during the transition.
    /// Individual-file persistence happens immediately on every mutation.
    pub fn save(&mut self) {
        // nothing to do; each mutation is already persisted
    }

    // -- Internal helpers ---------------------------------------------------

    /// Persist a single bookmark at `index` to its file.
    fn persist(&self, index: usize) {
        if index < self.bookmarks.len() {
            write_bookmark_file(&self.dir, &self.filenames[index], &self.bookmarks[index]);
        }
    }

    /// Reorder bookmarks and filenames according to a permutation.
    fn apply_order(&mut self, order: &[usize]) {
        let bm: Vec<Bookmark> = order.iter().map(|&i| self.bookmarks[i].clone()).collect();
        let fn_: Vec<String> = order.iter().map(|&i| self.filenames[i].clone()).collect();
        self.bookmarks = bm;
        self.filenames = fn_;
    }

    /// Derive a unique filename from a bookmark name.
    fn unique_filename(&self, name: &str) -> String {
        let base = sanitize_filename(name);
        let candidate = format!("{base}.json");
        if !self.filenames.contains(&candidate) && !self.dir.join(&candidate).exists() {
            return candidate;
        }
        // Append a numeric suffix to avoid collisions.
        for n in 1..10000 {
            let candidate = format!("{base}_{n}.json");
            if !self.filenames.contains(&candidate) && !self.dir.join(&candidate).exists() {
                return candidate;
            }
        }
        // Fallback with timestamp.
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        format!("{base}_{ts}.json")
    }
}

// ---------------------------------------------------------------------------
// Thumbnail helpers  (encode / decode base64 PNG)
// ---------------------------------------------------------------------------

/// Maximum thumbnail width in pixels.
const THUMB_MAX_WIDTH: u32 = 160;

/// Encode an RGBA pixel buffer as a base64 PNG string for embedding.
pub fn encode_thumbnail(pixels: &[u8], width: u32, height: u32) -> Option<String> {
    let img = image::RgbaImage::from_raw(width, height, pixels.to_vec())?;

    let thumb_w = width.min(THUMB_MAX_WIDTH);
    let thumb_h = (height as f64 * thumb_w as f64 / width as f64).round() as u32;
    let thumb = image::imageops::resize(
        &img,
        thumb_w,
        thumb_h.max(1),
        image::imageops::FilterType::Triangle,
    );

    // Encode to PNG in memory.
    let mut png_bytes: Vec<u8> = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(std::io::Cursor::new(&mut png_bytes));
    if let Err(e) = image::ImageEncoder::write_image(
        encoder,
        thumb.as_raw(),
        thumb_w,
        thumb_h.max(1),
        image::ExtendedColorType::Rgba8,
    ) {
        error!("Failed to encode thumbnail PNG: {e}");
        return None;
    }

    let b64 = BASE64.encode(&png_bytes);
    debug!(
        "Encoded thumbnail ({thumb_w}x{thumb_h}, {} bytes b64)",
        b64.len()
    );
    Some(b64)
}

/// Decode a base64 PNG thumbnail into (RGBA pixels, width, height).
pub fn decode_thumbnail(base64_str: &str) -> Option<(Vec<u8>, u32, u32)> {
    if base64_str.is_empty() {
        return None;
    }
    let png_bytes = match BASE64.decode(base64_str) {
        Ok(b) => b,
        Err(e) => {
            debug!("Failed to decode base64 thumbnail: {e}");
            return None;
        }
    };
    let img = match image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png) {
        Ok(i) => i,
        Err(e) => {
            debug!("Failed to decode PNG thumbnail: {e}");
            return None;
        }
    };
    let rgba = img.to_rgba8();
    let w = rgba.width();
    let h = rgba.height();
    Some((rgba.into_raw(), w, h))
}

// ---------------------------------------------------------------------------
// File-level persistence helpers
// ---------------------------------------------------------------------------

fn write_bookmark_file(dir: &std::path::Path, filename: &str, bookmark: &Bookmark) {
    let path = dir.join(filename);
    match serde_json::to_string_pretty(bookmark) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, &json) {
                error!("Failed to write bookmark file {}: {e}", path.display());
            } else {
                debug!("Wrote bookmark file: {}", path.display());
            }
        }
        Err(e) => error!("Failed to serialize bookmark: {e}"),
    }
}

fn delete_bookmark_file(dir: &std::path::Path, filename: &str) {
    let path = dir.join(filename);
    if let Err(e) = fs::remove_file(&path) {
        debug!("Failed to delete bookmark file {}: {e}", path.display());
    }
}

/// Turn a bookmark name into a safe filename (no extension).
fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = sanitized.trim().to_string();
    if trimmed.is_empty() {
        "bookmark".to_string()
    } else {
        trimmed
    }
}

// ---------------------------------------------------------------------------
// Legacy migration
// ---------------------------------------------------------------------------

/// If the old `bookmarks.json` exists in the config dir, migrate each entry
/// to an individual file in the `bookmarks/` directory.
fn migrate_legacy(bookmarks_dir: &std::path::Path) {
    let config = config_dir();
    let legacy_json = config.join("bookmarks.json");
    if !legacy_json.exists() {
        return;
    }

    info!("Found legacy bookmarks.json — migrating to individual files…");

    let json = match fs::read_to_string(&legacy_json) {
        Ok(j) => j,
        Err(e) => {
            error!("Failed to read legacy bookmarks.json: {e}");
            return;
        }
    };

    let old_bookmarks: Vec<Bookmark> = match serde_json::from_str(&json) {
        Ok(bms) => bms,
        Err(e) => {
            error!("Failed to parse legacy bookmarks.json: {e}");
            return;
        }
    };

    let thumb_dir = config.join("thumbnails");
    let mut migrated = 0;

    // Track filenames to avoid collisions during migration.
    let mut used_filenames: Vec<String> = Vec::new();

    for mut bm in old_bookmarks {
        // Embed the thumbnail if the legacy separate-file reference exists.
        if bm.has_legacy_thumbnail() {
            let thumb_path = thumb_dir.join(&bm.thumbnail_file);
            if let Ok(png_bytes) = fs::read(&thumb_path) {
                bm.thumbnail_png = BASE64.encode(&png_bytes);
                debug!(
                    "Embedded thumbnail {} into bookmark '{}'",
                    bm.thumbnail_file, bm.name
                );
            }
        }
        // Clear legacy field (it's skip_serializing so won't appear in output).
        bm.thumbnail_file = String::new();

        let base = sanitize_filename(&bm.name);
        let mut filename = format!("{base}.json");
        for n in 1..10000 {
            if !used_filenames.contains(&filename) && !bookmarks_dir.join(&filename).exists() {
                break;
            }
            filename = format!("{base}_{n}.json");
        }

        write_bookmark_file(bookmarks_dir, &filename, &bm);
        used_filenames.push(filename);
        migrated += 1;
    }

    info!("Migrated {migrated} bookmarks to individual files");

    // Rename the old file so we don't migrate again.
    let backup = config.join("bookmarks.json.migrated");
    if let Err(e) = fs::rename(&legacy_json, &backup) {
        warn!("Could not rename legacy bookmarks.json: {e}");
    } else {
        info!("Renamed bookmarks.json -> bookmarks.json.migrated");
    }

    // Clean up old thumbnails directory.
    if thumb_dir.exists() {
        let backup_thumbs = config.join("thumbnails.migrated");
        if let Err(e) = fs::rename(&thumb_dir, &backup_thumbs) {
            debug!("Could not rename thumbnails dir: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn config_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "MandelbRust")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The default directory where individual bookmark files are stored.
/// Uses a `bookmarks/` subdirectory next to the executable.
fn bookmarks_dir() -> PathBuf {
    crate::app_dir::exe_directory().join("bookmarks")
}

/// Return the default bookmarks directory path as a string (for UI display).
#[allow(dead_code)]
pub fn default_bookmarks_dir() -> String {
    bookmarks_dir().to_string_lossy().to_string()
}

/// Timestamp for ordering (seconds since epoch).
pub fn now_timestamp() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", dur.as_secs())
}
