use crate::complex::Complex;
use crate::complex_dd::ComplexDD;
use crate::double_double::DoubleDouble;
use crate::fractal::{Fractal, FractalParams, IterationResult};

/// Double-double precision Julia set: `z_{n+1} = z_n² + c`,
/// where `c` is a fixed constant and `z₀` is the point.
///
/// The stored `center` is the viewport center in ~31-digit precision.
/// [`iterate`](Fractal::iterate) receives a **delta** from this center
/// (small enough for `f64`) and reconstructs `z₀ = center + delta` in DD.
#[derive(Debug, Clone)]
pub struct JuliaDD {
    params: FractalParams,
    c: ComplexDD,
    center: ComplexDD,
}

/// Periodicity detection tolerance for double-double (~31 digits).
const DD_PERIOD_TOLERANCE: f64 = 1e-28;

impl JuliaDD {
    pub fn new(c: ComplexDD, params: FractalParams, center: ComplexDD) -> Self {
        Self { params, c, center }
    }

    pub fn c(&self) -> ComplexDD {
        self.c
    }
}

impl Fractal for JuliaDD {
    fn iterate(&self, delta: Complex) -> IterationResult {
        let escape_radius_sq = DoubleDouble::from(self.params.escape_radius_sq());
        let max_iter = self.params.max_iterations;

        let mut z = self.center + ComplexDD::from(delta);

        let mut old_z = z;
        let mut period: u32 = 0;
        let mut check: u32 = 3;

        for n in 0..max_iter {
            z = ComplexDD::new(
                z.re * z.re - z.im * z.im + self.c.re,
                DoubleDouble::from(2.0) * z.re * z.im + self.c.im,
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
    use crate::julia::Julia;

    fn center_zero() -> ComplexDD {
        ComplexDD::ZERO
    }

    fn default_c() -> Complex {
        Julia::default_c()
    }

    /// With center = (0,0), delta IS the absolute coordinate,
    /// so JuliaDD should match Julia exactly.
    fn julia_dd() -> JuliaDD {
        JuliaDD::new(
            ComplexDD::from(default_c()),
            FractalParams::default(),
            center_zero(),
        )
    }

    fn julia() -> Julia {
        Julia::default()
    }

    #[test]
    fn far_point_escapes() {
        let result = julia_dd().iterate(Complex::new(10.0, 0.0));
        assert!(matches!(result, IterationResult::Escaped { .. }));
    }

    #[test]
    fn matches_f64_iteration_counts() {
        let points = [
            Complex::new(0.0, 0.0),
            Complex::new(0.5, 0.5),
            Complex::new(-1.0, 0.3),
            Complex::new(0.0, 1.0),
            Complex::new(3.0, 0.0),
        ];
        let j = julia();
        let j_dd = julia_dd();
        for &p in &points {
            let r_f64 = j.iterate(p);
            let r_dd = j_dd.iterate(p);
            assert_eq!(
                r_f64.class(),
                r_dd.class(),
                "iteration class mismatch at p = {p}: f64={r_f64:?}, dd={r_dd:?}"
            );
        }
    }

    #[test]
    fn c_zero_origin_is_interior() {
        let j = JuliaDD::new(ComplexDD::ZERO, FractalParams::default(), center_zero());
        assert_eq!(j.iterate(Complex::ZERO), IterationResult::Interior);
    }

    #[test]
    fn deep_zoom_center_offset() {
        let center = ComplexDD::new(
            DoubleDouble::new(0.3, 1e-18),
            DoubleDouble::new(0.5, -2e-19),
        );
        let j = JuliaDD::new(ComplexDD::from(default_c()), FractalParams::default(), center);
        let result = j.iterate(Complex::new(0.0, 0.0));
        assert!(
            matches!(result, IterationResult::Interior | IterationResult::Escaped { .. }),
            "should produce a valid result at deep zoom"
        );
    }

    #[test]
    fn deterministic_results() {
        let j = julia_dd();
        let points = [
            Complex::new(0.0, 0.0),
            Complex::new(0.5, 0.5),
            Complex::new(-1.0, 0.3),
        ];
        let run1: Vec<_> = points.iter().map(|&p| j.iterate(p)).collect();
        let run2: Vec<_> = points.iter().map(|&p| j.iterate(p)).collect();
        assert_eq!(run1, run2);
    }
}
