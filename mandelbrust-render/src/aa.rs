use std::sync::Arc;

use rayon::prelude::*;
use tracing::debug;

use mandelbrust_core::{Fractal, IterationResult, Viewport};

use crate::iteration_buffer::IterationBuffer;
use crate::renderer::RenderCancel;

// ---------------------------------------------------------------------------
// AaSamples
// ---------------------------------------------------------------------------

/// Sparse storage for adaptive anti-aliasing sub-pixel samples.
///
/// Only boundary pixels (where the iteration class differs between
/// neighbours) receive extra samples.  Non-boundary pixels use the single
/// sample from the `IterationBuffer`.
#[derive(Clone)]
pub struct AaSamples {
    pub width: u32,
    pub height: u32,
    pub aa_level: u32,
    pub boundary_count: usize,
    /// Per-pixel offset into `data`.  `u32::MAX` = not supersampled.
    offsets: Vec<u32>,
    /// Flat array of sub-pixel `IterationResult`s.  Each supersampled pixel
    /// occupies `aa_level²` consecutive entries.
    data: Vec<IterationResult>,
}

impl AaSamples {
    /// Shift the AA data by a pixel offset, keeping samples that fall within
    /// the overlapping region and discarding those that fall outside.
    ///
    /// Uses the same coordinate convention as `IterationBuffer::shift`:
    /// `dx > 0` = content moves right, `dy > 0` = content moves down.
    pub fn shift(&mut self, dx: i32, dy: i32) {
        if dx == 0 && dy == 0 {
            return;
        }
        let w = self.width as i32;
        let h = self.height as i32;
        let n = (self.aa_level * self.aa_level) as usize;
        let pixel_count = (self.width * self.height) as usize;

        let mut new_offsets = vec![u32::MAX; pixel_count];
        let mut new_data: Vec<IterationResult> = Vec::new();
        let mut new_boundary_count = 0usize;

        let x_start = dx.max(0);
        let x_end = (w + dx).min(w).max(0);

        for dst_y in 0..h {
            let src_y = dst_y - dy;
            if src_y < 0 || src_y >= h {
                continue;
            }
            for dst_x in x_start..x_end {
                let src_x = dst_x - dx;
                let src_idx = src_y as usize * self.width as usize + src_x as usize;
                let off = self.offsets[src_idx];
                if off != u32::MAX {
                    let dst_idx = dst_y as usize * self.width as usize + dst_x as usize;
                    new_offsets[dst_idx] = new_data.len() as u32;
                    new_data.extend_from_slice(&self.data[off as usize..off as usize + n]);
                    new_boundary_count += 1;
                }
            }
        }

        self.offsets = new_offsets;
        self.data = new_data;
        self.boundary_count = new_boundary_count;
    }

    /// Get the sub-pixel samples for a pixel, or `None` if it was not
    /// supersampled.
    pub fn samples(&self, x: u32, y: u32) -> Option<&[IterationResult]> {
        let idx = (y * self.width + x) as usize;
        let off = self.offsets[idx];
        if off == u32::MAX {
            return None;
        }
        let n = (self.aa_level * self.aa_level) as usize;
        Some(&self.data[off as usize..off as usize + n])
    }
}

// ---------------------------------------------------------------------------
// Boundary detection
// ---------------------------------------------------------------------------

/// Identify pixels whose iteration class differs from at least one of their
/// 8 neighbours.  These are the pixels that benefit from supersampling.
fn detect_boundaries(iter_buf: &IterationBuffer) -> Vec<bool> {
    let w = iter_buf.width as usize;
    let h = iter_buf.height as usize;
    let mut mask = vec![false; w * h];

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let class = iter_buf.data[idx].class();

            'neighbours: for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && nx < w as i32 && ny >= 0 && ny < h as i32 {
                        let nidx = ny as usize * w + nx as usize;
                        if iter_buf.data[nidx].class() != class {
                            mask[idx] = true;
                            break 'neighbours;
                        }
                    }
                }
            }
        }
    }

    mask
}

// ---------------------------------------------------------------------------
// Adaptive supersampling
// ---------------------------------------------------------------------------

