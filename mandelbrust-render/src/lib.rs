pub mod aa;
pub mod buffer;
pub mod error;
pub mod iteration_buffer;
pub mod palette;
pub mod renderer;
pub mod tile;

pub use aa::{compute_aa, AaSamples};
pub use buffer::RenderBuffer;
pub use error::RenderError;
pub use iteration_buffer::IterationBuffer;
pub use palette::{builtin_palettes, ColorParams, Palette, StartFrom};
pub use renderer::{render, RenderCancel, RenderResult};
pub use tile::TILE_SIZE;

/// Convenience result type for the render crate.
pub type Result<T> = std::result::Result<T, RenderError>;
