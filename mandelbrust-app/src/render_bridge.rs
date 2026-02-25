use std::sync::mpsc;
use std::sync::Arc;

use eframe::egui;
use tracing::debug;

use mandelbrust_core::{
    Complex, ComplexDD, FractalParams, Julia, JuliaDD, Mandelbrot, MandelbrotDD, Viewport,
};
use mandelbrust_render::{compute_aa, render, RenderCancel, RenderResult};

use crate::app::{FractalMode, MandelbRustApp, DD_THRESHOLD_SCALE, PREVIEW_DOWNSCALE};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenderPhase {
    Idle,
    Rendering,
    Refining,
    Done,
}

impl RenderPhase {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Rendering => "Rendering\u{2026}",
            Self::Refining => "Refining\u{2026}",
            Self::Done => "Done",
        }
    }
}

pub(crate) struct RenderRequest {
    pub(crate) id: u64,
    pub(crate) viewport: Viewport,
    pub(crate) params: FractalParams,
    pub(crate) mode: FractalMode,
    pub(crate) julia_c: Complex,
    pub(crate) aa_level: u32,
}

pub(crate) enum RenderResponse {
    Preview { id: u64, result: RenderResult },
    Final { id: u64, result: RenderResult },
}

pub(crate) struct JuliaGridRequest {
    pub(crate) cols: u32,
    pub(crate) rows: u32,
    pub(crate) center_re: f64,
    pub(crate) center_im: f64,
    pub(crate) extent_half: f64,
    pub(crate) cell_size: u32,
    pub(crate) max_iterations: u32,
    pub(crate) aa_level: u32,
    pub(crate) cancel: Arc<RenderCancel>,
}

// ---------------------------------------------------------------------------
// impl MandelbRustApp — render dispatch & polling
// ---------------------------------------------------------------------------

impl MandelbRustApp {
    pub(crate) fn request_render(&mut self) {
        self.cancel.cancel();
        self.render_id += 1;

        if self.pan_completed {
            self.skip_preview_id = self.render_id;
            self.pan_completed = false;
        }

        let params = self.effective_params();
        debug!(
            id = self.render_id,
            max_iter = params.max_iterations,
            scale = self.viewport.scale,
            "Requesting render"
        );

        let req = RenderRequest {
            id: self.render_id,
            viewport: self.viewport,
            params,
            mode: self.mode,
            julia_c: self.julia_c,
            aa_level: self.aa_level,
        };

        let _ = self.tx_request.send(req);
        self.render_phase = RenderPhase::Rendering;
        self.needs_render = false;
    }

    pub(crate) fn request_drag_preview(&mut self) {
        self.cancel.cancel();
        self.render_id += 1;

        let mut viewport = self.viewport;
        viewport.offset_center(
            -(self.pan_offset.x as f64) * viewport.scale,
            self.pan_offset.y as f64 * viewport.scale,
        );

        let params = self.effective_params();
        let req = RenderRequest {
            id: self.render_id,
            viewport,
            params,
            mode: self.mode,
            julia_c: self.julia_c,
            aa_level: 0,
        };

        let _ = self.tx_request.send(req);
        self.render_phase = RenderPhase::Rendering;
    }

    pub(crate) fn poll_responses(&mut self, ctx: &egui::Context) {
        while let Ok(resp) = self.rx_response.try_recv() {
            match resp {
                RenderResponse::Preview { id, result } => {
                    if id == self.render_id && !result.cancelled {
                        if self.drag_active {
                            self.apply_drag_preview(ctx, result);
                        } else if id == self.skip_preview_id {
                            self.skip_preview_id = 0;
                            self.render_phase = RenderPhase::Refining;
                        } else {
                            self.apply_result(ctx, result);
                            self.render_phase = RenderPhase::Refining;
                        }
                    }
                }
                RenderResponse::Final { id, result } => {
                    if id == self.render_id && !result.cancelled {
                        self.apply_result(ctx, result);
                        self.render_phase = RenderPhase::Done;
                    }
                }
            }
        }
    }

