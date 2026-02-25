use eframe::egui;

use crate::app::{FractalMode, MandelbRustApp, HUD_CORNER_RADIUS, HUD_MARGIN};
use crate::j_preview;
use crate::render_bridge::RenderPhase;

impl MandelbRustApp {
    pub(crate) fn show_hud(&mut self, ctx: &egui::Context) {
        if !self.show_hud {
            return;
        }
        if self.show_julia_c_explorer {
            return;
        }

        let hud_alpha =
            (self.preferences.hud_panel_opacity.clamp(0.0, 1.0) * 255.0).round() as u8;

        // -- Top-left: viewport info --
        let top_y = HUD_MARGIN + self.menu_bar_height;
        egui::Area::new(egui::Id::new("hud_params"))
            .anchor(egui::Align2::LEFT_TOP, [HUD_MARGIN, top_y])
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
                        ui.label(format!("Precision: {}", self.precision_mode_label()));

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

        // -- Bottom-centre: render stats --
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
                            RenderPhase::Rendering | RenderPhase::Refining => {
                                egui::Color32::YELLOW
                            }
                            RenderPhase::Done => egui::Color32::from_rgb(100, 255, 100),
                        };
                        ui.colored_label(phase_color, self.render_phase.label());

                        ui.label(format!(
                            "{:.1} ms",
                            self.render_time.as_secs_f64() * 1000.0,
                        ));
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

        // -- J preview panel (Phase 10.5) --
        if self.preferences.show_j_preview {
            let size = self.preferences.minimap_size.side_pixels() as f32;
            let j_alpha = (hud_alpha as f32
                * self.preferences.minimap_opacity.clamp(0.0, 1.0))
            .round() as u8;
            let image_alpha =
                (255.0 * self.preferences.minimap_opacity.clamp(0.0, 1.0)).round() as u8;
            let texture = self.j_preview_texture.as_ref().map(|(h, _)| h);
            j_preview::draw_j_preview_panel(
                ctx,
                j_preview::JPreviewDrawParams {
                    size_px: size,
                    panel_alpha: j_alpha,
                    image_alpha,
                    texture,
                    loading: self.j_preview_loading,
                    preview_viewport: self.j_preview_viewport(),
                    julia_c: self.julia_c,
                    is_mandelbrot_preview: self.mode == FractalMode::Julia,
                },
            );
        }

        // -- Minimap --
        if self.preferences.show_minimap {
            self.show_minimap_panel(ctx, hud_alpha);
        }

        // -- Top-right toolbar + fractal params --
        self.show_top_right_toolbar(ctx);
    }
}
