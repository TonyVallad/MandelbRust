use eframe::egui;
use tracing::{debug, error};

use mandelbrust_core::DoubleDouble;

use crate::app::{defaults_for, FractalMode, MandelbRustApp};
use crate::app_dir;
use crate::app_state::AppScreen;

const TILE_CORNER_RADIUS: f32 = 4.0;
const CYAN: egui::Color32 = egui::Color32::from_rgb(80, 200, 255);
const PREVIEW_MAX_WIDTH: u32 = 512;

enum MenuAction {
    None,
    Resume,
    Mandelbrot,
    Julia,
    Bookmark,
}

impl MandelbRustApp {
    pub(crate) fn draw_main_menu(&mut self, ctx: &egui::Context) {
        self.ensure_menu_previews_loaded(ctx);

        let resume_details = self.format_resume_details();
        let bm_count = self.bookmark_store.bookmarks().len();

        let mut action = MenuAction::None;

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
            .show(ctx, |ui| {
                let available = ui.available_size();
                self.panel_size = [available.x.max(1.0) as u32, available.y.max(1.0) as u32];

                let tile_gap = 12.0_f32;
                let sep_width = 1.0_f32;
                let sep_gap = 14.0_f32;

                let h_gaps = sep_gap * 2.0 + sep_width + tile_gap * 2.0;
                let tile_width = ((available.x - h_gaps) / 4.0).clamp(100.0, 290.0);
                let tile_height = (available.y * 0.68).clamp(220.0, 460.0);

                let total_width =
                    tile_width * 4.0 + sep_gap * 2.0 + sep_width + tile_gap * 2.0;
                let x_offset = (available.x - total_width).max(0.0) / 2.0;
                let y_offset = (available.y - tile_height).max(0.0) / 2.0;

                ui.add_space(y_offset);

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.add_space(x_offset);

                    if draw_tile(
                        ui,
                        tile_width,
                        tile_height,
                        "Resume Exploration",
                        &resume_details,
                        self.resume_thumbnail.as_ref(),
                    )
                    .clicked()
                    {
                        action = MenuAction::Resume;
                    }

                    ui.add_space(sep_gap);
                    let (sep_rect, _) = ui.allocate_exact_size(
                        egui::vec2(sep_width, tile_height),
                        egui::Sense::hover(),
                    );
                    ui.painter()
                        .rect_filled(sep_rect, 0.0, egui::Color32::from_gray(55));
                    ui.add_space(sep_gap);

                    let mandelbrot_desc = "\
Explore the iconic\n\
Mandelbrot set.\n\
\n\
Z = Z\u{00b2} + C\n\
\n\
Starting from Z = 0, iterate\n\
the formula for each point C\n\
in the complex plane.\n\
Points that stay bounded\n\
belong to the set.";
                    if draw_tile(
                        ui,
                        tile_width,
                        tile_height,
                        "Mandelbrot Set",
                        mandelbrot_desc,
                        self.mandelbrot_preview.as_ref(),
                    )
                    .clicked()
                    {
                        action = MenuAction::Mandelbrot;
                    }

                    ui.add_space(tile_gap);

                    let julia_desc = "\
Discover the infinite variety\n\
of Julia sets.\n\
\n\
Z = Z\u{00b2} + C\n\
\n\
For a fixed value of C, iterate\n\
from every starting point Z.\n\
Each C produces a unique fractal.\n\
Choose C from the\n\
Julia C Explorer.";
                    if draw_tile(
                        ui,
                        tile_width,
                        tile_height,
                        "Julia's Sets",
                        julia_desc,
                        self.julia_preview.as_ref(),
                    )
                    .clicked()
                    {
                        action = MenuAction::Julia;
                    }

                    ui.add_space(tile_gap);

                    let bm_desc = if bm_count > 0 {
                        format!(
                            "Browse your saved bookmarks\n\
                             and revisit your favourite\n\
                             exploration spots.\n\
                             \n\
                             {bm_count} bookmark{}",
                            if bm_count == 1 { "" } else { "s" }
                        )
                    } else {
                        "Browse your saved bookmarks\n\
                         and revisit your favourite\n\
                         exploration spots.\n\
                         \n\
                         No bookmarks yet."
                            .to_string()
                    };
                    if draw_tile(
                        ui,
                        tile_width,
                        tile_height,
                        "Open Bookmark",
                        &bm_desc,
                        self.bookmarks_preview.as_ref(),
                    )
                    .clicked()
                    {
                        action = MenuAction::Bookmark;
                    }
                });
            });

        match action {
            MenuAction::Resume => {
                self.screen = AppScreen::FractalExplorer;
                self.needs_render = true;
            }
            MenuAction::Mandelbrot => {
                self.apply_mandelbrot_defaults();
                self.screen = AppScreen::FractalExplorer;
                self.needs_render = true;
            }
            MenuAction::Julia => {
                self.screen = AppScreen::JuliaCExplorer;
                self.julia_explorer_restart_pending = true;
            }
            MenuAction::Bookmark => {
                self.screen = AppScreen::BookmarkBrowser;
                self.bookmark_store.reload();
                self.browser_selected_bookmark = None;
            }
            MenuAction::None => {}
        }
    }

    fn format_resume_details(&self) -> String {
        let mode = self.mode.label();
        let mut s = format!("**Fractal:** {mode}");
        if self.mode == FractalMode::Julia {
            let c_re = format_f64_trimmed(self.julia_c.re);
            let c_im = format_f64_signed_trimmed(self.julia_c.im);
            s.push_str(&format!("\n\n**C Coordinates:**\n{c_re}\n{c_im}i"));
        }

        let center_re = format_dd_trimmed(self.viewport.center_dd.re);
        let center_im_raw = format_dd_trimmed(self.viewport.center_dd.im);
        let center_im = if center_im_raw.starts_with('-') {
            center_im_raw
        } else {
            format!("+{center_im_raw}")
        };

        s.push_str(&format!(
            "\n\n**Center:**\n{center_re}\n{center_im}i\
             \n\n**Zoom:** {:.2e}\
             \n\n**Iterations:** {}",
            1.0 / self.viewport.scale,
            self.params.max_iterations,
        ));
        s
    }

    /// Persist the current exploration state (viewport, mode, colors) to disk.
    pub(crate) fn save_exploration_state(&mut self) {
        self.preferences.last_view = Some(self.capture_last_view());
        self.preferences.last_display_color = Some(self.display_color.clone());
        self.preferences.save();
    }

    /// Save a resume preview PNG and load it as a texture, reusing an
    /// already-colorized pixel buffer to avoid recomputation.
    pub(crate) fn update_resume_preview(
        &mut self,
        ctx: &egui::Context,
        pixels: &[u8],
        width: u32,
        height: u32,
    ) {
        let dir = app_dir::previews_directory();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            error!("Failed to create previews directory: {e}");
            return;
        }
        let path = dir.join("resume_preview.png");
        save_preview_png(pixels, width, height, &path);
        self.resume_thumbnail = load_preview_texture(ctx, &path);

        self.save_exploration_state();
    }

    fn ensure_menu_previews_loaded(&mut self, ctx: &egui::Context) {
        let dir = app_dir::previews_directory();
        if self.resume_thumbnail.is_none() {
            let path = dir.join("resume_preview.png");
            if path.is_file() {
                self.resume_thumbnail = load_preview_texture(ctx, &path);
                if self.resume_thumbnail.is_some() {
                    debug!("Loaded resume preview from {}", path.display());
                }
            }
        }
        if self.mandelbrot_preview.is_none() {
            let path = dir.join("mandelbrot_preview.png");
            if path.is_file() {
                self.mandelbrot_preview = load_preview_texture(ctx, &path);
                if self.mandelbrot_preview.is_some() {
                    debug!("Loaded mandelbrot preview from {}", path.display());
                }
            }
        }
        if self.julia_preview.is_none() {
            let path = dir.join("julia_preview.png");
            if path.is_file() {
                self.julia_preview = load_preview_texture(ctx, &path);
                if self.julia_preview.is_some() {
                    debug!("Loaded julia preview from {}", path.display());
                }
            }
        }
        if self.bookmarks_preview.is_none() {
            let path = dir.join("bookmarks_preview.png");
            if path.is_file() {
                self.bookmarks_preview = load_preview_texture(ctx, &path);
                if self.bookmarks_preview.is_some() {
                    debug!("Loaded bookmarks preview from {}", path.display());
                }
            }
        }
    }

    pub(crate) fn apply_mandelbrot_defaults(&mut self) {
        let w = self.panel_size[0];
        let h = self.panel_size[1];
        let (mode, julia_c, params, viewport, display_color, aa_level) =
            defaults_for(w, h, &self.preferences);
        self.mode = mode;
        self.julia_c = julia_c;
        self.params = params;
        self.viewport = viewport;
        self.display_color = display_color;
        self.aa_level = aa_level;
        self.current_aa = None;
        self.current_iterations = None;
        self.texture = None;
        self.drag_preview = None;
        self.history.clear();
        self.history.push(viewport);
        self.history_pos = 0;
        self.last_jumped_bookmark_idx = None;
        self.bump_minimap_revision();
    }
}

