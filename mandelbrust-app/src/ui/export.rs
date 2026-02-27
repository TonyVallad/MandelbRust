//! Image export dialog and background export worker.

use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;

use eframe::egui;
use tracing::{debug, error, info};

use mandelbrust_core::{Complex, FractalParams, Viewport};
use mandelbrust_render::{ExportMetadata, RenderCancel};

use crate::app::{FractalMode, MandelbRustApp};
use crate::app_dir;
use crate::display_color::{
    DisplayColorSettings, PaletteMode as DisplayPaletteMode, StartFrom as DisplayStartFrom,
};
use crate::render_bridge::render_for_mode;

// ---------------------------------------------------------------------------
// Resolution presets
// ---------------------------------------------------------------------------

struct ResolutionPreset {
    label: &'static str,
    width: u32,
    height: u32,
}

const PRESETS: &[ResolutionPreset] = &[
    ResolutionPreset { label: "1280 x 720  (HD)", width: 1280, height: 720 },
    ResolutionPreset { label: "1920 x 1080  (Full HD)", width: 1920, height: 1080 },
    ResolutionPreset { label: "2560 x 1440  (QHD)", width: 2560, height: 1440 },
    ResolutionPreset { label: "3840 x 2160  (4K UHD)", width: 3840, height: 2160 },
    ResolutionPreset { label: "5120 x 2880  (5K)", width: 5120, height: 2880 },
    ResolutionPreset { label: "7680 x 4320  (8K UHD)", width: 7680, height: 4320 },
];

// ---------------------------------------------------------------------------
// Export state (held by MandelbRustApp)
// ---------------------------------------------------------------------------

/// Persistent state for the export dialog and background export worker.
pub(crate) struct ExportState {
    pub(crate) show_dialog: bool,

    pub(crate) image_name: String,
    pub(crate) preset_index: usize,
    pub(crate) custom_width: String,
    pub(crate) custom_height: String,
    pub(crate) max_iterations: String,
    pub(crate) aa_choice: u32,
    pub(crate) display_color: DisplayColorSettings,

    pub(crate) exporting: bool,
    pub(crate) export_cancel: Arc<RenderCancel>,
    pub(crate) export_result_rx: Option<mpsc::Receiver<ExportWorkerResult>>,
    pub(crate) export_notification: Option<(String, std::time::Instant, bool)>,
}

const CUSTOM_INDEX: usize = usize::MAX;

impl ExportState {
    pub(crate) fn new() -> Self {
        Self {
            show_dialog: false,
            image_name: String::new(),
            preset_index: CUSTOM_INDEX,
            custom_width: "1920".into(),
            custom_height: "1080".into(),
            max_iterations: "256".into(),
            aa_choice: 0,
            display_color: DisplayColorSettings::default(),
            exporting: false,
            export_cancel: Arc::new(RenderCancel::new()),
            export_result_rx: None,
            export_notification: None,
        }
    }

    fn is_custom(&self) -> bool {
        self.preset_index == CUSTOM_INDEX
    }

    fn export_width(&self) -> u32 {
        if self.is_custom() {
            self.custom_width.parse().unwrap_or(1920)
        } else {
            PRESETS.get(self.preset_index).map(|p| p.width).unwrap_or(1920)
        }
    }

    fn export_height(&self) -> u32 {
        if self.is_custom() {
            self.custom_height.parse().unwrap_or(1080)
        } else {
            PRESETS.get(self.preset_index).map(|p| p.height).unwrap_or(1080)
        }
    }

    fn export_max_iterations(&self) -> u32 {
        self.max_iterations.parse().unwrap_or(256)
    }

    fn selected_label(&self) -> String {
        if self.is_custom() {
            "Custom".into()
        } else {
            PRESETS.get(self.preset_index).map(|p| p.label.to_string()).unwrap_or("Custom".into())
        }
    }
}

pub(crate) enum ExportWorkerResult {
    Success(PathBuf),
    Error(String),
}

// ---------------------------------------------------------------------------
// Open / reset dialog
// ---------------------------------------------------------------------------

