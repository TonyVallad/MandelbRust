use crate::complex::Complex;
use crate::error::CoreError;

/// Defines the visible region of the complex plane.
///
/// The camera maps pixel coordinates to complex plane coordinates.
/// The viewport is centred on `center`, with `scale` defining how many
/// complex-plane units each pixel spans.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// Centre of the viewport in the complex plane.
    pub center: Complex,

    /// Complex-plane units per pixel.
    pub scale: f64,

    /// Viewport width in pixels.
    pub width: u32,

    /// Viewport height in pixels.
    pub height: u32,
}

impl Viewport {
    /// Default view: centred on the Mandelbrot set with the full set visible.
    ///
    /// The set fits in roughly `[-2.0, 0.47] × [-1.12, 1.12]`.  We pick
    /// a scale that ensures the whole set is visible regardless of aspect
    /// ratio, with a small margin for breathing room.
    pub fn default_mandelbrot(width: u32, height: u32) -> Self {
        // Bounding box of the interesting region, plus ~5 % padding.
        let target_re = 3.6; // real span
        let target_im = 2.6; // imaginary span
        let scale = (target_re / width as f64).max(target_im / height as f64);
        Self {
            center: Complex::new(-0.75, 0.0),
            scale,
            width,
            height,
        }
    }

    /// Default view for Julia sets, centred on the origin.
    ///
    /// Most Julia sets for typical parameters fit within `|z| < 2`, so the
    /// viewport spans roughly `[-2, 2] × [-2, 2]` with a small margin.
    pub fn default_julia(width: u32, height: u32) -> Self {
        let extent = 4.2; // 4.0 + padding
        let scale = (extent / width as f64).max(extent / height as f64);
        Self {
            center: Complex::new(0.0, 0.0),
            scale,
            width,
            height,
        }
    }

    /// Create a viewport with explicit parameters.
    pub fn new(center: Complex, scale: f64, width: u32, height: u32) -> crate::Result<Self> {
        if width == 0 || height == 0 {
            return Err(CoreError::InvalidViewport {
                reason: format!("dimensions must be > 0, got {width}×{height}"),
            });
        }
        if scale <= 0.0 || !scale.is_finite() {
            return Err(CoreError::InvalidViewport {
                reason: format!("scale must be positive and finite, got {scale}"),
            });
        }
        Ok(Self {
            center,
            scale,
            width,
            height,
        })
    }

    /// Map a pixel coordinate to a point on the complex plane.
    ///
    /// `(0, 0)` is the top-left pixel. The y-axis is flipped so that
    /// increasing pixel-y moves downward (decreasing imaginary part).
    #[inline]
    pub fn pixel_to_complex(&self, px: u32, py: u32) -> Complex {
        self.subpixel_to_complex(px as f64, py as f64)
    }

    /// Map fractional pixel coordinates to a complex-plane point.
    ///
    /// Like [`pixel_to_complex`](Self::pixel_to_complex) but accepts `f64`
    /// coordinates for sub-pixel sampling (used by anti-aliasing).
    #[inline]
    pub fn subpixel_to_complex(&self, px: f64, py: f64) -> Complex {
        let half_w = self.width as f64 / 2.0;
        let half_h = self.height as f64 / 2.0;
        Complex::new(
            self.center.re + (px - half_w) * self.scale,
            self.center.im - (py - half_h) * self.scale,
        )
    }

    /// The aspect ratio of the viewport (width / height).
    pub fn aspect_ratio(&self) -> f64 {
        self.width as f64 / self.height as f64
    }

    /// Create a lower-resolution viewport covering the same complex-plane region.
    ///
    /// Divides pixel dimensions by `factor` and scales up the per-pixel
    /// spacing proportionally, so the visible complex region stays the same.
    pub fn downscaled(&self, factor: u32) -> Self {
        let f = factor.max(1);
        Self {
            center: self.center,
            scale: self.scale * f as f64,
            width: self.width.div_ceil(f),
            height: self.height.div_ceil(f),
        }
    }

    /// The total extent of the viewport in complex-plane units.
    pub fn complex_width(&self) -> f64 {
        self.width as f64 * self.scale
    }

    /// The total extent of the viewport in complex-plane units.
    pub fn complex_height(&self) -> f64 {
        self.height as f64 * self.scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-10;

    #[test]
    fn default_mandelbrot_viewport() {
        let vp = Viewport::default_mandelbrot(800, 600);
        assert_eq!(vp.width, 800);
        assert_eq!(vp.height, 600);
        assert!((vp.center.re - (-0.75)).abs() < EPSILON);
        assert!((vp.center.im - 0.0).abs() < EPSILON);
        // The full set should be visible: viewport must span at least 3.5 × 2.5.
        assert!(vp.complex_width() >= 3.5);
        assert!(vp.complex_height() >= 2.5);
    }

    #[test]
    fn default_julia_viewport() {
        let vp = Viewport::default_julia(1280, 720);
        assert_eq!(vp.width, 1280);
        assert_eq!(vp.height, 720);
        assert!((vp.center.re).abs() < EPSILON);
        assert!((vp.center.im).abs() < EPSILON);
        // Julia sets fit within |z| < 2, so viewport must span at least 4 × 4.
        assert!(vp.complex_width() >= 4.0);
        assert!(vp.complex_height() >= 4.0);
    }

    #[test]
    fn downscaled_preserves_region() {
        let vp = Viewport::default_mandelbrot(1280, 720);
        let ds = vp.downscaled(4);
        assert_eq!(ds.width, 320);
        assert_eq!(ds.height, 180);
        assert_eq!(ds.center, vp.center);
        // The visible complex region should be (approximately) the same.
        let orig_w = vp.complex_width();
        let ds_w = ds.complex_width();
        assert!((orig_w - ds_w).abs() / orig_w < 0.01);
    }

    #[test]
    fn pixel_to_complex_center() {
        let vp = Viewport::new(Complex::new(0.0, 0.0), 0.01, 100, 100).unwrap();
        let c = vp.pixel_to_complex(50, 50);
        assert!((c.re - 0.0).abs() < EPSILON);
        assert!((c.im - 0.0).abs() < EPSILON);
    }

    #[test]
    fn pixel_to_complex_corners() {
        let vp = Viewport::new(Complex::new(0.0, 0.0), 1.0, 100, 100).unwrap();

        // Top-left pixel → positive imaginary, negative real
        let tl = vp.pixel_to_complex(0, 0);
        assert!((tl.re - (-50.0)).abs() < EPSILON);
        assert!((tl.im - 50.0).abs() < EPSILON);

        // Bottom-right pixel → negative imaginary, positive real
        let br = vp.pixel_to_complex(99, 99);
        assert!((br.re - 49.0).abs() < EPSILON);
        assert!((br.im - (-49.0)).abs() < EPSILON);
    }

    #[test]
    fn invalid_dimensions() {
        assert!(Viewport::new(Complex::ZERO, 0.01, 0, 100).is_err());
        assert!(Viewport::new(Complex::ZERO, 0.01, 100, 0).is_err());
    }

    #[test]
    fn invalid_scale() {
        assert!(Viewport::new(Complex::ZERO, 0.0, 100, 100).is_err());
        assert!(Viewport::new(Complex::ZERO, -1.0, 100, 100).is_err());
    }

    #[test]
    fn aspect_ratio() {
        let vp = Viewport::default_mandelbrot(1920, 1080);
        let ar = vp.aspect_ratio();
        assert!((ar - 1920.0 / 1080.0).abs() < EPSILON);
    }
}
