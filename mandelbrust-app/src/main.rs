mod bookmarks;
mod color_profiles;
mod display_color;
mod preferences;

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use eframe::egui;
use tracing::{debug, info, warn};

use mandelbrust_core::{Complex, FractalParams, Julia, Mandelbrot, Viewport};
use mandelbrust_render::{
    builtin_palettes, compute_aa, render, AaSamples, ColorParams, IterationBuffer, Palette,
    RenderCancel, RenderResult, StartFrom as RenderStartFrom,
};
use display_color::{
    DisplayColorSettings, PaletteMode as DisplayPaletteMode, StartFrom as DisplayStartFrom,
};

use bookmarks::{Bookmark, BookmarkStore};
use preferences::{AppPreferences, LastView};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Zoom sensitivity: maps scroll delta to an exponential scale factor.
const ZOOM_SPEED: f64 = 0.003;
/// Fraction of the viewport to pan per arrow-key press.
const PAN_FRACTION: f64 = 0.1;
/// Maximum undo/redo history entries.
const MAX_HISTORY: usize = 200;

/// Preview pass renders at 1/PREVIEW_DOWNSCALE of the full resolution.
const PREVIEW_DOWNSCALE: u32 = 4;
/// Extra iterations per doubling of zoom for adaptive mode.
const ADAPTIVE_ITER_RATE: f64 = 30.0;

/// Snapshot entry for bookmark display: (index, name, summary, mode, labels, thumbnail_png).
type BookmarkSnap = (usize, String, String, String, Vec<String>, String);
/// Below this scale, warn about f64 precision limits.
const PRECISION_WARN_SCALE: f64 = 1e-13;

/// Phase 9: HUD box margin (same for all corners).
const HUD_MARGIN: f32 = 8.0;
/// Phase 9: HUD box corner radius (viewport info, params, render stats, minimap).
const HUD_CORNER_RADIUS: f32 = 6.0;

// ---------------------------------------------------------------------------
// Fractal mode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FractalMode {
    Mandelbrot,
    Julia,
}

impl FractalMode {
    fn label(self) -> &'static str {
        match self {
            Self::Mandelbrot => "Mandelbrot",
            Self::Julia => "Julia",
        }
    }
}

// ---------------------------------------------------------------------------
// Bookmark explorer state
// ---------------------------------------------------------------------------

/// Which fractal tab is active in the bookmark explorer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BookmarkTab {
    All,
    Mandelbrot,
    Julia,
}

/// Label filter mode for the bookmark explorer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LabelFilterMode {
    /// Show all bookmarks regardless of labels.
    Off,
    /// Show only bookmarks that have at least one of the selected labels.
    Whitelist,
    /// Hide bookmarks that have any of the selected labels.
    Blacklist,
}

// ---------------------------------------------------------------------------
// Render status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderPhase {
    Idle,
    Rendering,
    Refining,
    Done,
}

impl RenderPhase {
    fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Rendering => "Rendering preview\u{2026}",
            Self::Refining => "Refining\u{2026}",
            Self::Done => "Done",
        }
    }
}

// ---------------------------------------------------------------------------
// Render thread communication
// ---------------------------------------------------------------------------

struct RenderRequest {
    id: u64,
    viewport: Viewport,
    params: FractalParams,
    mode: FractalMode,
    julia_c: Complex,
    aa_level: u32,
}

enum RenderResponse {
    Preview { id: u64, result: RenderResult },
    Final { id: u64, result: RenderResult },
}

/// Phase 10: request for the Julia C Explorer grid worker.
/// Grid shows C in [-0.75-extent_half, -0.75+extent_half]×[-extent_half, extent_half]; square grid (1:1 aspect) centered in viewport.
struct JuliaGridRequest {
    cols: u32,
    rows: u32,
    center_re: f64,
    center_im: f64,
    extent_half: f64,
    cell_size: u32,
    max_iterations: u32,
    aa_level: u32,
    cancel: Arc<RenderCancel>,
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

struct MandelbRustApp {
    // Fractal state
    mode: FractalMode,
    julia_c: Complex,
    params: FractalParams,
    viewport: Viewport,

    // Render thread
    tx_request: mpsc::Sender<RenderRequest>,
    rx_response: mpsc::Receiver<RenderResponse>,
    cancel: Arc<RenderCancel>,
    render_id: u64,
    render_phase: RenderPhase,
    needs_render: bool,

    // Last render stats
    texture: Option<egui::TextureHandle>,
    drag_preview: Option<egui::TextureHandle>,
    render_time: Duration,
    tiles_rendered: usize,
    tiles_mirrored: usize,
    tiles_border_traced: usize,

    // Coloring — Phase 5 (unified display/color settings, Phase 8)
    palettes: Vec<Palette>,
    display_color: DisplayColorSettings,
    current_iterations: Option<IterationBuffer>,

    // UI state
    panel_size: [u32; 2],
    show_hud: bool,
    show_controls: bool,
    show_palette_popup: bool,
    /// Selected profile name in the Display/color panel (for Load).
    color_profile_selected: String,
    /// Name buffer for "Save as profile" in the Display/color panel.
    color_profile_save_name: String,
    show_help: bool,
    show_crosshair: bool,
    drag_active: bool,
    pan_offset: egui::Vec2,
    cursor_complex: Option<Complex>,
    zoom_rect_start: Option<egui::Pos2>,

    // View history
    history: Vec<Viewport>,
    history_pos: usize,

    // Phase 4 features
    adaptive_iterations: bool,

    // Anti-aliasing
    aa_level: u32,
    current_aa: Option<AaSamples>,

    // Pan optimisation: skip the preview pass after a drag so the
    // shifted high-quality texture is not replaced by a low-res preview.
    pan_completed: bool,
    skip_preview_id: u64,

    // After a drag the old texture keeps being drawn at this offset
    // (so the drag preview can fill exposed edges) until the final
    // render arrives and resets it to zero.
    draw_offset: egui::Vec2,

    // Phase 6: bookmarks & preferences
    bookmark_store: BookmarkStore,
    preferences: AppPreferences,
    show_bookmarks: bool,
    show_save_dialog: bool,
    save_bookmark_name: String,
    save_bookmark_labels_selected: HashSet<String>,
    save_bookmark_new_label: String,
    bookmark_search: String,
    bookmark_tab: BookmarkTab,
    favorites_only: bool,
    editing_bookmark: Option<usize>,
    editing_name: String,
    selected_labels: HashSet<String>,
    label_filter_mode: LabelFilterMode,
    /// Texture cache keyed by bookmark index for decoded thumbnails.
    thumbnail_cache: HashMap<usize, egui::TextureHandle>,
    /// Bookmark indices whose thumbnails failed to decode (avoid retrying).
    failed_thumbnails: HashSet<usize>,

    /// Tracks which bookmark was last jumped to (for "Update or Save New" on S key).
    last_jumped_bookmark_idx: Option<usize>,
    /// Whether to show the "Update existing or Save new?" choice popup.
    show_update_or_save_dialog: bool,
    /// Editing buffer for the bookmarks directory path in the controls panel.
    bookmarks_dir_buf: String,

    // Phase 9: Minimap
    tx_minimap: mpsc::Sender<(RenderResult, u64)>,
    rx_minimap: mpsc::Receiver<(RenderResult, u64)>,
    /// Cached minimap texture and the revision it was built for.
    minimap_texture: Option<(egui::TextureHandle, u64)>,
    minimap_loading: bool,
    /// Bumped when mode, julia_c, display_color, or minimap prefs change; used for cache invalidation.
    minimap_revision: u64,
    /// Set from display/color panel when palette is changed in a context that borrows self; cleared in update().
    pending_minimap_bump: bool,

    // Phase 10: Julia C Explorer
    show_julia_c_explorer: bool,
    /// C extent half (grid shows ± this in re/im around center). Smaller = zoom in.
    julia_explorer_extent_half: f64,
    /// Cols×rows of the current grid (derived from viewport to fill it).
    julia_explorer_cols: u32,
    julia_explorer_rows: u32,
    /// Per-cell iteration data; keys (row, col).
    julia_explorer_cells: HashMap<(u32, u32), IterationBuffer>,
    /// Per-cell textures (derived from cells + current color settings).
    julia_explorer_textures: HashMap<(u32, u32), egui::TextureHandle>,
    /// When true, recolorize all cells and refresh textures (e.g. after display_color change).
    julia_explorer_recolorize: bool,
    /// Defer grid restart to next frame (avoids clearing textures while drawing).
    julia_explorer_restart_pending: bool,
    /// Set when user clicks a cell in the explorer; applied after the panel is drawn.
    julia_explorer_picked_c: Option<(f64, f64)>,
    tx_julia_grid_req: mpsc::Sender<JuliaGridRequest>,
    rx_julia_grid_resp: mpsc::Receiver<(u32, u32, RenderResult)>,
    grid_cancel: Arc<RenderCancel>,
}

impl MandelbRustApp {
    fn new(egui_ctx: &egui::Context, prefs: AppPreferences) -> Self {
        let julia_explorer_extent_half = prefs.julia_explorer_extent_half;
        let (tx_req, rx_req) = mpsc::channel();
        let (tx_resp, rx_resp) = mpsc::channel();
        let cancel = Arc::new(RenderCancel::new());

        let (tx_julia_grid_req, rx_julia_grid_req) = mpsc::channel();
        let (tx_julia_grid_resp, rx_julia_grid_resp) = mpsc::channel();
        let grid_cancel = Arc::new(RenderCancel::new());

        // Spawn the background render worker.
        let ctx = egui_ctx.clone();
        let cancel_clone = cancel.clone();
        thread::spawn(move || {
            render_worker(ctx, rx_req, tx_resp, cancel_clone);
        });

        // Phase 10: Julia C Explorer grid worker.
        thread::spawn(move || {
            julia_grid_worker(rx_julia_grid_req, tx_julia_grid_resp);
        });

        let w = prefs.window_width as u32;
        let h = prefs.window_height as u32;

        // Restore last view if configured, otherwise use defaults.
        let (mode, julia_c, params, viewport, mut display_color, aa_level) = if prefs.restore_last_view
        {
            if let Some(ref lv) = prefs.last_view {
                let m = match lv.mode.as_str() {
                    "Julia" => FractalMode::Julia,
                    _ => FractalMode::Mandelbrot,
                };
                let vp = Viewport::new(Complex::new(lv.center_re, lv.center_im), lv.scale, w, h)
                    .unwrap_or_else(|_| Viewport::default_mandelbrot(w, h));
                let p = FractalParams::new(lv.max_iterations, lv.escape_radius)
                    .unwrap_or_default();
                let dc = DisplayColorSettings {
                    palette_index: lv.palette_index,
                    smooth_coloring: lv.smooth_coloring,
                    ..DisplayColorSettings::default()
                };
                info!(
                    "Restoring last view: {} at zoom {:.2e}",
                    lv.mode,
                    1.0 / lv.scale
                );
                (
                    m,
                    Complex::new(lv.julia_c_re, lv.julia_c_im),
                    p,
                    vp,
                    dc,
                    lv.aa_level,
                )
            } else {
                defaults_for(w, h, &prefs)
            }
        } else {
            defaults_for(w, h, &prefs)
        };

        // Restore full display/color settings from last session so palette mode, start-from, etc. persist.
        if let Some(ref saved) = prefs.last_display_color {
            display_color = saved.clone();
        }
        let palettes = builtin_palettes();
        if display_color.palette_index >= palettes.len() {
            display_color.palette_index = 0;
        }

        let mut bookmark_store = BookmarkStore::load(&prefs.bookmarks_dir);
        bookmark_store.sort_by_date(); // Default: newest first.
        let bookmarks_dir_display = bookmark_store.directory().to_string_lossy().to_string();

        let (tx_minimap, rx_minimap) = mpsc::channel();

        let app = Self {
            mode,
            julia_c,
            params,
            viewport,

            tx_request: tx_req,
            rx_response: rx_resp,
            cancel,
            render_id: 0,
            render_phase: RenderPhase::Idle,
            needs_render: true,

            texture: None,
            drag_preview: None,
            render_time: Duration::ZERO,
            tiles_rendered: 0,
            tiles_mirrored: 0,
            tiles_border_traced: 0,

            palettes,
            display_color,
            current_iterations: None,

            panel_size: [w, h],
            show_hud: true,
            show_controls: false,
            show_palette_popup: false,
            color_profile_selected: String::new(),
            color_profile_save_name: String::new(),
            show_help: false,
            show_crosshair: false,
            drag_active: false,
            pan_offset: egui::Vec2::ZERO,
            cursor_complex: None,
            zoom_rect_start: None,

            history: vec![viewport],
            history_pos: 0,

            adaptive_iterations: true,

            aa_level,
            current_aa: None,

            pan_completed: false,
            skip_preview_id: 0,
            draw_offset: egui::Vec2::ZERO,

            bookmark_store,
            preferences: prefs,
            show_bookmarks: false,
            show_save_dialog: false,
            save_bookmark_name: String::new(),
            save_bookmark_labels_selected: HashSet::new(),
            save_bookmark_new_label: String::new(),
            bookmark_search: String::new(),
            bookmark_tab: match mode {
                FractalMode::Mandelbrot => BookmarkTab::Mandelbrot,
                FractalMode::Julia => BookmarkTab::Julia,
            },
            favorites_only: false,
            editing_bookmark: None,
            editing_name: String::new(),
            selected_labels: HashSet::new(),
            label_filter_mode: LabelFilterMode::Off,
            thumbnail_cache: HashMap::new(),
            failed_thumbnails: HashSet::new(),
            last_jumped_bookmark_idx: None,
            show_update_or_save_dialog: false,
            bookmarks_dir_buf: bookmarks_dir_display,

            tx_minimap,
            rx_minimap,
            minimap_texture: None,
            minimap_loading: false,
            minimap_revision: 0,
            pending_minimap_bump: false,

            show_julia_c_explorer: false,
            julia_explorer_extent_half,
            julia_explorer_cols: 0,
            julia_explorer_rows: 0,
            julia_explorer_cells: HashMap::new(),
            julia_explorer_textures: HashMap::new(),
            julia_explorer_recolorize: false,
            julia_explorer_restart_pending: false,
            julia_explorer_picked_c: None,
            tx_julia_grid_req: tx_julia_grid_req,
            rx_julia_grid_resp: rx_julia_grid_resp,
            grid_cancel,
        };
        color_profiles::ensure_default_profile();
        app
    }

