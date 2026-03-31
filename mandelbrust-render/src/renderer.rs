use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rayon::prelude::*;
use tracing::{debug, info};

use mandelbrust_core::{Complex, Fractal, IterationExtras, IterationResult, Viewport};

use crate::aa::AaSamples;
use crate::extras_buffer::ExtrasBuffer;
use crate::iteration_buffer::IterationBuffer;
use crate::tile::{build_tile_grid, classify_tiles_for_symmetry, ClassifiedTile, Tile, TileKind};

// ---------------------------------------------------------------------------
// Cancellation
// ---------------------------------------------------------------------------

/// Tracks the current render generation for cancellation and progress.
///
/// Incrementing the generation signals all in-flight tiles to stop early.
/// The progress counters let the UI display a progress bar.
#[derive(Debug)]
pub struct RenderCancel {
    generation: AtomicU64,
    progress_done: AtomicUsize,
    progress_total: AtomicUsize,
}

impl RenderCancel {
    pub fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            progress_done: AtomicUsize::new(0),
            progress_total: AtomicUsize::new(0),
        }
    }

    /// Cancel the current render by advancing the generation.
    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Read the current generation.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Reset progress for a new phase with `total` work units.
    pub fn reset_progress(&self, total: usize) {
        self.progress_total.store(total, Ordering::Relaxed);
        self.progress_done.store(0, Ordering::Relaxed);
    }

    /// Increment completed work units by one.
    pub fn inc_progress(&self) {
        self.progress_done.fetch_add(1, Ordering::Relaxed);
    }

    /// Read the current progress as `(done, total)`.
    pub fn progress(&self) -> (usize, usize) {
        (
            self.progress_done.load(Ordering::Relaxed),
            self.progress_total.load(Ordering::Relaxed),
        )
    }
}

