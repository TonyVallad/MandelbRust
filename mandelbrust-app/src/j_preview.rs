//! Phase 10.5: J preview panel (above minimap).
//!
//! In Mandelbrot mode: live Julia set at cursor c. In Julia mode: Mandelbrot with crosshair at c.
//! Same size/opacity as minimap; 4×4 AA; 1px white border 75%.

use eframe::egui;

use mandelbrust_core::{Complex, Viewport};

/// Gap between J preview panel and minimap (same as HUD margin).
const GAP: f32 = 8.0;
const ANCHOR_MARGIN: f32 = 8.0;
/// 1px white border at 75% opacity.
const BORDER_ALPHA: u8 = 191;

/// Parameters needed to draw the J preview panel.
pub struct JPreviewDrawParams<'a> {
    pub size_px: f32,
    pub panel_alpha: u8,
    pub image_alpha: u8,
    pub texture: Option<&'a egui::TextureHandle>,
    pub loading: bool,
    /// Julia mode: viewport of the Mandelbrot preview (to draw crosshair at c).
    pub preview_viewport: Viewport,
    pub julia_c: Complex,
    /// True when panel shows Mandelbrot (crosshair at c). False when Julia (no crosshair).
    pub is_mandelbrot_preview: bool,
}

/// Draw the J preview panel above the minimap. Anchor: RIGHT_BOTTOM, offset so the panel sits with a gap above the minimap.
pub fn draw_j_preview_panel(ctx: &egui::Context, params: JPreviewDrawParams<'_>) {
    let size = params.size_px;
    // Panel sits above minimap with GAP: minimap top = -ANCHOR_MARGIN - size, so panel bottom = minimap top - GAP = -ANCHOR_MARGIN - size - GAP. Anchor is panel bottom-right.
    let offset_y = -(ANCHOR_MARGIN + GAP + size);
    egui::Area::new(egui::Id::new("hud_j_preview"))
        .anchor(egui::Align2::RIGHT_BOTTOM, [-ANCHOR_MARGIN, offset_y])
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(egui::Color32::from_black_alpha(params.panel_alpha))
                .inner_margin(egui::Margin::ZERO)
                .corner_radius(0.0)
                .show(ui, |ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
                    let image_side = rect.width().min(rect.height());
                    let image_rect =
                        egui::Rect::from_center_size(rect.center(), egui::vec2(image_side, image_side));

                    if let Some(tex) = params.texture {
                        let uv =
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                        ui.painter().image(
                            tex.id(),
                            image_rect,
                            uv,
                            egui::Color32::from_white_alpha(params.image_alpha),
                        );
                    } else if params.loading {
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

                    if params.is_mandelbrot_preview {
                        let crosshair_color = egui::Color32::WHITE;
                        let vp = params.preview_viewport;
                        let px = (params.julia_c.re - vp.center.re) / vp.scale
                            + (vp.width as f64) * 0.5;
                        let py = (vp.height as f64) * 0.5
                            - (params.julia_c.im - vp.center.im) / vp.scale;
                        let cx = image_rect.min.x
                            + (px as f32 / vp.width as f32) * image_rect.width();
                        let cy = image_rect.min.y
                            + (py as f32 / vp.height as f32) * image_rect.height();
                        let stroke = egui::Stroke::new(1.0, crosshair_color);
                        ui.painter().line_segment(
                            [egui::pos2(cx, image_rect.min.y), egui::pos2(cx, image_rect.max.y)],
                            stroke,
                        );
                        ui.painter().line_segment(
                            [egui::pos2(image_rect.min.x, cy), egui::pos2(image_rect.max.x, cy)],
                            stroke,
                        );
                    }

                    let border_stroke =
                        egui::Stroke::new(1.0, egui::Color32::from_white_alpha(BORDER_ALPHA));
                    ui.painter().rect_stroke(rect, 0.0, border_stroke, egui::StrokeKind::Outside);
                });
        });
}
