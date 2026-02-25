use eframe::egui;

use crate::app::{defaults_for, MandelbRustApp};
use crate::app_state::AppScreen;

const TILE_CORNER_RADIUS: f32 = 4.0;
const CYAN: egui::Color32 = egui::Color32::from_rgb(80, 200, 255);

enum MenuAction {
    None,
    Resume,
    Mandelbrot,
    Julia,
    Bookmark,
}

impl MandelbRustApp {
    pub(crate) fn draw_main_menu(&mut self, ctx: &egui::Context) {
        let resume_details = self.format_resume_details();
        let bm_count = self.bookmark_store.bookmarks().len();

        let mut action = MenuAction::None;

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
            .show(ctx, |ui| {
                let available = ui.available_size();
                self.panel_size = [available.x.max(1.0) as u32, available.y.max(1.0) as u32];

                let outer_margin = 32.0_f32;
                let tile_gap = 16.0_f32;
                let sep_width = 1.0_f32;
                let sep_gap = 20.0_f32;

                let h_gaps = outer_margin * 2.0
                    + sep_gap * 2.0
                    + sep_width
                    + tile_gap * 2.0;
                let tile_width = ((available.x - h_gaps) / 4.0).clamp(120.0, 340.0);
                let tile_height = (available.y * 0.72).clamp(260.0, 520.0);

                let total_width =
                    tile_width * 4.0 + sep_gap * 2.0 + sep_width + tile_gap * 2.0;
                let x_offset = (available.x - total_width).max(0.0) / 2.0;
                let y_offset = (available.y - tile_height).max(0.0) / 2.0;

                ui.add_space(y_offset);

                ui.horizontal(|ui| {
                    ui.add_space(x_offset);

                    if draw_tile(ui, tile_width, tile_height, "Resume Exploration", &resume_details)
                        .clicked()
                    {
                        action = MenuAction::Resume;
                    }

                    ui.add_space(sep_gap);
                    let (sep_rect, _) = ui.allocate_exact_size(
                        egui::vec2(sep_width, tile_height),
                        egui::Sense::hover(),
                    );
                    ui.painter()
                        .rect_filled(sep_rect, 0.0, egui::Color32::from_gray(55));
                    ui.add_space(sep_gap);

                    let mandelbrot_desc = "Explore the iconic\nMandelbrot set\n\n\
                        z \u{2192} z\u{00b2} + c\n\n\
                        Open the fractal explorer\nwith default settings.";
                    if draw_tile(ui, tile_width, tile_height, "Mandelbrot Set", mandelbrot_desc)
                        .clicked()
                    {
                        action = MenuAction::Mandelbrot;
                    }

                    ui.add_space(tile_gap);

                    let julia_desc = "Discover the infinite variety\nof Julia sets\n\n\
                        z \u{2192} z\u{00b2} + c\n\n\
                        Choose a value of c from\nthe Julia C Explorer grid.";
                    if draw_tile(ui, tile_width, tile_height, "Julia\u{2019}s Sets", julia_desc)
                        .clicked()
                    {
                        action = MenuAction::Julia;
                    }

                    ui.add_space(tile_gap);

                    let bm_desc = if bm_count > 0 {
                        format!(
                            "Browse your saved bookmarks\nand revisit your favourite\nexploration spots.\n\n\
                             {bm_count} bookmark{}",
                            if bm_count == 1 { "" } else { "s" }
                        )
                    } else {
                        "Browse your saved bookmarks\nand revisit your favourite\nexploration spots.\n\n\
                         No bookmarks yet."
                            .to_string()
                    };
                    if draw_tile(ui, tile_width, tile_height, "Open Bookmark", &bm_desc).clicked() {
                        action = MenuAction::Bookmark;
                    }
                });
            });

        match action {
            MenuAction::Resume => {
                self.screen = AppScreen::FractalExplorer;
                self.needs_render = true;
            }
            MenuAction::Mandelbrot => {
                self.apply_mandelbrot_defaults();
                self.screen = AppScreen::FractalExplorer;
                self.needs_render = true;
            }
            MenuAction::Julia => {
                self.screen = AppScreen::JuliaCExplorer;
                self.julia_explorer_restart_pending = true;
            }
            MenuAction::Bookmark => {
                self.screen = AppScreen::BookmarkBrowser;
                self.bookmark_store.reload();
                self.browser_selected_bookmark = None;
            }
            MenuAction::None => {}
        }
    }

    fn format_resume_details(&self) -> String {
        if let Some(ref lv) = self.preferences.last_view {
            let mut s = format!("Fractal: {}", lv.mode);
            if lv.mode == "Julia" {
                s.push_str(&format!(
                    "\nC: {:.6} {:+.6}i",
                    lv.julia_c_re, lv.julia_c_im
                ));
            }
            s.push_str(&format!(
                "\n\nCenter:\n  {:.10}\n  {:+.10}i\n\nZoom: {:.2e}\nIterations: {}",
                lv.center_re,
                lv.center_im,
                1.0 / lv.scale,
                lv.max_iterations,
            ));
            s
        } else {
            "Start with default\nMandelbrot settings.".to_string()
        }
    }

    pub(crate) fn apply_mandelbrot_defaults(&mut self) {
        let w = self.panel_size[0];
        let h = self.panel_size[1];
        let (mode, julia_c, params, viewport, display_color, aa_level) =
            defaults_for(w, h, &self.preferences);
        self.mode = mode;
        self.julia_c = julia_c;
        self.params = params;
        self.viewport = viewport;
        self.display_color = display_color;
        self.aa_level = aa_level;
        self.current_aa = None;
        self.current_iterations = None;
        self.texture = None;
        self.drag_preview = None;
        self.history.clear();
        self.history.push(viewport);
        self.history_pos = 0;
        self.last_jumped_bookmark_idx = None;
        self.bump_minimap_revision();
    }
}

fn draw_tile(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    title: &str,
    details: &str,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        let bg = if response.hovered() {
            egui::Color32::from_rgb(32, 32, 40)
        } else {
            egui::Color32::from_rgb(20, 20, 26)
        };
        painter.rect_filled(rect, TILE_CORNER_RADIUS, bg);

        let border_color = if response.hovered() {
            egui::Color32::from_gray(70)
        } else {
            egui::Color32::from_gray(42)
        };
        painter.rect_stroke(
            rect,
            TILE_CORNER_RADIUS,
            egui::Stroke::new(0.5, border_color),
            egui::StrokeKind::Inside,
        );

        let padding = 16.0_f32;
        let inner = rect.shrink(padding);

        let preview_h = (inner.height() * 0.38).min(180.0);
        let preview_rect =
            egui::Rect::from_min_size(inner.min, egui::vec2(inner.width(), preview_h));
        painter.rect_filled(preview_rect, 3.0, egui::Color32::from_gray(28));
        painter.text(
            preview_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Preview",
            egui::FontId::proportional(13.0),
            egui::Color32::from_gray(55),
        );

        let title_y = preview_rect.max.y + 14.0;
        painter.text(
            egui::pos2(inner.center().x, title_y),
            egui::Align2::CENTER_TOP,
            title,
            egui::FontId::proportional(17.0),
            CYAN,
        );

        let details_y = title_y + 28.0;
        for (i, line) in details.lines().enumerate() {
            painter.text(
                egui::pos2(inner.center().x, details_y + i as f32 * 16.0),
                egui::Align2::CENTER_TOP,
                line,
                egui::FontId::proportional(12.0),
                egui::Color32::from_gray(150),
            );
        }
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    response
}