impl Default for RenderCancel {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// The result of a full-frame render.
///
/// Contains raw iteration data (no coloring) — the caller applies a palette
/// to produce displayable pixels.
pub struct RenderResult {
    pub iterations: IterationBuffer,
    pub extras: Option<ExtrasBuffer>,
    pub aa_samples: Option<AaSamples>,
    pub elapsed: Duration,
    pub cancelled: bool,
    pub tiles_rendered: usize,
    pub tiles_mirrored: usize,
    pub tiles_border_traced: usize,
}

/// Render-time options controlling optional features.
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Enable real-axis symmetry optimisation (Mandelbrot only).
    pub use_real_axis_symmetry: bool,
    /// Compute per-pixel extras (distance estimate, stripe average).
    /// Disables border tracing and symmetry when true.
    pub compute_extras: bool,
    /// Allow border-trace flood fill optimization.
    ///
    /// This should be disabled when smooth coloring is active because smooth
    /// coloring relies on per-pixel continuous values (`norm_sq`), which are
    /// lost when a full tile is filled from a single representative sample.
    pub allow_border_tracing: bool,
    /// Stripe density for interior stripe average coloring.
    pub stripe_density: f64,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            use_real_axis_symmetry: false,
            compute_extras: false,
            allow_border_tracing: true,
            stripe_density: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Border tracing
// ---------------------------------------------------------------------------

/// Map a pixel to the coordinate expected by the fractal: either an absolute
/// complex-plane point or a delta from the fractal's internal center.
#[inline]
fn map_pixel<F: Fractal>(fractal: &F, viewport: &Viewport, px: u32, py: u32) -> Complex {
    if fractal.uses_delta_coordinates() {
        viewport.pixel_to_delta(px, py)
    } else {
        viewport.pixel_to_complex(px, py)
    }
}

/// If every border pixel of the tile shares the same iteration class,
/// return the representative `IterationResult` so we can flood-fill.
fn check_border_uniform<F: Fractal>(
    fractal: &F,
    viewport: &Viewport,
    tile: &Tile,
) -> Option<IterationResult> {
    if tile.width < 3 || tile.height < 3 {
        return None;
    }

    let first = fractal.iterate(map_pixel(fractal, viewport, tile.x, tile.y));
    let class = first.class();

    // Top and bottom rows.
    for px in 0..tile.width {
        let top = fractal.iterate(map_pixel(fractal, viewport, tile.x + px, tile.y));
        if top.class() != class {
            return None;
        }
        let bot = fractal.iterate(map_pixel(
            fractal,
            viewport,
            tile.x + px,
            tile.y + tile.height - 1,
        ));
        if bot.class() != class {
            return None;
        }
    }

    // Left and right columns (corners already checked above).
    for py in 1..tile.height - 1 {
        let left = fractal.iterate(map_pixel(fractal, viewport, tile.x, tile.y + py));
        if left.class() != class {
            return None;
        }
        let right = fractal.iterate(map_pixel(
            fractal,
            viewport,
            tile.x + tile.width - 1,
            tile.y + py,
        ));
        if right.class() != class {
            return None;
        }
    }

    Some(first)
}

// ---------------------------------------------------------------------------
// Per-tile rendering
// ---------------------------------------------------------------------------

struct TileData {
    iterations: Vec<IterationResult>,
    extras: Option<Vec<IterationExtras>>,
}

/// Render a single tile, trying border-trace optimisation first.
///
/// When `compute_extras` is true, border tracing is skipped and per-pixel
/// extras (distance, stripe average) are computed alongside iteration data.
fn render_tile<F: Fractal>(
    fractal: &F,
    viewport: &Viewport,
    tile: &Tile,
    bt_count: &AtomicUsize,
    opts: &RenderOptions,
) -> TileData {
    if !opts.compute_extras && opts.allow_border_tracing {
        if let Some(fill) = check_border_uniform(fractal, viewport, tile) {
            bt_count.fetch_add(1, Ordering::Relaxed);
            return TileData {
                iterations: vec![fill; tile.pixel_count()],
                extras: None,
            };
        }
    }

    let count = tile.pixel_count();
    let mut iter_data = Vec::with_capacity(count);
    let mut extras_data = if opts.compute_extras {
        Some(Vec::with_capacity(count))
    } else {
        None
    };

    for py in 0..tile.height {
        for px in 0..tile.width {
            let c = map_pixel(fractal, viewport, tile.x + px, tile.y + py);
            if opts.compute_extras {
                let (result, ext) = fractal.iterate_with_extras(c, opts.stripe_density);
                iter_data.push(result);
                extras_data.as_mut().unwrap().push(ext);
            } else {
                iter_data.push(fractal.iterate(c));
            }
        }
    }

    TileData {
        iterations: iter_data,
        extras: extras_data,
    }
}

// ---------------------------------------------------------------------------
// Full-frame render
// ---------------------------------------------------------------------------

/// Render a full frame using the tiled, multithreaded pipeline.
///
/// The renderer is generic over the fractal type for static dispatch.
/// Tiles are processed in parallel via Rayon.  The `cancel` handle
/// can be used from another thread to abort the render.
///
/// Returns raw iteration data — apply a `Palette` to get displayable pixels.
pub fn render<F: Fractal + Sync>(
    fractal: &F,
    viewport: &Viewport,
    cancel: &Arc<RenderCancel>,
    opts: &RenderOptions,
) -> RenderResult {
    let start = Instant::now();
    let gen = cancel.generation();
    let bt_count = AtomicUsize::new(0);
    let max_iter = fractal.params().max_iterations;

    let tiles = build_tile_grid(viewport.width, viewport.height);
    let tile_count = tiles.len();
    debug!(
        tile_count,
        width = viewport.width,
        height = viewport.height,
        compute_extras = opts.compute_extras,
        "Starting tiled render"
    );

    // Symmetry disabled when extras are on (stripe avg is not symmetric).
    let use_symmetry = opts.use_real_axis_symmetry && !opts.compute_extras;
    let classified = if use_symmetry {
        classify_tiles_for_symmetry(&tiles, viewport.height, viewport.center.im)
    } else {
        None
    };

    let renderable_count = if let Some(ref ct) = classified {
        ct.iter()
            .filter(|c| !matches!(c.kind, TileKind::Mirror { .. }))
            .count()
    } else {
        tiles.len()
    };
    cancel.reset_progress(renderable_count);

    let (tile_data, cancelled, tiles_rendered, tiles_mirrored) = if let Some(ref ct) = classified {
        render_with_symmetry(fractal, viewport, ct, cancel, gen, &bt_count, opts)
    } else {
        render_all_tiles(fractal, viewport, &tiles, cancel, gen, &bt_count, opts)
    };

    let mut iterations = IterationBuffer::new(viewport.width, viewport.height, max_iter);
    let mut extras = if opts.compute_extras {
        Some(ExtrasBuffer::new(viewport.width, viewport.height))
    } else {
        None
    };

    if let Some(ref ct) = classified {
        assemble_symmetric(&mut iterations, extras.as_mut(), ct, &tile_data);
    } else {
        assemble_normal(&mut iterations, extras.as_mut(), &tiles, &tile_data);
    }

    let tiles_border_traced = bt_count.load(Ordering::Relaxed);
    let elapsed = start.elapsed();
    info!(
        elapsed_ms = elapsed.as_millis(),
        tiles_rendered, tiles_mirrored, tiles_border_traced, cancelled, "Render complete"
    );

    RenderResult {
        iterations,
        extras,
        aa_samples: None,
        elapsed,
        cancelled,
        tiles_rendered,
        tiles_mirrored,
        tiles_border_traced,
    }
}

fn render_all_tiles<F: Fractal + Sync>(
    fractal: &F,
    viewport: &Viewport,
    tiles: &[Tile],
    cancel: &Arc<RenderCancel>,
    gen: u64,
    bt_count: &AtomicUsize,
    opts: &RenderOptions,
) -> (Vec<Option<TileData>>, bool, usize, usize) {
    let results: Vec<Option<TileData>> = tiles
        .par_iter()
        .map(|tile| {
            if cancel.generation() != gen {
                return None;
            }
            let data = render_tile(fractal, viewport, tile, bt_count, opts);
            cancel.inc_progress();
            Some(data)
        })
        .collect();

    let cancelled = cancel.generation() != gen;
    let rendered = results.iter().filter(|r| r.is_some()).count();
    (results, cancelled, rendered, 0)
}

fn render_with_symmetry<F: Fractal + Sync>(
    fractal: &F,
    viewport: &Viewport,
    classified: &[ClassifiedTile],
    cancel: &Arc<RenderCancel>,
    gen: u64,
    bt_count: &AtomicUsize,
    opts: &RenderOptions,
) -> (Vec<Option<TileData>>, bool, usize, usize) {
    let results: Vec<Option<TileData>> = classified
        .par_iter()
        .map(|ct| {
            if cancel.generation() != gen {
                return None;
            }
            match ct.kind {
                TileKind::Mirror { .. } => None,
                _ => {
                    let data = render_tile(fractal, viewport, &ct.tile, bt_count, opts);
                    cancel.inc_progress();
                    Some(data)
                }
            }
        })
        .collect();

    let cancelled = cancel.generation() != gen;
    let rendered = results.iter().filter(|r| r.is_some()).count();
    let mirrored = classified
        .iter()
        .filter(|ct| matches!(ct.kind, TileKind::Mirror { .. }))
        .count();
    (results, cancelled, rendered, mirrored)
}

fn assemble_normal(
    buffer: &mut IterationBuffer,
    mut extras: Option<&mut ExtrasBuffer>,
    tiles: &[Tile],
    tile_data: &[Option<TileData>],
) {
    for (tile, data) in tiles.iter().zip(tile_data.iter()) {
        if let Some(d) = data {
            buffer.blit_tile(tile, &d.iterations);
            if let (Some(ext_buf), Some(ext_data)) = (extras.as_deref_mut(), d.extras.as_ref()) {
                ext_buf.blit_tile(tile, ext_data);
            }
        }
    }
}

fn assemble_symmetric(
    buffer: &mut IterationBuffer,
    mut extras: Option<&mut ExtrasBuffer>,
    classified: &[ClassifiedTile],
    tile_data: &[Option<TileData>],
) {
    for (ct, data) in classified.iter().zip(tile_data.iter()) {
        if let Some(d) = data {
            buffer.blit_tile(&ct.tile, &d.iterations);
            if let (Some(ext_buf), Some(ext_data)) = (extras.as_deref_mut(), d.extras.as_ref()) {
                ext_buf.blit_tile(&ct.tile, ext_data);
            }
        }
    }

    for ct in classified.iter() {
        if let TileKind::Mirror { primary_index } = ct.kind {
            if let Some(ref primary_data) = tile_data[primary_index] {
                buffer.blit_tile_mirrored(&ct.tile, &primary_data.iterations);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mandelbrust_core::{FractalParams, Mandelbrot};

    fn opts_standard() -> RenderOptions {
        RenderOptions {
            use_real_axis_symmetry: true,
            ..Default::default()
        }
    }

    #[test]
    fn basic_render_produces_iteration_data() {
        let mandelbrot = Mandelbrot::default();
        let viewport = Viewport::default_mandelbrot(128, 128);
        let cancel = Arc::new(RenderCancel::new());

        let result = render(&mandelbrot, &viewport, &cancel, &opts_standard());

        assert!(!result.cancelled);
        assert_eq!(result.iterations.data.len(), 128 * 128);
        assert!(result.tiles_rendered > 0);
        assert!(result.extras.is_none());
    }

    #[test]
    fn symmetry_render_mirrors_tiles() {
        let params = FractalParams::new(64, 2.0).unwrap();
        let mandelbrot = Mandelbrot::new(params);
        let viewport =
            Viewport::new(mandelbrust_core::Complex::new(-0.5, 0.0), 0.01, 128, 128).unwrap();
        let cancel = Arc::new(RenderCancel::new());

        let result = render(&mandelbrot, &viewport, &cancel, &opts_standard());

        assert!(!result.cancelled);
        assert!(
            result.tiles_mirrored > 0,
            "symmetry should mirror some tiles"
        );
    }

    #[test]
    fn border_tracing_fills_uniform_tiles() {
        let params = FractalParams::new(256, 2.0).unwrap();
        let mandelbrot = Mandelbrot::new(params);
        let viewport =
            Viewport::new(mandelbrust_core::Complex::new(5.0, 5.0), 0.001, 128, 128).unwrap();
        let cancel = Arc::new(RenderCancel::new());

        let result = render(&mandelbrot, &viewport, &cancel, &opts_standard());

        assert!(!result.cancelled);
        assert!(
            result.tiles_border_traced > 0,
            "tiles far outside the set should be border-traced"
        );
    }

    #[test]
    fn render_with_extras_produces_buffers() {
        let mandelbrot = Mandelbrot::default();
        let viewport = Viewport::default_mandelbrot(128, 128);
        let cancel = Arc::new(RenderCancel::new());
        let opts = RenderOptions {
            use_real_axis_symmetry: false,
            compute_extras: true,
            allow_border_tracing: false,
            stripe_density: 1.0,
        };

        let result = render(&mandelbrot, &viewport, &cancel, &opts);

        assert!(!result.cancelled);
        let ext = result.extras.as_ref().expect("extras should be present");
        assert_eq!(ext.distance.len(), 128 * 128);
        assert_eq!(ext.stripe_avg.len(), 128 * 128);
    }

    #[test]
    fn cancellation_stops_render() {
        let mandelbrot = Mandelbrot::new(FractalParams::new(50000, 2.0).unwrap());
        let viewport = Viewport::default_mandelbrot(1024, 1024);
        let cancel = Arc::new(RenderCancel::new());

        let cancel_clone = Arc::clone(&cancel);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(5));
            cancel_clone.cancel();
        });

        let result = render(&mandelbrot, &viewport, &cancel, &opts_standard());
        if result.cancelled {
            let total_tiles = ((1024 + 63) / 64) * ((1024 + 63) / 64);
            assert!(
                result.tiles_rendered < total_tiles,
                "not all tiles should have been rendered"
            );
        }
    }
}
