//! Color picker widget: RGB/hex input fields and a visual HSV picker.

use eframe::egui;

/// HSV color in [0..360, 0..1, 0..1].
#[derive(Debug, Clone, Copy)]
pub(crate) struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

impl Hsv {
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        let rf = r as f32 / 255.0;
        let gf = g as f32 / 255.0;
        let bf = b as f32 / 255.0;
        let max = rf.max(gf).max(bf);
        let min = rf.min(gf).min(bf);
        let delta = max - min;

        let h = if delta < 1e-6 {
            0.0
        } else if (max - rf).abs() < 1e-6 {
            60.0 * (((gf - bf) / delta) % 6.0)
        } else if (max - gf).abs() < 1e-6 {
            60.0 * ((bf - rf) / delta + 2.0)
        } else {
            60.0 * ((rf - gf) / delta + 4.0)
        };
        let h = if h < 0.0 { h + 360.0 } else { h };
        let s = if max < 1e-6 { 0.0 } else { delta / max };
        let v = max;
        Self { h, s, v }
    }

    pub fn to_rgb(self) -> (u8, u8, u8) {
        let c = self.v * self.s;
        let x = c * (1.0 - ((self.h / 60.0) % 2.0 - 1.0).abs());
        let m = self.v - c;
        let (r1, g1, b1) = match self.h as u32 {
            0..=59 => (c, x, 0.0),
            60..=119 => (x, c, 0.0),
            120..=179 => (0.0, c, x),
            180..=239 => (0.0, x, c),
            240..=299 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        (
            ((r1 + m) * 255.0).round() as u8,
            ((g1 + m) * 255.0).round() as u8,
            ((b1 + m) * 255.0).round() as u8,
        )
    }
}

/// Persistent state for the color picker.
#[derive(Debug, Clone)]
pub(crate) struct ColorPickerState {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub hex_text: String,
    hsv: Hsv,
}

impl Default for ColorPickerState {
    fn default() -> Self {
        Self {
            r: 255,
            g: 0,
            b: 0,
            hex_text: "#FF0000".into(),
            hsv: Hsv::from_rgb(255, 0, 0),
        }
    }
}

impl ColorPickerState {
    pub fn set_rgb(&mut self, r: u8, g: u8, b: u8) {
        self.r = r;
        self.g = g;
        self.b = b;
        self.hex_text = format!("#{:02X}{:02X}{:02X}", r, g, b);
        self.hsv = Hsv::from_rgb(r, g, b);
    }

    fn sync_from_hsv(&mut self) {
        let (r, g, b) = self.hsv.to_rgb();
        self.r = r;
        self.g = g;
        self.b = b;
        self.hex_text = format!("#{:02X}{:02X}{:02X}", r, g, b);
    }

    fn sync_from_hex(&mut self) {
        let hex = self.hex_text.trim();
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        if hex.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                self.r = r;
                self.g = g;
                self.b = b;
                self.hsv = Hsv::from_rgb(r, g, b);
            }
        }
    }
}

