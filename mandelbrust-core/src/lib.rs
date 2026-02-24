pub mod complex;
pub mod complex_dd;
pub mod double_double;
pub mod error;
pub mod fractal;
pub mod julia;
pub mod julia_dd;
pub mod mandelbrot;
pub mod mandelbrot_dd;
pub mod viewport;

// Re-export primary types for convenience.
pub use complex::Complex;
pub use complex_dd::ComplexDD;
pub use double_double::DoubleDouble;
pub use error::CoreError;
pub use fractal::{Fractal, FractalParams, IterationResult};
pub use julia::Julia;
pub use julia_dd::JuliaDD;
pub use mandelbrot::Mandelbrot;
pub use mandelbrot_dd::MandelbrotDD;
pub use viewport::Viewport;

/// Convenience result type for the core crate.
pub type Result<T> = std::result::Result<T, CoreError>;
