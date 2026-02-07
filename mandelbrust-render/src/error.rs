use thiserror::Error;

/// Errors originating from the rendering pipeline.
#[derive(Debug, Error)]
pub enum RenderError {
    #[error("invalid tile size: {0}×{0} (must be > 0)")]
    InvalidTileSize(u32),

    #[error("invalid image dimensions: {width}×{height}")]
    InvalidDimensions { width: u32, height: u32 },

    #[error("render cancelled")]
    Cancelled,

    #[error(transparent)]
    Core(#[from] mandelbrust_core::CoreError),
}