    // -- Bookmark helpers ------------------------------------------------------

    /// Capture the current exploration state as a `Bookmark`.
    /// Generates a PNG thumbnail from the current iteration buffer.
    fn capture_bookmark(&self, name: String, labels: Vec<String>) -> Bookmark {
        // Generate base64 PNG thumbnail from current render.
        let thumbnail_png = self
            .current_iterations
            .as_ref()
            .and_then(|iter_buf| {
                let buf = self.colorize_current(iter_buf, self.current_aa.as_ref());
                bookmarks::encode_thumbnail(&buf.pixels, buf.width, buf.height)
            })
            .unwrap_or_default();

        Bookmark {
            name,
            mode: self.mode.label().to_string(),
            center_re: self.viewport.center.re,
            center_im: self.viewport.center.im,
            scale: self.viewport.scale,
            max_iterations: self.params.max_iterations,
            escape_radius: self.params.escape_radius,
            palette_index: self.display_color.palette_index,
            smooth_coloring: self.display_color.smooth_coloring,
            display_color: Some(self.display_color.clone()),
            aa_level: self.aa_level,
            julia_c_re: self.julia_c.re,
            julia_c_im: self.julia_c.im,
            labels,
            notes: String::new(),
            created_at: bookmarks::now_timestamp(),
            thumbnail_png,
            thumbnail_file: String::new(),
        }
    }

    /// Open the "Save New Bookmark" dialog with default labels pre-selected.
    fn open_save_new_dialog(&mut self) {
        self.show_save_dialog = true;
        self.save_bookmark_name.clear();
        self.save_bookmark_new_label.clear();
        let defaults = bookmarks::suggest_default_labels(
            self.mode.label(),
            self.viewport.scale,
            self.params.max_iterations,
        );
        self.save_bookmark_labels_selected = defaults.into_iter().collect();
    }

    /// Update an existing bookmark in-place with the current viewport and params.
    /// Replaces the thumbnail too.
    fn update_bookmark(&mut self, idx: usize) {
        if idx >= self.bookmark_store.bookmarks().len() {
            return;
        }

        // Generate a new base64 PNG thumbnail.
        let thumbnail_png = self
            .current_iterations
            .as_ref()
            .and_then(|iter_buf| {
                let buf = self.colorize_current(iter_buf, self.current_aa.as_ref());
                bookmarks::encode_thumbnail(&buf.pixels, buf.width, buf.height)
            })
            .unwrap_or_default();

        // Invalidate cached texture for this bookmark.
        self.thumbnail_cache.remove(&idx);

        self.bookmark_store.update_viewport(idx, |bm| {
            bm.mode = self.mode.label().to_string();
            bm.center_re = self.viewport.center.re;
            bm.center_im = self.viewport.center.im;
            bm.scale = self.viewport.scale;
            bm.max_iterations = self.params.max_iterations;
            bm.escape_radius = self.params.escape_radius;
            bm.palette_index = self.display_color.palette_index;
            bm.smooth_coloring = self.display_color.smooth_coloring;
            bm.display_color = Some(self.display_color.clone());
            bm.aa_level = self.aa_level;
            bm.julia_c_re = self.julia_c.re;
            bm.julia_c_im = self.julia_c.im;
            bm.thumbnail_png = thumbnail_png;
        });
        info!(
            "Updated bookmark: {}",
            self.bookmark_store.bookmarks()[idx].name
        );
    }

    /// Get or lazily decode a thumbnail texture for a bookmark (by index).
    fn get_thumbnail(
        &mut self,
        bm_index: usize,
        base64_png: &str,
        ctx: &egui::Context,
    ) -> Option<&egui::TextureHandle> {
        if base64_png.is_empty() || self.failed_thumbnails.contains(&bm_index) {
            return None;
        }
        if let std::collections::hash_map::Entry::Vacant(entry) =
            self.thumbnail_cache.entry(bm_index)
        {
            if let Some((pixels, w, h)) = bookmarks::decode_thumbnail(base64_png) {
                let image =
                    egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
                let handle = ctx.load_texture(
                    format!("thumb_{bm_index}"),
                    image,
                    egui::TextureOptions::LINEAR,
                );
                entry.insert(handle);
            } else {
                self.failed_thumbnails.insert(bm_index);
                return None;
            }
        }
        self.thumbnail_cache.get(&bm_index)
    }

    /// Restore the app state from a bookmark.
    fn jump_to_bookmark(&mut self, bm: &Bookmark) {
        self.mode = match bm.mode.as_str() {
            "Julia" => FractalMode::Julia,
            _ => FractalMode::Mandelbrot,
        };
        self.julia_c = Complex::new(bm.julia_c_re, bm.julia_c_im);
        self.params.max_iterations = bm.max_iterations;
        self.params.set_escape_radius(bm.escape_radius);
        if let Some(ref dc) = bm.display_color {
            self.display_color = dc.clone();
            if self.display_color.palette_index >= self.palettes.len() {
                self.display_color.palette_index = 0;
            }
        } else {
            if bm.palette_index < self.palettes.len() {
                self.display_color.palette_index = bm.palette_index;
            }
            self.display_color.smooth_coloring = bm.smooth_coloring;
        }
        self.aa_level = bm.aa_level;
        self.bump_minimap_revision(); // display_color may have changed

        self.push_history();
        self.viewport = Viewport::new(
            Complex::new(bm.center_re, bm.center_im),
            bm.scale,
            self.panel_size[0],
            self.panel_size[1],
        )
        .unwrap_or_else(|_| Viewport::default_mandelbrot(self.panel_size[0], self.panel_size[1]));
        self.needs_render = true;
        info!("Jumped to bookmark: {}", bm.name);
    }

    /// Capture current state into a `LastView` for preferences.
    fn capture_last_view(&self) -> LastView {
        LastView {
            mode: self.mode.label().to_string(),
            center_re: self.viewport.center.re,
            center_im: self.viewport.center.im,
            scale: self.viewport.scale,
            max_iterations: self.params.max_iterations,
            escape_radius: self.params.escape_radius,
            palette_index: self.display_color.palette_index,
            smooth_coloring: self.display_color.smooth_coloring,
            aa_level: self.aa_level,
            julia_c_re: self.julia_c.re,
            julia_c_im: self.julia_c.im,
        }
    }

    // -- Palette helpers -------------------------------------------------------

    fn current_palette(&self) -> &Palette {
        &self.palettes[self.display_color.palette_index]
    }

    fn color_params(&self) -> ColorParams {
        let start_from = match self.display_color.start_from {
            DisplayStartFrom::None => RenderStartFrom::None,
            DisplayStartFrom::Black => RenderStartFrom::Black,
            DisplayStartFrom::White => RenderStartFrom::White,
        };
        ColorParams {
            smooth: self.display_color.smooth_coloring,
            cycle_length: self.display_color.cycle_length(self.params.max_iterations),
            start_from,
            low_threshold_start: self.display_color.low_threshold_start,
            low_threshold_end: self.display_color.low_threshold_end,
        }
    }

    /// Colorize an iteration buffer, using AA data when available.
    fn colorize_current(
        &self,
        iter_buf: &IterationBuffer,
        aa: Option<&AaSamples>,
    ) -> mandelbrust_render::RenderBuffer {
        let params = self.color_params();
        if let Some(aa) = aa {
            self.current_palette().colorize_aa(iter_buf, aa, &params)
        } else {
            self.current_palette().colorize(iter_buf, &params)
        }
    }

    /// Re-colorize from stored iteration data and update the texture.
    fn recolorize(&mut self, ctx: &egui::Context) {
        if let Some(ref iter_buf) = self.current_iterations {
            let buffer = self.colorize_current(iter_buf, self.current_aa.as_ref());
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [buffer.width as usize, buffer.height as usize],
                &buffer.pixels,
            );
            self.texture = Some(ctx.load_texture("fractal", image, egui::TextureOptions::LINEAR));
            // The texture now matches the (possibly shifted) iteration data,
            // so any lingering draw_offset is no longer valid.
            self.draw_offset = egui::Vec2::ZERO;
        }
    }

    // -- Effective parameters --------------------------------------------------

    fn effective_params(&self) -> FractalParams {
        if !self.adaptive_iterations {
            return self.params;
        }
        let default_scale = 3.6 / 1280.0_f64;
        let zoom = default_scale / self.viewport.scale;
        if zoom <= 1.0 {
            return self.params;
        }
        let bonus = (zoom.log2() * ADAPTIVE_ITER_RATE) as u32;
        self.params
            .with_max_iterations(self.params.max_iterations.saturating_add(bonus))
    }

    fn effective_max_iterations(&self) -> u32 {
        self.effective_params().max_iterations
    }

    // -- Pan offset ------------------------------------------------------------

    /// Apply any accumulated drag offset to the viewport centre.
    ///
    /// Called automatically before history operations and viewport modifications
    /// so the viewport is always up-to-date.
    fn commit_pan_offset(&mut self) {
        if self.pan_offset != egui::Vec2::ZERO {
            self.viewport.center.re -= self.pan_offset.x as f64 * self.viewport.scale;
            self.viewport.center.im += self.pan_offset.y as f64 * self.viewport.scale;
            self.pan_offset = egui::Vec2::ZERO;
        }
    }

    // -- History ---------------------------------------------------------------

