use thiserror::Error;

/// Errors originating from the core fractal engine.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid max iterations: {0} (must be >= 1)")]
    InvalidMaxIterations(u32),

    #[error("invalid escape radius: {0} (must be > 0.0)")]
    InvalidEscapeRadius(f64),

    #[error("invalid viewport: {reason}")]
    InvalidViewport { reason: String },
}
