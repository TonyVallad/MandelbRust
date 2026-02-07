use crate::complex::Complex;
use crate::error::CoreError;

/// The result of iterating a single point.
///
/// The core engine stores only raw iteration data. The smooth coloring
/// formula (`ν = n + 1 − ln(ln|z|) / ln(2)`) is deferred to the coloring
/// pass in `mandelbrust-render`, keeping the hot loop lean.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IterationResult {
    /// The orbit escaped after `iterations` steps.
    /// `norm_sq` is `|z|²` at the moment of escape.
    Escaped { iterations: u32, norm_sq: f64 },

    /// The point is (likely) inside the set — it did not escape within
    /// `max_iterations`, or was detected as periodic.
    Interior,
}

impl IterationResult {
    /// Integer classification for comparing neighbouring pixels.
    ///
    /// Two pixels "match" when they share the same class. Used by border
    /// tracing and AA boundary detection.
    #[inline]
    pub fn class(&self) -> u64 {
        match self {
            Self::Escaped { iterations, .. } => *iterations as u64,
            Self::Interior => u64::MAX,
        }
    }
}

/// Parameters controlling fractal iteration.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FractalParams {
    /// Maximum number of iterations before declaring a point interior.
    pub max_iterations: u32,

    /// Bailout radius — if `|z|` exceeds this, the orbit has escaped.
    /// Stored directly; the iteration loop compares against `escape_radius²`.
    pub escape_radius: f64,
}

impl FractalParams {
    pub const DEFAULT_MAX_ITERATIONS: u32 = 256;
    pub const DEFAULT_ESCAPE_RADIUS: f64 = 2.0;

    pub fn new(max_iterations: u32, escape_radius: f64) -> crate::Result<Self> {
        if max_iterations < 1 {
            return Err(CoreError::InvalidMaxIterations(max_iterations));
        }
        if escape_radius <= 0.0 || !escape_radius.is_finite() {
            return Err(CoreError::InvalidEscapeRadius(escape_radius));
        }
        Ok(Self {
            max_iterations,
            escape_radius,
        })
    }

    /// Pre-computed squared escape radius for the inner loop.
    #[inline]
    pub fn escape_radius_sq(&self) -> f64 {
        self.escape_radius * self.escape_radius
    }
}

impl Default for FractalParams {
    fn default() -> Self {
        Self {
            max_iterations: Self::DEFAULT_MAX_ITERATIONS,
            escape_radius: Self::DEFAULT_ESCAPE_RADIUS,
        }
    }
}

/// Trait implemented by all fractal types.
///
/// Designed for **static dispatch** — renderers should be generic over
/// `F: Fractal` rather than using `dyn Fractal`, so the compiler can
/// inline and optimize the hot iteration loop.
pub trait Fractal {
    /// Iterate a single point and return the result.
    fn iterate(&self, point: Complex) -> IterationResult;

    /// Access the iteration parameters.
    fn params(&self) -> &FractalParams;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_params() {
        let p = FractalParams::default();
        assert_eq!(p.max_iterations, 256);
        assert!((p.escape_radius - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn valid_params() {
        let p = FractalParams::new(1000, 4.0).unwrap();
        assert_eq!(p.max_iterations, 1000);
        assert!((p.escape_radius_sq() - 16.0).abs() < f64::EPSILON);
    }

    #[test]
    fn invalid_max_iterations() {
        assert!(FractalParams::new(0, 2.0).is_err());
    }

    #[test]
    fn invalid_escape_radius() {
        assert!(FractalParams::new(256, 0.0).is_err());
        assert!(FractalParams::new(256, -1.0).is_err());
        assert!(FractalParams::new(256, f64::NAN).is_err());
        assert!(FractalParams::new(256, f64::INFINITY).is_err());
    }
}