    fn push_history(&mut self) {
        self.commit_pan_offset();
        self.history.truncate(self.history_pos + 1);
        self.history.push(self.viewport);
        self.history_pos = self.history.len() - 1;
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
            self.history_pos = self.history.len() - 1;
        }
    }

    fn go_back(&mut self) {
        self.commit_pan_offset();
        if self.history_pos > 0 {
            self.history_pos -= 1;
            self.viewport = self.history[self.history_pos];
            self.needs_render = true;
        }
    }

    fn go_forward(&mut self) {
        self.commit_pan_offset();
        if self.history_pos + 1 < self.history.len() {
            self.history_pos += 1;
            self.viewport = self.history[self.history_pos];
            self.needs_render = true;
        }
    }

    // -- Navigation ------------------------------------------------------------

    fn zoom_at_cursor(&mut self, cursor_px: u32, cursor_py: u32, factor: f64) {
        let target = self.viewport.pixel_to_complex(cursor_px, cursor_py);
        self.viewport.center = Complex::new(
            target.re + (self.viewport.center.re - target.re) * factor,
            target.im + (self.viewport.center.im - target.im) * factor,
        );
        self.viewport.scale *= factor;
        self.needs_render = true;
    }

    fn zoom_center(&mut self, factor: f64) {
        self.push_history();
        self.viewport.scale *= factor;
        self.needs_render = true;
    }

    fn pan_by_fraction(&mut self, fx: f64, fy: f64) {
        self.push_history();
        self.viewport.center.re += fx * self.viewport.complex_width();
        self.viewport.center.im += fy * self.viewport.complex_height();
        self.needs_render = true;
    }

    fn default_viewport(&self) -> Viewport {
        let (w, h) = (self.viewport.width, self.viewport.height);
        match self.mode {
            FractalMode::Mandelbrot => Viewport::default_mandelbrot(w, h),
            FractalMode::Julia => Viewport::default_julia(w, h),
        }
    }

    /// Viewport for the minimap: default overview of the current fractal (square, 1:1).
    /// Mandelbrot: default Mandelbrot view. Julia: default Julia view (zoomed-out Julia set).
    fn minimap_viewport(&self) -> Viewport {
        let size = self.preferences.minimap_size.side_pixels();
        match self.mode {
            FractalMode::Mandelbrot => Viewport::default_mandelbrot(size, size),
            FractalMode::Julia => Viewport::default_julia(size, size),
        }
    }

    /// Bump minimap cache revision so the overview will be re-rendered.
    fn bump_minimap_revision(&mut self) {
        self.minimap_revision = self.minimap_revision.wrapping_add(1);
    }

    fn request_minimap_if_invalid(&mut self, ctx: &egui::Context) {
        if self.minimap_loading {
            return;
        }
        let current_rev = self.minimap_revision;
        let texture_rev = self.minimap_texture.as_ref().map(|(_, r)| *r).unwrap_or(current_rev.wrapping_add(1));
        if texture_rev == current_rev {
            return;
        }
        self.minimap_loading = true;
        let params = self.params.with_max_iterations(self.preferences.minimap_iterations);
        let viewport = self.minimap_viewport();
        let tx = self.tx_minimap.clone();
        let revision = current_rev;
        let mode = self.mode;
        let julia_c = self.julia_c;
        const MINIMAP_AA: u32 = 4;
        thread::spawn(move || {
            let cancel = Arc::new(RenderCancel::new());
            let result = render_for_mode(mode, params, julia_c, &viewport, &cancel, MINIMAP_AA);
            let _ = tx.send((result, revision));
        });
        ctx.request_repaint();
    }

    fn poll_minimap_response(&mut self, ctx: &egui::Context) {
        while let Ok((result, revision)) = self.rx_minimap.try_recv() {
            if result.cancelled {
                self.minimap_loading = false;
                continue;
            }
            let params = self.color_params();
            let buffer = if let Some(ref aa) = result.aa_samples {
                self.current_palette().colorize_aa(&result.iterations, aa, &params)
            } else {
                self.current_palette().colorize(&result.iterations, &params)
            };
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [buffer.width as usize, buffer.height as usize],
                &buffer.pixels,
            );
            let handle = ctx.load_texture("minimap", image, egui::TextureOptions::LINEAR);
            self.minimap_texture = Some((handle, revision));
            self.minimap_loading = false;
            ctx.request_repaint();
        }
    }

    fn show_minimap_panel(&mut self, ctx: &egui::Context, hud_alpha: u8) {
        let size = self.preferences.minimap_size.side_pixels() as f32;
        let vp = self.minimap_viewport();
        let minimap_alpha = (hud_alpha as f32 * self.preferences.minimap_opacity.clamp(0.0, 1.0)).round() as u8;
        let image_alpha = (255.0 * self.preferences.minimap_opacity.clamp(0.0, 1.0)).round() as u8;

        // Same margin from viewport edges as other HUD panels. For RIGHT_BOTTOM, negative offset = inset from corner.
        const MINIMAP_ANCHOR_MARGIN: f32 = 8.0;
        egui::Area::new(egui::Id::new("hud_minimap"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-MINIMAP_ANCHOR_MARGIN, -MINIMAP_ANCHOR_MARGIN])
            .show(ctx, |ui| {
                // No inner margin: white border is the outmost layer; no black band outside it.
                egui::Frame::NONE
                    .fill(egui::Color32::from_black_alpha(minimap_alpha))
                    .inner_margin(egui::Margin::ZERO)
                    .corner_radius(0.0)
                    .show(ui, |ui| {
                        let (rect, _response) =
                            ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());

                        // Draw the fractal in a square sub-rect so aspect ratio is always 1:1 (avoids deformation).
                        let image_side = rect.width().min(rect.height());
                        let image_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(image_side, image_side));

                        let to_minimap = |c: Complex| {
                            let px = (c.re - vp.center.re) / vp.scale + (vp.width as f64) * 0.5;
                            let py = (vp.height as f64) * 0.5 - (c.im - vp.center.im) / vp.scale;
                            let sx = image_rect.min.x + (px as f32 / vp.width as f32) * image_rect.width();
                            let sy = image_rect.min.y + (py as f32 / vp.height as f32) * image_rect.height();
                            (sx, sy)
                        };

                        let valid_texture = self
                            .minimap_texture
                            .as_ref()
                            .filter(|(_, rev)| *rev == self.minimap_revision)
                            .map(|(h, _)| h);

                        if let Some(tex) = valid_texture {
                            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                            ui.painter().image(
                                tex.id(),
                                image_rect,
                                uv,
                                egui::Color32::from_white_alpha(image_alpha),
                            );
                        } else if self.minimap_loading {
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "Updating…",
                                egui::FontId::proportional(14.0),
                                egui::Color32::GRAY,
                            );
                        } else {
                            ui.painter().rect_filled(
                                rect,
                                0.0,
                                egui::Color32::from_black_alpha(120),
                            );
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "Updating…",
                                egui::FontId::proportional(12.0),
                                egui::Color32::GRAY,
                            );
                        }

                        let crosshair_alpha = (self.preferences.crosshair_opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
                        let crosshair_color = egui::Color32::from_white_alpha(crosshair_alpha);

                        // Viewport rectangle (cyan) and crosshairs only outside it (both modes)
                        let cx = self.viewport.center.re;
                        let cy = self.viewport.center.im;
                        let w = self.viewport.width as f64 * self.viewport.scale;
                        let h = self.viewport.height as f64 * self.viewport.scale;
                        let (min_x, min_y) = to_minimap(Complex::new(cx - w * 0.5, cy + h * 0.5));
                        let (max_x, max_y) = to_minimap(Complex::new(cx + w * 0.5, cy - h * 0.5));
                        let min_x = min_x.clamp(image_rect.min.x, image_rect.max.x);
                        let max_x = max_x.clamp(image_rect.min.x, image_rect.max.x);
                        let min_y = min_y.clamp(image_rect.min.y, image_rect.max.y);
                        let max_y = max_y.clamp(image_rect.min.y, image_rect.max.y);
                        let viewport_rect = egui::Rect::from_min_max(
                            egui::pos2(min_x, min_y),
                            egui::pos2(max_x, max_y),
                        );
                        let stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(0, 255, 255));
                        ui.painter().rect_stroke(viewport_rect, 0.0, stroke, egui::StrokeKind::Outside);
                        let center_x = (min_x + max_x) * 0.5;
                        let center_y = (min_y + max_y) * 0.5;
                        // Crosshairs from image edge to viewport rect edge only (not inside the cyan rect)
                        if image_rect.min.y < viewport_rect.min.y {
                            ui.painter().line_segment(
                                [egui::pos2(center_x, image_rect.min.y), egui::pos2(center_x, viewport_rect.min.y)],
                                egui::Stroke::new(1.0, crosshair_color),
                            );
                        }
                        if viewport_rect.max.y < image_rect.max.y {
                            ui.painter().line_segment(
                                [egui::pos2(center_x, viewport_rect.max.y), egui::pos2(center_x, image_rect.max.y)],
                                egui::Stroke::new(1.0, crosshair_color),
                            );
                        }
                        if image_rect.min.x < viewport_rect.min.x {
                            ui.painter().line_segment(
                                [egui::pos2(image_rect.min.x, center_y), egui::pos2(viewport_rect.min.x, center_y)],
                                egui::Stroke::new(1.0, crosshair_color),
                            );
                        }
                        if viewport_rect.max.x < image_rect.max.x {
                            ui.painter().line_segment(
                                [egui::pos2(viewport_rect.max.x, center_y), egui::pos2(image_rect.max.x, center_y)],
                                egui::Stroke::new(1.0, crosshair_color),
                            );
                        }

                        // Minimap panel border: 1px white, 75% opacity
                        let border_stroke = egui::Stroke::new(1.0, egui::Color32::from_white_alpha(191));
                        ui.painter().rect_stroke(rect, 0.0, border_stroke, egui::StrokeKind::Outside);
                    });
            });
    }

    fn reset_view(&mut self) {
        self.push_history();
        self.viewport = self.default_viewport();
        self.needs_render = true;
    }

    // -- Background rendering --------------------------------------------------

    fn request_render(&mut self) {
        // Cancel any in-flight render.
        self.cancel.cancel();

        self.render_id += 1;

        // After a pan, the shifted texture already provides good coverage.
        // Record this render_id so poll_responses can skip its preview.
        if self.pan_completed {
            self.skip_preview_id = self.render_id;
            self.pan_completed = false;
        }

        let params = self.effective_params();
        debug!(
            id = self.render_id,
            max_iter = params.max_iterations,
            scale = self.viewport.scale,
            "Requesting render"
        );

        let req = RenderRequest {
            id: self.render_id,
            viewport: self.viewport,
            params,
            mode: self.mode,
            julia_c: self.julia_c,
            aa_level: self.aa_level,
        };

        let _ = self.tx_request.send(req);
        self.render_phase = RenderPhase::Rendering;
        self.needs_render = false;
    }

    /// Request a low-res preview for the current panned viewport.
    ///
    /// Called on every drag frame.  Because requests arrive faster than the
    /// render thread can produce full passes, the thread only ever finishes
    /// preview passes during an active drag.
    fn request_drag_preview(&mut self) {
        self.cancel.cancel();
        self.render_id += 1;

        // Viewport with accumulated pan offset applied.
        let mut viewport = self.viewport;
        viewport.center.re -= self.pan_offset.x as f64 * viewport.scale;
        viewport.center.im += self.pan_offset.y as f64 * viewport.scale;

        let params = self.effective_params();
        let req = RenderRequest {
            id: self.render_id,
            viewport,
            params,
            mode: self.mode,
            julia_c: self.julia_c,
            aa_level: 0, // No AA for drag previews.
        };

        let _ = self.tx_request.send(req);
        self.render_phase = RenderPhase::Rendering;
    }

    /// Phase 10: Process incoming Julia grid cell results; colorize and cache texture.
    fn poll_julia_grid_responses(&mut self, ctx: &egui::Context) {
        while let Ok((i, j, result)) = self.rx_julia_grid_resp.try_recv() {
            if result.cancelled {
                continue;
            }
            self.julia_explorer_cells
                .insert((i, j), result.iterations.clone());
            let mut params = self.color_params();
            params.cycle_length = self.display_color.cycle_length(result.iterations.max_iterations);
            let buffer = if let Some(aa) = result.aa_samples.as_ref() {
                self.current_palette().colorize_aa(&result.iterations, aa, &params)
            } else {
                self.current_palette().colorize(&result.iterations, &params)
            };
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [buffer.width as usize, buffer.height as usize],
                &buffer.pixels,
            );
            let name = format!("julia_cell_{}_{}", i, j);
            let tex = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
            self.julia_explorer_textures.insert((i, j), tex);
        }
    }

    fn poll_responses(&mut self, ctx: &egui::Context) {
        while let Ok(resp) = self.rx_response.try_recv() {
            match resp {
                RenderResponse::Preview { id, result } => {
                    if id == self.render_id && !result.cancelled {
                        if self.drag_active {
                            // During drag: update background preview only.
                            self.apply_drag_preview(ctx, result);
                        } else if id == self.skip_preview_id {
                            // After a pan the shifted texture is better than
                            // the preview — skip it and wait for the full pass.
                            self.skip_preview_id = 0;
                            self.render_phase = RenderPhase::Refining;
                        } else {
                            self.apply_result(ctx, result);
                            self.render_phase = RenderPhase::Refining;
                        }
                    }
                }
                RenderResponse::Final { id, result } => {
                    if id == self.render_id && !result.cancelled {
                        self.apply_result(ctx, result);
                        self.render_phase = RenderPhase::Done;
                    }
                }
            }
        }
    }

    fn apply_result(&mut self, ctx: &egui::Context, result: RenderResult) {
        self.render_time = result.elapsed;
        self.tiles_rendered = result.tiles_rendered;
        self.tiles_mirrored = result.tiles_mirrored;
        self.tiles_border_traced = result.tiles_border_traced;

        // Colorize using the current palette, with AA if available.
        let buffer = self.colorize_current(&result.iterations, result.aa_samples.as_ref());
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [buffer.width as usize, buffer.height as usize],
            &buffer.pixels,
        );
        self.texture = Some(ctx.load_texture("fractal", image, egui::TextureOptions::LINEAR));
        self.current_iterations = Some(result.iterations);
        self.current_aa = result.aa_samples;

        // The new render covers the full viewport — clear pan artefacts.
        self.drag_preview = None;
        self.draw_offset = egui::Vec2::ZERO;
    }

    /// Colorize a preview result into the drag background texture.
    fn apply_drag_preview(&mut self, ctx: &egui::Context, result: RenderResult) {
        let params = self.color_params();
        let buffer = self.current_palette().colorize(&result.iterations, &params);
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [buffer.width as usize, buffer.height as usize],
            &buffer.pixels,
        );
        self.drag_preview =
            Some(ctx.load_texture("drag_preview", image, egui::TextureOptions::LINEAR));
    }

    fn cancel_render(&mut self) {
        self.cancel.cancel();
        if self.render_phase == RenderPhase::Rendering || self.render_phase == RenderPhase::Refining
        {
            self.render_phase = RenderPhase::Done;
            info!("Render cancelled by user");
        }
    }

    /// Phase 10: Start (or restart) the Julia C Explorer grid render.
    /// Uses julia_explorer_cols/rows (from viewport) and extent_half for C range.
    fn start_julia_grid_request(&mut self) {
        const JULIA_EXPLORER_CENTER_RE: f64 = -0.75;
        const JULIA_EXPLORER_CENTER_IM: f64 = 0.0;
        let cols = self.julia_explorer_cols.max(1);
        let rows = self.julia_explorer_rows.max(1);
        let cell_size_px = self.preferences.julia_explorer_cell_size_px.clamp(16, 256);
        self.julia_explorer_cells.clear();
        self.julia_explorer_textures.clear();
        self.grid_cancel.cancel();
        let new_cancel = Arc::new(RenderCancel::new());
        self.grid_cancel = new_cancel.clone();
        let req = JuliaGridRequest {
            cols,
            rows,
            center_re: JULIA_EXPLORER_CENTER_RE,
            center_im: JULIA_EXPLORER_CENTER_IM,
            extent_half: self.julia_explorer_extent_half,
            cell_size: cell_size_px,
            max_iterations: self.preferences.julia_explorer_max_iterations,
            aa_level: 4,
            cancel: new_cancel,
        };
        let _ = self.tx_julia_grid_req.send(req);
    }

    fn check_resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 && (width != self.panel_size[0] || height != self.panel_size[1])
        {
            self.panel_size = [width, height];
            self.viewport.width = width;
            self.viewport.height = height;
            self.needs_render = true;
        }
    }

    // -- Precision warning -----------------------------------------------------

    fn precision_warning(&self) -> Option<&'static str> {
        if self.viewport.scale < PRECISION_WARN_SCALE {
            warn!(
                "Approaching f64 precision limits at scale={:.2e}",
                self.viewport.scale
            );
            Some("Approaching f64 precision limits \u{2014} artifacts may appear")
        } else {
            None
        }
    }

    // -- Input handling --------------------------------------------------------

    fn handle_canvas_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        // Track cursor position in complex plane.
        self.cursor_complex = response.hover_pos().map(|pos| {
            let px = (pos.x - response.rect.min.x) as u32;
            let py = (pos.y - response.rect.min.y) as u32;
            self.viewport.pixel_to_complex(px, py)
        });

        // -- Mouse wheel zoom (cursor-centred) --------------------------------
        let scroll_y = ctx.input(|i| i.raw_scroll_delta.y);
        if scroll_y.abs() > 0.0 && response.hovered() {
            if let Some(pos) = response.hover_pos() {
                let px = (pos.x - response.rect.min.x).max(0.0) as u32;
                let py = (pos.y - response.rect.min.y).max(0.0) as u32;
                let factor = (1.0 - scroll_y as f64 * ZOOM_SPEED).clamp(0.1, 10.0);
                if !self.drag_active {
                    self.push_history();
                }
                self.zoom_at_cursor(px, py, factor);
            }
        }

        // -- Left-click drag: pan -------------------------------------------------
        // During drag we accumulate a pixel offset, shift the existing high-
        // quality texture, and continuously request low-res previews so that
        // newly exposed edges show a preview instead of black.
        if response.drag_started_by(egui::PointerButton::Primary) {
            self.drag_active = true;
            self.push_history();
        }
        if response.dragged_by(egui::PointerButton::Primary) {
            self.pan_offset += response.drag_delta();
            self.request_drag_preview();
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            self.drag_active = false;

            // Immediately cancel in-flight drag renders and invalidate their
            // responses so stale results (which still carry the old render_id)
            // don't overwrite the shifted texture on the next frame.
            self.cancel.cancel();
            self.render_id += 1;

            // Keep drawing the existing texture at the drag offset until the
            // final render arrives.  The drag preview fills exposed edges
            // with a low-res image, avoiding a black flash.
            self.draw_offset = self.pan_offset;

            let dx = self.pan_offset.x.round() as i32;
            let dy = self.pan_offset.y.round() as i32;
            self.commit_pan_offset();

            // Shift stored iteration + AA data so they stay in sync with the
            // new viewport (needed for correct palette switching).
            if let Some(ref mut iter_buf) = self.current_iterations {
                iter_buf.shift(dx, dy);
                if let Some(ref mut aa) = self.current_aa {
                    aa.shift(dx, dy);
                }
            }

            self.pan_completed = true;
            self.needs_render = true;
        }

        // -- Right-click drag: selection zoom ------------------------------------
        // The user draws a rectangle; when released the viewport zooms to fit
        // that region.
        if response.drag_started_by(egui::PointerButton::Secondary) {
            self.zoom_rect_start = response.interact_pointer_pos();
        }
        if response.drag_stopped_by(egui::PointerButton::Secondary) {
            if let (Some(start), Some(end)) = (self.zoom_rect_start.take(), response.hover_pos()) {
                let dx = (end.x - start.x).abs();
                let dy = (end.y - start.y).abs();

                // Minimum drag size to avoid accidental micro-zooms.
                if dx > 5.0 || dy > 5.0 {
                    let rect = response.rect;
                    let vp_w = rect.width();
                    let vp_h = rect.height();

                    // Fraction of viewport covered by the drag (aspect-locked).
                    let fraction = (dx / vp_w).max(dy / vp_h).max(0.01);

                    // Midpoint of the selection → new centre.
                    let mid_x = (start.x + end.x) / 2.0;
                    let mid_y = (start.y + end.y) / 2.0;
                    let px = (mid_x - rect.min.x) as u32;
                    let py = (mid_y - rect.min.y) as u32;
                    let new_center = self.viewport.pixel_to_complex(px, py);

                    self.push_history();
                    self.viewport.center = new_center;
                    self.viewport.scale *= fraction as f64;
                    self.needs_render = true;
                }
            }
            self.zoom_rect_start = None;
        }

        // -- Click to select Julia parameter (in Julia mode, Shift+click) ------
        if self.mode == FractalMode::Julia && response.clicked() && ctx.input(|i| i.modifiers.shift)
        {
            if let Some(c) = self.cursor_complex {
                self.julia_c = c;
                self.bump_minimap_revision();
                self.needs_render = true;
            }
        }
    }

    fn handle_keyboard(&mut self, ctx: &egui::Context) {
        // When a text widget has focus, suppress single-letter shortcuts so
        // the user can type freely without toggling HUD, AA, etc.
        let text_editing = ctx.memory(|m| m.focused().is_some());

        ctx.input(|input| {
            // Arrow keys: pan (always active)
            if input.key_pressed(egui::Key::ArrowLeft) {
                self.pan_by_fraction(-PAN_FRACTION, 0.0);
            }
            if input.key_pressed(egui::Key::ArrowRight) {
                self.pan_by_fraction(PAN_FRACTION, 0.0);
            }
            if input.key_pressed(egui::Key::ArrowUp) {
                self.pan_by_fraction(0.0, PAN_FRACTION);
            }
            if input.key_pressed(egui::Key::ArrowDown) {
                self.pan_by_fraction(0.0, -PAN_FRACTION);
            }

            // +/- : zoom (always active)
            if input.key_pressed(egui::Key::Plus) || input.key_pressed(egui::Key::Equals) {
                self.zoom_center(0.8);
            }
            if input.key_pressed(egui::Key::Minus) {
                self.zoom_center(1.25);
            }

            // Escape: cancel render / close dialogs (always active)
            if input.key_pressed(egui::Key::Escape) {
                if self.show_update_or_save_dialog {
                    self.show_update_or_save_dialog = false;
                } else if self.show_save_dialog {
                    self.show_save_dialog = false;
                } else if self.show_julia_c_explorer {
                    self.show_julia_c_explorer = false;
                } else if self.show_bookmarks {
                    self.show_bookmarks = false;
                } else if self.show_help {
                    self.show_help = false;
                } else if self.show_controls {
                    self.show_controls = false;
                } else {
                    self.cancel_render();
                }
            }

            if text_editing {
                return; // Skip letter-key shortcuts while typing.
            }

            // R: reset view
            if input.key_pressed(egui::Key::R) && !input.modifiers.ctrl {
                self.reset_view();
            }

            // H: toggle HUD
            if input.key_pressed(egui::Key::H) {
                self.show_hud = !self.show_hud;
            }

            // C: toggle crosshair
            if input.key_pressed(egui::Key::C) {
                self.show_crosshair = !self.show_crosshair;
            }

            // S: open save-bookmark dialog (or update-or-save if a bookmark was jumped to)
            if input.key_pressed(egui::Key::S) && !input.modifiers.ctrl {
                if self.last_jumped_bookmark_idx.is_some() {
                    // Ask user whether to update existing or save new.
                    self.show_update_or_save_dialog = true;
                } else {
                    self.open_save_new_dialog();
                }
            }

            // B: toggle bookmark browser (default to current fractal tab)
            if input.key_pressed(egui::Key::B) {
                self.show_bookmarks = !self.show_bookmarks;
                if self.show_bookmarks {
                    // Reload from disk so externally added files appear.
                    self.bookmark_store.reload();
                    self.thumbnail_cache.clear();
                    self.failed_thumbnails.clear();
                    self.bookmark_tab = match self.mode {
                        FractalMode::Mandelbrot => BookmarkTab::Mandelbrot,
                        FractalMode::Julia => BookmarkTab::Julia,
                    };
                }
            }

            // J: open Julia C Explorer (Phase 10). Grid request is sent after first draw (cols/rows from viewport).
            if input.key_pressed(egui::Key::J) {
                let was_julia = self.mode == FractalMode::Julia;
                self.mode = FractalMode::Julia;
                if !was_julia {
                    if let Some(c) = self.cursor_complex {
                        self.julia_c = c;
                        self.bump_minimap_revision();
                    }
                    self.push_history();
                    self.viewport = self.default_viewport();
                    self.needs_render = true;
                }
                self.show_julia_c_explorer = true;
            }

            // M: toggle minimap (Phase 9)
            if input.key_pressed(egui::Key::M) {
                self.preferences.show_minimap = !self.preferences.show_minimap;
                self.preferences.save();
            }
        });

        if text_editing {
            return;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Backspace) && !i.modifiers.shift) {
            self.go_back();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Backspace) && i.modifiers.shift) {
            self.go_forward();
        }

        // A: toggle anti-aliasing (Off → 2×2 → 4×4 → Off).
        if ctx.input(|i| i.key_pressed(egui::Key::A)) {
            let old_aa = self.aa_level;
            self.aa_level = match old_aa {
                0 => 2,
                2 => 4,
                _ => 0,
            };
            if self.aa_level == 0 {
                self.current_aa = None;
                self.recolorize(ctx);
            } else {
                self.needs_render = true;
            }
        }
    }

    // -- HUD -------------------------------------------------------------------

    fn show_hud(&mut self, ctx: &egui::Context) {
        if !self.show_hud {
            return;
        }
        if self.show_julia_c_explorer {
            return;
        }

        // -- Top-left: viewport info (read-only) ---------------------------------
        let hud_alpha = (self.preferences.hud_panel_opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        egui::Area::new(egui::Id::new("hud_params"))
            .anchor(egui::Align2::LEFT_TOP, [HUD_MARGIN, HUD_MARGIN])
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(egui::Color32::from_black_alpha(hud_alpha))
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(HUD_CORNER_RADIUS)
                    .show(ui, |ui| {
                        ui.style_mut().visuals.override_text_color =
                            Some(egui::Color32::from_rgb(220, 220, 220));

                        ui.label(format!("Mode: {}", self.mode.label()));
                        if self.mode == FractalMode::Julia {
                            ui.label(format!(
                                "Julia c: {:.6} {:+.6}i",
                                self.julia_c.re, self.julia_c.im
                            ));
                        }
                        ui.label(format!(
                            "Center: {:.10} {:+.10}i",
                            self.viewport.center.re, self.viewport.center.im
                        ));
                        let zoom_level = 1.0 / self.viewport.scale;
                        ui.label(format!("Zoom: {zoom_level:.2e}"));
                        ui.label(format!("Iterations: {}", self.params.max_iterations));

                        ui.label(format!(
                            "Palette: {} ({})",
                            self.current_palette().name,
                            if self.display_color.smooth_coloring {
                                "smooth"
                            } else {
                                "raw"
                            }
                        ));

                        if let Some(warning) = self.precision_warning() {
                            ui.colored_label(egui::Color32::from_rgb(255, 180, 50), warning);
                        }
                    });
            });

        // -- Bottom-centre: render stats (Phase 9: moved from bottom-right for minimap) --
        // Negative y: egui adds offset to anchor; bottom anchor is at screen bottom, so -y moves panel up.
        egui::Area::new(egui::Id::new("hud_render"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -HUD_MARGIN])
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(egui::Color32::from_black_alpha(hud_alpha))
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(HUD_CORNER_RADIUS)
                    .show(ui, |ui| {
                        ui.set_min_width(180.0);
                        ui.style_mut().visuals.override_text_color =
                            Some(egui::Color32::from_rgb(200, 200, 200));
                        ui.style_mut().spacing.item_spacing.y = 2.0;

                        let phase_color = match self.render_phase {
                            RenderPhase::Idle => egui::Color32::GRAY,
                            RenderPhase::Rendering | RenderPhase::Refining => egui::Color32::YELLOW,
                            RenderPhase::Done => egui::Color32::from_rgb(100, 255, 100),
                        };
                        ui.colored_label(phase_color, self.render_phase.label());

                        ui.label(format!("{:.1} ms", self.render_time.as_secs_f64() * 1000.0,));
                        ui.label(format!(
                            "{} tiles, {} mirrored, {} bt",
                            self.tiles_rendered, self.tiles_mirrored, self.tiles_border_traced,
                        ));

                        if let Some(ref aa) = self.current_aa {
                            ui.label(format!(
                                "AA {}x{} ({} boundary px)",
                                aa.aa_level, aa.aa_level, aa.boundary_count
                            ));
                        } else if self.aa_level > 0 {
                            ui.label(format!("AA {}x{} (pending)", self.aa_level, self.aa_level));
                        }
                    });
            });

        // -- Bottom-right: minimap (Phase 9) ------------------------------------
        let show_minimap = self.preferences.show_minimap;
        if show_minimap {
            self.show_minimap_panel(ctx, hud_alpha);
        }

        // -- Top-right: toolbar icons + fractal params --------------------------
        self.show_top_right_toolbar(ctx);
    }

    // -- Top-right toolbar & fractal params ------------------------------------

    fn show_top_right_toolbar(&mut self, ctx: &egui::Context) {
        use egui_material_icons::icons::*;

        let hud_alpha = (self.preferences.hud_panel_opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        let icon_on = egui::Color32::from_rgb(200, 200, 200);
        let icon_off = egui::Color32::from_rgb(90, 90, 90);
        let mi = |icon: &str| egui::RichText::new(icon).size(18.0).color(icon_on);
        let mi_state = |icon: &str, active: bool| {
            egui::RichText::new(icon)
                .size(18.0)
                .color(if active { icon_on } else { icon_off })
        };

        let mut params_changed = false;
        let mut palette_changed = false;
        let mut mode_changed = false;

        // ---- Icon bar ----
        // Fixed cell width so every icon gets the same space, evenly distributed.
        let cell = egui::vec2(26.0, 22.0);

        // Helper: add a single icon button centered inside a fixed-width cell.
        // Returns the Response so callers can chain .clicked(), etc.
        let add_icon_btn =
            |ui: &mut egui::Ui, label: egui::RichText, enabled: bool| -> egui::Response {
                ui.allocate_ui_with_layout(cell, egui::Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
                    ui.add_enabled(enabled, egui::Button::new(label).frame(false))
                })
                .inner
            };

        egui::Area::new(egui::Id::new("hud_toolbar"))
            .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(egui::Color32::from_black_alpha(160))
                    .inner_margin(egui::Margin::same(4))
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;

                            // Back
                            if add_icon_btn(ui, mi(ICON_ARROW_BACK), self.history_pos > 0)
                                .on_hover_text("Back")
                                .clicked()
                            {
                                self.go_back();
                            }
                            // Forward
                            if add_icon_btn(
                                ui,
                                mi(ICON_ARROW_FORWARD),
                                self.history_pos + 1 < self.history.len(),
                            )
                            .on_hover_text("Forward")
                            .clicked()
                            {
                                self.go_forward();
                            }
                            // Reset view
                            if add_icon_btn(ui, mi(ICON_RESTART_ALT), true)
                                .on_hover_text("Reset view")
                                .clicked()
                            {
                                self.reset_view();
                            }
                            // Display/color settings
                            let pal_name = self.palettes[self.display_color.palette_index].name;
                            if add_icon_btn(ui, mi(ICON_PALETTE), true)
                                .on_hover_text(format!("Display/color settings ({pal_name})"))
                                .clicked()
                            {
                                self.show_palette_popup = !self.show_palette_popup;
                            }
                            // AA cycle
                            let aa_on = self.aa_level > 0;
                            let aa_label = match self.aa_level {
                                2 => "2x2",
                                4 => "4x4",
                                _ => "Off",
                            };
                            if add_icon_btn(ui, mi_state(ICON_DEBLUR, aa_on), true)
                                .on_hover_text(format!(
                                    "Anti-aliasing: {aa_label} (click to cycle)"
                                ))
                                .clicked()
                            {
                                self.aa_level = match self.aa_level {
                                    0 => 2,
                                    2 => 4,
                                    _ => 0,
                                };
                                if self.aa_level == 0 {
                                    self.current_aa = None;
                                    palette_changed = true;
                                } else {
                                    params_changed = true;
                                }
                            }
                            // Save bookmark (always available; icon not greyed out)
                            if add_icon_btn(ui, mi(ICON_BOOKMARK_ADD), true)
                                .on_hover_text("Save bookmark (S)")
                                .clicked()
                            {
                                if self.last_jumped_bookmark_idx.is_some() {
                                    self.show_update_or_save_dialog = true;
                                } else {
                                    self.open_save_new_dialog();
                                }
                            }
                            // Bookmarks explorer
                            if add_icon_btn(
                                ui,
                                mi_state(ICON_BOOKMARKS, self.show_bookmarks),
                                true,
                            )
                            .on_hover_text("Bookmark explorer (B)")
                            .clicked()
                            {
                                self.show_bookmarks = !self.show_bookmarks;
                                if self.show_bookmarks {
                                    self.bookmark_store.reload();
                                    self.thumbnail_cache.clear();
                                    self.failed_thumbnails.clear();
                                }
                            }
                            // Minimap (Phase 9)
                            if add_icon_btn(
                                ui,
                                mi_state(ICON_MAP, self.preferences.show_minimap),
                                true,
                            )
                            .on_hover_text("Minimap (M)")
                            .clicked()
                            {
                                self.preferences.show_minimap = !self.preferences.show_minimap;
                                self.preferences.save();
                            }
                            // Controls & shortcuts
                            if add_icon_btn(ui, mi(ICON_HELP_OUTLINE), true)
                                .on_hover_text("Controls & shortcuts")
                                .clicked()
                            {
                                self.show_help = !self.show_help;
                            }
                            // Settings (always last)
                            if add_icon_btn(ui, mi(ICON_SETTINGS), true)
                                .on_hover_text("Settings")
                                .clicked()
                            {
                                self.show_controls = !self.show_controls;
                            }
                        });
                    });
            });

        // ---- Display/color settings panel ----
        if self.show_palette_popup {
            egui::Window::new("Display / color")
                .id(egui::Id::new("display_color_panel"))
                .collapsible(true)
                .resizable(true)
                .default_width(280.0)
                .anchor(egui::Align2::RIGHT_TOP, [-8.0, 38.0])
                .frame(
                    egui::Frame::NONE
                        .fill(egui::Color32::from_black_alpha(220))
                        .inner_margin(egui::Margin::same(10))
                        .corner_radius(6.0),
                )
                .show(ctx, |ui| {
                    ui.style_mut().visuals.override_text_color =
                        Some(egui::Color32::from_rgb(220, 220, 220));

                    // Profiles
                    ui.heading("Profiles");
                    let profile_names = color_profiles::list_profiles();
                    if self.color_profile_selected.is_empty() && !profile_names.is_empty() {
                        self.color_profile_selected = profile_names[0].clone();
                    }
                    egui::ComboBox::from_id_salt(egui::Id::new("color_profile_list"))
                        .selected_text(
                            if self.color_profile_selected.is_empty() {
                                "(none)"
                            } else {
                                &self.color_profile_selected
                            },
                        )
                        .show_ui(ui, |ui| {
                            for name in &profile_names {
                                ui.selectable_value(
                                    &mut self.color_profile_selected,
                                    name.clone(),
                                    name.as_str(),
                                );
                            }
                        });
                    if ui.button("Load").clicked() && !self.color_profile_selected.is_empty() {
                        let mut loaded =
                            color_profiles::load_profile(&self.color_profile_selected);
                        if loaded.palette_index >= self.palettes.len() {
                            loaded.palette_index = 0;
                        }
                        self.display_color = loaded;
                        palette_changed = true;
                        self.bump_minimap_revision();
                    }
                    ui.horizontal(|ui| {
                        ui.label("Save as:");
                        ui.text_edit_singleline(&mut self.color_profile_save_name);
                        let save_name = self.color_profile_save_name.trim();
                        let save_name = if save_name.is_empty() {
                            "Default"
                        } else {
                            save_name
                        };
                        if ui.button("Save").clicked() {
                            if let Err(e) =
                                color_profiles::save_profile(save_name, &self.display_color)
                            {
                                warn!("Failed to save color profile: {}", e);
                            }
                        }
                    });

                    ui.add_space(8.0);
                    // Palette list
                    ui.heading("Palette");
                    for (i, pal) in self.palettes.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let swatch = pal.preview_colors(40);
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(40.0, 12.0), egui::Sense::hover());
                            let painter = ui.painter_at(rect);
                            for (j, c) in swatch.iter().enumerate() {
                                painter.rect_filled(
                                    egui::Rect::from_min_size(
                                        egui::pos2(rect.min.x + j as f32, rect.min.y),
                                        egui::vec2(1.0, 12.0),
                                    ),
                                    0.0,
                                    egui::Color32::from_rgb(c[0], c[1], c[2]),
                                );
                            }
                            let label = if i == self.display_color.palette_index {
                                egui::RichText::new(pal.name).strong()
                            } else {
                                egui::RichText::new(pal.name)
                            };
                            if ui
                                .selectable_label(i == self.display_color.palette_index, label)
                                .clicked()
                            {
                                self.display_color.palette_index = i;
                                palette_changed = true;
                                self.pending_minimap_bump = true;
                            }
                        });
                    }

                    ui.add_space(8.0);
                    ui.heading("Palette mode");
                    ui.horizontal(|ui| {
                        let by_cycles = matches!(self.display_color.palette_mode, DisplayPaletteMode::ByCycles { .. });
                        if ui.selectable_label(by_cycles, "By cycles").clicked() {
                            let n = match self.display_color.palette_mode {
                                DisplayPaletteMode::ByCycles { n } => n,
                                DisplayPaletteMode::ByCycleLength { .. } => 1,
                            };
                            self.display_color.palette_mode = DisplayPaletteMode::ByCycles { n };
                            palette_changed = true;
                            self.bump_minimap_revision();
                        }
                        if ui.selectable_label(!by_cycles, "By cycle length").clicked() {
                            let len = match self.display_color.palette_mode {
                                DisplayPaletteMode::ByCycles { .. } => 256,
                                DisplayPaletteMode::ByCycleLength { len } => len,
                            };
                            self.display_color.palette_mode = DisplayPaletteMode::ByCycleLength { len };
                            palette_changed = true;
                            self.bump_minimap_revision();
                        }
                    });
                    let (mut mode_val, is_cycles) = match self.display_color.palette_mode {
                        DisplayPaletteMode::ByCycles { n } => (n as i32, true),
                        DisplayPaletteMode::ByCycleLength { len } => (len as i32, false),
                    };
                    if ui.add(egui::DragValue::new(&mut mode_val).range(1..=i32::MAX)).changed() {
                        let v = mode_val.max(1) as u32;
                        self.display_color.palette_mode = if is_cycles {
                            DisplayPaletteMode::ByCycles { n: v }
                        } else {
                            DisplayPaletteMode::ByCycleLength { len: v }
                        };
                        palette_changed = true;
                        self.bump_minimap_revision();
                    }
                    ui.label(if is_cycles { "cycles" } else { "iterations per cycle" });

                    ui.add_space(8.0);
                    ui.heading("Start from");
                    ui.horizontal(|ui| {
                        for (opt, label) in [
                            (DisplayStartFrom::None, "None"),
                            (DisplayStartFrom::Black, "Black"),
                            (DisplayStartFrom::White, "White"),
                        ] {
                            if ui
                                .selectable_label(self.display_color.start_from == opt, label)
                                .clicked()
                            {
                                self.display_color.start_from = opt;
                                palette_changed = true;
                                self.bump_minimap_revision();
                            }
                        }
                    });
                    if self.display_color.start_from != DisplayStartFrom::None {
                        ui.horizontal(|ui| {
                            ui.label("Threshold start:");
                            let mut start = self.display_color.low_threshold_start as i32;
                            if ui.add(egui::DragValue::new(&mut start).range(0..=i32::MAX)).changed() {
                                self.display_color.low_threshold_start = start.max(0) as u32;
                                palette_changed = true;
                                self.bump_minimap_revision();
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Threshold end:");
                            let mut end = self.display_color.low_threshold_end as i32;
                            if ui.add(egui::DragValue::new(&mut end).range(0..=i32::MAX)).changed() {
                                self.display_color.low_threshold_end = end.max(0) as u32;
                                palette_changed = true;
                                self.bump_minimap_revision();
                            }
                        });
                    }

                    ui.add_space(8.0);
                    if ui
                        .checkbox(&mut self.display_color.smooth_coloring, "Smooth coloring (log-log)")
                        .changed()
                    {
                        palette_changed = true;
                        self.bump_minimap_revision();
                    }
                });
        }

        // ---- Cursor coordinates (below toolbar, only with crosshair) ----
        if self.show_crosshair {
            if let Some(c) = self.cursor_complex {
                egui::Area::new(egui::Id::new("hud_cursor"))
                    .anchor(egui::Align2::RIGHT_TOP, [-8.0, 38.0])
                    .show(ctx, |ui| {
                        ui.style_mut().visuals.override_text_color =
                            Some(egui::Color32::from_rgb(220, 220, 220));
                        ui.label(format!("{:.10} {:+.10}i", c.re, c.im));
                    });
            }
        }

        // ---- Fractal parameters panel (bottom-left) ----
        // Negative y: egui adds offset to anchor; bottom anchor is at screen bottom, so -y moves panel up.
        egui::Area::new(egui::Id::new("hud_fractal_params"))
            .anchor(egui::Align2::LEFT_BOTTOM, [HUD_MARGIN, -HUD_MARGIN])
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(egui::Color32::from_black_alpha(hud_alpha))
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(HUD_CORNER_RADIUS)
                    .show(ui, |ui| {
                        ui.style_mut().visuals.override_text_color =
                            Some(egui::Color32::from_rgb(220, 220, 220));

                        // Fractal mode selector.
                        let old_mode = self.mode;
                        ui.horizontal(|ui| {
                            ui.label("Fractal:");
                            ui.selectable_value(
                                &mut self.mode,
                                FractalMode::Mandelbrot,
                                "Mandelbrot",
                            );
                            ui.selectable_value(&mut self.mode, FractalMode::Julia, "Julia");
                        });
                        mode_changed = self.mode != old_mode;

                        if self.mode == FractalMode::Julia {
                            const JULIA_C_RANGE: f64 = 2.0;
                            let mut re = self.julia_c.re;
                            let mut im = self.julia_c.im;
                            let re_range = -JULIA_C_RANGE..=JULIA_C_RANGE;
                            let im_range = -JULIA_C_RANGE..=JULIA_C_RANGE;
                            // Show enough decimals so values are fully visible in the field.
                            const C_DECIMALS: usize = 10;

                            ui.horizontal(|ui| {
                                ui.label("Re(c):");
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut re)
                                            .range(re_range)
                                            .fixed_decimals(C_DECIMALS),
                                    )
                                    .changed()
                                {
                                    self.julia_c.re = re;
                                    self.bump_minimap_revision();
                                    params_changed = true;
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Im(c):");
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut im)
                                            .range(im_range)
                                            .fixed_decimals(C_DECIMALS),
                                    )
                                    .changed()
                                {
                                    self.julia_c.im = im;
                                    self.bump_minimap_revision();
                                    params_changed = true;
                                }
                            });
                            ui.weak("Shift+Click to pick c");
                        }

                        ui.add_space(2.0);

                        // Iteration slider.
                        let iter_cap = self.params.max_iterations.max(10_000) as f32;
                        let mut max_iter = self.params.max_iterations as f32;
                        let old_iter = max_iter;
                        ui.add(
                            egui::Slider::new(&mut max_iter, 10.0..=iter_cap)
                                .text("Iter")
                                .logarithmic(true),
                        );
                        ui.horizontal(|ui| {
                            if ui.small_button("x10").clicked() {
                                max_iter = (self
                                    .params
                                    .max_iterations
                                    .saturating_mul(10)
                                    .min(10_000_000))
                                    as f32;
                            }
                            if ui.small_button("/10").clicked() {
                                max_iter = (self.params.max_iterations / 10).max(10) as f32;
                            }
                        });
                        if (max_iter - old_iter).abs() > 0.5 {
                            self.params.max_iterations = max_iter as u32;
                            params_changed = true;
                        }

                        // Escape radius.
                        let mut escape_r = self.params.escape_radius as f32;
                        let old_escape = escape_r;
                        ui.add(
                            egui::Slider::new(&mut escape_r, 2.0..=1000.0)
                                .text("Esc R")
                                .logarithmic(true),
                        );
                        if (escape_r - old_escape).abs() > 0.01 {
                            self.params.set_escape_radius(escape_r as f64);
                            params_changed = true;
                        }

                        // Adaptive iterations checkbox.
                        ui.checkbox(&mut self.adaptive_iterations, "Adaptive iterations");
                        if self.adaptive_iterations {
                            let eff = self.effective_max_iterations();
                            ui.weak(format!("Effective: {eff}"));
                        }
                    });
            });

        // Apply changes.
        if mode_changed {
            self.push_history();
            self.viewport = self.default_viewport();
            self.bump_minimap_revision();
            self.needs_render = true;
        } else if params_changed {
            self.needs_render = true;
        }
        if palette_changed {
            self.recolorize(ctx);
            self.julia_explorer_recolorize = true;
        }
    }

    // -- Settings panel (preferences only) -------------------------------------

    fn show_controls_panel(&mut self, ctx: &egui::Context) {
        if !self.show_controls || !self.show_hud {
            return;
        }

        let mut open = true;
        egui::Window::new("Settings")
            .open(&mut open)
            .resizable(true)
            .default_width(320.0)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(egui::Color32::from_rgba_unmultiplied(10, 10, 10, 210)),
            )
            .show(ctx, |ui| {
                ui.checkbox(
                    &mut self.preferences.restore_last_view,
                    "Restore last view on startup",
                );

                ui.add_space(6.0);
                ui.label("Bookmarks folder:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.bookmarks_dir_buf)
                        .desired_width(ui.available_width()),
                );
                ui.horizontal(|ui| {
                    if ui.small_button("Browse...").clicked() {
                        let start = std::path::Path::new(&self.bookmarks_dir_buf);
                        let mut dialog = rfd::FileDialog::new();
                        if start.is_dir() {
                            dialog = dialog.set_directory(start);
                        }
                        if let Some(folder) = dialog.pick_folder() {
                            self.bookmarks_dir_buf = folder.to_string_lossy().to_string();
                        }
                    }
                    if ui.small_button("Apply").clicked() {
                        let new_dir = self.bookmarks_dir_buf.trim().to_string();
                        self.preferences.bookmarks_dir = new_dir.clone();
                        self.preferences.save();
                        self.bookmark_store.set_directory(&new_dir);
                        self.thumbnail_cache.clear();
                        self.failed_thumbnails.clear();
                        self.bookmarks_dir_buf = self
                            .bookmark_store
                            .directory()
                            .to_string_lossy()
                            .to_string();
                    }
                    if ui.small_button("Reset").clicked() {
                        self.preferences.bookmarks_dir = String::new();
                        self.preferences.save();
                        self.bookmark_store.set_directory("");
                        self.thumbnail_cache.clear();
                        self.failed_thumbnails.clear();
                        self.bookmarks_dir_buf = self
                            .bookmark_store
                            .directory()
                            .to_string_lossy()
                            .to_string();
                    }
                });

                ui.add_space(10.0);
                ui.heading("Minimap & HUD");
                ui.horizontal(|ui| {
                    ui.label("Minimap size:");
                    egui::ComboBox::from_id_salt(egui::Id::new("minimap_size"))
                        .selected_text(match self.preferences.minimap_size {
                            preferences::MinimapSize::Small => "Small (128)",
                            preferences::MinimapSize::Medium => "Medium (256)",
                            preferences::MinimapSize::Large => "Large (384)",
                        })
                        .show_ui(ui, |ui| {
                            use preferences::MinimapSize;
                            for size in [MinimapSize::Small, MinimapSize::Medium, MinimapSize::Large] {
                                let label = match size {
                                    MinimapSize::Small => "Small (128 px)",
                                    MinimapSize::Medium => "Medium (256 px)",
                                    MinimapSize::Large => "Large (384 px)",
                                };
                                if ui
                                    .selectable_value(&mut self.preferences.minimap_size, size, label)
                                    .changed()
                                {
                                    self.preferences.save();
                                    self.bump_minimap_revision();
                                }
                            }
                        });
                });
                if ui
                    .add(egui::Slider::new(&mut self.preferences.minimap_zoom_half_extent, 0.5..=10.0)
                        .text("Minimap zoom (range ±"))
                    .changed()
                {
                    self.preferences.save();
                    self.bump_minimap_revision();
                }
                ui.label("(complex-plane half-extent, default 2 = -2..2)");
                if ui
                    .add(egui::Slider::new(&mut self.preferences.minimap_iterations, 50..=2000)
                        .text("Minimap iterations")
                        .logarithmic(true))
                    .changed()
                {
                    self.preferences.save();
                    self.bump_minimap_revision();
                }
                if ui
                    .add(egui::Slider::new(&mut self.preferences.minimap_opacity, 0.0..=1.0)
                        .text("Minimap opacity"))
                    .changed()
                {
                    self.preferences.save();
                }
                if ui
                    .add(egui::Slider::new(&mut self.preferences.crosshair_opacity, 0.0..=1.0)
                        .text("Crosshair opacity"))
                    .changed()
                {
                    self.preferences.save();
                }
                if ui
                    .add(egui::Slider::new(&mut self.preferences.hud_panel_opacity, 0.0..=1.0)
                        .text("HUD panel opacity"))
                    .changed()
                {
                    self.preferences.save();
                }
                ui.add_space(10.0);
                ui.heading("Julia C Explorer");
                ui.label("Square grid (1:1 C aspect), centered in viewport.");
                if ui
                    .add(egui::Slider::new(&mut self.preferences.julia_explorer_max_iterations, 50..=500)
                        .text("Grid preview iterations (default 100)"))
                    .changed()
                {
                    self.preferences.save();
                }
                if ui
                    .add(
                        egui::Slider::new(&mut self.preferences.julia_explorer_extent_half, 0.05..=4.0)
                            .logarithmic(true)
                            .text("C extent (zoom)"),
                    )
                    .changed()
                {
                    self.preferences.save();
                    self.julia_explorer_extent_half = self.preferences.julia_explorer_extent_half;
                    self.julia_explorer_restart_pending = true;
                }
                ui.horizontal(|ui| {
                    ui.label("Square size (px):");
                    if ui
                        .add(
                            egui::DragValue::new(&mut self.preferences.julia_explorer_cell_size_px)
                                .range(16..=256)
                                .suffix(""),
                        )
                        .changed()
                    {
                        self.preferences.save();
                        self.julia_explorer_restart_pending = true;
                    }
                });
            });

        if !open {
            self.show_controls = false;
        }
    }

    // -- Help / controls window --------------------------------------------------

    fn show_help_window(&mut self, ctx: &egui::Context) {
        if !self.show_help || !self.show_hud {
            return;
        }

        let mut open = true;
        egui::Window::new("Controls & Shortcuts")
            .open(&mut open)
            .resizable(false)
            .default_width(340.0)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(egui::Color32::from_rgba_unmultiplied(10, 10, 10, 210)),
            )
            .show(ctx, |ui| {
                ui.style_mut().visuals.override_text_color =
                    Some(egui::Color32::from_rgb(220, 220, 220));

                ui.heading("Keyboard");
                ui.add_space(2.0);
                egui::Grid::new("help_kb")
                    .num_columns(2)
                    .spacing([12.0, 2.0])
                    .show(ui, |ui| {
                        let keys: &[(&str, &str)] = &[
                            ("H", "Toggle HUD"),
                            ("M", "Toggle minimap"),
                            ("S", "Save bookmark"),
                            ("B", "Bookmark explorer"),
                            ("J", "Julia C Explorer (pick c from grid)"),
                            ("C", "Toggle crosshair"),
                            ("A", "Cycle anti-aliasing (Off / 2x2 / 4x4)"),
                            ("R", "Reset view"),
                            ("Esc", "Cancel render / close dialogs"),
                            ("Arrow keys", "Pan viewport"),
                            ("+ / -", "Zoom in / out"),
                            ("Backspace", "Navigate back"),
                            ("Shift+Backspace", "Navigate forward"),
                        ];
                        for &(k, d) in keys {
                            ui.label(egui::RichText::new(k).strong().color(egui::Color32::WHITE));
                            ui.label(d);
                            ui.end_row();
                        }
                    });

                ui.add_space(8.0);
                ui.heading("Mouse");
                ui.add_space(2.0);
                egui::Grid::new("help_mouse")
                    .num_columns(2)
                    .spacing([12.0, 2.0])
                    .show(ui, |ui| {
                        let actions: &[(&str, &str)] = &[
                            ("Left drag", "Pan"),
                            ("Right drag", "Selection-box zoom"),
                            ("Scroll wheel", "Zoom at cursor"),
                            ("Shift+Click", "Pick Julia c value"),
                        ];
                        for &(k, d) in actions {
                            ui.label(egui::RichText::new(k).strong().color(egui::Color32::WHITE));
                            ui.label(d);
                            ui.end_row();
                        }
                    });

                ui.add_space(8.0);
                ui.heading("Toolbar icons");
                ui.add_space(2.0);
                {
                    use egui_material_icons::icons::*;
                    let icons: &[(&str, &str)] = &[
                        (ICON_ARROW_BACK, "Navigate back"),
                        (ICON_ARROW_FORWARD, "Navigate forward"),
                        (ICON_RESTART_ALT, "Reset view"),
                        (ICON_PALETTE, "Display/color settings (palette, cycles, smooth)"),
                        (ICON_DEBLUR, "Cycle anti-aliasing"),
                        (ICON_BOOKMARK_ADD, "Save bookmark"),
                        (ICON_BOOKMARKS, "Bookmark explorer"),
                        (ICON_MAP, "Minimap (M)"),
                        (ICON_HELP_OUTLINE, "This help window"),
                        (ICON_SETTINGS, "Open settings"),
                    ];
                    egui::Grid::new("help_toolbar")
                        .num_columns(2)
                        .spacing([12.0, 2.0])
                        .show(ui, |ui| {
                            for &(k, d) in icons {
                                ui.label(
                                    egui::RichText::new(k)
                                        .size(18.0)
                                        .color(egui::Color32::WHITE),
                                );
                                ui.label(d);
                                ui.end_row();
                            }
                        });
                }
            });

        if !open {
            self.show_help = false;
        }
    }

    // -- Update-or-save choice dialog ------------------------------------------

    fn show_update_or_save_choice(&mut self, ctx: &egui::Context) {
        if !self.show_update_or_save_dialog {
            return;
        }

        let bm_name = self
            .last_jumped_bookmark_idx
            .and_then(|idx| {
                self.bookmark_store
                    .bookmarks()
                    .get(idx)
                    .map(|bm| bm.name.clone())
            })
            .unwrap_or_else(|| "bookmark".to_string());

        let mut open = true;
        let mut do_update = false;
        let mut do_save_new = false;

        egui::Window::new("Save Bookmark")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!(
                    "You navigated from bookmark \"{bm_name}\".\nWhat would you like to do?"
                ));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Update existing").clicked() {
                        do_update = true;
                    }
                    if ui.button("Save as new").clicked() {
                        do_save_new = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_update_or_save_dialog = false;
                    }
                });
            });

        if do_update {
            if let Some(idx) = self.last_jumped_bookmark_idx {
                self.update_bookmark(idx);
            }
            self.show_update_or_save_dialog = false;
        } else if do_save_new {
            self.show_update_or_save_dialog = false;
            self.open_save_new_dialog();
        }

        if !open {
            self.show_update_or_save_dialog = false;
        }
    }

    // -- Save-bookmark dialog --------------------------------------------------

    fn show_save_bookmark_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_save_dialog {
            return;
        }

        // Collect all known labels for the toggle list.
        let all_known: Vec<String> = {
            let mut set: HashSet<String> = HashSet::new();
            for bm in self.bookmark_store.bookmarks() {
                for l in &bm.labels {
                    set.insert(l.clone());
                }
            }
            // Also include the currently selected ones (they might be new defaults).
            for l in &self.save_bookmark_labels_selected {
                set.insert(l.clone());
            }
            let mut v: Vec<String> = set.into_iter().collect();
            v.sort_by(|a, b| {
                // "Favorites" always first, then alphabetical.
                let fa = a == "Favorites";
                let fb = b == "Favorites";
                fb.cmp(&fa)
                    .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
            });
            v
        };

        let auto_name = self.bookmark_store.next_auto_name(self.mode.label());

        let mut open = true;
        let mut do_save = false;

        egui::Window::new("Save Bookmark")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                // Name.
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.save_bookmark_name);
                });
                if self.save_bookmark_name.trim().is_empty() {
                    ui.weak(format!("Leave empty to auto-name: {auto_name}"));
                }

                ui.add_space(4.0);

                // Label toggles — existing labels shown as selectable chips.
                ui.label("Labels:");
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                    for label in &all_known {
                        let selected = self.save_bookmark_labels_selected.contains(label);
                        let text = if selected {
                            egui::RichText::new(format!("[x] {label}"))
                                .color(egui::Color32::WHITE)
                                .background_color(egui::Color32::from_rgb(50, 100, 170))
                        } else {
                            egui::RichText::new(label)
                                .color(egui::Color32::from_gray(180))
                                .background_color(egui::Color32::from_gray(50))
                        };
                        if ui.add(egui::Button::new(text).small()).clicked() {
                            if selected {
                                self.save_bookmark_labels_selected.remove(label);
                            } else {
                                self.save_bookmark_labels_selected.insert(label.clone());
                            }
                        }
                    }
                });

                // New label input.
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("New label:");
                    let resp = ui.text_edit_singleline(&mut self.save_bookmark_new_label);
                    if (resp.lost_focus() && ui.input(|inp| inp.key_pressed(egui::Key::Enter)))
                        || ui.small_button("+").clicked()
                    {
                        let new = self.save_bookmark_new_label.trim().to_string();
                        if !new.is_empty() {
                            self.save_bookmark_labels_selected.insert(new);
                            self.save_bookmark_new_label.clear();
                        }
                    }
                });
                ui.weak("Use / for nesting (e.g. Spirals/Double)");

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        do_save = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_save_dialog = false;
                    }
                });
            });

        if do_save {
            let name = {
                let trimmed = self.save_bookmark_name.trim().to_string();
                if trimmed.is_empty() {
                    auto_name
                } else {
                    trimmed
                }
            };
            let labels: Vec<String> = self.save_bookmark_labels_selected.iter().cloned().collect();
            let bm = self.capture_bookmark(name, labels);
            self.bookmark_store.add(bm);
            self.show_save_dialog = false;
        }

        if !open {
            self.show_save_dialog = false;
        }
    }

    // -- Bookmark browser window -----------------------------------------------

    fn show_bookmark_window(&mut self, ctx: &egui::Context) {
        if !self.show_bookmarks || !self.show_hud {
            return;
        }

        // Pre-compute data from the bookmark store (avoids borrow conflicts).
        let all_labels = bookmarks::collect_all_labels(self.bookmark_store.bookmarks());
        let label_tree = bookmarks::build_label_tree(&all_labels);
        let leaf_labels = bookmarks::collect_leaf_labels(self.bookmark_store.bookmarks());
        let snapshot: Vec<BookmarkSnap> = self
            .bookmark_store
            .bookmarks()
            .iter()
            .enumerate()
            .map(|(i, bm)| {
                (
                    i,
                    bm.name.clone(),
                    bm.summary(),
                    bm.mode.clone(),
                    bm.labels.clone(),
                    bm.thumbnail_png.clone(),
                )
            })
            .collect();

        let query = self.bookmark_search.clone();
        let selected = self.selected_labels.clone();
        let filter_mode = self.label_filter_mode;
        let tab = self.bookmark_tab;
        let fav_only = self.favorites_only;

        // Filter: text query + favorites toggle + label whitelist/blacklist + tab.
        let passes_filter = |name: &str, mode: &str, labels: &[String]| -> bool {
            // Tab filter.
            let tab_ok = match tab {
                BookmarkTab::All => true,
                BookmarkTab::Mandelbrot => mode == "Mandelbrot",
                BookmarkTab::Julia => mode == "Julia",
            };
            // Favorites toggle (independent).
            let fav_ok = !fav_only || labels.iter().any(|l| l == "Favorites");
            // Text search.
            let q_ok = query.is_empty()
                || name.to_lowercase().contains(&query.to_lowercase())
                || labels
                    .iter()
                    .any(|l| l.to_lowercase().contains(&query.to_lowercase()));
            // Label filter.
            let l_ok = match filter_mode {
                LabelFilterMode::Off => true,
                _ => {
                    if selected.is_empty() {
                        true
                    } else {
                        let has_match = labels.iter().any(|l| {
                            selected
                                .iter()
                                .any(|s| l == s || l.starts_with(&format!("{s}/")))
                        });
                        match filter_mode {
                            LabelFilterMode::Whitelist => has_match,
                            LabelFilterMode::Blacklist => !has_match,
                            _ => true,
                        }
                    }
                }
            };
            tab_ok && fav_ok && q_ok && l_ok
        };

        let mut open = true;
        let mut jump_idx: Option<usize> = None;
        let mut delete_idx: Option<usize> = None;
        let mut rename_action: Option<(usize, String)> = None;
        let mut toggle_fav_idx: Option<usize> = None;

        egui::Window::new("Bookmarks")
            .open(&mut open)
            .resizable(true)
            .default_width(520.0)
            .default_height(480.0)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(egui::Color32::from_rgba_unmultiplied(10, 10, 10, 210)),
            )
            .show(ctx, |ui| {
                // ---- Fractal tabs + favorites toggle ----
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.bookmark_tab, BookmarkTab::All, "All");
                    // Favorites toggle (independent of fractal tab).
                    let fav_label = if self.favorites_only {
                        egui::RichText::new("\u{2605} Fav").strong()
                    } else {
                        egui::RichText::new("\u{2606} Fav")
                    };
                    if ui
                        .selectable_label(self.favorites_only, fav_label)
                        .clicked()
                    {
                        self.favorites_only = !self.favorites_only;
                    }
                    ui.selectable_value(
                        &mut self.bookmark_tab,
                        BookmarkTab::Mandelbrot,
                        "Mandelbrot",
                    );
                    ui.selectable_value(&mut self.bookmark_tab, BookmarkTab::Julia, "Julia");
                });

                // ---- Search + sort toolbar ----
                ui.horizontal(|ui| {
                    ui.label("Search:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.bookmark_search).desired_width(160.0),
                    );
                    if ui.small_button("A-Z").clicked() {
                        self.bookmark_store.sort_by_name();
                        self.thumbnail_cache.clear();
                        self.failed_thumbnails.clear();
                    }
                    if ui.small_button("Date").clicked() {
                        self.bookmark_store.sort_by_date();
                        self.thumbnail_cache.clear();
                        self.failed_thumbnails.clear();
                    }
                });

                ui.separator();

                // ---- Label filter section ----
                if !leaf_labels.is_empty() {
                    egui::CollapsingHeader::new("Label filter")
                        .default_open(false)
                        .show(ui, |ui| {
                            // Mode selector.
                            ui.horizontal(|ui| {
                                ui.label("Mode:");
                                ui.selectable_value(
                                    &mut self.label_filter_mode,
                                    LabelFilterMode::Off,
                                    "Off",
                                );
                                ui.selectable_value(
                                    &mut self.label_filter_mode,
                                    LabelFilterMode::Whitelist,
                                    "Whitelist",
                                );
                                ui.selectable_value(
                                    &mut self.label_filter_mode,
                                    LabelFilterMode::Blacklist,
                                    "Blacklist",
                                );
                            });
                            if self.label_filter_mode != LabelFilterMode::Off {
                                if ui.small_button("Clear selection").clicked() {
                                    self.selected_labels.clear();
                                }
                                // Show "Favorites" first if it exists, then the tree.
                                if leaf_labels.iter().any(|l| l == "Favorites") {
                                    let mut fav = self.selected_labels.contains("Favorites");
                                    if ui
                                        .checkbox(
                                            &mut fav,
                                            egui::RichText::new("* Favorites").strong(),
                                        )
                                        .changed()
                                    {
                                        if fav {
                                            self.selected_labels.insert("Favorites".to_string());
                                        } else {
                                            self.selected_labels.remove("Favorites");
                                        }
                                    }
                                    ui.separator();
                                }
                                for node in &label_tree {
                                    if node.name == "Favorites" {
                                        continue; // Already shown above.
                                    }
                                    self.draw_label_tree_node(ui, node);
                                }
                            }
                        });
                    ui.separator();
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    // ---- Filtered bookmarks ----
                    let filtered: Vec<usize> = snapshot
                        .iter()
                        .filter(|(_, name, _, mode, labels, _)| passes_filter(name, mode, labels))
                        .map(|(i, ..)| *i)
                        .collect();

                    if !filtered.is_empty() {
                        self.draw_bookmark_grid(
                            ui,
                            ctx,
                            &snapshot,
                            &filtered,
                            "main_grid",
                            &mut jump_idx,
                            &mut delete_idx,
                            &mut rename_action,
                            &mut toggle_fav_idx,
                        );
                    } else if snapshot.is_empty() {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.weak("No bookmarks yet. Press S to save one.");
                        });
                    } else {
                        ui.add_space(10.0);
                        ui.vertical_centered(|ui| {
                            ui.weak("No bookmarks match the current filter.");
                        });
                    }
                });
            });

        // Apply deferred actions.
        if let Some((idx, new_name)) = rename_action {
            self.bookmark_store.rename(idx, new_name);
        }
        if let Some(idx) = toggle_fav_idx {
            self.bookmark_store.toggle_label(idx, "Favorites");
        }
        if let Some(idx) = jump_idx {
            let bm = self.bookmark_store.bookmarks()[idx].clone();
            self.last_jumped_bookmark_idx = Some(idx);
            self.jump_to_bookmark(&bm);
        }
        if let Some(idx) = delete_idx {
            self.thumbnail_cache.clear();
            self.failed_thumbnails.clear();
            self.bookmark_store.remove(idx);
            // If the deleted bookmark was the last jumped-to, clear the reference.
            if self.last_jumped_bookmark_idx == Some(idx) {
                self.last_jumped_bookmark_idx = None;
            } else if self.last_jumped_bookmark_idx.is_some_and(|last| last > idx) {
                // Adjust index after removal.
                self.last_jumped_bookmark_idx = self.last_jumped_bookmark_idx.map(|last| last - 1);
            }
        }

        if !open {
            self.show_bookmarks = false;
        }
    }

    // -- Julia C Explorer (Phase 10): drawn in central panel, no popup --------

    /// Draw the Julia C Explorer in the central panel (main view). Cells touch with no spacing.
    fn draw_julia_c_explorer_in_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let cell_size_px = self.preferences.julia_explorer_cell_size_px.clamp(16, 256) as f32;

        // Recolorize all grid cells when display/color changed.
        if self.julia_explorer_recolorize {
            self.julia_explorer_recolorize = false;
            let mut params = self.color_params();
            for ((i, j), buf) in &self.julia_explorer_cells {
                params.cycle_length =
                    self.display_color.cycle_length(buf.max_iterations);
                let buffer = self.current_palette().colorize(buf, &params);
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [buffer.width as usize, buffer.height as usize],
                    &buffer.pixels,
                );
                let name = format!("julia_cell_{}_{}", i, j);
                let tex = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
                self.julia_explorer_textures.insert((*i, *j), tex);
            }
        }

        // Update panel size when explorer is shown (so resize is tracked).
        let available0 = ui.available_size();
        self.check_resize(available0.x.max(1.0) as u32, available0.y.max(1.0) as u32);

        const CENTER_RE: f64 = -0.75;
        const CENTER_IM: f64 = 0.0;
        let extent_half = self.julia_explorer_extent_half;

        ui.style_mut().visuals.override_text_color =
            Some(egui::Color32::from_rgb(220, 220, 220));

        // Toolbar: square size (px), C extent (zoom), Display/color, hint.
        ui.horizontal(|ui| {
            ui.label("Square size (px):");
            let mut px_val = self.preferences.julia_explorer_cell_size_px as i32;
            if ui
                .add(egui::DragValue::new(&mut px_val).range(16..=256))
                .changed()
            {
                let v = px_val.clamp(16, 256) as u32;
                self.preferences.julia_explorer_cell_size_px = v;
                self.preferences.save();
                self.julia_explorer_restart_pending = true;
            }
            ui.label("C extent (zoom):");
            let mut ext_val = extent_half;
            if ui
                .add(
                    egui::Slider::new(&mut ext_val, 0.05..=4.0)
                        .logarithmic(true)
                        .text(""),
                )
                .changed()
                && ext_val > 0.0
            {
                self.julia_explorer_extent_half = ext_val;
                self.preferences.julia_explorer_extent_half = ext_val;
                self.preferences.save();
                self.julia_explorer_restart_pending = true;
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Display / color…").clicked() {
                self.show_palette_popup = true;
            }
            ui.weak("Center (-0.75, 0). Smaller C extent = zoom in. Click cell to set c. Esc to close.");
        });

        // Square grid (N×N) so C plane has 1:1 aspect ratio; centered in available space.
        let available = ui.available_size();
        let cols_cap = (available.x / cell_size_px).floor() as u32;
        let rows_cap = (available.y / cell_size_px).floor() as u32;
        let n = cols_cap.min(rows_cap).max(1);
        let cols = n;
        let rows = n;
        if cols != self.julia_explorer_cols || rows != self.julia_explorer_rows {
            self.julia_explorer_cols = cols;
            self.julia_explorer_rows = rows;
            self.julia_explorer_restart_pending = true;
        }
        let center_j = (cols - 1) / 2;
        let center_i = (rows - 1) / 2;
        let cell_width = 2.0 * extent_half / n as f64;
        let cell_width_re = cell_width;
        let cell_width_im = cell_width;

        let side = n as f32 * cell_size_px;
        ui.add_space((available.y - side) / 2.0);
        ui.horizontal(|ui| {
            ui.add_space((available.x - side) / 2.0);
            let (grid_rect, _) = ui.allocate_exact_size(
                egui::vec2(side, side),
                egui::Sense::hover(),
            );
            for i in 0..rows {
                for j in 0..cols {
                    let j_off = j as i64 - center_j as i64;
                    let i_off = i as i64 - center_i as i64;
                    let c_re = CENTER_RE + j_off as f64 * cell_width_re;
                    let c_im = CENTER_IM - i_off as f64 * cell_width_im;
                    let cell_rect = egui::Rect::from_min_size(
                        egui::pos2(
                            grid_rect.min.x + j as f32 * cell_size_px,
                            grid_rect.min.y + i as f32 * cell_size_px,
                        ),
                        egui::vec2(cell_size_px, cell_size_px),
                    );
                    let inner = ui.scope_builder(egui::UiBuilder::new().max_rect(cell_rect), |ui| {
                        ui.allocate_exact_size(egui::vec2(cell_size_px, cell_size_px), egui::Sense::click())
                    });
                    let resp = inner.inner.1;
                    if resp.clicked() {
                        self.julia_explorer_picked_c = Some((c_re, c_im));
                    }
                    resp.on_hover_text(format!("C = {:.6} {:+.6}i", c_re, c_im));
                    let rect = inner.response.rect;
                    if let Some(tex) = self.julia_explorer_textures.get(&(i, j)) {
                        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                        ui.painter().image(tex.id(), rect, uv, egui::Color32::WHITE);
                    } else {
                        ui.painter().rect_filled(rect, 0.0, egui::Color32::from_gray(50));
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "…",
                            egui::FontId::proportional(14.0),
                            egui::Color32::GRAY,
                        );
                    }
                }
            }
        });
        ui.add_space((available.y - side) / 2.0);
    }

    fn show_julia_c_explorer_window(&mut self, _ctx: &egui::Context) {
        // Julia C Explorer is now drawn in the central panel; no popup window.
    }

    /// Draw a grid of bookmark cards for a list of indices.
    #[allow(clippy::too_many_arguments)]
    fn draw_bookmark_grid(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        snapshot: &[BookmarkSnap],
        indices: &[usize],
        grid_id: &str,
        jump_idx: &mut Option<usize>,
        delete_idx: &mut Option<usize>,
        rename_action: &mut Option<(usize, String)>,
        toggle_fav_idx: &mut Option<usize>,
    ) {
        let card_width = 150.0_f32;
        let thumb_height = 84.0_f32;
        let spacing = 8.0_f32;

        // Calculate how many columns fit.
        let available_width = ui.available_width();
        let cols = ((available_width + spacing) / (card_width + spacing))
            .floor()
            .max(1.0) as usize;

        egui::Grid::new(ui.id().with(grid_id))
            .num_columns(cols)
            .spacing([spacing, spacing])
            .show(ui, |ui| {
                for (ci, &idx) in indices.iter().enumerate() {
                    let Some((i, ref name, _, _, ref labels, ref thumb_png)) =
                        snapshot.iter().find(|(si, ..)| *si == idx)
                    else {
                        continue;
                    };

                    ui.vertical(|ui| {
                        ui.set_width(card_width);

                        // Thumbnail or placeholder.
                        let thumb_tex = self.get_thumbnail(*i, thumb_png, ctx);
                        let thumb_rect = ui.allocate_exact_size(
                            egui::vec2(card_width, thumb_height),
                            egui::Sense::click(),
                        );

                        if let Some(tex) = thumb_tex {
                            let uv = egui::Rect::from_min_max(
                                egui::pos2(0.0, 0.0),
                                egui::pos2(1.0, 1.0),
                            );
                            ui.painter()
                                .image(tex.id(), thumb_rect.0, uv, egui::Color32::WHITE);
                        } else {
                            ui.painter().rect_filled(
                                thumb_rect.0,
                                4.0,
                                egui::Color32::from_gray(40),
                            );
                            ui.painter().text(
                                thumb_rect.0.center(),
                                egui::Align2::CENTER_CENTER,
                                "No preview",
                                egui::FontId::proportional(10.0),
                                egui::Color32::GRAY,
                            );
                        }

                        if thumb_rect.1.clicked() {
                            *jump_idx = Some(*i);
                        }

                        // Name + actions.
                        if self.editing_bookmark == Some(*i) {
                            let resp = ui.text_edit_singleline(&mut self.editing_name);
                            if resp.lost_focus()
                                || ui.input(|inp| inp.key_pressed(egui::Key::Enter))
                            {
                                let new_name = self.editing_name.trim().to_string();
                                if !new_name.is_empty() {
                                    *rename_action = Some((*i, new_name));
                                }
                                self.editing_bookmark = None;
                            }
                        } else {
                            ui.horizontal(|ui| {
                                ui.set_width(card_width);
                                if ui
                                    .add(
                                        egui::Label::new(
                                            egui::RichText::new(name).strong().size(11.0),
                                        )
                                        .truncate(),
                                    )
                                    .on_hover_text(name)
                                    .clicked()
                                {
                                    *jump_idx = Some(*i);
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.x = 1.0;
                                        if ui.small_button("\u{1f5d1}").clicked() {
                                            *delete_idx = Some(*i);
                                        }
                                        if ui.small_button("\u{270f}").clicked() {
                                            self.editing_bookmark = Some(*i);
                                            self.editing_name = name.clone();
                                        }
                                        let is_fav = labels.iter().any(|l| l == "Favorites");
                                        let star = if is_fav { "\u{2605}" } else { "\u{2606}" };
                                        if ui
                                            .small_button(star)
                                            .on_hover_text(if is_fav {
                                                "Remove from Favorites"
                                            } else {
                                                "Add to Favorites"
                                            })
                                            .clicked()
                                        {
                                            *toggle_fav_idx = Some(*i);
                                        }
                                    },
                                );
                            });
                        }

                        // Label chips.
                        if !labels.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(3.0, 2.0);
                                for label in labels {
                                    let short = label.split('/').next_back().unwrap_or(label);
                                    ui.label(
                                        egui::RichText::new(short)
                                            .size(9.0)
                                            .color(egui::Color32::from_rgb(140, 180, 220))
                                            .background_color(egui::Color32::from_black_alpha(80)),
                                    );
                                }
                            });
                        }
                    });

                    // End row after `cols` cards.
                    if (ci + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
    }

    /// Recursively draw a label tree node with a toggle checkbox.
    fn draw_label_tree_node(&mut self, ui: &mut egui::Ui, node: &bookmarks::LabelNode) {
        let is_selected = self.selected_labels.contains(&node.full_path);

        if node.children.is_empty() {
            // Leaf label.
            let mut checked = is_selected;
            if ui.checkbox(&mut checked, &node.name).changed() {
                if checked {
                    self.selected_labels.insert(node.full_path.clone());
                } else {
                    self.selected_labels.remove(&node.full_path);
                }
            }
        } else {
            // Parent with children — collapsing header.
            let mut checked = is_selected;
            ui.horizontal(|ui| {
                if ui.checkbox(&mut checked, "").changed() {
                    if checked {
                        self.selected_labels.insert(node.full_path.clone());
                    } else {
                        self.selected_labels.remove(&node.full_path);
                    }
                }
            });
            // Show children indented under a collapsing header.
            ui.indent(egui::Id::new(&node.full_path), |ui| {
                egui::CollapsingHeader::new(&node.name)
                    .default_open(true)
                    .show(ui, |ui| {
                        for child in &node.children {
                            self.draw_label_tree_node(ui, child);
                        }
                    });
            });
        }
    }
}