    pub(crate) fn apply_result(&mut self, ctx: &egui::Context, result: RenderResult) {
        self.render_time = result.elapsed;
        self.tiles_rendered = result.tiles_rendered;
        self.tiles_mirrored = result.tiles_mirrored;
        self.tiles_border_traced = result.tiles_border_traced;

        let buffer = self.colorize_current(&result.iterations, result.aa_samples.as_ref());
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [buffer.width as usize, buffer.height as usize],
            &buffer.pixels,
        );
        self.texture = Some(ctx.load_texture("fractal", image, egui::TextureOptions::LINEAR));
        self.current_iterations = Some(result.iterations);
        self.current_aa = result.aa_samples;

        self.drag_preview = None;
        self.draw_offset = egui::Vec2::ZERO;
    }

    pub(crate) fn apply_drag_preview(&mut self, ctx: &egui::Context, result: RenderResult) {
        let params = self.color_params();
        let buffer = self.current_palette().colorize(&result.iterations, &params);
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [buffer.width as usize, buffer.height as usize],
            &buffer.pixels,
        );
        self.drag_preview =
            Some(ctx.load_texture("drag_preview", image, egui::TextureOptions::LINEAR));
    }

    pub(crate) fn cancel_render(&mut self) {
        self.cancel.cancel();
        if self.render_phase == RenderPhase::Rendering
            || self.render_phase == RenderPhase::Refining
        {
            self.render_phase = RenderPhase::Done;
            tracing::info!("Render cancelled by user");
        }
    }

    pub(crate) fn poll_julia_grid_responses(&mut self, ctx: &egui::Context) {
        while let Ok((i, j, result)) = self.rx_julia_grid_resp.try_recv() {
            if result.cancelled {
                continue;
            }
            self.julia_explorer_cells
                .insert((i, j), result.iterations.clone());
            let mut params = self.color_params();
            params.cycle_length = self.display_color.cycle_length(result.iterations.max_iterations);
            let buffer = if let Some(aa) = result.aa_samples.as_ref() {
                self.current_palette()
                    .colorize_aa(&result.iterations, aa, &params)
            } else {
                self.current_palette().colorize(&result.iterations, &params)
            };
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [buffer.width as usize, buffer.height as usize],
                &buffer.pixels,
            );
            let name = format!("julia_cell_{}_{}", i, j);
            let tex = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
            self.julia_explorer_textures.insert((i, j), tex);
        }
    }

    pub(crate) fn start_julia_grid_request(&mut self) {
        const JULIA_EXPLORER_CENTER_RE: f64 = -0.75;
        const JULIA_EXPLORER_CENTER_IM: f64 = 0.0;
        let cols = self.julia_explorer_cols.max(1);
        let rows = self.julia_explorer_rows.max(1);
        let cell_size_px = self.preferences.julia_explorer_cell_size_px.clamp(16, 256);
        self.julia_explorer_cells.clear();
        self.julia_explorer_textures.clear();
        self.grid_cancel.cancel();
        let new_cancel = Arc::new(RenderCancel::new());
        self.grid_cancel = new_cancel.clone();
        let req = JuliaGridRequest {
            cols,
            rows,
            center_re: JULIA_EXPLORER_CENTER_RE,
            center_im: JULIA_EXPLORER_CENTER_IM,
            extent_half: self.julia_explorer_extent_half,
            cell_size: cell_size_px,
            max_iterations: self.preferences.julia_explorer_max_iterations,
            aa_level: 4,
            cancel: new_cancel,
        };
        let _ = self.tx_julia_grid_req.send(req);
    }
}

// ---------------------------------------------------------------------------
// Free functions — render helpers
// ---------------------------------------------------------------------------

fn drain_latest(initial: RenderRequest, rx: &mpsc::Receiver<RenderRequest>) -> RenderRequest {
    let mut req = initial;
    while let Ok(newer) = rx.try_recv() {
        req = newer;
    }
    req
}

fn do_render<F: mandelbrust_core::Fractal + Sync>(
    fractal: &F,
    viewport: &Viewport,
    cancel: &Arc<RenderCancel>,
    aa_level: u32,
    use_real_axis_symmetry: bool,
) -> RenderResult {
    let mut result = render(fractal, viewport, cancel, use_real_axis_symmetry);
    if aa_level > 0 && !result.cancelled {
        let aa_start = std::time::Instant::now();
        result.aa_samples = compute_aa(fractal, viewport, &result.iterations, aa_level, cancel);
        result.elapsed += aa_start.elapsed();
    }
    result
}

