use eframe::egui;

use mandelbrust_core::ComplexDD;

use crate::app::{FractalMode, MandelbRustApp, PAN_FRACTION, ZOOM_SPEED};

impl MandelbRustApp {
    pub(crate) fn handle_canvas_input(&mut self, ctx: &egui::Context, response: &egui::Response) {
        self.cursor_complex = response.hover_pos().map(|pos| {
            let px = (pos.x - response.rect.min.x) as u32;
            let py = (pos.y - response.rect.min.y) as u32;
            self.viewport.pixel_to_complex(px, py)
        });

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
            self.cancel.cancel();
            self.render_id += 1;
            self.draw_offset = self.pan_offset;

            let dx = self.pan_offset.x.round() as i32;
            let dy = self.pan_offset.y.round() as i32;
            self.commit_pan_offset();

            if let Some(ref mut iter_buf) = self.current_iterations {
                iter_buf.shift(dx, dy);
                if let Some(ref mut aa) = self.current_aa {
                    aa.shift(dx, dy);
                }
            }

            self.pan_completed = true;
            self.needs_render = true;
        }

        if response.drag_started_by(egui::PointerButton::Secondary) {
            self.zoom_rect_start = response.interact_pointer_pos();
        }
        if response.drag_stopped_by(egui::PointerButton::Secondary) {
            if let (Some(start), Some(end)) = (self.zoom_rect_start.take(), response.hover_pos()) {
                let dx = (end.x - start.x).abs();
                let dy = (end.y - start.y).abs();
                if dx > 5.0 || dy > 5.0 {
                    let rect = response.rect;
                    let vp_w = rect.width();
                    let vp_h = rect.height();
                    let fraction = (dx / vp_w).max(dy / vp_h).max(0.01);
                    let mid_x = (start.x + end.x) / 2.0;
                    let mid_y = (start.y + end.y) / 2.0;
                    let px = (mid_x - rect.min.x) as u32;
                    let py = (mid_y - rect.min.y) as u32;
                    let delta = self.viewport.pixel_to_delta(px, py);
                    let new_center = self.viewport.center_dd + ComplexDD::from(delta);
                    self.push_history();
                    self.viewport.set_center_dd(new_center);
                    self.viewport.scale *= fraction as f64;
                    self.needs_render = true;
                }
            }
            self.zoom_rect_start = None;
        }

        if self.mode == FractalMode::Julia && response.clicked() && ctx.input(|i| i.modifiers.shift)
        {
            if let Some(c) = self.cursor_complex {
                self.julia_c = c;
                self.bump_minimap_revision();
                self.needs_render = true;
            }
        }

        if self.mode == FractalMode::Mandelbrot
            && self.preferences.show_j_preview
            && response.clicked()
            && !ctx.input(|i| i.modifiers.shift)
        {
            if let Some(c) = self.cursor_complex {
                self.julia_c = c;
                self.mode = FractalMode::Julia;
                self.push_history();
                self.viewport = self.default_viewport();
                self.bump_minimap_revision();
                self.needs_render = true;
            }
        }
    }

    pub(crate) fn handle_keyboard(&mut self, ctx: &egui::Context) {
        let text_editing = ctx.memory(|m| m.focused().is_some());

        ctx.input(|input| {
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

            if input.key_pressed(egui::Key::Plus) || input.key_pressed(egui::Key::Equals) {
                self.zoom_center(0.8);
            }
            if input.key_pressed(egui::Key::Minus) {
                self.zoom_center(1.25);
            }

            if input.key_pressed(egui::Key::Escape) {
                if self.active_dialog != crate::app::ActiveDialog::None {
                    self.active_dialog = crate::app::ActiveDialog::None;
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
                return;
            }

            if input.key_pressed(egui::Key::R) && !input.modifiers.ctrl {
                self.reset_view();
            }
            if input.key_pressed(egui::Key::H) {
                self.show_hud = !self.show_hud;
            }
            if input.key_pressed(egui::Key::C) {
                self.show_crosshair = !self.show_crosshair;
            }
            if input.key_pressed(egui::Key::S) && !input.modifiers.ctrl {
                if self.last_jumped_bookmark_idx.is_some() {
                    self.active_dialog = crate::app::ActiveDialog::UpdateOrSave;
                } else {
                    self.open_save_new_dialog();
                }
            }
            if input.key_pressed(egui::Key::B) {
                self.show_bookmarks = !self.show_bookmarks;
                if self.show_bookmarks {
                    self.bookmark_store.reload();
                    self.bookmark_tab = match self.mode {
                        FractalMode::Mandelbrot => crate::app::BookmarkTab::Mandelbrot,
                        FractalMode::Julia => crate::app::BookmarkTab::Julia,
                    };
                }
            }
            if input.key_pressed(egui::Key::J) {
                self.preferences.show_j_preview = !self.preferences.show_j_preview;
                self.preferences.save();
                self.bump_j_preview_revision();
            }
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
}