// ---------------------------------------------------------------------------
// eframe::App
// ---------------------------------------------------------------------------

impl eframe::App for MandelbRustApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());

        if self.pending_minimap_bump {
            self.bump_minimap_revision();
            self.pending_minimap_bump = false;
        }
        // Poll for background render results.
        self.poll_responses(ctx);
        self.poll_julia_grid_responses(ctx);
        if self.julia_explorer_restart_pending {
            self.julia_explorer_restart_pending = false;
            self.start_julia_grid_request();
        }
        self.poll_minimap_response(ctx);
        if self.show_hud && !self.show_julia_c_explorer && self.preferences.show_minimap {
            self.request_minimap_if_invalid(ctx);
        }

        // Central panel: fractal canvas or Julia C Explorer grid.
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                if self.show_julia_c_explorer {
                    self.draw_julia_c_explorer_in_panel(ui, ctx);
                    return;
                }
                let available = ui.available_size();
                let width = available.x.max(1.0) as u32;
                let height = available.y.max(1.0) as u32;

                // Detect resize.
                self.check_resize(width, height);

                // Submit render request if needed.
                if self.needs_render {
                    self.request_render();
                }

                // Draw the fractal image and capture interaction.
                let (response, painter) =
                    ui.allocate_painter(available, egui::Sense::click_and_drag());

                // Draw the low-res drag preview first (fills newly exposed edges
                // during and after drag), then the high-quality texture on top,
                // shifted by the active pan + any lingering draw offset.
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                if let Some(ref bg) = self.drag_preview {
                    painter.image(bg.id(), response.rect, uv, egui::Color32::WHITE);
                }
                if let Some(ref tex) = self.texture {
                    let offset = self.pan_offset + self.draw_offset;
                    let draw_rect = response.rect.translate(offset);
                    painter.image(tex.id(), draw_rect, uv, egui::Color32::WHITE);
                }

                // Progress bar (visible while rendering or refining).
                if self.render_phase == RenderPhase::Rendering
                    || self.render_phase == RenderPhase::Refining
                {
                    let (done, total) = self.cancel.progress();
                    if total > 0 {
                        let frac = (done as f32 / total as f32).clamp(0.0, 1.0);
                        let bar_h = 3.0;
                        let bar_y = response.rect.max.y - bar_h;
                        let bar_w = response.rect.width();

                        // Background.
                        let bg_rect = egui::Rect::from_min_size(
                            egui::pos2(response.rect.min.x, bar_y),
                            egui::vec2(bar_w, bar_h),
                        );
                        painter.rect_filled(
                            bg_rect,
                            0.0,
                            egui::Color32::from_rgba_premultiplied(0, 0, 0, 120),
                        );

                        // Filled portion.
                        if frac > 0.0 {
                            let fill_rect = egui::Rect::from_min_size(
                                egui::pos2(response.rect.min.x, bar_y),
                                egui::vec2(bar_w * frac, bar_h),
                            );
                            painter.rect_filled(
                                fill_rect,
                                0.0,
                                egui::Color32::from_rgb(80, 200, 255),
                            );
                        }
                    }
                }

                // Crosshair overlay.
                if self.show_crosshair {
                    let crosshair_color =
                        egui::Color32::from_rgba_premultiplied(200, 200, 200, 140);
                    let stroke = egui::Stroke::new(1.0, crosshair_color);

                    // Cursor crosshair.
                    if let Some(pos) = response.hover_pos() {
                        let rect = response.rect;
                        painter.line_segment(
                            [egui::pos2(rect.min.x, pos.y), egui::pos2(rect.max.x, pos.y)],
                            stroke,
                        );
                        painter.line_segment(
                            [egui::pos2(pos.x, rect.min.y), egui::pos2(pos.x, rect.max.y)],
                            stroke,
                        );
                    }

                    // Viewport centre indicator: small circle + cross.
                    let center = response.rect.center();
                    let arm = 8.0;
                    let center_color = egui::Color32::from_rgba_premultiplied(255, 160, 80, 180);
                    let center_stroke = egui::Stroke::new(1.5, center_color);
                    painter.circle_stroke(center, 4.0, center_stroke);
                    painter.line_segment(
                        [
                            egui::pos2(center.x - arm, center.y),
                            egui::pos2(center.x + arm, center.y),
                        ],
                        center_stroke,
                    );
                    painter.line_segment(
                        [
                            egui::pos2(center.x, center.y - arm),
                            egui::pos2(center.x, center.y + arm),
                        ],
                        center_stroke,
                    );
                }

                // Selection rectangle overlay (left-click drag zoom).
                if let Some(start) = self.zoom_rect_start {
                    if let Some(end) = response.hover_pos() {
                        let sel_rect = egui::Rect::from_two_pos(start, end);
                        painter.rect_stroke(
                            sel_rect,
                            0.0,
                            egui::Stroke::new(
                                2.0,
                                egui::Color32::from_rgba_premultiplied(0, 180, 220, 140),
                            ),
                            egui::StrokeKind::Outside,
                        );
                    }
                }

                // Handle mouse/drag input on the canvas.
                self.handle_canvas_input(ctx, &response);
            });

        // Apply Julia C pick from explorer (after panel is drawn).
        if let Some((c_re, c_im)) = self.julia_explorer_picked_c.take() {
            self.julia_c = Complex::new(c_re, c_im);
            self.bump_minimap_revision();
            self.needs_render = true;
            self.show_julia_c_explorer = false;
        }

        // Handle keyboard input (global).
        self.handle_keyboard(ctx);

        // Overlays.
        self.show_hud(ctx);
        self.show_controls_panel(ctx);
        self.show_help_window(ctx);
        self.show_bookmark_window(ctx);
        self.show_julia_c_explorer_window(ctx);
        self.show_update_or_save_choice(ctx);
        self.show_save_bookmark_dialog(ctx);

        // Keep repainting while a render is in progress, the crosshair
        // needs to track the cursor, or a zoom selection is active.
        if self.render_phase == RenderPhase::Rendering
            || self.render_phase == RenderPhase::Refining
            || self.show_crosshair
            || self.zoom_rect_start.is_some()
            || self.show_julia_c_explorer
        {
            ctx.request_repaint();
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Save last view, display/color settings, and bookmarks on shutdown.
        self.preferences.last_view = Some(self.capture_last_view());
        self.preferences.last_display_color = Some(self.display_color.clone());
        self.preferences.save();
        self.bookmark_store.save();
        info!("Saved preferences and bookmarks on exit");
    }
}

