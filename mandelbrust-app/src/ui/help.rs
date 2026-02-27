use eframe::egui;

use crate::app::MandelbRustApp;

impl MandelbRustApp {
    pub(crate) fn show_help_window(&mut self, ctx: &egui::Context) {
        if !self.show_help || !self.show_hud {
            return;
        }

        let mut open = true;
        egui::Window::new("Controls & Shortcuts")
            .open(&mut open)
            .resizable(false)
            .default_width(340.0)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(egui::Color32::from_rgba_unmultiplied(10, 10, 10, 210)),
            )
            .show(ctx, |ui| {
                ui.style_mut().visuals.override_text_color =
                    Some(egui::Color32::from_rgb(220, 220, 220));

                ui.heading("Keyboard");
                ui.add_space(2.0);
                egui::Grid::new("help_kb")
                    .num_columns(2)
                    .spacing([12.0, 2.0])
                    .show(ui, |ui| {
                        let keys: &[(&str, &str)] = &[
                            ("H", "Toggle HUD"),
                            ("M", "Toggle minimap"),
                            ("S", "Save bookmark"),
                            ("B", "Bookmark explorer"),
                            ("J", "Toggle J preview panel (above minimap)"),
                            ("C", "Toggle crosshair"),
                            ("A", "Cycle anti-aliasing (Off / 2x2 / 4x4)"),
                            ("R", "Reset view"),
                            ("Esc", "Cancel render / close dialogs"),
                            ("Arrow keys", "Pan viewport"),
                            ("+ / -", "Zoom in / out"),
                            ("Backspace", "Navigate back"),
                            ("Shift+Backspace", "Navigate forward"),
                        ];
                        for &(k, d) in keys {
                            ui.label(
                                egui::RichText::new(k).strong().color(egui::Color32::WHITE),
                            );
                            ui.label(d);
                            ui.end_row();
                        }
                    });

                ui.add_space(8.0);
                ui.heading("Mouse");
                ui.add_space(2.0);
                egui::Grid::new("help_mouse")
                    .num_columns(2)
                    .spacing([12.0, 2.0])
                    .show(ui, |ui| {
                        let actions: &[(&str, &str)] = &[
                            ("Left drag", "Pan"),
                            ("Right drag", "Selection-box zoom"),
                            ("Scroll wheel", "Zoom at cursor"),
                            ("Click Julia (bottom-left)", "Open Julia C Explorer (pick c)"),
                            ("Shift+Click", "Pick Julia c value (Julia mode)"),
                            ("Left-click (Mandelbrot, J on)", "Load Julia at cursor c"),
                        ];
                        for &(k, d) in actions {
                            ui.label(
                                egui::RichText::new(k).strong().color(egui::Color32::WHITE),
                            );
                            ui.label(d);
                            ui.end_row();
                        }
                    });

                ui.add_space(8.0);
                ui.heading("Toolbar icons");
                ui.add_space(2.0);
                {
                    use egui_material_icons::icons::*;
                    let icons: &[(&str, &str)] = &[
                        (ICON_ARROW_BACK, "Navigate back"),
                        (ICON_ARROW_FORWARD, "Navigate forward"),
                        (ICON_RESTART_ALT, "Reset view"),
                        (ICON_PALETTE, "Display/color settings (palette, cycles, smooth)"),
                        (ICON_DEBLUR, "Cycle anti-aliasing"),
                        (ICON_BOOKMARK_ADD, "Save bookmark"),
                        (ICON_BOOKMARKS, "Bookmark explorer"),
                        (ICON_MAP, "Minimap (M)"),
                        (ICON_HELP_OUTLINE, "This help window"),
                        (ICON_SETTINGS, "Open settings"),
                    ];
                    egui::Grid::new("help_toolbar")
                        .num_columns(2)
                        .spacing([12.0, 2.0])
                        .show(ui, |ui| {
                            for &(k, d) in icons {
                                ui.label(
                                    egui::RichText::new(k)
                                        .size(18.0)
                                        .color(egui::Color32::WHITE),
                                );
                                ui.label(d);
                                ui.end_row();
                            }
                        });
                }
            });

        if !open {
            self.show_help = false;
        }
    }
}
