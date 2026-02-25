use eframe::egui;

use crate::app::{FractalMode, MandelbRustApp, HUD_CORNER_RADIUS, HUD_MARGIN};
use crate::color_profiles;
use crate::display_color::{PaletteMode as DisplayPaletteMode, StartFrom as DisplayStartFrom};

const TOOLBAR_MARGIN: f32 = 8.0;

impl MandelbRustApp {
    pub(crate) fn show_top_right_toolbar(&mut self, ctx: &egui::Context) {
        use egui_material_icons::icons::*;

        let hud_alpha =
            (self.preferences.hud_panel_opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
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

        let cell = egui::vec2(26.0, 22.0);

        let add_icon_btn =
            |ui: &mut egui::Ui, label: egui::RichText, enabled: bool| -> egui::Response {
                ui.allocate_ui_with_layout(
                    cell,
                    egui::Layout::centered_and_justified(egui::Direction::TopDown),
                    |ui| ui.add_enabled(enabled, egui::Button::new(label).frame(false)),
                )
                .inner
            };

        let top_y = TOOLBAR_MARGIN + self.menu_bar_height;
        egui::Area::new(egui::Id::new("hud_toolbar"))
            .anchor(egui::Align2::RIGHT_TOP, [-TOOLBAR_MARGIN, top_y])
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(egui::Color32::from_black_alpha(160))
                    .inner_margin(egui::Margin::same(4))
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;

                            if add_icon_btn(ui, mi(ICON_ARROW_BACK), self.history_pos > 0)
                                .on_hover_text("Back")
                                .clicked()
                            {
                                self.go_back();
                            }
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
                            if add_icon_btn(ui, mi(ICON_RESTART_ALT), true)
                                .on_hover_text("Reset view")
                                .clicked()
                            {
                                self.reset_view();
                            }
                            let pal_name = self.palettes[self.display_color.palette_index].name;
                            if add_icon_btn(ui, mi(ICON_PALETTE), true)
                                .on_hover_text(format!("Display/color settings ({pal_name})"))
                                .clicked()
                            {
                                self.show_palette_popup = !self.show_palette_popup;
                            }
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
                            if add_icon_btn(ui, mi(ICON_BOOKMARK_ADD), true)
                                .on_hover_text("Save bookmark (S)")
                                .clicked()
                            {
                                if self.last_jumped_bookmark_idx.is_some() {
                                    self.active_dialog = crate::app::ActiveDialog::UpdateOrSave;
                                } else {
                                    self.open_save_new_dialog();
                                }
                            }
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
                                }
                            }
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
                            if add_icon_btn(ui, mi(ICON_HELP_OUTLINE), true)
                                .on_hover_text("Controls & shortcuts")
                                .clicked()
                            {
                                self.show_help = !self.show_help;
                            }
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
                .anchor(egui::Align2::RIGHT_TOP, [-TOOLBAR_MARGIN, 38.0 + self.menu_bar_height])
                .frame(
                    egui::Frame::NONE
                        .fill(egui::Color32::from_black_alpha(220))
                        .inner_margin(egui::Margin::same(10))
                        .corner_radius(6.0),
                )
                .show(ctx, |ui| {
                    ui.style_mut().visuals.override_text_color =
                        Some(egui::Color32::from_rgb(220, 220, 220));

                    ui.heading("Profiles");
                    let profile_names = color_profiles::list_profiles();
                    if self.color_profile_selected.is_empty() && !profile_names.is_empty() {
                        self.color_profile_selected = profile_names[0].clone();
                    }
                    egui::ComboBox::from_id_salt(egui::Id::new("color_profile_list"))
                        .selected_text(if self.color_profile_selected.is_empty() {
                            "(none)"
                        } else {
                            &self.color_profile_selected
                        })
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
                                tracing::warn!("Failed to save color profile: {}", e);
                            }
                        }
                    });

                    ui.add_space(8.0);
                    ui.heading("Palette");
                    for (i, pal) in self.palettes.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let swatch = pal.preview_colors(40);
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(40.0, 12.0),
                                egui::Sense::hover(),
                            );
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
                        let by_cycles = matches!(
                            self.display_color.palette_mode,
                            DisplayPaletteMode::ByCycles { .. }
                        );
                        if ui.selectable_label(by_cycles, "By cycles").clicked() {
                            let n = match self.display_color.palette_mode {
                                DisplayPaletteMode::ByCycles { n } => n,
                                DisplayPaletteMode::ByCycleLength { .. } => 1,
                            };
                            self.display_color.palette_mode =
                                DisplayPaletteMode::ByCycles { n };
                            palette_changed = true;
                            self.bump_minimap_revision();
                        }
                        if ui.selectable_label(!by_cycles, "By cycle length").clicked() {
                            let len = match self.display_color.palette_mode {
                                DisplayPaletteMode::ByCycles { .. } => 256,
                                DisplayPaletteMode::ByCycleLength { len } => len,
                            };
                            self.display_color.palette_mode =
                                DisplayPaletteMode::ByCycleLength { len };
                            palette_changed = true;
                            self.bump_minimap_revision();
                        }
                    });
                    let (mut mode_val, is_cycles) = match self.display_color.palette_mode {
                        DisplayPaletteMode::ByCycles { n } => (n as i32, true),
                        DisplayPaletteMode::ByCycleLength { len } => (len as i32, false),
                    };
                    if ui
                        .add(egui::DragValue::new(&mut mode_val).range(1..=i32::MAX))
                        .changed()
                    {
                        let v = mode_val.max(1) as u32;
                        self.display_color.palette_mode = if is_cycles {
                            DisplayPaletteMode::ByCycles { n: v }
                        } else {
                            DisplayPaletteMode::ByCycleLength { len: v }
                        };
                        palette_changed = true;
                        self.bump_minimap_revision();
                    }
                    ui.label(if is_cycles {
                        "cycles"
                    } else {
                        "iterations per cycle"
                    });

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
                            if ui
                                .add(egui::DragValue::new(&mut start).range(0..=i32::MAX))
                                .changed()
                            {
                                self.display_color.low_threshold_start = start.max(0) as u32;
                                palette_changed = true;
                                self.bump_minimap_revision();
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Threshold end:");
                            let mut end = self.display_color.low_threshold_end as i32;
                            if ui
                                .add(egui::DragValue::new(&mut end).range(0..=i32::MAX))
                                .changed()
                            {
                                self.display_color.low_threshold_end = end.max(0) as u32;
                                palette_changed = true;
                                self.bump_minimap_revision();
                            }
                        });
                    }

                    ui.add_space(8.0);
                    if ui
                        .checkbox(
                            &mut self.display_color.smooth_coloring,
                            "Smooth coloring (log-log)",
                        )
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
                    .anchor(egui::Align2::RIGHT_TOP, [-TOOLBAR_MARGIN, 38.0 + self.menu_bar_height])
                    .show(ctx, |ui| {
                        ui.style_mut().visuals.override_text_color =
                            Some(egui::Color32::from_rgb(220, 220, 220));
                        ui.label(format!("{:.10} {:+.10}i", c.re, c.im));
                    });
            }
        }

        // ---- Fractal parameters panel (bottom-left) ----
        let old_mode = self.mode;
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

                        ui.horizontal(|ui| {
                            ui.label("Fractal:");
                            ui.selectable_value(
                                &mut self.mode,
                                FractalMode::Mandelbrot,
                                "Mandelbrot",
                            );
                            if ui
                                .selectable_label(self.mode == FractalMode::Julia, "Julia")
                                .clicked()
                            {
                                self.show_julia_c_explorer = true;
                            }
                        });
                        mode_changed = self.mode != old_mode;

                        if self.mode == FractalMode::Julia {
                            const JULIA_C_RANGE: f64 = 2.0;
                            let mut re = self.julia_c.re;
                            let mut im = self.julia_c.im;
                            let re_range = -JULIA_C_RANGE..=JULIA_C_RANGE;
                            let im_range = -JULIA_C_RANGE..=JULIA_C_RANGE;
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

                        ui.checkbox(&mut self.adaptive_iterations, "Adaptive iterations");
                        if self.adaptive_iterations {
                            let eff = self.effective_max_iterations();
                            ui.weak(format!("Effective: {eff}"));
                        }
                    });
            });

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
}