// ---------------------------------------------------------------------------
// Background render worker
// ---------------------------------------------------------------------------

/// Drain all pending requests from the channel, keeping only the latest.
fn drain_latest(initial: RenderRequest, rx: &mpsc::Receiver<RenderRequest>) -> RenderRequest {
    let mut req = initial;
    while let Ok(newer) = rx.try_recv() {
        req = newer;
    }
    req
}

/// Render + optional AA for a concrete fractal type.
fn do_render<F: mandelbrust_core::Fractal + Sync>(
    fractal: &F,
    viewport: &Viewport,
    cancel: &Arc<RenderCancel>,
    aa_level: u32,
    use_real_axis_symmetry: bool,
) -> RenderResult {
    let mut result = render(fractal, viewport, cancel, use_real_axis_symmetry);
    if aa_level > 0 && !result.cancelled {
        let aa_start = std::time::Instant::now();
        result.aa_samples = compute_aa(fractal, viewport, &result.iterations, aa_level, cancel);
        result.elapsed += aa_start.elapsed();
    }
    result
}

/// Dispatch a render to the appropriate fractal type (preserving static dispatch).
fn render_for_mode(
    mode: FractalMode,
    params: FractalParams,
    julia_c: Complex,
    viewport: &Viewport,
    cancel: &Arc<RenderCancel>,
    aa_level: u32,
) -> RenderResult {
    let use_symmetry = mode == FractalMode::Mandelbrot;
    match mode {
        FractalMode::Mandelbrot => do_render(&Mandelbrot::new(params), viewport, cancel, aa_level, use_symmetry),
        FractalMode::Julia => do_render(&Julia::new(julia_c, params), viewport, cancel, aa_level, use_symmetry),
    }
}

