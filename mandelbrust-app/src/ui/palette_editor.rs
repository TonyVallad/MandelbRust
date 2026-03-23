//! Palette editor UI: gradient bar, draggable color stops, and palette management.
//!
//! The editor is shown in a separate popup window. Endpoint stops at positions
//! 0.0 and 1.0 are always present and cannot be moved or deleted. A "Lock end
//! to start" toggle keeps the last stop color in sync with the first for
//! seamless multi-cycle tiling.

use eframe::egui;

use mandelbrust_core::palette_data::{ColorStop, PaletteDefinition, Rgb};
use mandelbrust_render::Palette;

use super::color_picker::{show_color_picker, ColorPickerState};
use crate::app::MandelbRustApp;

/// Persistent state for the palette editor.
#[derive(Debug, Clone)]
pub(crate) struct PaletteEditorState {
    pub selected_stop: Option<usize>,
    pub picker: ColorPickerState,
    pub rename_text: String,
    pub renaming: bool,
    pub dragging_stop: Option<usize>,
    pub confirm_space_evenly: bool,
}

impl Default for PaletteEditorState {
    fn default() -> Self {
        Self {
            selected_stop: None,
            picker: ColorPickerState::default(),
            rename_text: String::new(),
            renaming: false,
            dragging_stop: None,
            confirm_space_evenly: false,
        }
    }
}

/// Response flags from palette editor UI.
pub(crate) struct PaletteEditorResponse {
    pub palette_changed: bool,
    pub needs_recolorize: bool,
}

impl MandelbRustApp {
    // -----------------------------------------------------------------------
    // Palette list (shown inside the Palette tab of Display/color window)
    // -----------------------------------------------------------------------

    /// Show the palette selection list: user palettes first, then builtins.
    /// Returns true if the palette selection changed (needs recolorize).
    pub(crate) fn show_palette_list(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        let current_custom = self.display_color.custom_palette_name.clone();
        let using_builtin = current_custom.is_none();

        // --- User palettes ---
        ui.heading("User palettes");
        let names: Vec<String> =
            self.user_palette_defs.iter().map(|d| d.name.clone()).collect();
        for (i, name) in names.iter().enumerate() {
            ui.horizontal(|ui| {
                let swatch = palette_preview_strip(&self.user_palette_defs[i], 40);
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(40.0, 12.0), egui::Sense::hover());
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
                let is_active = current_custom.as_deref() == Some(name.as_str());
                let label = if is_active {
                    egui::RichText::new(name.as_str()).strong()
                } else {
                    egui::RichText::new(name.as_str())
                };
                if ui.selectable_label(is_active, label).clicked() {
                    self.display_color.custom_palette_name = Some(name.clone());
                    changed = true;
                    self.pending_minimap_bump = true;
                }
            });
        }

        ui.horizontal(|ui| {
            if ui.small_button("New").clicked() {
                let pname = next_palette_name(&self.user_palette_defs);
                let mut def = PaletteDefinition::new(
                    pname.clone(),
                    vec![
                        ColorStop {
                            position: 0.0,
                            color: Rgb::new(0, 0, 0),
                        },
                        ColorStop {
                            position: 1.0,
                            color: Rgb::new(255, 255, 255),
                        },
                    ],
                );
                def.enforce_lock();
                if let Err(e) = crate::palette_io::save_palette(&def) {
                    tracing::warn!("Failed to save palette: {e}");
                }
                self.user_palette_cache.push(Palette::from_definition(&def));
                self.user_palette_defs.push(def);
                self.display_color.custom_palette_name = Some(pname);
                self.palette_editor_state.selected_stop = None;
                self.palette_editor_state.renaming = true;
                self.palette_editor_state.rename_text =
                    self.display_color.custom_palette_name.clone().unwrap_or_default();
                self.show_palette_editor_window = true;
                changed = true;
                self.pending_minimap_bump = true;
            }
            if self.display_color.custom_palette_name.is_some() {
                if ui.small_button("Edit").clicked() {
                    self.show_palette_editor_window = true;
                }
            }
        });

        ui.add_space(4.0);

