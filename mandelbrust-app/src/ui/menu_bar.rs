use eframe::egui;

use crate::app::{ActiveDialog, BookmarkTab, FractalMode, MandelbRustApp};
use crate::app_state::AppScreen;

impl MandelbRustApp {
    /// Draw the top menu bar. Must be called **before** `CentralPanel` so that
    /// `egui` reserves vertical space for it. The menu bar is always visible,
    /// regardless of HUD state or active screen.
    pub(crate) fn draw_menu_bar(&mut self, ctx: &egui::Context) {
        let resp = egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                self.menu_file(ui, ctx);
                self.menu_edit(ui, ctx);
                self.menu_fractal(ui, ctx);
                self.menu_view(ui);
                self.menu_help(ui);
            });
        });
        self.menu_bar_height = resp.response.rect.height();
    }

    fn menu_file(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let in_explorer = self.screen == AppScreen::FractalExplorer;

        ui.menu_button("File", |ui| {
            if self.screen != AppScreen::MainMenu {
                if ui.button("Main Menu").clicked() {
                    ui.close();
                    self.screen = AppScreen::MainMenu;
                }
                ui.separator();
            }
            if ui
                .add_enabled(in_explorer, shortcut_item("Save Bookmark", "S"))
                .clicked()
            {
                ui.close();
                if self.last_jumped_bookmark_idx.is_some() {
                    self.active_dialog = ActiveDialog::UpdateOrSave;
                } else {
                    self.open_save_new_dialog();
                }
            }
            if ui
                .add(shortcut_item("Open Bookmarks", "B"))
                .clicked()
            {
                ui.close();
                if in_explorer {
                    self.show_bookmarks = !self.show_bookmarks;
                    if self.show_bookmarks {
                        self.bookmark_store.reload();
                        self.bookmark_tab = match self.mode {
                            FractalMode::Mandelbrot => BookmarkTab::Mandelbrot,
                            FractalMode::Julia => BookmarkTab::Julia,
                        };
                    }
                } else {
                    self.screen = AppScreen::BookmarkBrowser;
                    self.bookmark_store.reload();
                    self.browser_selected_bookmark = None;
                }
            }
            ui.separator();
            let export_item = egui::Button::new("Export Image…");
            if ui
                .add_enabled(false, export_item)
                .on_disabled_hover_text("Coming in a future update")
                .clicked()
            {
                ui.close();
            }
            ui.separator();
            if ui.button("Quit").clicked() {
                ui.close();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }

    fn menu_edit(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.menu_button("Edit", |ui| {
            if ui.button("Copy Coordinates").clicked() {
                ui.close();
                let text = self.format_coordinates_for_clipboard();
                ctx.copy_text(text);
            }
            ui.separator();
            if ui
                .add(shortcut_item("Reset View", "R"))
                .clicked()
            {
                ui.close();
                self.reset_view();
            }
        });
    }

    fn menu_fractal(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.menu_button("Fractal", |ui| {
            let is_mandelbrot = self.mode == FractalMode::Mandelbrot;
            if ui
                .add_enabled(!is_mandelbrot, egui::Button::new("Switch to Mandelbrot"))
                .clicked()
            {
                ui.close();
                self.switch_to_mandelbrot();
            }
            if ui
                .add_enabled(is_mandelbrot, egui::Button::new("Switch to Julia"))
                .clicked()
            {
                ui.close();
                self.switch_to_julia(ctx);
            }
            ui.separator();
            if ui.button("Julia C Explorer").clicked() {
                ui.close();
                self.show_julia_c_explorer = !self.show_julia_c_explorer;
            }
        });
    }

    fn menu_view(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("View", |ui| {
            if ui
                .add(shortcut_item(
                    if self.show_hud { "Hide HUD" } else { "Show HUD" },
                    "H",
                ))
                .clicked()
            {
                ui.close();
                self.show_hud = !self.show_hud;
            }
            if ui
                .add(shortcut_item(
                    if self.preferences.show_minimap {
                        "Hide Minimap"
                    } else {
                        "Show Minimap"
                    },
                    "M",
                ))
                .clicked()
            {
                ui.close();
                self.preferences.show_minimap = !self.preferences.show_minimap;
                self.preferences.save();
            }
            if ui
                .add(shortcut_item(
                    if self.preferences.show_j_preview {
                        "Hide J Preview"
                    } else {
                        "Show J Preview"
                    },
                    "J",
                ))
                .clicked()
            {
                ui.close();
                self.preferences.show_j_preview = !self.preferences.show_j_preview;
                self.preferences.save();
                self.bump_j_preview_revision();
            }
            if ui
                .add(shortcut_item(
                    if self.show_crosshair {
                        "Hide Crosshair"
                    } else {
                        "Show Crosshair"
                    },
                    "C",
                ))
                .clicked()
            {
                ui.close();
                self.show_crosshair = !self.show_crosshair;
            }
            ui.separator();
            if ui
                .add(shortcut_item("Cycle Anti-Aliasing", "A"))
                .clicked()
            {
                ui.close();
                self.cycle_aa();
            }
            ui.separator();
            if ui.button("Settings…").clicked() {
                ui.close();
                self.show_controls = !self.show_controls;
            }
        });
    }

    fn menu_help(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Help", |ui| {
            if ui.button("Keyboard Shortcuts").clicked() {
                ui.close();
                self.show_help = true;
            }
            ui.separator();
            if ui.button("About MandelbRust").clicked() {
                ui.close();
                self.show_about = true;
            }
        });
    }

    fn format_coordinates_for_clipboard(&self) -> String {
        let zoom = 1.0 / self.viewport.scale;
        let mut text = format!(
            "Mode: {}\nCenter: {:.15} {:+.15}i\nZoom: {zoom:.6e}\nIterations: {}",
            self.mode.label(),
            self.viewport.center.re,
            self.viewport.center.im,
            self.params.max_iterations,
        );
        if self.mode == FractalMode::Julia {
            text.push_str(&format!(
                "\nJulia c: {:.15} {:+.15}i",
                self.julia_c.re, self.julia_c.im
            ));
        }
        text
    }

    fn switch_to_mandelbrot(&mut self) {
        self.mode = FractalMode::Mandelbrot;
        self.push_history();
        self.viewport = self.default_viewport();
        self.bump_minimap_revision();
        self.needs_render = true;
    }

    fn switch_to_julia(&mut self, ctx: &egui::Context) {
        let _ = ctx;
        self.mode = FractalMode::Julia;
        self.push_history();
        self.viewport = self.default_viewport();
        self.bump_minimap_revision();
        self.needs_render = true;
    }

    pub(crate) fn cycle_aa(&mut self) {
        self.aa_level = match self.aa_level {
            0 => 2,
            2 => 4,
            _ => 0,
        };
        if self.aa_level == 0 {
            self.current_aa = None;
        }
        self.needs_render = true;
    }

    pub(crate) fn draw_about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut open = true;
        egui::Window::new("About MandelbRust")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(4.0);
                    ui.heading(
                        egui::RichText::new("MandelbRust")
                            .strong()
                            .color(egui::Color32::from_rgb(80, 200, 255)),
                    );
                    ui.add_space(4.0);
                    ui.label("A high-performance fractal explorer written in Rust.");
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("github.com/TonyVallad/MandelbRust")
                            .small()
                            .color(egui::Color32::from_rgb(160, 160, 160)),
                    );
                    ui.add_space(4.0);
                });
            });
        if !open {
            self.show_about = false;
        }
    }
}

/// Build a `Button` with a right-aligned keyboard shortcut hint.
fn shortcut_item(label: &str, shortcut: &str) -> egui::Button<'static> {
    let text = format!("{label}    {shortcut}");
    egui::Button::new(
        egui::RichText::new(text).size(13.0),
    )
    .wrap_mode(egui::TextWrapMode::Extend)
}
