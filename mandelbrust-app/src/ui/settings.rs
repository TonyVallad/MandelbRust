use eframe::egui;

use crate::app::MandelbRustApp;
use crate::preferences;

impl MandelbRustApp {
    pub(crate) fn show_controls_panel(&mut self, ctx: &egui::Context) {
        if !self.show_controls || !self.show_hud {
            return;
        }

        let mut open = true;
        egui::Window::new("Settings")
            .open(&mut open)
            .resizable(true)
            .default_width(320.0)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(egui::Color32::from_rgba_unmultiplied(10, 10, 10, 210)),
            )
            .show(ctx, |ui| {
                ui.checkbox(
                    &mut self.preferences.restore_last_view,
                    "Restore last view on startup",
                );

                ui.add_space(6.0);
                ui.label("Bookmarks folder:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.bookmarks_dir_buf)
                        .desired_width(ui.available_width()),
                );
                ui.horizontal(|ui| {
                    if ui.small_button("Browse...").clicked() {
                        let start = std::path::Path::new(&self.bookmarks_dir_buf);
                        let mut dialog = rfd::FileDialog::new();
                        if start.is_dir() {
                            dialog = dialog.set_directory(start);
                        }
                        if let Some(folder) = dialog.pick_folder() {
                            self.bookmarks_dir_buf = folder.to_string_lossy().to_string();
                        }
                    }
                    if ui.small_button("Apply").clicked() {
                        let new_dir = self.bookmarks_dir_buf.trim().to_string();
                        self.preferences.bookmarks_dir = new_dir.clone();
                        self.preferences.save();
                        self.bookmark_store.set_directory(&new_dir);
                        self.thumbnail_cache.clear();
                        self.failed_thumbnails.clear();
                        self.bookmarks_dir_buf = self
                            .bookmark_store
                            .directory()
                            .to_string_lossy()
                            .to_string();
                    }
                    if ui.small_button("Reset").clicked() {
                        self.preferences.bookmarks_dir = String::new();
                        self.preferences.save();
                        self.bookmark_store.set_directory("");
                        self.thumbnail_cache.clear();
                        self.failed_thumbnails.clear();
                        self.bookmarks_dir_buf = self
                            .bookmark_store
                            .directory()
                            .to_string_lossy()
                            .to_string();
                    }
                });

                ui.add_space(10.0);
                ui.heading("Minimap & HUD");
                ui.horizontal(|ui| {
                    ui.label("Minimap size:");
                    egui::ComboBox::from_id_salt(egui::Id::new("minimap_size"))
                        .selected_text(match self.preferences.minimap_size {
                            preferences::MinimapSize::Small => "Small (128)",
                            preferences::MinimapSize::Medium => "Medium (256)",
                            preferences::MinimapSize::Large => "Large (384)",
                        })
                        .show_ui(ui, |ui| {
                            use preferences::MinimapSize;
                            for size in
                                [MinimapSize::Small, MinimapSize::Medium, MinimapSize::Large]
                            {
                                let label = match size {
                                    MinimapSize::Small => "Small (128 px)",
                                    MinimapSize::Medium => "Medium (256 px)",
                                    MinimapSize::Large => "Large (384 px)",
                                };
                                if ui
                                    .selectable_value(
                                        &mut self.preferences.minimap_size,
                                        size,
                                        label,
                                    )
                                    .changed()
                                {
                                    self.preferences.save();
                                    self.bump_minimap_revision();
                                }
                            }
                        });
                });
                if ui
                    .add(
                        egui::Slider::new(
                            &mut self.preferences.minimap_zoom_half_extent,
                            0.5..=10.0,
                        )
                        .text("Minimap zoom (range \u{00b1}"),
                    )
                    .changed()
                {
                    self.preferences.save();
                    self.bump_minimap_revision();
                }
                ui.label("(complex-plane half-extent, default 2 = -2..2)");
                if ui
                    .add(
                        egui::Slider::new(&mut self.preferences.minimap_iterations, 50..=2000)
                            .text("Minimap iterations")
                            .logarithmic(true),
                    )
                    .changed()
                {
                    self.preferences.save();
                    self.bump_minimap_revision();
                }
                if ui
                    .add(
                        egui::Slider::new(&mut self.preferences.minimap_opacity, 0.0..=1.0)
                            .text("Minimap opacity"),
                    )
                    .changed()
                {
                    self.preferences.save();
                }
                if ui
                    .add(
                        egui::Slider::new(&mut self.preferences.crosshair_opacity, 0.0..=1.0)
                            .text("Crosshair opacity"),
                    )
                    .changed()
                {
                    self.preferences.save();
                }
                ui.add_space(6.0);
                ui.heading("J preview panel");
                if ui
                    .checkbox(
                        &mut self.preferences.show_j_preview,
                        "Show J preview panel",
                    )
                    .changed()
                {
                    self.preferences.save();
                    self.bump_j_preview_revision();
                }
                if ui
                    .add(
                        egui::Slider::new(
                            &mut self.preferences.julia_preview_iterations,
                            50..=1000,
                        )
                        .text("Julia preview iterations")
                        .logarithmic(true),
                    )
                    .changed()
                {
                    self.preferences.save();
                    self.bump_j_preview_revision();
                }
                ui.label("(Mandelbrot mode: live Julia at cursor; default 250)");
                if ui
                    .add(
                        egui::Slider::new(&mut self.preferences.hud_panel_opacity, 0.0..=1.0)
                            .text("HUD panel opacity"),
                    )
                    .changed()
                {
                    self.preferences.save();
                }
                ui.add_space(10.0);
                ui.heading("Julia C Explorer");
                ui.label("Square grid (1:1 C aspect), centered in viewport.");
                if ui
                    .add(
                        egui::Slider::new(
                            &mut self.preferences.julia_explorer_max_iterations,
                            50..=500,
                        )
                        .text("Grid preview iterations (default 100)"),
                    )
                    .changed()
                {
                    self.preferences.save();
                }
                if ui
                    .add(
                        egui::Slider::new(
                            &mut self.preferences.julia_explorer_extent_half,
                            0.05..=4.0,
                        )
                        .logarithmic(true)
                        .text("C extent (zoom)"),
                    )
                    .changed()
                {
                    self.preferences.save();
                    self.julia_explorer_extent_half =
                        self.preferences.julia_explorer_extent_half;
                    self.julia_explorer_restart_pending = true;
                }
                ui.horizontal(|ui| {
                    ui.label("Square size (px):");
                    if ui
                        .add(
                            egui::DragValue::new(
                                &mut self.preferences.julia_explorer_cell_size_px,
                            )
                            .range(16..=256)
                            .suffix(""),
                        )
                        .changed()
                    {
                        self.preferences.save();
                        self.julia_explorer_restart_pending = true;
                    }
                });
            });

        if !open {
            self.show_controls = false;
        }
    }
}