        // --- Builtin palettes ---
        ui.heading("Built-in palettes");
        for (i, pal) in self.palettes.iter().enumerate() {
            ui.horizontal(|ui| {
                let swatch = pal.preview_colors(40);
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(40.0, 12.0), egui::Sense::hover());
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
                let is_selected = using_builtin && i == self.display_color.palette_index;
                let label = if is_selected {
                    egui::RichText::new(pal.name.as_str()).strong()
                } else {
                    egui::RichText::new(pal.name.as_str())
                };
                if ui.selectable_label(is_selected, label).clicked() {
                    self.display_color.palette_index = i;
                    self.display_color.custom_palette_name = None;
                    changed = true;
                    self.pending_minimap_bump = true;
                }
            });
        }

        changed
    }

    // -----------------------------------------------------------------------
    // Palette editor popup window
    // -----------------------------------------------------------------------

    pub(crate) fn show_palette_editor_popup(
        &mut self,
        ctx: &egui::Context,
    ) -> PaletteEditorResponse {
        let mut resp = PaletteEditorResponse {
            palette_changed: false,
            needs_recolorize: false,
        };

        if !self.show_palette_editor_window {
            return resp;
        }

        let active_idx = self
            .display_color
            .custom_palette_name
            .as_ref()
            .and_then(|name| self.user_palette_defs.iter().position(|d| d.name == *name));

        let Some(idx) = active_idx else {
            self.show_palette_editor_window = false;
            return resp;
        };

        let mut open = true;
        let title = format!(
            "Palette Editor — {}",
            self.user_palette_defs[idx].name.as_str()
        );

        egui::Window::new(title)
            .id(egui::Id::new("palette_editor_window"))
            .open(&mut open)
            .resizable(true)
            .default_width(320.0)
            .frame(
                egui::Frame::NONE
                    .fill(egui::Color32::from_black_alpha(230))
                    .inner_margin(egui::Margin::same(10))
                    .corner_radius(6.0),
            )
            .show(ctx, |ui| {
                ui.style_mut().visuals.override_text_color =
                    Some(egui::Color32::from_rgb(220, 220, 220));

                // Rename / delete
                let mut deleted = false;
                ui.horizontal(|ui| {
                    if self.palette_editor_state.renaming {
                        let done = ui
                            .add(
                                egui::TextEdit::singleline(
                                    &mut self.palette_editor_state.rename_text,
                                )
                                .desired_width(120.0),
                            )
                            .lost_focus();
                        if done || ui.small_button("Ok").clicked() {
                            let new_name =
                                self.palette_editor_state.rename_text.trim().to_string();
                            if !new_name.is_empty()
                                && new_name != self.user_palette_defs[idx].name
                            {
                                let old = self.user_palette_defs[idx].name.clone();
                                let _ = crate::palette_io::rename_palette(&old, &new_name);
                                self.user_palette_defs[idx].name = new_name.clone();
                                self.user_palette_cache[idx].name = new_name.clone();
                                self.display_color.custom_palette_name = Some(new_name);
                            }
                            self.palette_editor_state.renaming = false;
                        }
                    } else {
                        if ui.small_button("Rename").clicked() {
                            self.palette_editor_state.renaming = true;
                            self.palette_editor_state.rename_text =
                                self.user_palette_defs[idx].name.clone();
                        }
                        if ui.small_button("Delete").clicked() {
                            deleted = true;
                        }
                    }
                });
                if deleted {
                    let name = self.user_palette_defs[idx].name.clone();
                    let _ = crate::palette_io::delete_palette(&name);
                    self.user_palette_defs.remove(idx);
                    self.user_palette_cache.remove(idx);
                    self.display_color.custom_palette_name = None;
                    self.palette_editor_state.selected_stop = None;
                    resp.needs_recolorize = true;
                    resp.palette_changed = true;
                    self.pending_minimap_bump = true;
                    self.show_palette_editor_window = false;
                    return;
                }

                ui.add_space(4.0);

                // --- Lock end to start toggle ---
                let mut lock = self.user_palette_defs[idx].lock_end_to_start;
                if ui.checkbox(&mut lock, "Lock end color to start").changed() {
                    self.user_palette_defs[idx].lock_end_to_start = lock;
                    if lock {
                        self.user_palette_defs[idx].enforce_lock();
                    }
                    resp.palette_changed = true;
                }

                ui.add_space(4.0);

                // --- Gradient bar ---
                let bar_width = ui.available_width().min(280.0);
                let bar_height = 24.0;
                let (bar_rect, bar_resp) = ui
                    .allocate_exact_size(egui::vec2(bar_width, bar_height), egui::Sense::click());

                paint_gradient_bar(ui.painter(), bar_rect, &self.user_palette_defs[idx]);

                // Add stop on click
                if bar_resp.clicked() {
                    if let Some(pos) = bar_resp.interact_pointer_pos() {
                        let t = ((pos.x - bar_rect.min.x) / bar_width).clamp(0.01, 0.99) as f64;
                        let rgba = self.user_palette_defs[idx].sample(t);
                        let new_stop = ColorStop {
                            position: t,
                            color: Rgb::new(rgba[0], rgba[1], rgba[2]),
                        };
                        self.user_palette_defs[idx].colors.push(new_stop);
                        self.user_palette_defs[idx].sort_stops();
                        let new_si = self.user_palette_defs[idx]
                            .colors
                            .iter()
                            .position(|s| (s.position - t).abs() < 1e-9)
                            .unwrap_or(0);
                        self.palette_editor_state.selected_stop = Some(new_si);
                        let c = new_stop.color;
                        self.palette_editor_state.picker.set_rgb(c.r, c.g, c.b);
                        resp.palette_changed = true;
                    }
                }

                // --- Color stop markers ---
                let stop_radius = 5.0_f32;
                let marker_y = bar_rect.max.y + 4.0;
                let (_markers_rect, markers_resp) = ui.allocate_exact_size(
                    egui::vec2(bar_width, stop_radius * 2.0 + 4.0),
                    egui::Sense::click_and_drag(),
                );

                let num_stops = self.user_palette_defs[idx].colors.len();
                for (si, stop) in self.user_palette_defs[idx].colors.iter().enumerate() {
                    let x = bar_rect.min.x + stop.position as f32 * bar_width;
                    let center = egui::pos2(x, marker_y + stop_radius);
                    let is_selected = self.palette_editor_state.selected_stop == Some(si);
                    let is_endpoint = si == 0 || si == num_stops - 1;
                    let stroke_color = if is_selected {
                        egui::Color32::YELLOW
                    } else if is_endpoint {
                        egui::Color32::from_rgb(180, 180, 180)
                    } else {
                        egui::Color32::WHITE
                    };

                    // Endpoints drawn as squares, inner stops as circles
                    if is_endpoint {
                        let half = stop_radius;
                        let ep_rect = egui::Rect::from_center_size(
                            center,
                            egui::vec2(half * 2.0, half * 2.0),
                        );
                        ui.painter().rect_filled(
                            ep_rect,
                            2.0,
                            egui::Color32::from_rgb(
                                stop.color.r,
                                stop.color.g,
                                stop.color.b,
                            ),
                        );
                        ui.painter().rect_stroke(
                            ep_rect,
                            2.0,
                            egui::Stroke::new(
                                if is_selected { 2.0 } else { 1.0 },
                                stroke_color,
                            ),
                            egui::StrokeKind::Outside,
                        );
                    } else {
                        ui.painter().circle_filled(
                            center,
                            stop_radius,
                            egui::Color32::from_rgb(
                                stop.color.r,
                                stop.color.g,
                                stop.color.b,
                            ),
                        );
                        ui.painter().circle_stroke(
                            center,
                            stop_radius,
                            egui::Stroke::new(
                                if is_selected { 2.0 } else { 1.0 },
                                stroke_color,
                            ),
                        );
                    }
                }

                // Handle drag / select on markers
                if markers_resp.drag_started() {
                    if let Some(pos) = markers_resp.interact_pointer_pos() {
                        let closest = closest_stop(
                            &self.user_palette_defs[idx].colors,
                            pos.x,
                            bar_rect.min.x,
                            bar_width,
                        );
                        self.palette_editor_state.dragging_stop = closest;
                        if let Some(si) = closest {
                            self.palette_editor_state.selected_stop = Some(si);
                            let c = self.user_palette_defs[idx].colors[si].color;
                            self.palette_editor_state.picker.set_rgb(c.r, c.g, c.b);
                        }
                    }
                }

                if markers_resp.dragged() {
                    if let Some(si) = self.palette_editor_state.dragging_stop {
                        let is_endpoint = si == 0
                            || si == self.user_palette_defs[idx].colors.len() - 1;
                        if !is_endpoint {
                            if let Some(pos) = markers_resp.interact_pointer_pos() {
                                let t = ((pos.x - bar_rect.min.x) / bar_width)
                                    .clamp(0.01, 0.99)
                                    as f64;
                                self.user_palette_defs[idx].colors[si].position = t;
                                resp.palette_changed = true;
                            }
                        }
                    }
                }

                if markers_resp.drag_stopped() {
                    if self.palette_editor_state.dragging_stop.is_some() {
                        self.user_palette_defs[idx].sort_stops();
                        if let Some(si) = self.palette_editor_state.selected_stop {
                            if si < self.user_palette_defs[idx].colors.len() {
                                let c = self.user_palette_defs[idx].colors[si].color;
                                self.palette_editor_state.picker.set_rgb(c.r, c.g, c.b);
                            }
                        }
                    }
                    self.palette_editor_state.dragging_stop = None;
                }

                if markers_resp.clicked() && self.palette_editor_state.dragging_stop.is_none() {
                    if let Some(pos) = markers_resp.interact_pointer_pos() {
                        let closest = closest_stop(
                            &self.user_palette_defs[idx].colors,
                            pos.x,
                            bar_rect.min.x,
                            bar_width,
                        );
                        self.palette_editor_state.selected_stop = closest;
                        if let Some(si) = closest {
                            let c = self.user_palette_defs[idx].colors[si].color;
                            self.palette_editor_state.picker.set_rgb(c.r, c.g, c.b);
                        }
                    }
                }

                // Right-click to remove (not endpoints)
                if markers_resp.secondary_clicked() {
                    if let Some(pos) = markers_resp.interact_pointer_pos() {
                        let closest = closest_stop(
                            &self.user_palette_defs[idx].colors,
                            pos.x,
                            bar_rect.min.x,
                            bar_width,
                        );
                        if let Some(si) = closest {
                            let nstops = self.user_palette_defs[idx].colors.len();
                            let is_endpoint = si == 0 || si == nstops - 1;
                            if !is_endpoint && nstops > 2 {
                                self.user_palette_defs[idx].colors.remove(si);
                                self.palette_editor_state.selected_stop = None;
                                resp.palette_changed = true;
                            }
                        }
                    }
                }

                // --- Buttons row ---
                ui.horizontal(|ui| {
                    // Remove selected (non-endpoint) stop
                    if let Some(si) = self.palette_editor_state.selected_stop {
                        let nstops = self.user_palette_defs[idx].colors.len();
                        let is_endpoint = si == 0 || si == nstops - 1;
                        if !is_endpoint && nstops > 2 {
                            if ui.small_button("Remove stop").clicked() {
                                self.user_palette_defs[idx].colors.remove(si);
                                self.palette_editor_state.selected_stop = None;
                                resp.palette_changed = true;
                            }
                        }
                    }

                    // Space evenly
                    if self.user_palette_defs[idx].colors.len() > 2 {
                        if ui.small_button("Space evenly").clicked() {
                            self.palette_editor_state.confirm_space_evenly = true;
                        }
                    }
                });

                ui.add_space(4.0);

                // --- Color picker for selected stop ---
                if let Some(si) = self.palette_editor_state.selected_stop {
                    if si < self.user_palette_defs[idx].colors.len() {
                        let nstops = self.user_palette_defs[idx].colors.len();
                        let is_last = si == nstops - 1;
                        let locked_end =
                            is_last && self.user_palette_defs[idx].lock_end_to_start;

                        ui.separator();
                        let stop_label = if si == 0 {
                            "Start".to_string()
                        } else if is_last {
                            "End".to_string()
                        } else {
                            format!(
                                "Stop {} — pos {:.3}",
                                si, self.user_palette_defs[idx].colors[si].position
                            )
                        };
                        ui.label(stop_label);

                        if locked_end {
                            ui.weak("(locked to start color)");
                        } else if show_color_picker(
                            ui,
                            &mut self.palette_editor_state.picker,
                        ) {
                            let pk = &self.palette_editor_state.picker;
                            self.user_palette_defs[idx].colors[si].color =
                                Rgb::new(pk.r, pk.g, pk.b);
                            // If editing start stop and lock is on, sync end
                            if si == 0 && self.user_palette_defs[idx].lock_end_to_start {
                                self.user_palette_defs[idx].enforce_lock();
                            }
                            resp.palette_changed = true;
                        }
                    }
                }

                // Rebuild cache and save on change
                if resp.palette_changed {
                    if self.user_palette_defs[idx].lock_end_to_start {
                        self.user_palette_defs[idx].enforce_lock();
                    }
                    self.user_palette_cache[idx] =
                        Palette::from_definition(&self.user_palette_defs[idx]);
                    let _ = crate::palette_io::save_palette(&self.user_palette_defs[idx]);
                    resp.needs_recolorize = true;
                }
            });

        if !open {
            self.show_palette_editor_window = false;
        }

        // --- Space evenly confirmation dialog ---
        if self.palette_editor_state.confirm_space_evenly {
            if let Some(idx) = active_idx {
                let mut do_it = false;
                let mut cancel = false;
                egui::Window::new("Space colors evenly?")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(
                            "This will redistribute all inner color stops at equal intervals.",
                        );
                        ui.label("Endpoint positions (0.0 and 1.0) will not change.");
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button("Confirm").clicked() {
                                do_it = true;
                            }
                            if ui.button("Cancel").clicked() {
                                cancel = true;
                            }
                        });
                    });

                if do_it {
                    let nstops = self.user_palette_defs[idx].colors.len();
                    if nstops > 2 {
                        for i in 1..nstops - 1 {
                            self.user_palette_defs[idx].colors[i].position =
                                i as f64 / (nstops - 1) as f64;
                        }
                        self.user_palette_defs[idx].sort_stops();
                        self.user_palette_cache[idx] =
                            Palette::from_definition(&self.user_palette_defs[idx]);
                        let _ =
                            crate::palette_io::save_palette(&self.user_palette_defs[idx]);
                        resp.palette_changed = true;
                        resp.needs_recolorize = true;
                    }
                    self.palette_editor_state.confirm_space_evenly = false;
                }
                if cancel {
                    self.palette_editor_state.confirm_space_evenly = false;
                }
            } else {
                self.palette_editor_state.confirm_space_evenly = false;
            }
        }

        resp
    }
}