/// Long-running render worker.  Receives requests via `rx`, sends progressive
/// results (preview then final) back via `tx`, and calls `ctx.request_repaint()`
/// so the UI wakes up to display them.
fn render_worker(
    ctx: egui::Context,
    rx: mpsc::Receiver<RenderRequest>,
    tx: mpsc::Sender<RenderResponse>,
    cancel: Arc<RenderCancel>,
) {
    while let Ok(initial) = rx.recv() {
        let mut req = drain_latest(initial, &rx);

        // Inner loop: process the current request, but restart if a newer
        // request arrives between the preview and full passes.
        loop {
            // --- Preview pass: low resolution, same iterations ----------------
            // Using the same max_iterations as the full pass ensures colours
            // are consistent between preview and final (no interior/escaped
            // flipping).  The 1/16 pixel count already makes the preview fast.
            let preview_vp = req.viewport.downscaled(PREVIEW_DOWNSCALE);

            let preview =
                render_for_mode(req.mode, req.params, req.julia_c, &preview_vp, &cancel, 0);

            if preview.cancelled {
                break; // Cancelled — outer loop will pick up the newer request.
            }

            if tx
                .send(RenderResponse::Preview {
                    id: req.id,
                    result: preview,
                })
                .is_err()
            {
                return; // Channel closed.
            }
            ctx.request_repaint();

            // Check for a newer request before starting the expensive full pass.
            if let Ok(newer) = rx.try_recv() {
                req = drain_latest(newer, &rx);
                continue; // Restart inner loop with newer request.
            }

            // --- Full pass: full resolution, full iterations, optional AA ----
            let full = render_for_mode(
                req.mode,
                req.params,
                req.julia_c,
                &req.viewport,
                &cancel,
                req.aa_level,
            );

            if full.cancelled {
                break;
            }

            if tx
                .send(RenderResponse::Final {
                    id: req.id,
                    result: full,
                })
                .is_err()
            {
                return;
            }
            ctx.request_repaint();

            break; // Done with this request.
        }
    }
}

