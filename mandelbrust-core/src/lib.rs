pub mod complex;
pub mod error;
pub mod fractal;
pub mod julia;
pub mod mandelbrot;
pub mod viewport;

// Re-export primary types for convenience.
pub use complex::Complex;
pub use error::CoreError;
pub use fractal::{Fractal, FractalParams, IterationResult};
pub use julia::Julia;
pub use mandelbrot::Mandelbrot;
pub use viewport::Viewport;

/// Convenience result type for the core crate.
pub type Result<T> = std::result::Result<T, CoreError>;
