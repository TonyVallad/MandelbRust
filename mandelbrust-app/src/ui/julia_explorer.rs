use eframe::egui;

use crate::app::{FractalMode, MandelbRustApp};
use crate::app_state::AppScreen;

const CYAN: egui::Color32 = egui::Color32::from_rgb(80, 200, 255);

impl MandelbRustApp {
    /// Full-screen Julia C Explorer (launched from the main menu).
    pub(crate) fn draw_julia_c_explorer_screen(&mut self, ctx: &egui::Context) {
        self.poll_julia_grid_responses(ctx);
        if self.julia_explorer_restart_pending {
            self.julia_explorer_restart_pending = false;
            self.start_julia_grid_request();
        }

        let mut go_back = false;

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("\u{2190} Back").clicked() {
                        go_back = true;
                    }
                    ui.add_space(8.0);
                    ui.heading(egui::RichText::new("Julia C Explorer").color(CYAN));
                    ui.add_space(16.0);
                    ui.weak("Pick a value of c, then explore the Julia set.");
                });
                ui.separator();
                self.draw_julia_c_explorer_in_panel(ui, ctx);
            });

        if go_back {
            self.screen = AppScreen::MainMenu;
            self.grid_cancel.cancel();
            return;
        }

        if let Some((c_re, c_im)) = self.julia_explorer_picked_c.take() {
            self.julia_c = mandelbrust_core::Complex::new(c_re, c_im);
            self.mode = FractalMode::Julia;
            self.push_history();
            self.viewport = self.default_viewport();
            self.bump_minimap_revision();
            self.needs_render = true;
            self.screen = AppScreen::FractalExplorer;
        }

        ctx.request_repaint();
    }

    pub(crate) fn draw_julia_c_explorer_in_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
    ) {
        let cell_size_px = self.preferences.julia_explorer_cell_size_px.clamp(16, 256) as f32;

        if self.julia_explorer_recolorize {
            self.julia_explorer_recolorize = false;
            let mut params = self.color_params();
            for ((i, j), buf) in &self.julia_explorer_cells {
                params.cycle_length = self.display_color.cycle_length(buf.max_iterations);
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

        let available0 = ui.available_size();
        self.check_resize(available0.x.max(1.0) as u32, available0.y.max(1.0) as u32);

        const CENTER_RE: f64 = -0.75;
        const CENTER_IM: f64 = 0.0;
        let extent_half = self.julia_explorer_extent_half;

        ui.style_mut().visuals.override_text_color =
            Some(egui::Color32::from_rgb(220, 220, 220));

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
            if ui.button("Display / color\u{2026}").clicked() {
                self.show_palette_popup = true;
            }
            ui.weak(
                "Center (-0.75, 0). Smaller C extent = zoom in. Click cell to set c. Esc to close.",
            );
        });

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
            let (grid_rect, _) =
                ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
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
                    let inner = ui.scope_builder(
                        egui::UiBuilder::new().max_rect(cell_rect),
                        |ui| {
                            ui.allocate_exact_size(
                                egui::vec2(cell_size_px, cell_size_px),
                                egui::Sense::click(),
                            )
                        },
                    );
                    let resp = inner.inner.1;
                    if resp.clicked() {
                        self.julia_explorer_picked_c = Some((c_re, c_im));
                    }
                    resp.on_hover_text(format!("C = {:.6} {:+.6}i", c_re, c_im));
                    let rect = inner.response.rect;
                    if let Some(tex) = self.julia_explorer_textures.get(&(i, j)) {
                        let uv = egui::Rect::from_min_max(
                            egui::pos2(0.0, 0.0),
                            egui::pos2(1.0, 1.0),
                        );
                        ui.painter()
                            .image(tex.id(), rect, uv, egui::Color32::WHITE);
                    } else {
                        ui.painter()
                            .rect_filled(rect, 0.0, egui::Color32::from_gray(50));
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "\u{2026}",
                            egui::FontId::proportional(14.0),
                            egui::Color32::GRAY,
                        );
                    }
                }
            }
        });
        ui.add_space((available.y - side) / 2.0);
    }

    pub(crate) fn show_julia_c_explorer_window(&mut self, _ctx: &egui::Context) {
        // Julia C Explorer is drawn in the central panel; no popup window.
    }
}