fn palette_preview_strip(def: &PaletteDefinition, count: usize) -> Vec<[u8; 4]> {
    (0..count)
        .map(|i| def.sample(i as f64 / count as f64))
        .collect()
}

fn paint_gradient_bar(painter: &egui::Painter, rect: egui::Rect, def: &PaletteDefinition) {
    let w = rect.width() as usize;
    for x in 0..w {
        let t = x as f64 / w as f64;
        let rgba = def.sample(t);
        let col_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x + x as f32, rect.min.y),
            egui::vec2(1.0, rect.height()),
        );
        painter.rect_filled(
            col_rect,
            0.0,
            egui::Color32::from_rgb(rgba[0], rgba[1], rgba[2]),
        );
    }
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, egui::Color32::from_gray(80)),
        egui::StrokeKind::Outside,
    );
}

fn closest_stop(
    stops: &[ColorStop],
    mouse_x: f32,
    bar_left: f32,
    bar_width: f32,
) -> Option<usize> {
    let threshold = 10.0_f32;
    let mut best: Option<(usize, f32)> = None;
    for (i, stop) in stops.iter().enumerate() {
        let x = bar_left + stop.position as f32 * bar_width;
        let dist = (mouse_x - x).abs();
        if dist < threshold {
            if best.map_or(true, |(_, bd)| dist < bd) {
                best = Some((i, dist));
            }
        }
    }
    best.map(|(i, _)| i)
}

fn next_palette_name(existing: &[PaletteDefinition]) -> String {
    for n in 1u32.. {
        let candidate = format!("Palette {n}");
        if !existing.iter().any(|d| d.name == candidate) {
            return candidate;
        }
    }
    "Palette".to_string()
}