// ---------------------------------------------------------------------------
// DoubleDouble / f64 formatting with trailing-zero trimming
// ---------------------------------------------------------------------------

/// Format a DoubleDouble value as a decimal string with full precision,
/// trimming trailing zeros after the decimal point.
fn format_dd_trimmed(dd: DoubleDouble) -> String {
    let negative = dd.is_negative();
    let mut val = dd.abs();

    let int_part = val.hi.trunc();
    val = val - DoubleDouble::from(int_part);
    if val.is_negative() {
        val = DoubleDouble::ZERO;
    }

    let mut result = if negative {
        format!("-{}", int_part as u64)
    } else {
        format!("{}", int_part as u64)
    };
    result.push('.');

    for _ in 0..30 {
        val = val * DoubleDouble::from(10.0);
        let digit = val.hi.trunc().clamp(0.0, 9.0) as u8;
        val = val - DoubleDouble::from(digit as f64);
        if val.is_negative() {
            val = DoubleDouble::ZERO;
        }
        result.push((b'0' + digit) as char);
    }

    trim_trailing_zeros(&mut result);
    result
}

/// Format an f64 with full precision, trimming trailing zeros.
fn format_f64_trimmed(v: f64) -> String {
    let mut s = format!("{v:.15}");
    trim_trailing_zeros(&mut s);
    s
}

