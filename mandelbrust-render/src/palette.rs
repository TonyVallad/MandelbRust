use mandelbrust_core::IterationResult;
use rayon::prelude::*;

use crate::aa::AaSamples;
use crate::buffer::RenderBuffer;
use crate::iteration_buffer::IterationBuffer;

const LUT_SIZE: usize = 256;

// ---------------------------------------------------------------------------
// Color params (cycle mode, start-from black/white)
// ---------------------------------------------------------------------------

/// Fade the first few iterations from solid black or white into the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartFrom {
    None,
    Black,
    White,
}

/// Parameters for mapping iterations to palette color (cycle length, smooth, start-from).
#[derive(Debug, Clone)]
pub struct ColorParams {
    pub smooth: bool,
    /// Effective cycle length in iterations; palette repeats every this many.
    pub cycle_length: u32,
    pub start_from: StartFrom,
    pub low_threshold_start: u32,
    pub low_threshold_end: u32,
}

impl ColorParams {
    /// Default: smooth, one cycle over full range, no start-from fade.
    pub fn from_smooth(smooth: bool) -> Self {
        Self {
            smooth,
            cycle_length: u32::MAX,
            start_from: StartFrom::None,
            low_threshold_start: 10,
            low_threshold_end: 30,
        }
    }
}

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

/// A color palette backed by a gradient lookup table.
///
/// Each palette is a ring of `LUT_SIZE` RGBA colors.  Iteration results are
/// mapped to a fractional index and the final color is linearly interpolated
/// between adjacent entries.
#[derive(Clone)]
pub struct Palette {
    pub name: &'static str,
    colors: Vec<[u8; 4]>,
}

impl Palette {
    pub fn new(name: &'static str, colors: Vec<[u8; 4]>) -> Self {
        assert!(!colors.is_empty());
        Self { name, colors }
    }

    /// Map a single iteration result to an RGBA color using cycle mode and optional start-from fade.
    pub fn color(&self, result: IterationResult, params: &ColorParams) -> [u8; 4] {
        match result {
            IterationResult::Interior => [0, 0, 0, 255],
            IterationResult::Escaped {
                iterations,
                norm_sq,
            } => {
                let t = if params.smooth {
                    smooth_iteration(iterations, norm_sq)
                } else {
                    iterations as f64
                };
                let cycle_len = params.cycle_length as f64;
                let cycle_pos = if cycle_len <= 0.0 || !cycle_len.is_finite() {
                    0.0
                } else {
                    (t % cycle_len) / cycle_len
                };
                let lut_t = cycle_pos * self.colors.len() as f64;
                let palette_color = self.sample(lut_t);

                if params.start_from == StartFrom::None {
                    return palette_color;
                }
                let (low_start, low_end) = (params.low_threshold_start, params.low_threshold_end);
                if low_end <= low_start || iterations >= low_end {
                    return palette_color;
                }
                if iterations <= low_start {
                    return match params.start_from {
                        StartFrom::Black => [0, 0, 0, 255],
                        StartFrom::White => [255, 255, 255, 255],
                        StartFrom::None => palette_color,
                    };
                }
                let blend =
                    (iterations - low_start) as f64 / (low_end - low_start) as f64;
                let (r, g, b) = match params.start_from {
                    StartFrom::Black => (0u8, 0u8, 0u8),
                    StartFrom::White => (255u8, 255u8, 255u8),
                    StartFrom::None => return palette_color,
                };
                let inv = 1.0 - blend;
                [
                    (r as f64 * inv + palette_color[0] as f64 * blend) as u8,
                    (g as f64 * inv + palette_color[1] as f64 * blend) as u8,
                    (b as f64 * inv + palette_color[2] as f64 * blend) as u8,
                    255,
                ]
            }
        }
    }

    /// Colorize an entire iteration buffer into an RGBA pixel buffer.
    pub fn colorize(&self, iter_buf: &IterationBuffer, params: &ColorParams) -> RenderBuffer {
        let len = iter_buf.data.len();
        let mut pixels = vec![0u8; len * 4];
        pixels
            .par_chunks_mut(4)
            .zip(iter_buf.data.par_iter())
            .for_each(|(pixel, &result)| {
                let c = self.color(result, params);
                pixel[0] = c[0];
                pixel[1] = c[1];
                pixel[2] = c[2];
                pixel[3] = c[3];
            });
        RenderBuffer {
            width: iter_buf.width,
            height: iter_buf.height,
            pixels,
        }
    }

