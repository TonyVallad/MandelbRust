use crate::complex::Complex;
use crate::fractal::{Fractal, FractalParams, IterationResult};

/// The Mandelbrot set: `z_{n+1} = z_n² + c`, starting from `z₀ = 0`.
///
/// The point `c` is the coordinate on the complex plane.
#[derive(Debug, Clone)]
pub struct Mandelbrot {
    params: FractalParams,
}

impl Mandelbrot {
    pub fn new(params: FractalParams) -> Self {
        Self { params }
    }
}

impl Default for Mandelbrot {
    fn default() -> Self {
        Self::new(FractalParams::default())
    }
}

/// Returns `true` if `c` lies inside the main cardioid.
///
/// This is a closed-form check that avoids iterating ~30–40% of visible
/// points at the default zoom level.
#[inline]
fn in_cardioid(re: f64, im: f64) -> bool {
    let im2 = im * im;
    let q = (re - 0.25) * (re - 0.25) + im2;
    q * (q + (re - 0.25)) <= 0.25 * im2
}

/// Returns `true` if `c` lies inside the period-2 bulb.
#[inline]
fn in_period2_bulb(re: f64, im: f64) -> bool {
    (re + 1.0) * (re + 1.0) + im * im <= 0.0625
}

impl Fractal for Mandelbrot {
    fn iterate(&self, c: Complex) -> IterationResult {
        // Fast rejection: skip iteration for points known to be interior.
        if in_cardioid(c.re, c.im) || in_period2_bulb(c.re, c.im) {
            return IterationResult::Interior;
        }

        let escape_radius_sq = self.params.escape_radius_sq();
        let max_iter = self.params.max_iterations;

        let mut z = Complex::ZERO;

        // Brent's cycle detection state.
        let mut old_z = z;
        let mut period: u32 = 0;
        let mut check: u32 = 3;

        for n in 0..max_iter {
            // z = z² + c
            z = Complex::new(z.re * z.re - z.im * z.im + c.re, 2.0 * z.re * z.im + c.im);

            let norm_sq = z.norm_sq();
            if norm_sq > escape_radius_sq {
                return IterationResult::Escaped {
                    iterations: n,
                    norm_sq,
                };
            }

            // Periodicity detection (Brent's algorithm).
            // Skip the first 32 iterations (orbits rarely converge early)
            // and only check every 4th iteration to reduce branch overhead.
            if n >= 32 && n & 3 == 0 {
                if (z.re - old_z.re).abs() < 1e-13 && (z.im - old_z.im).abs() < 1e-13 {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mb() -> Mandelbrot {
        Mandelbrot::default()
    }

    #[test]
    fn origin_is_interior() {
        assert_eq!(
            mb().iterate(Complex::new(0.0, 0.0)),
            IterationResult::Interior
        );
    }

    #[test]
    fn far_point_escapes_immediately() {
        let result = mb().iterate(Complex::new(10.0, 0.0));
        match result {
            IterationResult::Escaped { iterations, .. } => {
                assert_eq!(iterations, 0, "should escape on the very first iteration");
            }
            IterationResult::Interior => panic!("far point should escape"),
        }
    }

    #[test]
    fn minus_one_is_interior() {
        // c = -1 gives the orbit 0 → -1 → 0 → -1 … (period 2)
        assert_eq!(
            mb().iterate(Complex::new(-1.0, 0.0)),
            IterationResult::Interior
        );
    }

    #[test]
    fn cardioid_center_is_interior() {
        // c = 0.25 is the cusp of the main cardioid.
        assert_eq!(
            mb().iterate(Complex::new(0.24, 0.0)),
            IterationResult::Interior
        );
    }

    #[test]
    fn period2_bulb_interior() {
        // c = -1.0 is the centre of the period-2 bulb.
        assert_eq!(
            mb().iterate(Complex::new(-1.0, 0.0)),
            IterationResult::Interior
        );
    }

    #[test]
    fn positive_real_axis_escapes() {
        // c = 0.5 is outside the set.
        let result = mb().iterate(Complex::new(0.5, 0.0));
        assert!(
            matches!(result, IterationResult::Escaped { .. }),
            "0.5 + 0i should escape"
        );
    }

    #[test]
    fn known_escape_count() {
        // c = 1.0: z₀=0, z₁=1, z₂=2 (escapes at n=2 since |2| > 2 → actually |z|²=4 > 4)
        // Actually: z₁=1, |1|²=1 ≤ 4; z₂=1+1=2, |2|²=4 ≤ 4; z₃=4+1=5, |5|²=25 > 4 → escapes at n=2
        let result = mb().iterate(Complex::new(1.0, 0.0));
        match result {
            IterationResult::Escaped { iterations, .. } => {
                assert_eq!(iterations, 2);
            }
            _ => panic!("c=1.0 should escape"),
        }
    }

    #[test]
    fn deterministic_results() {
        let m = mb();
        let points = [
            Complex::new(0.0, 0.0),
            Complex::new(-0.75, 0.1),
            Complex::new(0.3, 0.5),
            Complex::new(-2.0, 0.0),
            Complex::new(1.0, 1.0),
        ];
        let run1: Vec<_> = points.iter().map(|&c| m.iterate(c)).collect();
        let run2: Vec<_> = points.iter().map(|&c| m.iterate(c)).collect();
        assert_eq!(run1, run2, "iteration results must be deterministic");
    }
}
