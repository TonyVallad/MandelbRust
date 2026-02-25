use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use eframe::egui;
use tracing::info;

use mandelbrust_core::{
    Complex, ComplexDD, DoubleDouble, FractalParams, Julia, Viewport,
};
use mandelbrust_render::{
    builtin_palettes, AaSamples, ColorParams, IterationBuffer, Palette, RenderCancel,
    RenderResult, StartFrom as RenderStartFrom,
};

use crate::app_state::AppScreen;
use crate::bookmarks::BookmarkStore;
use crate::color_profiles;
use crate::display_color::{
    DisplayColorSettings, StartFrom as DisplayStartFrom,
};
use crate::preferences::{AppPreferences, LastView};
use crate::render_bridge::{
    julia_grid_worker, render_worker, JuliaGridRequest, RenderPhase, RenderRequest,
    RenderResponse,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub(crate) const ZOOM_SPEED: f64 = 0.003;
pub(crate) const PAN_FRACTION: f64 = 0.1;
pub(crate) const MAX_HISTORY: usize = 200;
pub(crate) const PREVIEW_DOWNSCALE: u32 = 4;
pub(crate) const ADAPTIVE_ITER_RATE: f64 = 30.0;
pub(crate) const DD_THRESHOLD_SCALE: f64 = 1e-13;
pub(crate) const DD_WARN_SCALE: f64 = 1e-28;
pub(crate) const HUD_MARGIN: f32 = 8.0;
pub(crate) const HUD_CORNER_RADIUS: f32 = 6.0;

/// Snapshot of a bookmark for display in the explorer grid.
#[allow(dead_code)]
pub(crate) struct BookmarkSnap {
    pub(crate) index: usize,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) summary: String,
    pub(crate) mode: String,
    pub(crate) labels: Vec<String>,
    pub(crate) thumbnail_png: String,
}

/// Maximum number of decoded thumbnail textures to keep in memory.
pub(crate) const THUMBNAIL_CACHE_CAPACITY: usize = 64;

// ---------------------------------------------------------------------------
// Fractal mode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FractalMode {
    Mandelbrot,
    Julia,
}

impl FractalMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Mandelbrot => "Mandelbrot",
            Self::Julia => "Julia",
        }
    }
}

// ---------------------------------------------------------------------------
// Bookmark explorer state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BookmarkTab {
    All,
    Mandelbrot,
    Julia,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LabelFilterMode {
    Off,
    Whitelist,
    Blacklist,
}

// ---------------------------------------------------------------------------
// Active dialog state (modal overlays that stack on top of floating panels)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActiveDialog {
    None,
    SaveBookmark,
    UpdateOrSave,
}

// ---------------------------------------------------------------------------
// Window icon
// ---------------------------------------------------------------------------

fn load_window_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../icon.ico");
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Ico).ok()?;
    let rgba = img.to_rgba8();
    const SIZE: u32 = 32;
    let resized = image::imageops::resize(
        &rgba,
        SIZE,
        SIZE,
        image::imageops::FilterType::Lanczos3,
    );
    Some(egui::IconData {
        rgba: resized.as_raw().clone(),
        width: SIZE,
        height: SIZE,
    })
}

// ---------------------------------------------------------------------------
// Application struct
// ---------------------------------------------------------------------------

pub(crate) struct MandelbRustApp {
    // Screen state
    pub(crate) screen: AppScreen,

    // Fractal state
    pub(crate) mode: FractalMode,
    pub(crate) julia_c: Complex,
    pub(crate) params: FractalParams,
    pub(crate) viewport: Viewport,

    // Render thread
    pub(crate) tx_request: mpsc::Sender<RenderRequest>,
    pub(crate) rx_response: mpsc::Receiver<RenderResponse>,
    pub(crate) cancel: Arc<RenderCancel>,
    pub(crate) render_id: u64,
    pub(crate) render_phase: RenderPhase,
    pub(crate) needs_render: bool,