pub(crate) fn render_for_mode(
    mode: FractalMode,
    params: FractalParams,
    julia_c: Complex,
    viewport: &Viewport,
    cancel: &Arc<RenderCancel>,
    aa_level: u32,
) -> RenderResult {
    let use_symmetry = mode == FractalMode::Mandelbrot;
    let use_dd = viewport.scale < DD_THRESHOLD_SCALE;
    match (mode, use_dd) {
        (FractalMode::Mandelbrot, false) => {
            do_render(&Mandelbrot::new(params), viewport, cancel, aa_level, use_symmetry)
        }
        (FractalMode::Mandelbrot, true) => do_render(
            &MandelbrotDD::new(params, viewport.center_dd),
            viewport,
            cancel,
            aa_level,
            use_symmetry,
        ),
        (FractalMode::Julia, false) => {
            do_render(&Julia::new(julia_c, params), viewport, cancel, aa_level, false)
        }
        (FractalMode::Julia, true) => do_render(
            &JuliaDD::new(ComplexDD::from(julia_c), params, viewport.center_dd),
            viewport,
            cancel,
            aa_level,
            false,
        ),
    }
}

pub(crate) fn render_worker(
    ctx: egui::Context,
    rx: mpsc::Receiver<RenderRequest>,
    tx: mpsc::Sender<RenderResponse>,
    cancel: Arc<RenderCancel>,
) {
    while let Ok(initial) = rx.recv() {
        let mut req = drain_latest(initial, &rx);

        loop {
            let preview_vp = req.viewport.downscaled(PREVIEW_DOWNSCALE);
            let preview =
                render_for_mode(req.mode, req.params, req.julia_c, &preview_vp, &cancel, 0);

            if preview.cancelled {
                break;
            }

            if tx
                .send(RenderResponse::Preview {
                    id: req.id,
                    result: preview,
                })
                .is_err()
            {
                return;
            }
            ctx.request_repaint();

            if let Ok(newer) = rx.try_recv() {
                req = drain_latest(newer, &rx);
                continue;
            }

            let full = render_for_mode(
                req.mode,
                req.params,
                req.julia_c,
                &req.viewport,
                &cancel,
                req.aa_level,
            );

            if full.cancelled {
                break;
            }

            if tx
                .send(RenderResponse::Final {
                    id: req.id,
                    result: full,
                })
                .is_err()
            {
                return;
            }
            ctx.request_repaint();

            break;
        }
    }
}

pub(crate) fn julia_grid_worker(
    rx: mpsc::Receiver<JuliaGridRequest>,
    tx: mpsc::Sender<(u32, u32, RenderResult)>,
) {
    while let Ok(req) = rx.recv() {
        let gen = req.cancel.generation();
        let params = FractalParams::new(req.max_iterations, 2.0).unwrap_or_default();
        let scale = 3.0 / req.cell_size as f64;
        let viewport = match Viewport::new(Complex::new(0.0, 0.0), scale, req.cell_size, req.cell_size) {
            Ok(vp) => vp,
            Err(_) => continue,
        };
        let center_j = (req.cols - 1) / 2;
        let center_i = (req.rows - 1) / 2;
        let cell_width_re = 2.0 * req.extent_half / req.cols as f64;
        let cell_width_im = 2.0 * req.extent_half / req.rows as f64;

        for i in 0..req.rows {
            if req.cancel.generation() != gen {
                break;
            }
            for j in 0..req.cols {
                if req.cancel.generation() != gen {
                    break;
                }
                let j_off = j as i64 - center_j as i64;
                let i_off = i as i64 - center_i as i64;
                let c_re = req.center_re + j_off as f64 * cell_width_re;
                let c_im = req.center_im - i_off as f64 * cell_width_im;
                let c = Complex::new(c_re, c_im);
                let julia = Julia::new(c, params);
                let result = do_render(&julia, &viewport, &req.cancel, req.aa_level, false);
                if tx.send((i, j, result)).is_err() {
                    return;
                }
            }
        }
    }
}