    /// Colorize with adaptive anti-aliasing.
    ///
    /// Non-boundary pixels are coloured from `iter_buf` (single sample).
    /// Boundary pixels colour each sub-pixel sample individually and average
    /// the resulting RGBA values, producing smooth edges.
    pub fn colorize_aa(
        &self,
        iter_buf: &IterationBuffer,
        aa: &AaSamples,
        params: &ColorParams,
    ) -> RenderBuffer {
        let w = iter_buf.width;
        let h = iter_buf.height;
        let len = (w * h) as usize;
        let mut pixels = vec![0u8; len * 4];
        let n = aa.aa_level * aa.aa_level;

        pixels
            .par_chunks_mut(4)
            .enumerate()
            .for_each(|(idx, pixel)| {
                let x = (idx as u32) % w;
                let y = (idx as u32) / w;
                let color = if let Some(samples) = aa.samples(x, y) {
                    let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
                    for &s in samples {
                        let c = self.color(s, params);
                        r += c[0] as u32;
                        g += c[1] as u32;
                        b += c[2] as u32;
                    }
                    [(r / n) as u8, (g / n) as u8, (b / n) as u8, 255]
                } else {
                    self.color(iter_buf.data[idx], params)
                };
                pixel[0] = color[0];
                pixel[1] = color[1];
                pixel[2] = color[2];
                pixel[3] = color[3];
            });

        RenderBuffer {
            width: w,
            height: h,
            pixels,
        }
    }

    /// Generate a preview strip (for UI palette bar).
    pub fn preview_colors(&self, count: usize) -> Vec<[u8; 4]> {
        (0..count)
            .map(|i| {
                let t = i as f64 * self.colors.len() as f64 / count as f64;
                self.sample(t)
            })
            .collect()
    }

    fn sample(&self, t: f64) -> [u8; 4] {
        let len = self.colors.len() as f64;
        let idx = t.rem_euclid(len);
        let lo = idx.floor() as usize % self.colors.len();
        let hi = (lo + 1) % self.colors.len();
        let frac = idx - idx.floor();
        lerp_color(self.colors[lo], self.colors[hi], frac)
    }
}

impl Default for Palette {
    fn default() -> Self {
        classic()
    }
}

// ---------------------------------------------------------------------------
// Smooth coloring
// ---------------------------------------------------------------------------

/// Compute the smooth (continuous) iteration count.
///
/// Uses the standard renormalization formula:
///   ν = n + 1 − log₂(ln(|zₙ|))
fn smooth_iteration(iterations: u32, norm_sq: f64) -> f64 {
    let log_zn = norm_sq.ln() * 0.5; // ln(|z_n|)
    if log_zn <= 0.0 {
        return iterations as f64;
    }
    iterations as f64 + 1.0 - log_zn.ln() / std::f64::consts::LN_2
}

fn lerp_color(a: [u8; 4], b: [u8; 4], t: f64) -> [u8; 4] {
    let inv = 1.0 - t;
    [
        (a[0] as f64 * inv + b[0] as f64 * t) as u8,
        (a[1] as f64 * inv + b[1] as f64 * t) as u8,
        (a[2] as f64 * inv + b[2] as f64 * t) as u8,
        255,
    ]
}

// ---------------------------------------------------------------------------
// Builtin palettes
// ---------------------------------------------------------------------------

pub fn builtin_palettes() -> Vec<Palette> {
    vec![classic(), fire(), ocean(), neon(), grayscale()]
}

/// Build a gradient LUT by interpolating between color stops.
fn gradient_lut(stops: &[(f64, [u8; 3])]) -> Vec<[u8; 4]> {
    (0..LUT_SIZE)
        .map(|i| {
            let t = i as f64 / LUT_SIZE as f64;
            let mut lo = 0;
            for (j, &(pos, _)) in stops.iter().enumerate() {
                if pos <= t {
                    lo = j;
                }
            }
            let hi = (lo + 1).min(stops.len() - 1);
            let (lo_t, lo_c) = stops[lo];
            let (hi_t, hi_c) = stops[hi];
            let frac = if (hi_t - lo_t).abs() < 1e-10 {
                0.0
            } else {
                ((t - lo_t) / (hi_t - lo_t)).clamp(0.0, 1.0)
            };
            let inv = 1.0 - frac;
            [
                (lo_c[0] as f64 * inv + hi_c[0] as f64 * frac) as u8,
                (lo_c[1] as f64 * inv + hi_c[1] as f64 * frac) as u8,
                (lo_c[2] as f64 * inv + hi_c[2] as f64 * frac) as u8,
                255,
            ]
        })
        .collect()
}

fn classic() -> Palette {
    let stops = &[
        (0.0, [0, 7, 100]),
        (0.16, [32, 107, 203]),
        (0.42, [237, 255, 255]),
        (0.6425, [255, 170, 0]),
        (0.8575, [0, 2, 0]),
        (1.0, [0, 7, 100]),
    ];
    Palette::new("Classic", gradient_lut(stops))
}

fn fire() -> Palette {
    let stops = &[
        (0.0, [0, 0, 0]),
        (0.25, [128, 0, 0]),
        (0.5, [255, 128, 0]),
        (0.75, [255, 255, 0]),
        (1.0, [255, 255, 255]),
    ];
    Palette::new("Fire", gradient_lut(stops))
}

fn ocean() -> Palette {
    let stops = &[
        (0.0, [0, 0, 30]),
        (0.3, [0, 50, 120]),
        (0.6, [0, 150, 200]),
        (0.8, [100, 220, 255]),
        (1.0, [240, 255, 255]),
    ];
    Palette::new("Ocean", gradient_lut(stops))
}

