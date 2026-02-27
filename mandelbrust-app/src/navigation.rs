use eframe::egui;

use mandelbrust_core::{ComplexDD, DoubleDouble, Viewport};

use crate::app::{MandelbRustApp, DD_THRESHOLD_SCALE, DD_WARN_SCALE, MAX_HISTORY};

impl MandelbRustApp {
    pub(crate) fn commit_pan_offset(&mut self) {
        if self.pan_offset != egui::Vec2::ZERO {
            self.viewport.offset_center(
                -(self.pan_offset.x as f64) * self.viewport.scale,
                self.pan_offset.y as f64 * self.viewport.scale,
            );
            self.pan_offset = egui::Vec2::ZERO;
        }
    }

    pub(crate) fn push_history(&mut self) {
        self.commit_pan_offset();
        self.history.truncate(self.history_pos + 1);
        self.history.push(self.viewport);
        self.history_pos = self.history.len() - 1;
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
            self.history_pos = self.history.len() - 1;
        }
    }

    pub(crate) fn go_back(&mut self) {
        self.commit_pan_offset();
        if self.history_pos > 0 {
            self.history_pos -= 1;
            self.viewport = self.history[self.history_pos];
            self.needs_render = true;
        }
    }

    pub(crate) fn go_forward(&mut self) {
        self.commit_pan_offset();
        if self.history_pos + 1 < self.history.len() {
            self.history_pos += 1;
            self.viewport = self.history[self.history_pos];
            self.needs_render = true;
        }
    }

    pub(crate) fn zoom_at_cursor(&mut self, cursor_px: u32, cursor_py: u32, factor: f64) {
        let delta = self.viewport.pixel_to_delta(cursor_px, cursor_py);
        let target = self.viewport.center_dd + ComplexDD::from(delta);
        let factor_dd = DoubleDouble::from(factor);
        let diff = ComplexDD::new(
            self.viewport.center_dd.re - target.re,
            self.viewport.center_dd.im - target.im,
        );
        let new_center = ComplexDD::new(
            target.re + diff.re * factor_dd,
            target.im + diff.im * factor_dd,
        );
        self.viewport.set_center_dd(new_center);
        self.viewport.scale *= factor;
        self.needs_render = true;
    }

    pub(crate) fn zoom_center(&mut self, factor: f64) {
        self.push_history();
        self.viewport.scale *= factor;
        self.needs_render = true;
    }

    pub(crate) fn pan_by_fraction(&mut self, fx: f64, fy: f64) {
        self.push_history();
        self.viewport.offset_center(
            fx * self.viewport.complex_width(),
            fy * self.viewport.complex_height(),
        );
        self.needs_render = true;
    }

    pub(crate) fn default_viewport(&self) -> Viewport {
        let (w, h) = (self.viewport.width, self.viewport.height);
        match self.mode {
            crate::app::FractalMode::Mandelbrot => Viewport::default_mandelbrot(w, h),
            crate::app::FractalMode::Julia => Viewport::default_julia(w, h),
        }
    }

    pub(crate) fn reset_view(&mut self) {
        self.push_history();
        self.viewport = self.default_viewport();
        self.needs_render = true;
    }

    pub(crate) fn check_resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 && (width != self.panel_size[0] || height != self.panel_size[1])
        {
            self.panel_size = [width, height];
            self.viewport.width = width;
            self.viewport.height = height;
            self.needs_render = true;
        }
    }

    pub(crate) fn precision_warning(&self) -> Option<&'static str> {
        if self.viewport.scale < DD_WARN_SCALE {
            Some("Approaching double-double precision limits \u{2014} artifacts may appear")
        } else {
            None
        }
    }

    pub(crate) fn precision_mode_label(&self) -> &'static str {
        if self.viewport.scale < DD_THRESHOLD_SCALE {
            "f64\u{00d7}2"
        } else {
            "f64"
        }
    }
}
