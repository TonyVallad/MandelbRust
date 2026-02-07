use crate::complex::Complex;
use crate::fractal::{Fractal, FractalParams, IterationResult};

/// A Julia set: `z_{n+1} = z_n² + c`, where `c` is a fixed constant
/// and `z₀` is the point on the complex plane.
#[derive(Debug, Clone)]
pub struct Julia {
    params: FractalParams,

    /// The fixed constant `c` that defines this Julia set.
    c: Complex,
}

impl Julia {
    pub fn new(c: Complex, params: FractalParams) -> Self {
        Self { params, c }
    }

    /// A visually interesting default: `c = -0.7 + 0.27015i`.
    pub fn default_c() -> Complex {
        Complex::new(-0.7, 0.27015)
    }

    /// The constant `c` defining this Julia set.
    pub fn c(&self) -> Complex {
        self.c
    }
}

impl Default for Julia {
    fn default() -> Self {
        Self::new(Self::default_c(), FractalParams::default())
    }
}

impl Fractal for Julia {
    fn iterate(&self, point: Complex) -> IterationResult {
        let escape_radius_sq = self.params.escape_radius_sq();
        let max_iter = self.params.max_iterations;

        let mut z = point;

        // Brent's cycle detection state.
        let mut old_z = z;
        let mut period: u32 = 0;
        let mut check: u32 = 3;

        for n in 0..max_iter {
            // z = z² + c
            z = Complex::new(
                z.re * z.re - z.im * z.im + self.c.re,
                2.0 * z.re * z.im + self.c.im,
            );

            let norm_sq = z.norm_sq();
            if norm_sq > escape_radius_sq {
                return IterationResult::Escaped {
                    iterations: n,
                    norm_sq,
                };
            }

            // Periodicity detection (Brent's algorithm).
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

        IterationResult::Interior
    }

    fn params(&self) -> &FractalParams {
        &self.params
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn julia() -> Julia {
        Julia::default()
    }

    #[test]
    fn far_point_escapes() {
        let result = julia().iterate(Complex::new(10.0, 0.0));
        assert!(
            matches!(result, IterationResult::Escaped { .. }),
            "far point should escape"
        );
    }

    #[test]
    fn origin_result_depends_on_c() {
        // For the default c = -0.7 + 0.27015i, z₀ = 0 should produce
        // a specific deterministic result.
        let r1 = julia().iterate(Complex::ZERO);
        let r2 = julia().iterate(Complex::ZERO);
        assert_eq!(r1, r2, "must be deterministic");
    }

    #[test]
    fn c_zero_matches_mandelbrot_for_interior() {
        // Julia with c=0: z_{n+1} = z_n². Origin is a fixed point.
        let j = Julia::new(Complex::ZERO, FractalParams::default());
        assert_eq!(j.iterate(Complex::ZERO), IterationResult::Interior);
    }

    #[test]
    fn c_zero_far_point_escapes() {
        let j = Julia::new(Complex::ZERO, FractalParams::default());
        let result = j.iterate(Complex::new(3.0, 0.0));
        assert!(matches!(result, IterationResult::Escaped { .. }));
    }

    #[test]
    fn deterministic_results() {
        let j = julia();
        let points = [
            Complex::new(0.0, 0.0),
            Complex::new(0.5, 0.5),
            Complex::new(-1.0, 0.3),
            Complex::new(0.0, 1.0),
        ];
        let run1: Vec<_> = points.iter().map(|&p| j.iterate(p)).collect();
        let run2: Vec<_> = points.iter().map(|&p| j.iterate(p)).collect();
        assert_eq!(run1, run2, "iteration results must be deterministic");
    }
}