    // Last render stats
    pub(crate) texture: Option<egui::TextureHandle>,
    pub(crate) drag_preview: Option<egui::TextureHandle>,
    pub(crate) render_time: Duration,
    pub(crate) tiles_rendered: usize,
    pub(crate) tiles_mirrored: usize,
    pub(crate) tiles_border_traced: usize,

    // Coloring
    pub(crate) palettes: Vec<Palette>,
    pub(crate) display_color: DisplayColorSettings,
    pub(crate) current_iterations: Option<IterationBuffer>,

    // UI state
    pub(crate) panel_size: [u32; 2],
    pub(crate) show_hud: bool,
    pub(crate) show_controls: bool,
    pub(crate) show_palette_popup: bool,
    pub(crate) color_profile_selected: String,
    pub(crate) color_profile_save_name: String,
    pub(crate) show_help: bool,
    pub(crate) show_crosshair: bool,
    pub(crate) show_about: bool,
    pub(crate) menu_bar_height: f32,
    pub(crate) drag_active: bool,
    pub(crate) pan_offset: egui::Vec2,
    pub(crate) cursor_complex: Option<Complex>,
    pub(crate) zoom_rect_start: Option<egui::Pos2>,

    // View history
    pub(crate) history: Vec<Viewport>,
    pub(crate) history_pos: usize,

    // Adaptive iterations
    pub(crate) adaptive_iterations: bool,

    // Anti-aliasing
    pub(crate) aa_level: u32,
    pub(crate) current_aa: Option<AaSamples>,

    // Pan optimisation
    pub(crate) pan_completed: bool,
    pub(crate) skip_preview_id: u64,
    pub(crate) draw_offset: egui::Vec2,

    // IO worker (file I/O off the UI thread)
    pub(crate) io_resp_rx: mpsc::Receiver<crate::io_worker::IoResponse>,

    // Bookmarks & preferences
    pub(crate) bookmark_store: BookmarkStore,
    pub(crate) preferences: AppPreferences,
    pub(crate) show_bookmarks: bool,
    pub(crate) active_dialog: ActiveDialog,
    pub(crate) save_bookmark_name: String,
    pub(crate) save_bookmark_labels_selected: HashSet<String>,
    pub(crate) save_bookmark_new_label: String,
    pub(crate) bookmark_search: String,
    pub(crate) bookmark_tab: BookmarkTab,
    pub(crate) favorites_only: bool,
    pub(crate) editing_bookmark: Option<usize>,
    pub(crate) editing_name: String,
    pub(crate) selected_labels: HashSet<String>,
    pub(crate) label_filter_mode: LabelFilterMode,
    pub(crate) thumbnail_cache: HashMap<String, egui::TextureHandle>,
    pub(crate) failed_thumbnails: HashSet<String>,
    pub(crate) last_jumped_bookmark_idx: Option<usize>,
    pub(crate) bookmarks_dir_buf: String,
    pub(crate) browser_selected_bookmark: Option<usize>,

    // Minimap
    pub(crate) tx_minimap: mpsc::Sender<(RenderResult, u64)>,
    pub(crate) rx_minimap: mpsc::Receiver<(RenderResult, u64)>,
    pub(crate) minimap_texture: Option<(egui::TextureHandle, u64)>,
    pub(crate) minimap_loading: bool,
    pub(crate) minimap_revision: u64,
    pub(crate) pending_minimap_bump: bool,

    // Julia C Explorer
    pub(crate) show_julia_c_explorer: bool,
    pub(crate) julia_explorer_extent_half: f64,
    pub(crate) julia_explorer_cols: u32,
    pub(crate) julia_explorer_rows: u32,
    pub(crate) julia_explorer_cells: HashMap<(u32, u32), IterationBuffer>,
    pub(crate) julia_explorer_textures: HashMap<(u32, u32), egui::TextureHandle>,
    pub(crate) julia_explorer_recolorize: bool,
    pub(crate) julia_explorer_restart_pending: bool,
    pub(crate) julia_explorer_picked_c: Option<(f64, f64)>,
    pub(crate) tx_julia_grid_req: mpsc::Sender<JuliaGridRequest>,
    pub(crate) rx_julia_grid_resp: mpsc::Receiver<(u32, u32, RenderResult)>,
    pub(crate) grid_cancel: Arc<RenderCancel>,