/// Show the color picker UI. Returns `true` if the color changed.
pub(crate) fn show_color_picker(ui: &mut egui::Ui, state: &mut ColorPickerState) -> bool {
    let mut changed = false;

    // Visual SV box + hue bar
    let sv_size = egui::vec2(160.0, 120.0);
    let hue_bar_width = 20.0;
    let spacing = 6.0;

    ui.horizontal(|ui| {
        // SV square
        let (sv_rect, sv_resp) = ui.allocate_exact_size(sv_size, egui::Sense::click_and_drag());
        paint_sv_box(ui.painter(), sv_rect, state.hsv.h);

        let sv_cursor_x = sv_rect.min.x + state.hsv.s * sv_size.x;
        let sv_cursor_y = sv_rect.min.y + (1.0 - state.hsv.v) * sv_size.y;
        ui.painter().circle_stroke(
            egui::pos2(sv_cursor_x, sv_cursor_y),
            4.0,
            egui::Stroke::new(1.5, egui::Color32::WHITE),
        );

        if sv_resp.dragged() || sv_resp.clicked() {
            if let Some(pos) = sv_resp.interact_pointer_pos() {
                state.hsv.s = ((pos.x - sv_rect.min.x) / sv_size.x).clamp(0.0, 1.0);
                state.hsv.v = (1.0 - (pos.y - sv_rect.min.y) / sv_size.y).clamp(0.0, 1.0);
                state.sync_from_hsv();
                changed = true;
            }
        }

        ui.add_space(spacing);

        // Hue bar
        let hue_size = egui::vec2(hue_bar_width, sv_size.y);
        let (hue_rect, hue_resp) =
            ui.allocate_exact_size(hue_size, egui::Sense::click_and_drag());
        paint_hue_bar(ui.painter(), hue_rect);

        let hue_cursor_y = hue_rect.min.y + (state.hsv.h / 360.0) * hue_size.y;
        ui.painter().hline(
            hue_rect.x_range(),
            hue_cursor_y,
            egui::Stroke::new(2.0, egui::Color32::WHITE),
        );

        if hue_resp.dragged() || hue_resp.clicked() {
            if let Some(pos) = hue_resp.interact_pointer_pos() {
                state.hsv.h =
                    (((pos.y - hue_rect.min.y) / hue_size.y) * 360.0).clamp(0.0, 359.99);
                state.sync_from_hsv();
                changed = true;
            }
        }
    });

    ui.add_space(4.0);

    // Color preview swatch
    let (preview_rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 20.0), egui::Sense::hover());
    ui.painter().rect_filled(
        preview_rect,
        3.0,
        egui::Color32::from_rgb(state.r, state.g, state.b),
    );

    ui.add_space(4.0);

    // RGB sliders
    ui.horizontal(|ui| {
        ui.label("R:");
        let mut r = state.r as u32;
        if ui
            .add(egui::DragValue::new(&mut r).range(0..=255).speed(1))
            .changed()
        {
            state.set_rgb(r as u8, state.g, state.b);
            changed = true;
        }
        ui.label("G:");
        let mut g = state.g as u32;
        if ui
            .add(egui::DragValue::new(&mut g).range(0..=255).speed(1))
            .changed()
        {
            state.set_rgb(state.r, g as u8, state.b);
            changed = true;
        }
        ui.label("B:");
        let mut b = state.b as u32;
        if ui
            .add(egui::DragValue::new(&mut b).range(0..=255).speed(1))
            .changed()
        {
            state.set_rgb(state.r, state.g, b as u8);
            changed = true;
        }
    });

    // Hex field
    ui.horizontal(|ui| {
        ui.label("Hex:");
        let resp = ui.add(
            egui::TextEdit::singleline(&mut state.hex_text)
                .desired_width(80.0)
                .char_limit(7),
        );
        if resp.changed() {
            state.sync_from_hex();
            changed = true;
        }
    });

    changed
}

fn paint_sv_box(painter: &egui::Painter, rect: egui::Rect, hue: f32) {
    let n = 32u32;
    let dx = rect.width() / n as f32;
    let dy = rect.height() / n as f32;
    for xi in 0..n {
        for yi in 0..n {
            let s = (xi as f32 + 0.5) / n as f32;
            let v = 1.0 - (yi as f32 + 0.5) / n as f32;
            let (r, g, b) = (Hsv { h: hue, s, v }).to_rgb();
            let cell = egui::Rect::from_min_size(
                egui::pos2(rect.min.x + xi as f32 * dx, rect.min.y + yi as f32 * dy),
                egui::vec2(dx + 0.5, dy + 0.5),
            );
            painter.rect_filled(cell, 0.0, egui::Color32::from_rgb(r, g, b));
        }
    }
}

fn paint_hue_bar(painter: &egui::Painter, rect: egui::Rect) {
    let n = 36u32;
    let dy = rect.height() / n as f32;
    for i in 0..n {
        let h = (i as f32 + 0.5) / n as f32 * 360.0;
        let (r, g, b) = (Hsv { h, s: 1.0, v: 1.0 }).to_rgb();
        let cell = egui::Rect::from_min_size(
            egui::pos2(rect.min.x, rect.min.y + i as f32 * dy),
            egui::vec2(rect.width(), dy + 0.5),
        );
        painter.rect_filled(cell, 0.0, egui::Color32::from_rgb(r, g, b));
    }
}