impl MandelbRustApp {
    pub(crate) fn open_export_dialog(&mut self, ctx: &egui::Context) {
        let base_iter = self.params.max_iterations;
        let aa = self.aa_level;
        let dc = self.display_color.clone();
        let state = &mut self.export_state;
        state.show_dialog = true;
        state.image_name.clear();
        state.max_iterations = base_iter.to_string();
        state.aa_choice = aa;
        state.display_color = dc;

        let screen_w;
        let screen_h;
        if let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) {
            screen_w = monitor.x as u32;
            screen_h = monitor.y as u32;
        } else {
            screen_w = 1920;
            screen_h = 1080;
        }

        let mut best_idx = CUSTOM_INDEX;
        for (i, p) in PRESETS.iter().enumerate() {
            if p.width == screen_w && p.height == screen_h {
                best_idx = i;
                break;
            }
        }
        if best_idx == CUSTOM_INDEX {
            for (i, p) in PRESETS.iter().enumerate() {
                if p.width == 1920 && p.height == 1080 {
                    best_idx = i;
                    break;
                }
            }
        }
        state.preset_index = best_idx;
        state.custom_width = screen_w.to_string();
        state.custom_height = screen_h.to_string();
    }
}

// ---------------------------------------------------------------------------
// Draw export dialog
// ---------------------------------------------------------------------------