    // J preview panel
    pub(crate) tx_jpreview: mpsc::Sender<(RenderResult, u64)>,
    pub(crate) rx_jpreview: mpsc::Receiver<(RenderResult, u64)>,
    pub(crate) j_preview_texture: Option<(egui::TextureHandle, u64)>,
    pub(crate) j_preview_loading: bool,
    pub(crate) j_preview_revision: u64,
    pub(crate) j_preview_cancel: Arc<RenderCancel>,
    pub(crate) last_j_preview_cursor: Option<Complex>,
}

// ---------------------------------------------------------------------------
// Constructor
// ---------------------------------------------------------------------------

impl MandelbRustApp {
    pub(crate) fn new(egui_ctx: &egui::Context, prefs: AppPreferences) -> Self {
        let julia_explorer_extent_half = prefs.julia_explorer_extent_half;
        let (tx_req, rx_req) = mpsc::channel();
        let (tx_resp, rx_resp) = mpsc::channel();
        let cancel = Arc::new(RenderCancel::new());

        let (tx_julia_grid_req, rx_julia_grid_req) = mpsc::channel();
        let (tx_julia_grid_resp, rx_julia_grid_resp) = mpsc::channel();
        let grid_cancel = Arc::new(RenderCancel::new());

        let (tx_jpreview, rx_jpreview) = mpsc::channel();
        let j_preview_cancel = Arc::new(RenderCancel::new());

        let ctx = egui_ctx.clone();
        let cancel_clone = cancel.clone();
        thread::spawn(move || {
            render_worker(ctx, rx_req, tx_resp, cancel_clone);
        });

        thread::spawn(move || {
            julia_grid_worker(rx_julia_grid_req, tx_julia_grid_resp);
        });

        let w = prefs.window_width as u32;
        let h = prefs.window_height as u32;

        let (mode, julia_c, params, viewport, mut display_color, aa_level) =
            if prefs.restore_last_view {
                if let Some(ref lv) = prefs.last_view {
                    let m = match lv.mode.as_str() {
                        "Julia" => FractalMode::Julia,
                        _ => FractalMode::Mandelbrot,
                    };
                    let center_dd = ComplexDD::new(
                        DoubleDouble::new(lv.center_re, lv.center_re_lo),
                        DoubleDouble::new(lv.center_im, lv.center_im_lo),
                    );
                    let vp = Viewport::new_dd(center_dd, lv.scale, w, h)
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

        if let Some(ref saved) = prefs.last_display_color {
            display_color = saved.clone();
        }
        let palettes = builtin_palettes();
        if display_color.palette_index >= palettes.len() {
            display_color.palette_index = 0;
        }

        let mut bookmark_store = BookmarkStore::load(&prefs.bookmarks_dir);
        bookmark_store.sort_by_date();
        let bookmarks_dir_display = bookmark_store.directory().to_string_lossy().to_string();

        let (io_tx, io_resp_rx) = crate::io_worker::spawn_io_worker();
        bookmark_store.set_io_sender(io_tx.clone());
        let mut prefs = prefs;
        prefs.set_io_sender(io_tx);

        let (tx_minimap, rx_minimap) = mpsc::channel();

        let app = Self {
            screen: AppScreen::MainMenu,

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
            show_about: false,
            menu_bar_height: 0.0,
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

            io_resp_rx,

            bookmark_store,
            preferences: prefs,
            show_bookmarks: false,
            active_dialog: ActiveDialog::None,
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
            bookmarks_dir_buf: bookmarks_dir_display,
            browser_selected_bookmark: None,

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
            tx_julia_grid_req,
            rx_julia_grid_resp,
            grid_cancel,
            tx_jpreview,
            rx_jpreview,
            j_preview_texture: None,
            j_preview_loading: false,
            j_preview_revision: 0,
            j_preview_cancel,
            last_j_preview_cursor: None,
        };
        color_profiles::ensure_default_profile();
        app
    }

    // -- Palette helpers ---------------------------------------------------

    pub(crate) fn current_palette(&self) -> &Palette {
        &self.palettes[self.display_color.palette_index]
    }

    pub(crate) fn color_params(&self) -> ColorParams {
        let start_from = match self.display_color.start_from {
            DisplayStartFrom::None => RenderStartFrom::None,
            DisplayStartFrom::Black => RenderStartFrom::Black,
            DisplayStartFrom::White => RenderStartFrom::White,
        };
        ColorParams {
            smooth: self.display_color.smooth_coloring,
            cycle_length: self
                .display_color
                .cycle_length(self.params.max_iterations),
            start_from,
            low_threshold_start: self.display_color.low_threshold_start,
            low_threshold_end: self.display_color.low_threshold_end,
        }
    }

    pub(crate) fn colorize_current(
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

    pub(crate) fn recolorize(&mut self, ctx: &egui::Context) {
        if let Some(ref iter_buf) = self.current_iterations {
            let buffer = self.colorize_current(iter_buf, self.current_aa.as_ref());
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [buffer.width as usize, buffer.height as usize],
                &buffer.pixels,
            );
            self.texture =
                Some(ctx.load_texture("fractal", image, egui::TextureOptions::LINEAR));
            self.draw_offset = egui::Vec2::ZERO;
        }
    }

    // -- Effective parameters ----------------------------------------------

    pub(crate) fn effective_params(&self) -> FractalParams {
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

    pub(crate) fn effective_max_iterations(&self) -> u32 {
        self.effective_params().max_iterations
    }

    pub(crate) fn capture_last_view(&self) -> LastView {
        LastView {
            mode: self.mode.label().to_string(),
            center_re: self.viewport.center_dd.re.hi,
            center_im: self.viewport.center_dd.im.hi,
            center_re_lo: self.viewport.center_dd.re.lo,
            center_im_lo: self.viewport.center_dd.im.lo,
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
}

// ---------------------------------------------------------------------------
// IO worker polling
// ---------------------------------------------------------------------------

impl MandelbRustApp {
    /// Drain any pending IO responses (e.g. bookmark directory scans).
    fn poll_io_responses(&mut self) {
        use crate::io_worker::IoResponse;
        while let Ok(resp) = self.io_resp_rx.try_recv() {
            match resp {
                IoResponse::BookmarksScanComplete {
                    bookmarks,
                    filenames,
                } => {
                    self.bookmark_store.apply_scan_result(bookmarks, filenames);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Fractal explorer update (extracted from update() for clarity)
// ---------------------------------------------------------------------------

impl MandelbRustApp {
    fn update_fractal_explorer(&mut self, ctx: &egui::Context) {
        if self.pending_minimap_bump {
            self.bump_minimap_revision();
            self.pending_minimap_bump = false;
        }
        self.poll_responses(ctx);
        self.poll_julia_grid_responses(ctx);
        if self.julia_explorer_restart_pending {
            self.julia_explorer_restart_pending = false;
            self.start_julia_grid_request();
        }
        self.poll_minimap_response(ctx);
        self.poll_j_preview_response(ctx);
        if self.show_hud && !self.show_julia_c_explorer && self.preferences.show_minimap {
            self.request_minimap_if_invalid(ctx);
        }
        if self.show_hud && !self.show_julia_c_explorer && self.preferences.show_j_preview {
            self.request_j_preview_if_needed(ctx);
        }

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

                self.check_resize(width, height);

                if self.needs_render {
                    self.request_render();
                }

                let (response, painter) =
                    ui.allocate_painter(available, egui::Sense::click_and_drag());

                let uv =
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                if let Some(ref bg) = self.drag_preview {
                    painter.image(bg.id(), response.rect, uv, egui::Color32::WHITE);
                }
                if let Some(ref tex) = self.texture {
                    let offset = self.pan_offset + self.draw_offset;
                    let draw_rect = response.rect.translate(offset);
                    painter.image(tex.id(), draw_rect, uv, egui::Color32::WHITE);
                }

                if self.render_phase == RenderPhase::Rendering
                    || self.render_phase == RenderPhase::Refining
                {
                    let (done, total) = self.cancel.progress();
                    if total > 0 {
                        let frac = (done as f32 / total as f32).clamp(0.0, 1.0);
                        let bar_h = 3.0;
                        let bar_y = response.rect.max.y - bar_h;
                        let bar_w = response.rect.width();

                        let bg_rect = egui::Rect::from_min_size(
                            egui::pos2(response.rect.min.x, bar_y),
                            egui::vec2(bar_w, bar_h),
                        );
                        painter.rect_filled(
                            bg_rect,
                            0.0,
                            egui::Color32::from_rgba_premultiplied(0, 0, 0, 120),
                        );

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

                if self.show_crosshair {
                    let crosshair_color =
                        egui::Color32::from_rgba_premultiplied(200, 200, 200, 140);
                    let stroke = egui::Stroke::new(1.0, crosshair_color);

                    if let Some(pos) = response.hover_pos() {
                        let rect = response.rect;
                        painter.line_segment(
                            [
                                egui::pos2(rect.min.x, pos.y),
                                egui::pos2(rect.max.x, pos.y),
                            ],
                            stroke,
                        );
                        painter.line_segment(
                            [
                                egui::pos2(pos.x, rect.min.y),
                                egui::pos2(pos.x, rect.max.y),
                            ],
                            stroke,
                        );
                    }

                    let center = response.rect.center();
                    let arm = 8.0;
                    let center_color =
                        egui::Color32::from_rgba_premultiplied(255, 160, 80, 180);
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

                self.handle_canvas_input(ctx, &response);
            });

        if let Some((c_re, c_im)) = self.julia_explorer_picked_c.take() {
            self.julia_c = Complex::new(c_re, c_im);
            self.bump_minimap_revision();
            self.needs_render = true;
            self.show_julia_c_explorer = false;
        }

        self.handle_keyboard(ctx);

        self.show_hud(ctx);
        self.show_bookmark_window(ctx);
        self.show_julia_c_explorer_window(ctx);
        self.show_update_or_save_choice(ctx);
        self.show_save_bookmark_dialog(ctx);

        if self.render_phase == RenderPhase::Rendering
            || self.render_phase == RenderPhase::Refining
            || self.show_crosshair
            || self.zoom_rect_start.is_some()
            || self.show_julia_c_explorer
        {
            ctx.request_repaint();
        }
    }
}

// ---------------------------------------------------------------------------
// eframe::App
// ---------------------------------------------------------------------------

impl eframe::App for MandelbRustApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());
        self.poll_io_responses();
        self.draw_menu_bar(ctx);

        match self.screen {
            AppScreen::MainMenu => self.draw_main_menu(ctx),
            AppScreen::BookmarkBrowser => self.draw_bookmark_browser(ctx),
            AppScreen::JuliaCExplorer => self.draw_julia_c_explorer_screen(ctx),
            AppScreen::FractalExplorer => self.update_fractal_explorer(ctx),
        }

        self.show_controls_panel(ctx);
        self.show_help_window(ctx);
        self.draw_about_window(ctx);

        let text_editing = ctx.memory(|m| m.focused().is_some());
        if !text_editing {
            match self.screen {
                AppScreen::BookmarkBrowser => {
                    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.screen = AppScreen::MainMenu;
                        self.browser_selected_bookmark = None;
                    }
                }
                AppScreen::JuliaCExplorer => {
                    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.screen = AppScreen::MainMenu;
                        self.grid_cancel.cancel();
                    }
                }
                _ => {}
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.preferences.last_view = Some(self.capture_last_view());
        self.preferences.last_display_color = Some(self.display_color.clone());
        self.preferences.save();
        self.bookmark_store.save();
        info!("Saved preferences and bookmarks on exit");
    }
}

// ---------------------------------------------------------------------------
// Default state helper
// ---------------------------------------------------------------------------

pub(crate) fn defaults_for(
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

pub(crate) fn run() -> eframe::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting MandelbRust");

    let prefs = AppPreferences::load();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("MandelbRust")
        .with_inner_size([prefs.window_width, prefs.window_height]);
    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(Arc::new(icon));
    }

    let options = eframe::NativeOptions {
        viewport,
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