/// Phase 10: Renders a grid of small Julia set previews. Center cell is at (center_re, center_im);
/// other cells go out from there by cell_width per step.
fn julia_grid_worker(
    rx: mpsc::Receiver<JuliaGridRequest>,
    tx: mpsc::Sender<(u32, u32, RenderResult)>,
) {
    while let Ok(req) = rx.recv() {
        let gen = req.cancel.generation();
        let params = FractalParams::new(req.max_iterations, 2.0).unwrap_or_default();
        // Each cell shows Julia set in z-plane −1.5..1.5 so the set fills the cell (reduces "empty" uniform cells).
        let scale = 3.0 / req.cell_size as f64;
        let viewport = match Viewport::new(
            Complex::new(0.0, 0.0),
            scale,
            req.cell_size,
            req.cell_size,
        ) {
            Ok(vp) => vp,
            Err(_) => continue,
        };
        let center_j = (req.cols - 1) / 2;
        let center_i = (req.rows - 1) / 2;
        let cell_width_re = 2.0 * req.extent_half / req.cols as f64;
        let cell_width_im = 2.0 * req.extent_half / req.rows as f64;

        for i in 0..req.rows {
            if req.cancel.generation() != gen {
                break;
            }
            for j in 0..req.cols {
                if req.cancel.generation() != gen {
                    break;
                }
                let j_off = j as i64 - center_j as i64;
                let i_off = i as i64 - center_i as i64;
                let c_re = req.center_re + j_off as f64 * cell_width_re;
                let c_im = req.center_im - i_off as f64 * cell_width_im;
                let c = Complex::new(c_re, c_im);
                let julia = Julia::new(c, params);
                let result = do_render(&julia, &viewport, &req.cancel, req.aa_level, false);
                if tx.send((i, j, result)).is_err() {
                    return;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Default state helper
// ---------------------------------------------------------------------------

fn defaults_for(
    w: u32,
    h: u32,
    prefs: &AppPreferences,
) -> (
    FractalMode,
    Complex,
    FractalParams,
    Viewport,
    DisplayColorSettings,
    u32,
) {
    let display_color = DisplayColorSettings {
        palette_index: prefs.default_palette_index,
        smooth_coloring: true,
        ..DisplayColorSettings::default()
    };
    (
        FractalMode::Mandelbrot,
        Julia::default_c(),
        FractalParams::default().with_max_iterations(prefs.default_max_iterations),
        Viewport::default_mandelbrot(w, h),
        display_color,
        2,
    )
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting MandelbRust");

    let prefs = AppPreferences::load();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MandelbRust")
            .with_inner_size([prefs.window_width, prefs.window_height]),
        ..Default::default()
    };

    eframe::run_native(
        "MandelbRust",
        options,
        Box::new(move |cc| {
            egui_material_icons::initialize(&cc.egui_ctx);
            Ok(Box::new(MandelbRustApp::new(&cc.egui_ctx, prefs)))
        }),
    )
}
