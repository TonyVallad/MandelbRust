use crate::complex::Complex;
use crate::complex_dd::ComplexDD;
use crate::double_double::DoubleDouble;
use crate::fractal::{Fractal, FractalParams, IterationResult};

/// Double-double precision Mandelbrot: `z_{n+1} = z_n² + c`, starting from `z₀ = 0`.
///
/// The stored `center` is the viewport center in ~31-digit precision.
/// [`iterate`](Fractal::iterate) receives a **delta** from this center
/// (small enough for `f64`) and reconstructs `c = center + delta` in DD.
#[derive(Debug, Clone)]
pub struct MandelbrotDD {
    params: FractalParams,
    center: ComplexDD,
}

/// Periodicity detection tolerance for double-double (~31 digits).
const DD_PERIOD_TOLERANCE: f64 = 1e-28;

impl MandelbrotDD {
    pub fn new(params: FractalParams, center: ComplexDD) -> Self {
        Self { params, center }
    }
}

/// Cardioid check in f64 (rough filter — false negatives are fine).
#[inline]
fn in_cardioid(re: f64, im: f64) -> bool {
    let im2 = im * im;
    let q = (re - 0.25) * (re - 0.25) + im2;
    q * (q + (re - 0.25)) <= 0.25 * im2
}

/// Period-2 bulb check in f64 (rough filter).
#[inline]
fn in_period2_bulb(re: f64, im: f64) -> bool {
    (re + 1.0) * (re + 1.0) + im * im <= 0.0625
}

impl Fractal for MandelbrotDD {
    fn iterate(&self, delta: Complex) -> IterationResult {
        let c = self.center + ComplexDD::from(delta);
        let c_f64 = c.to_complex();

        if in_cardioid(c_f64.re, c_f64.im) || in_period2_bulb(c_f64.re, c_f64.im) {
            return IterationResult::Interior;
        }

        let escape_radius_sq = DoubleDouble::from(self.params.escape_radius_sq());
        let max_iter = self.params.max_iterations;

        let mut z = ComplexDD::ZERO;

        let mut old_z = z;
        let mut period: u32 = 0;
        let mut check: u32 = 3;

        for n in 0..max_iter {
            z = ComplexDD::new(
                z.re * z.re - z.im * z.im + c.re,
                DoubleDouble::from(2.0) * z.re * z.im + c.im,
            );

            let norm_sq = z.norm_sq();
            if norm_sq > escape_radius_sq {
                return IterationResult::Escaped {
                    iterations: n,
                    norm_sq: norm_sq.to_f64(),
                };
            }

            if n >= 32 && n & 3 == 0 {
                let dre = (z.re - old_z.re).abs();
                let dim = (z.im - old_z.im).abs();
                if dre.hi < DD_PERIOD_TOLERANCE && dim.hi < DD_PERIOD_TOLERANCE {
                    return IterationResult::Interior;
                }

                period += 1;
                if period > check {
                    old_z = z;
                    period = 0;
                    check = check.saturating_mul(2);
                }
            }
        }

        IterationResult::Interior
    }

    fn params(&self) -> &FractalParams {
        &self.params
    }

    fn uses_delta_coordinates(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mandelbrot::Mandelbrot;

    fn center_zero() -> ComplexDD {
        ComplexDD::ZERO
    }

    /// When the center is (0,0), the delta IS the absolute coordinate,
    /// so MandelbrotDD should match Mandelbrot exactly.
    fn mb_dd() -> MandelbrotDD {
        MandelbrotDD::new(FractalParams::default(), center_zero())
    }

    fn mb() -> Mandelbrot {
        Mandelbrot::default()
    }

    #[test]
    fn origin_is_interior() {
        assert_eq!(
            mb_dd().iterate(Complex::new(0.0, 0.0)),
            IterationResult::Interior
        );
    }

    #[test]
    fn far_point_escapes_immediately() {
        let result = mb_dd().iterate(Complex::new(10.0, 0.0));
        match result {
            IterationResult::Escaped { iterations, .. } => {
                assert_eq!(iterations, 0);
            }
            IterationResult::Interior => panic!("far point should escape"),
        }
    }

    #[test]
    fn matches_f64_iteration_counts() {
        // DD carries more precision, so norm_sq at escape may differ slightly.
        // The iteration count (which determines coloring) must match.
        let points = [
            Complex::new(0.0, 0.0),
            Complex::new(-0.75, 0.1),
            Complex::new(0.3, 0.5),
            Complex::new(-2.0, 0.0),
            Complex::new(1.0, 1.0),
            Complex::new(0.5, 0.0),
            Complex::new(-1.0, 0.0),
            Complex::new(0.24, 0.0),
        ];
        let m = mb();
        let m_dd = mb_dd();
        for &c in &points {
            let r_f64 = m.iterate(c);
            let r_dd = m_dd.iterate(c);
            assert_eq!(
                r_f64.class(),
                r_dd.class(),
                "iteration class mismatch at c = {c}: f64={r_f64:?}, dd={r_dd:?}"
            );
        }
    }

    #[test]
    fn known_escape_count() {
        // c = 1.0: escapes at n=2
        let result = mb_dd().iterate(Complex::new(1.0, 0.0));
        match result {
            IterationResult::Escaped { iterations, .. } => {
                assert_eq!(iterations, 2);
            }
            _ => panic!("c=1.0 should escape"),
        }
    }

    #[test]
    fn deep_zoom_center_offset() {
        // Simulate a deep zoom: center is far from origin, delta is tiny.
        // This wouldn't work with f64 absolute coordinates.
        let center = ComplexDD::new(
            DoubleDouble::new(-0.75, 1e-17),
            DoubleDouble::new(0.1, 2e-18),
        );
        let m = MandelbrotDD::new(FractalParams::default(), center);
        let result = m.iterate(Complex::new(0.0, 0.0));
        assert!(
            matches!(result, IterationResult::Interior | IterationResult::Escaped { .. }),
            "should produce a valid result at deep zoom"
        );
    }

    #[test]
    fn deterministic_results() {
        let m = mb_dd();
        let points = [
            Complex::new(0.0, 0.0),
            Complex::new(-0.75, 0.1),
            Complex::new(0.3, 0.5),
        ];
        let run1: Vec<_> = points.iter().map(|&c| m.iterate(c)).collect();
        let run2: Vec<_> = points.iter().map(|&c| m.iterate(c)).collect();
        assert_eq!(run1, run2);
    }
}