/// Compute adaptive anti-aliasing data for an already-rendered frame.
///
/// 1.  Detect boundary pixels from the `IterationBuffer`.
/// 2.  For each boundary pixel, compute `aa_level²` sub-pixel samples using
///     the fractal on a regular grid inside the pixel.
/// 3.  Return an `AaSamples` structure that the palette can use during
///     colourisation.
///
/// Returns `None` if there are no boundary pixels or the render was
/// cancelled.
pub fn compute_aa<F: Fractal + Sync>(
    fractal: &F,
    viewport: &Viewport,
    iter_buf: &IterationBuffer,
    aa_level: u32,
    cancel: &Arc<RenderCancel>,
) -> Option<AaSamples> {
    let gen = cancel.generation();

    // Step 1: detect boundaries.
    let mask = detect_boundaries(iter_buf);
    let boundary_count = mask.iter().filter(|&&b| b).count();

    if boundary_count == 0 {
        return None;
    }

    debug!(boundary_count, aa_level, "Starting AA pass");

    // Step 2: build offset array.
    let n = aa_level * aa_level;
    let pixel_count = (iter_buf.width * iter_buf.height) as usize;
    let mut offsets = vec![u32::MAX; pixel_count];
    let mut offset = 0u32;
    for (idx, &is_boundary) in mask.iter().enumerate() {
        if is_boundary {
            offsets[idx] = offset;
            offset += n;
        }
    }

    // Step 3: collect boundary pixel coordinates.
    let boundary_pixels: Vec<(u32, u32)> = mask
        .iter()
        .enumerate()
        .filter(|(_, &b)| b)
        .map(|(idx, _)| {
            let x = (idx % iter_buf.width as usize) as u32;
            let y = (idx / iter_buf.width as usize) as u32;
            (x, y)
        })
        .collect();

    // Step 4: compute sub-pixel samples in parallel.
    cancel.reset_progress(boundary_count);
    let inv = 1.0 / aa_level as f64;
    let samples: Vec<Vec<IterationResult>> = boundary_pixels
        .par_iter()
        .map(|&(x, y)| {
            if cancel.generation() != gen {
                return vec![IterationResult::Interior; n as usize];
            }
            let mut sub = Vec::with_capacity(n as usize);
            for sy in 0..aa_level {
                for sx in 0..aa_level {
                    let px = x as f64 + (sx as f64 + 0.5) * inv;
                    let py = y as f64 + (sy as f64 + 0.5) * inv;
                    let c = viewport.subpixel_to_complex(px, py);
                    sub.push(fractal.iterate(c));
                }
            }
            cancel.inc_progress();
            sub
        })
        .collect();

    if cancel.generation() != gen {
        return None;
    }

    let data: Vec<IterationResult> = samples.into_iter().flatten().collect();

    debug!(
        boundary_count,
        total_samples = data.len(),
        "AA pass complete"
    );

    Some(AaSamples {
        width: iter_buf.width,
        height: iter_buf.height,
        aa_level,
        boundary_count,
        offsets,
        data,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use mandelbrust_core::{Mandelbrot, Viewport};

    #[test]
    fn detect_boundaries_finds_edges() {
        let mandelbrot = Mandelbrot::default();
        let viewport = Viewport::default_mandelbrot(64, 64);
        let cancel = Arc::new(RenderCancel::new());

        let result = crate::render(&mandelbrot, &viewport, &cancel, true);
        let mask = detect_boundaries(&result.iterations);

        // There should be some boundary pixels (the set boundary is non-trivial).
        let boundary_count = mask.iter().filter(|&&b| b).count();
        assert!(
            boundary_count > 0,
            "should detect boundary pixels in a Mandelbrot render"
        );
        // Not all pixels should be boundaries.
        assert!(
            boundary_count < 64 * 64,
            "not every pixel should be a boundary"
        );
    }

    #[test]
    fn compute_aa_produces_samples() {
        let mandelbrot = Mandelbrot::default();
        let viewport = Viewport::default_mandelbrot(64, 64);
        let cancel = Arc::new(RenderCancel::new());

        let result = crate::render(&mandelbrot, &viewport, &cancel, true);
        let aa = compute_aa(&mandelbrot, &viewport, &result.iterations, 2, &cancel);

        let aa = aa.expect("should produce AA samples");
        assert_eq!(aa.aa_level, 2);
        assert!(aa.boundary_count > 0);
        assert_eq!(aa.data.len(), aa.boundary_count * 4); // 2×2 = 4 samples each
    }

    #[test]
    fn uniform_image_has_no_boundaries() {
        // Far outside the set — all pixels escape at the same iteration.
        let mandelbrot = Mandelbrot::default();
        let viewport =
            Viewport::new(mandelbrust_core::Complex::new(10.0, 10.0), 0.001, 64, 64).unwrap();
        let cancel = Arc::new(RenderCancel::new());

        let result = crate::render(&mandelbrot, &viewport, &cancel, true);
        let aa = compute_aa(&mandelbrot, &viewport, &result.iterations, 2, &cancel);

        assert!(aa.is_none(), "uniform image should have no boundary pixels");
    }
}