fn neon() -> Palette {
    let stops = &[
        (0.0, [10, 0, 20]),
        (0.2, [80, 0, 150]),
        (0.4, [200, 0, 200]),
        (0.6, [0, 200, 255]),
        (0.8, [0, 255, 100]),
        (1.0, [10, 0, 20]),
    ];
    Palette::new("Neon", gradient_lut(stops))
}

fn grayscale() -> Palette {
    let stops = &[(0.0, [0, 0, 0]), (1.0, [255, 255, 255])];
    Palette::new("Grayscale", gradient_lut(stops))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interior_is_black() {
        let p = Palette::default();
        let params = ColorParams::from_smooth(true);
        assert_eq!(p.color(IterationResult::Interior, &params), [0, 0, 0, 255]);
    }

    #[test]
    fn escaped_is_not_black() {
        let p = Palette::default();
        let params = ColorParams::from_smooth(true);
        let c = p.color(
            IterationResult::Escaped {
                iterations: 10,
                norm_sq: 5.0,
            },
            &params,
        );
        assert!(c[0] > 0 || c[1] > 0 || c[2] > 0);
        assert_eq!(c[3], 255);
    }

    #[test]
    fn smooth_and_raw_differ() {
        let p = Palette::default();
        let result = IterationResult::Escaped {
            iterations: 20,
            norm_sq: 10.0,
        };
        // Use a small cycle length so smooth (≈20.8) and raw (20) map to different LUT positions.
        let params_smooth = ColorParams {
            smooth: true,
            cycle_length: 50,
            start_from: StartFrom::None,
            low_threshold_start: 10,
            low_threshold_end: 30,
        };
        let params_raw = ColorParams {
            smooth: false,
            cycle_length: 50,
            start_from: StartFrom::None,
            low_threshold_start: 10,
            low_threshold_end: 30,
        };
        let smooth = p.color(result, &params_smooth);
        let raw = p.color(result, &params_raw);
        assert_ne!(smooth, raw, "smooth and raw iteration count should map to different colors");
    }

    #[test]
    fn builtin_palettes_have_correct_size() {
        for pal in builtin_palettes() {
            assert_eq!(pal.colors.len(), LUT_SIZE);
        }
    }

    #[test]
    fn colorize_produces_correct_size() {
        let p = Palette::default();
        let buf = IterationBuffer::new(64, 48, 256);
        let rb = p.colorize(&buf, &ColorParams::from_smooth(true));
        assert_eq!(rb.width, 64);
        assert_eq!(rb.height, 48);
        assert_eq!(rb.pixels.len(), 64 * 48 * 4);
    }

    #[test]
    fn preview_colors_length() {
        let p = Palette::default();
        assert_eq!(p.preview_colors(100).len(), 100);
    }

    #[test]
    fn cycle_length_wraps_position() {
        let p = Palette::default();
        let cycle_len = 100u32;
        let params = ColorParams {
            smooth: false,
            cycle_length: cycle_len,
            start_from: StartFrom::None,
            low_threshold_start: 10,
            low_threshold_end: 30,
        };
        let c0 = p.color(
            IterationResult::Escaped {
                iterations: 0,
                norm_sq: 1.0,
            },
            &params,
        );
        let c100 = p.color(
            IterationResult::Escaped {
                iterations: cycle_len,
                norm_sq: 1.0,
            },
            &params,
        );
        assert_eq!(c0, c100, "cycle position should wrap at cycle_length");
    }

    #[test]
    fn start_from_black_below_threshold() {
        let p = Palette::default();
        let params = ColorParams {
            smooth: false,
            cycle_length: u32::MAX,
            start_from: StartFrom::Black,
            low_threshold_start: 10,
            low_threshold_end: 30,
        };
        let c = p.color(
            IterationResult::Escaped {
                iterations: 5,
                norm_sq: 1.0,
            },
            &params,
        );
        assert_eq!(c, [0, 0, 0, 255]);
    }

    #[test]
    fn start_from_white_below_threshold() {
        let p = Palette::default();
        let params = ColorParams {
            smooth: false,
            cycle_length: u32::MAX,
            start_from: StartFrom::White,
            low_threshold_start: 10,
            low_threshold_end: 30,
        };
        let c = p.color(
            IterationResult::Escaped {
                iterations: 5,
                norm_sq: 1.0,
            },
            &params,
        );
        assert_eq!(c, [255, 255, 255, 255]);
    }

    #[test]
    fn start_from_blend_between_thresholds() {
        let p = Palette::default();
        let params = ColorParams {
            smooth: false,
            cycle_length: u32::MAX,
            start_from: StartFrom::Black,
            low_threshold_start: 10,
            low_threshold_end: 30,
        };
        let c_low = p.color(
            IterationResult::Escaped {
                iterations: 10,
                norm_sq: 1.0,
            },
            &params,
        );
        let c_high = p.color(
            IterationResult::Escaped {
                iterations: 30,
                norm_sq: 1.0,
            },
            &params,
        );
        assert_eq!(c_low, [0, 0, 0, 255]);
        assert!(c_high[0] > 0 || c_high[1] > 0 || c_high[2] > 0);
    }
}