impl MandelbRustApp {
    pub(crate) fn draw_export_dialog(&mut self, ctx: &egui::Context) {
        self.poll_export_result();
        self.draw_export_notification(ctx);

        if !self.export_state.show_dialog {
            return;
        }

        let mut open = true;
        let mut do_export = false;

        egui::Window::new("Export Image")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(360.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);

                let hint = self.default_export_name();
                ui.horizontal(|ui| {
                    ui.label("Image name:");
                    ui.add(egui::TextEdit::singleline(&mut self.export_state.image_name)
                        .desired_width(220.0)
                        .hint_text(hint));
                });

                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label("Resolution:");
                    let label = self.export_state.selected_label();
                    egui::ComboBox::from_id_salt("export_resolution")
                        .selected_text(label)
                        .width(220.0)
                        .show_ui(ui, |ui| {
                            for (i, preset) in PRESETS.iter().enumerate() {
                                ui.selectable_value(
                                    &mut self.export_state.preset_index,
                                    i,
                                    preset.label,
                                );
                            }
                            ui.selectable_value(
                                &mut self.export_state.preset_index,
                                CUSTOM_INDEX,
                                "Custom",
                            );
                        });
                });

                if self.export_state.is_custom() {
                    ui.horizontal(|ui| {
                        ui.add_space(80.0);
                        ui.label("Width:");
                        ui.add(egui::TextEdit::singleline(&mut self.export_state.custom_width)
                            .desired_width(60.0));
                        ui.label("Height:");
                        ui.add(egui::TextEdit::singleline(&mut self.export_state.custom_height)
                            .desired_width(60.0));
                    });
                }

                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label("Max iterations:");
                    ui.add(egui::TextEdit::singleline(&mut self.export_state.max_iterations)
                        .desired_width(80.0));
                });

                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label("Anti-aliasing:");
                    egui::ComboBox::from_id_salt("export_aa")
                        .selected_text(aa_label(self.export_state.aa_choice))
                        .width(100.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.export_state.aa_choice, 0, "Off");
                            ui.selectable_value(&mut self.export_state.aa_choice, 2, "2x2");
                            ui.selectable_value(&mut self.export_state.aa_choice, 4, "4x4");
                        });
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                ui.label(egui::RichText::new("Color settings").strong());
                ui.add_space(2.0);

                // Palette picker
                ui.horizontal(|ui| {
                    ui.label("Palette:");
                    let pal_idx = self.export_state.display_color.palette_index;
                    let pal_name = self.palettes.get(pal_idx).map(|p| p.name).unwrap_or("?");
                    egui::ComboBox::from_id_salt("export_palette")
                        .selected_text(pal_name)
                        .width(140.0)
                        .show_ui(ui, |ui| {
                            for (i, pal) in self.palettes.iter().enumerate() {
                                ui.selectable_value(
                                    &mut self.export_state.display_color.palette_index,
                                    i,
                                    pal.name,
                                );
                            }
                        });
                });

                // Palette mode
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Mode:");
                    let by_cycles = matches!(
                        self.export_state.display_color.palette_mode,
                        DisplayPaletteMode::ByCycles { .. }
                    );
                    if ui.selectable_label(by_cycles, "By cycles").clicked() && !by_cycles {
                        self.export_state.display_color.palette_mode =
                            DisplayPaletteMode::ByCycles { n: 1 };
                    }
                    if ui.selectable_label(!by_cycles, "By cycle length").clicked() && by_cycles {
                        self.export_state.display_color.palette_mode =
                            DisplayPaletteMode::ByCycleLength { len: 256 };
                    }
                });

                ui.horizontal(|ui| {
                    let (mut mode_val, is_cycles) = match self.export_state.display_color.palette_mode {
                        DisplayPaletteMode::ByCycles { n } => (n as i32, true),
                        DisplayPaletteMode::ByCycleLength { len } => (len as i32, false),
                    };
                    ui.add_space(48.0);
                    if ui
                        .add(egui::DragValue::new(&mut mode_val).range(1..=i32::MAX))
                        .changed()
                    {
                        let v = mode_val.max(1) as u32;
                        self.export_state.display_color.palette_mode = if is_cycles {
                            DisplayPaletteMode::ByCycles { n: v }
                        } else {
                            DisplayPaletteMode::ByCycleLength { len: v }
                        };
                    }
                    ui.label(if is_cycles { "cycles" } else { "iterations per cycle" });
                });

                // Start from
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Start from:");
                    for (opt, label) in [
                        (DisplayStartFrom::None, "None"),
                        (DisplayStartFrom::Black, "Black"),
                        (DisplayStartFrom::White, "White"),
                    ] {
                        ui.selectable_value(
                            &mut self.export_state.display_color.start_from,
                            opt,
                            label,
                        );
                    }
                });

                if self.export_state.display_color.start_from != DisplayStartFrom::None {
                    ui.horizontal(|ui| {
                        ui.add_space(48.0);
                        ui.label("Start:");
                        let mut start = self.export_state.display_color.low_threshold_start as i32;
                        if ui
                            .add(egui::DragValue::new(&mut start).range(0..=i32::MAX))
                            .changed()
                        {
                            self.export_state.display_color.low_threshold_start = start.max(0) as u32;
                        }
                        ui.label("End:");
                        let mut end = self.export_state.display_color.low_threshold_end as i32;
                        if ui
                            .add(egui::DragValue::new(&mut end).range(0..=i32::MAX))
                            .changed()
                        {
                            self.export_state.display_color.low_threshold_end = end.max(0) as u32;
                        }
                    });
                }

                // Smooth coloring
                ui.add_space(2.0);
                ui.checkbox(
                    &mut self.export_state.display_color.smooth_coloring,
                    "Smooth coloring",
                );

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                let valid = self.export_state.export_width() >= 1
                    && self.export_state.export_height() >= 1
                    && self.export_state.export_max_iterations() >= 1;
                let busy = self.export_state.exporting;

                ui.horizontal(|ui| {
                    if busy {
                        let (done, total) = self.export_state.export_cancel.progress();
                        let pct = if total > 0 { done as f32 / total as f32 } else { 0.0 };
                        ui.add(egui::ProgressBar::new(pct).desired_width(200.0).text(
                            format!("Exporting… {:.0}%", pct * 100.0),
                        ));
                        if ui.button("Cancel").clicked() {
                            self.export_state.export_cancel.cancel();
                        }
                    } else {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add_enabled(valid, egui::Button::new("Export")).clicked() {
                                do_export = true;
                            }
                            if ui.button("Cancel").clicked() {
                                self.export_state.show_dialog = false;
                            }
                        });
                    }
                });

                ui.add_space(4.0);
            });

        if !open {
            self.export_state.show_dialog = false;
        }

        if do_export {
            self.start_export();
        }
    }

    fn default_export_name(&self) -> String {
        let fractal = self.mode.label();
        let iter = self.export_state.export_max_iterations();
        let w = self.export_state.export_width();
        let h = self.export_state.export_height();
        format!("{fractal}_{iter}_{w}x{h}")
    }

    fn draw_export_notification(&mut self, ctx: &egui::Context) {
        let Some((ref msg, at, is_error)) = self.export_state.export_notification else {
            return;
        };
        let elapsed = at.elapsed().as_secs_f32();
        if elapsed > 5.0 {
            self.export_state.export_notification = None;
            return;
        }
        let alpha = ((5.0 - elapsed) / 1.0).clamp(0.0, 1.0);
        let color = if is_error {
            egui::Color32::from_rgba_unmultiplied(255, 100, 100, (alpha * 255.0) as u8)
        } else {
            egui::Color32::from_rgba_unmultiplied(100, 255, 140, (alpha * 255.0) as u8)
        };
        egui::Area::new(egui::Id::new("export_notification"))
            .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -40.0])
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(egui::Color32::from_black_alpha((alpha * 200.0) as u8))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(msg.clone()).color(color));
                    });
            });
        ctx.request_repaint();
    }
}