/// Like `format_f64_trimmed` but with a leading `+` for non-negative values.
fn format_f64_signed_trimmed(v: f64) -> String {
    let mut s = if v >= 0.0 {
        format!("+{v:.15}")
    } else {
        format!("{v:.15}")
    };
    trim_trailing_zeros(&mut s);
    s
}

fn trim_trailing_zeros(s: &mut String) {
    if s.contains('.') {
        let trimmed = s.trim_end_matches('0');
        let trimmed = if trimmed.ends_with('.') {
            &trimmed[..trimmed.len() - 1]
        } else {
            trimmed
        };
        s.truncate(trimmed.len());
    }
}

// ---------------------------------------------------------------------------
// Preview PNG save / load
// ---------------------------------------------------------------------------

fn save_preview_png(pixels: &[u8], width: u32, height: u32, path: &std::path::Path) {
    let Some(img) = image::RgbaImage::from_raw(width, height, pixels.to_vec()) else {
        return;
    };
    let (save_w, save_h) = if width > PREVIEW_MAX_WIDTH {
        let ratio = PREVIEW_MAX_WIDTH as f64 / width as f64;
        (
            PREVIEW_MAX_WIDTH,
            (height as f64 * ratio).round().max(1.0) as u32,
        )
    } else {
        (width, height)
    };

    let final_img = if save_w != width {
        image::imageops::resize(
            &img,
            save_w,
            save_h,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        img
    };

    if let Err(e) = final_img.save(path) {
        error!("Failed to save preview PNG: {e}");
    } else {
        debug!(
            "Saved resume preview {}x{} to {}",
            save_w,
            save_h,
            path.display()
        );
    }
}

fn load_preview_texture(
    ctx: &egui::Context,
    path: &std::path::Path,
) -> Option<egui::TextureHandle> {
    let img = image::open(path).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    let color_image =
        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "preview".to_string());
    Some(ctx.load_texture(
        name,
        color_image,
        egui::TextureOptions::LINEAR,
    ))
}

// ---------------------------------------------------------------------------
// Tile drawing
// ---------------------------------------------------------------------------

