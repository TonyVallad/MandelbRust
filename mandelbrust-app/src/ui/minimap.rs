use std::sync::Arc;
use std::thread;

use eframe::egui;

use mandelbrust_core::{Complex, Viewport};
use mandelbrust_render::RenderCancel;

use crate::app::{FractalMode, MandelbRustApp};
use crate::render_bridge::render_for_mode;

impl MandelbRustApp {
    pub(crate) fn minimap_viewport(&self) -> Viewport {
        let size = self.preferences.minimap_size.side_pixels();
        match self.mode {
            FractalMode::Mandelbrot => Viewport::default_mandelbrot(size, size),
            FractalMode::Julia => Viewport::default_julia(size, size),
        }
    }

    pub(crate) fn bump_minimap_revision(&mut self) {
        self.minimap_revision = self.minimap_revision.wrapping_add(1);
        self.bump_j_preview_revision();
    }

    pub(crate) fn bump_j_preview_revision(&mut self) {
        self.j_preview_revision = self.j_preview_revision.wrapping_add(1);
        self.last_j_preview_cursor = None;
    }

    pub(crate) fn request_minimap_if_invalid(&mut self, ctx: &egui::Context) {
        if self.minimap_loading {
            return;
        }
        let current_rev = self.minimap_revision;
        let texture_rev = self
            .minimap_texture
            .as_ref()
            .map(|(_, r)| *r)
            .unwrap_or(current_rev.wrapping_add(1));
        if texture_rev == current_rev {
            return;
        }
        self.minimap_loading = true;
        let params = self
            .params
            .with_max_iterations(self.preferences.minimap_iterations);
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

    pub(crate) fn poll_minimap_response(&mut self, ctx: &egui::Context) {
        while let Ok((result, revision)) = self.rx_minimap.try_recv() {
            if result.cancelled {
                self.minimap_loading = false;
                continue;
            }
            let params = self.color_params();
            let buffer = if let Some(ref aa) = result.aa_samples {
                self.current_palette()
                    .colorize_aa(&result.iterations, aa, &params)
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

    pub(crate) fn j_preview_viewport(&self) -> Viewport {
        let size = self.preferences.minimap_size.side_pixels();
        match self.mode {
            FractalMode::Mandelbrot => Viewport::default_julia(size, size),
            FractalMode::Julia => Viewport::default_mandelbrot(size, size),
        }
    }

    pub(crate) fn request_j_preview_if_needed(&mut self, ctx: &egui::Context) {
        if !self.preferences.show_j_preview {
            return;
        }
        const J_PREVIEW_AA: u32 = 4;
        let size = self.preferences.minimap_size.side_pixels();

        match self.mode {
            FractalMode::Mandelbrot => {
                let Some(cursor_c) = self.cursor_complex else {
                    return;
                };
                if self.j_preview_loading {
                    return;
                }
                if self.last_j_preview_cursor == Some(cursor_c) {
                    return;
                }
                self.last_j_preview_cursor = Some(cursor_c);
                self.j_preview_loading = true;
                self.j_preview_revision = self.j_preview_revision.wrapping_add(1);
                let revision = self.j_preview_revision;
                let params = self
                    .params
                    .with_max_iterations(self.preferences.julia_preview_iterations);
                let viewport = Viewport::default_julia(size, size);
                let tx = self.tx_jpreview.clone();
                let cancel = self.j_preview_cancel.clone();
                thread::spawn(move || {
                    let result = render_for_mode(
                        FractalMode::Julia,
                        params,
                        cursor_c,
                        &viewport,
                        &cancel,
                        J_PREVIEW_AA,
                    );
                    let _ = tx.send((result, revision));
                });
            }
            FractalMode::Julia => {
                if self.j_preview_loading {
                    return;
                }
                let current_rev = self.j_preview_revision;
                let texture_rev = self
                    .j_preview_texture
                    .as_ref()
                    .map(|(_, r)| *r)
                    .unwrap_or(current_rev.wrapping_add(1));
                if texture_rev == current_rev {
                    return;
                }
                self.j_preview_loading = true;
                let params = self
                    .params
                    .with_max_iterations(self.preferences.minimap_iterations);
                let viewport = Viewport::default_mandelbrot(size, size);
                let tx = self.tx_jpreview.clone();
                let revision = current_rev;
                let julia_c = self.julia_c;
                let cancel = self.j_preview_cancel.clone();
                thread::spawn(move || {
                    let result = render_for_mode(
                        FractalMode::Mandelbrot,
                        params,
                        julia_c,
                        &viewport,
                        &cancel,
                        J_PREVIEW_AA,
                    );
                    let _ = tx.send((result, revision));
                });
            }
        }
        ctx.request_repaint();
    }

    pub(crate) fn poll_j_preview_response(&mut self, ctx: &egui::Context) {
        while let Ok((result, revision)) = self.rx_jpreview.try_recv() {
            if result.cancelled {
                self.j_preview_loading = false;
                continue;
            }
            self.j_preview_loading = false;
            let should_store = self
                .j_preview_texture
                .as_ref()
                .map_or(true, |(_, r)| revision >= *r);
            if !should_store {
                continue;
            }
            let params = self.color_params();
            let buffer = if let Some(ref aa) = result.aa_samples {
                self.current_palette()
                    .colorize_aa(&result.iterations, aa, &params)
            } else {
                self.current_palette().colorize(&result.iterations, &params)
            };
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [buffer.width as usize, buffer.height as usize],
                &buffer.pixels,
            );
            let handle = ctx.load_texture("j_preview", image, egui::TextureOptions::LINEAR);
            self.j_preview_texture = Some((handle, revision));
            ctx.request_repaint();
        }
    }

    pub(crate) fn show_minimap_panel(&mut self, ctx: &egui::Context, hud_alpha: u8) {
        let size = self.preferences.minimap_size.side_pixels() as f32;
        let vp = self.minimap_viewport();
        let minimap_alpha = (hud_alpha as f32
            * self.preferences.minimap_opacity.clamp(0.0, 1.0))
        .round() as u8;
        let image_alpha =
            (255.0 * self.preferences.minimap_opacity.clamp(0.0, 1.0)).round() as u8;

        const MINIMAP_ANCHOR_MARGIN: f32 = 8.0;
        egui::Area::new(egui::Id::new("hud_minimap"))
            .anchor(
                egui::Align2::RIGHT_BOTTOM,
                [-MINIMAP_ANCHOR_MARGIN, -MINIMAP_ANCHOR_MARGIN],
            )
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(egui::Color32::from_black_alpha(minimap_alpha))
                    .inner_margin(egui::Margin::ZERO)
                    .corner_radius(0.0)
                    .show(ui, |ui| {
                        let (rect, _response) =
                            ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
                        let image_side = rect.width().min(rect.height());
                        let image_rect = egui::Rect::from_center_size(
                            rect.center(),
                            egui::vec2(image_side, image_side),
                        );

                        let to_minimap = |c: Complex| {
                            let px = (c.re - vp.center.re) / vp.scale + (vp.width as f64) * 0.5;
                            let py = (vp.height as f64) * 0.5
                                - (c.im - vp.center.im) / vp.scale;
                            let sx = image_rect.min.x
                                + (px as f32 / vp.width as f32) * image_rect.width();
                            let sy = image_rect.min.y
                                + (py as f32 / vp.height as f32) * image_rect.height();
                            (sx, sy)
                        };

                        let valid_texture = self
                            .minimap_texture
                            .as_ref()
                            .filter(|(_, rev)| *rev == self.minimap_revision)
                            .map(|(h, _)| h);

                        if let Some(tex) = valid_texture {
                            let uv = egui::Rect::from_min_max(
                                egui::pos2(0.0, 0.0),
                                egui::pos2(1.0, 1.0),
                            );
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
                                "Updating\u{2026}",
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
                                "Updating\u{2026}",
                                egui::FontId::proportional(12.0),
                                egui::Color32::GRAY,
                            );
                        }

                        let crosshair_alpha = (self
                            .preferences
                            .crosshair_opacity
                            .clamp(0.0, 1.0)
                            * 255.0)
                            .round() as u8;
                        let crosshair_color =
                            egui::Color32::from_white_alpha(crosshair_alpha);

                        let cx = self.viewport.center.re;
                        let cy = self.viewport.center.im;
                        let w = self.viewport.width as f64 * self.viewport.scale;
                        let h = self.viewport.height as f64 * self.viewport.scale;
                        let (min_x, min_y) =
                            to_minimap(Complex::new(cx - w * 0.5, cy + h * 0.5));
                        let (max_x, max_y) =
                            to_minimap(Complex::new(cx + w * 0.5, cy - h * 0.5));
                        let min_x = min_x.clamp(image_rect.min.x, image_rect.max.x);
                        let max_x = max_x.clamp(image_rect.min.x, image_rect.max.x);
                        let min_y = min_y.clamp(image_rect.min.y, image_rect.max.y);
                        let max_y = max_y.clamp(image_rect.min.y, image_rect.max.y);
                        let viewport_rect = egui::Rect::from_min_max(
                            egui::pos2(min_x, min_y),
                            egui::pos2(max_x, max_y),
                        );
                        let stroke =
                            egui::Stroke::new(1.5, egui::Color32::from_rgb(0, 255, 255));
                        ui.painter().rect_stroke(
                            viewport_rect,
                            0.0,
                            stroke,
                            egui::StrokeKind::Outside,
                        );
                        let center_x = (min_x + max_x) * 0.5;
                        let center_y = (min_y + max_y) * 0.5;
                        if image_rect.min.y < viewport_rect.min.y {
                            ui.painter().line_segment(
                                [
                                    egui::pos2(center_x, image_rect.min.y),
                                    egui::pos2(center_x, viewport_rect.min.y),
                                ],
                                egui::Stroke::new(1.0, crosshair_color),
                            );
                        }
                        if viewport_rect.max.y < image_rect.max.y {
                            ui.painter().line_segment(
                                [
                                    egui::pos2(center_x, viewport_rect.max.y),
                                    egui::pos2(center_x, image_rect.max.y),
                                ],
                                egui::Stroke::new(1.0, crosshair_color),
                            );
                        }
                        if image_rect.min.x < viewport_rect.min.x {
                            ui.painter().line_segment(
                                [
                                    egui::pos2(image_rect.min.x, center_y),
                                    egui::pos2(viewport_rect.min.x, center_y),
                                ],
                                egui::Stroke::new(1.0, crosshair_color),
                            );
                        }
                        if viewport_rect.max.x < image_rect.max.x {
                            ui.painter().line_segment(
                                [
                                    egui::pos2(viewport_rect.max.x, center_y),
                                    egui::pos2(image_rect.max.x, center_y),
                                ],
                                egui::Stroke::new(1.0, crosshair_color),
                            );
                        }

                        let border_stroke = egui::Stroke::new(
                            1.0,
                            egui::Color32::from_white_alpha(191),
                        );
                        ui.painter().rect_stroke(
                            rect,
                            0.0,
                            border_stroke,
                            egui::StrokeKind::Outside,
                        );
                    });
            });
    }
}