// ---------------------------------------------------------------------------
// Background export
// ---------------------------------------------------------------------------

impl MandelbRustApp {
    fn start_export(&mut self) {
        let w = self.export_state.export_width();
        let h = self.export_state.export_height();
        let max_iter = self.export_state.export_max_iterations();
        let aa_level = self.export_state.aa_choice;

        let name = if self.export_state.image_name.trim().is_empty() {
            self.default_export_name()
        } else {
            sanitize_filename(&self.export_state.image_name)
        };

        let fractal_dir_name = self.mode.label().to_lowercase();
        let out_dir = app_dir::images_directory().join(&fractal_dir_name);
        if let Err(e) = std::fs::create_dir_all(&out_dir) {
            error!("Failed to create export directory: {e}");
            self.export_state.export_notification = Some((
                format!("Export failed: {e}"),
                std::time::Instant::now(),
                true,
            ));
            return;
        }

        let path = unique_path(&out_dir, &name, "png");

        let params = FractalParams::new(max_iter, self.params.escape_radius)
            .unwrap_or_else(|_| FractalParams::new(max_iter, 2.0).unwrap_or_default());

        let viewer_complex_w = self.viewport.complex_width();
        let viewer_complex_h = self.viewport.complex_height();
        let export_scale = (viewer_complex_w / w as f64).max(viewer_complex_h / h as f64);
        let viewport = Viewport::new_dd(self.viewport.center_dd, export_scale, w, h)
            .unwrap_or(self.viewport);

        let mode = self.mode;
        let julia_c = self.julia_c;
        let export_dc = &self.export_state.display_color;
        let pal_idx = export_dc.palette_index.min(self.palettes.len().saturating_sub(1));
        let palette = self.palettes[pal_idx].clone();
        let color_params = Self::color_params_from_display(export_dc, max_iter);
        let display_color = export_dc.clone();

        let center_re = format!("{:.15}", self.viewport.center.re);
        let center_im = format!("{:+.15}", self.viewport.center.im);
        let zoom_str = format!("{:.6e}", 1.0 / self.viewport.scale);

        let metadata = ExportMetadata {
            fractal_type: mode.label().to_string(),
            center_re,
            center_im,
            zoom: zoom_str,
            max_iterations: max_iter,
            escape_radius: params.escape_radius,
            julia_c_re: if mode == FractalMode::Julia {
                Some(format!("{:.15}", julia_c.re))
            } else {
                None
            },
            julia_c_im: if mode == FractalMode::Julia {
                Some(format!("{:+.15}", julia_c.im))
            } else {
                None
            },
            aa_level,
            palette_name: palette.name.to_string(),
            smooth_coloring: display_color.smooth_coloring,
            width: w,
            height: h,
        };

        let cancel = Arc::new(RenderCancel::new());
        self.export_state.export_cancel = cancel.clone();
        self.export_state.exporting = true;

        let (tx, rx) = mpsc::channel();
        self.export_state.export_result_rx = Some(rx);

        debug!("Export started: {} → {}", name, path.display());

        let job = ExportJob {
            mode, params, julia_c, viewport, cancel, aa_level,
            palette, color_params, metadata, path,
        };

        let ctx = self.egui_ctx.clone();
        if let Err(e) = std::thread::Builder::new()
            .name("export-worker".into())
            .spawn(move || {
                let result = export_worker(&job);
                let _ = tx.send(result);
                ctx.request_repaint();
            })
        {
            error!("Failed to spawn export thread: {e}");
            self.export_state.exporting = false;
        }
    }