fn draw_tile(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    title: &str,
    details: &str,
    preview_texture: Option<&egui::TextureHandle>,
) -> egui::Response {
    let (raw_rect, response) =
        ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

    if ui.is_rect_visible(raw_rect) {
        let ppp = ui.ctx().pixels_per_point();
        let rect = snap_rect(raw_rect, ppp);
        let painter = ui.painter();

        let bg = if response.hovered() {
            egui::Color32::from_rgb(32, 32, 40)
        } else {
            egui::Color32::from_rgb(20, 20, 26)
        };
        painter.rect_filled(rect, TILE_CORNER_RADIUS, bg);

        let border_color = if response.hovered() {
            egui::Color32::from_gray(70)
        } else {
            egui::Color32::from_gray(42)
        };
        painter.rect_stroke(
            rect,
            TILE_CORNER_RADIUS,
            egui::Stroke::new(0.5, border_color),
            egui::StrokeKind::Inside,
        );

        let preview_h = (rect.height() * 0.40).min(180.0);
        let preview_rect = egui::Rect::from_min_max(
            rect.min,
            egui::pos2(rect.max.x, rect.min.y + preview_h),
        );

        if let Some(tex) = preview_texture {
            let tex_size = tex.size_vec2();
            let tex_aspect = tex_size.x / tex_size.y;
            let rect_aspect = preview_rect.width() / preview_rect.height();

            let uv = if (tex_aspect - rect_aspect).abs() < 0.01 {
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0))
            } else if tex_aspect > rect_aspect {
                let frac = rect_aspect / tex_aspect;
                let margin = (1.0 - frac) / 2.0;
                egui::Rect::from_min_max(
                    egui::pos2(margin, 0.0),
                    egui::pos2(1.0 - margin, 1.0),
                )
            } else {
                let frac = tex_aspect / rect_aspect;
                let margin = (1.0 - frac) / 2.0;
                egui::Rect::from_min_max(
                    egui::pos2(0.0, margin),
                    egui::pos2(1.0, 1.0 - margin),
                )
            };

            painter.image(tex.id(), preview_rect, uv, egui::Color32::WHITE);
        } else {
            painter.rect_filled(preview_rect, 0.0, egui::Color32::from_gray(28));
            painter.text(
                preview_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Preview",
                egui::FontId::proportional(13.0),
                egui::Color32::from_gray(55),
            );
        }

        let title_y = preview_rect.max.y + 12.0;
        painter.text(
            egui::pos2(rect.center().x, title_y),
            egui::Align2::CENTER_TOP,
            title,
            egui::FontId::proportional(16.0),
            CYAN,
        );

        let details_y = title_y + 26.0;
        let tile_center_x = rect.center().x;
        paint_rich_lines(ui, painter, tile_center_x, details_y, details);
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    response
}

// ---------------------------------------------------------------------------
// Rich-text per-line painting  (supports **bold** markup, each line centered)
// ---------------------------------------------------------------------------

const LINE_HEIGHT: f32 = 17.0;

fn paint_rich_lines(
    ui: &egui::Ui,
    painter: &egui::Painter,
    center_x: f32,
    start_y: f32,
    text: &str,
) {
    let normal = egui::TextFormat {
        font_id: egui::FontId::proportional(13.0),
        color: egui::Color32::from_gray(150),
        ..Default::default()
    };
    let bold = egui::TextFormat {
        font_id: egui::FontId::proportional(14.0),
        color: egui::Color32::from_gray(225),
        ..Default::default()
    };

    for (i, line) in text.lines().enumerate() {
        let y = start_y + i as f32 * LINE_HEIGHT;

        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = f32::INFINITY;

        let mut remaining = line;
        while !remaining.is_empty() {
            if let Some(start) = remaining.find("**") {
                if start > 0 {
                    job.append(&remaining[..start], 0.0, normal.clone());
                }
                let after = &remaining[start + 2..];
                if let Some(end) = after.find("**") {
                    job.append(&after[..end], 0.0, bold.clone());
                    remaining = &after[end + 2..];
                } else {
                    job.append(remaining, 0.0, normal.clone());
                    break;
                }
            } else {
                job.append(remaining, 0.0, normal.clone());
                break;
            }
        }

        let galley = ui.ctx().fonts_mut(|f| f.layout_job(job));
        let line_w = galley.size().x;
        painter.galley(
            egui::pos2(center_x - line_w / 2.0, y),
            galley,
            egui::Color32::PLACEHOLDER,
        );
    }
}

/// Round a rect's coordinates to physical pixel boundaries so that painted
/// elements (images, fills) don't leave sub-pixel gaps.
fn snap_rect(r: egui::Rect, pixels_per_point: f32) -> egui::Rect {
    let snap = |v: f32| (v * pixels_per_point).round() / pixels_per_point;
    egui::Rect::from_min_max(
        egui::pos2(snap(r.min.x), snap(r.min.y)),
        egui::pos2(snap(r.max.x), snap(r.max.y)),
    )
}