    fn poll_export_result(&mut self) {
        let rx = match self.export_state.export_result_rx.as_ref() {
            Some(rx) => rx,
            None => return,
        };
        if let Ok(result) = rx.try_recv() {
            self.export_state.exporting = false;
            self.export_state.export_result_rx = None;
            match result {
                ExportWorkerResult::Success(path) => {
                    info!("Export complete: {}", path.display());
                    let short = path.file_name()
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string());
                    self.export_state.export_notification = Some((
                        format!("Exported: {short}"),
                        std::time::Instant::now(),
                        false,
                    ));
                }
                ExportWorkerResult::Error(msg) => {
                    error!("Export failed: {msg}");
                    self.export_state.export_notification = Some((
                        format!("Export failed: {msg}"),
                        std::time::Instant::now(),
                        true,
                    ));
                }
            }
        }
    }

    fn color_params_from_display(
        dc: &DisplayColorSettings,
        max_iterations: u32,
    ) -> mandelbrust_render::ColorParams {
        let start_from = match dc.start_from {
            DisplayStartFrom::None => mandelbrust_render::StartFrom::None,
            DisplayStartFrom::Black => mandelbrust_render::StartFrom::Black,
            DisplayStartFrom::White => mandelbrust_render::StartFrom::White,
        };
        mandelbrust_render::ColorParams {
            smooth: dc.smooth_coloring,
            cycle_length: dc.cycle_length(max_iterations),
            start_from,
            low_threshold_start: dc.low_threshold_start,
            low_threshold_end: dc.low_threshold_end,
        }
    }
}

// ---------------------------------------------------------------------------
// Worker function (runs on a background thread)
// ---------------------------------------------------------------------------

struct ExportJob {
    mode: FractalMode,
    params: FractalParams,
    julia_c: Complex,
    viewport: Viewport,
    cancel: Arc<RenderCancel>,
    aa_level: u32,
    palette: mandelbrust_render::Palette,
    color_params: mandelbrust_render::ColorParams,
    metadata: ExportMetadata,
    path: PathBuf,
}

fn export_worker(job: &ExportJob) -> ExportWorkerResult {
    let result = render_for_mode(
        job.mode, job.params, job.julia_c, &job.viewport, &job.cancel, job.aa_level,
    );

    if result.cancelled {
        return ExportWorkerResult::Error("Export cancelled".into());
    }

    let buffer = if let Some(ref aa) = result.aa_samples {
        job.palette.colorize_aa(&result.iterations, aa, &job.color_params)
    } else {
        job.palette.colorize(&result.iterations, &job.color_params)
    };

    match mandelbrust_render::export_png(
        &buffer.pixels,
        buffer.width,
        buffer.height,
        &job.path,
        &job.metadata,
    ) {
        Ok(()) => ExportWorkerResult::Success(job.path.clone()),
        Err(e) => ExportWorkerResult::Error(e),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn aa_label(level: u32) -> &'static str {
    match level {
        0 => "Off",
        2 => "2x2",
        4 => "4x4",
        _ => "Off",
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' })
        .collect::<String>()
        .trim()
        .to_string()
}

fn unique_path(dir: &std::path::Path, name: &str, ext: &str) -> PathBuf {
    let base = dir.join(format!("{name}.{ext}"));
    if !base.exists() {
        return base;
    }
    for i in 1..10000 {
        let candidate = dir.join(format!("{name}_{i:03}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{name}_export.{ext}"))
}
