use std::sync::Arc;

use mandelbrust_core::{Complex, FractalParams, Julia, Mandelbrot, Viewport};
use mandelbrust_render::{render, ColorParams, Palette, RenderCancel, RenderOptions};

fn opts(symmetry: bool) -> RenderOptions {
    RenderOptions {
        use_real_axis_symmetry: symmetry,
        ..Default::default()
    }
}

#[test]
fn end_to_end_mandelbrot_render() {
    let mandelbrot = Mandelbrot::default();
    let viewport = Viewport::default_mandelbrot(200, 150);
    let cancel = Arc::new(RenderCancel::new());

    let result = render(&mandelbrot, &viewport, &cancel, &opts(true));

    assert!(!result.cancelled);
    assert_eq!(result.iterations.width, 200);
    assert_eq!(result.iterations.height, 150);
    assert_eq!(result.iterations.data.len(), 200 * 150);
    assert!(result.tiles_rendered > 0);
    assert!(result.elapsed.as_nanos() > 0);

    let palette = Palette::default();
    let buffer = palette.colorize(&result.iterations, &ColorParams::from_smooth(true));
    let has_non_black = buffer
        .pixels
        .chunks_exact(4)
        .any(|px| px[0] > 0 || px[1] > 0 || px[2] > 0);
    assert!(
        has_non_black,
        "rendered image should contain non-black pixels"
    );
}

#[test]
fn end_to_end_julia_render() {
    let julia = Julia::default();
    let viewport = Viewport::new(Complex::new(0.0, 0.0), 0.02, 100, 100).unwrap();
    let cancel = Arc::new(RenderCancel::new());

    let result = render(&julia, &viewport, &cancel, &opts(false));

    assert!(!result.cancelled);
    assert_eq!(result.iterations.data.len(), 100 * 100);
}

#[test]
fn render_determinism() {
    let mandelbrot = Mandelbrot::default();
    let viewport = Viewport::default_mandelbrot(128, 96);
    let cancel = Arc::new(RenderCancel::new());

    let r1 = render(&mandelbrot, &viewport, &cancel, &opts(true));
    let r2 = render(&mandelbrot, &viewport, &cancel, &opts(true));

    assert_eq!(
        r1.iterations.data, r2.iterations.data,
        "renders must be deterministic"
    );
}

#[test]
fn symmetry_produces_correct_image() {
    let params = FractalParams::new(128, 2.0).unwrap();
    let mandelbrot = Mandelbrot::new(params);
    let cancel = Arc::new(RenderCancel::new());

    let vp_sym = Viewport::new(Complex::new(-0.5, 0.0), 0.01, 128, 128).unwrap();
    let result_sym = render(&mandelbrot, &vp_sym, &cancel, &opts(true));

    let vp_nosym = Viewport::new(Complex::new(-0.5, 0.001), 0.01, 128, 128).unwrap();
    let result_nosym = render(&mandelbrot, &vp_nosym, &cancel, &opts(true));

    assert!(result_sym.tiles_mirrored > 0, "symmetry should be used");
    assert_eq!(result_nosym.tiles_mirrored, 0, "no symmetry off-axis");

    assert!(!result_sym.cancelled);
    assert!(!result_nosym.cancelled);
}

#[test]
fn palette_switch_without_recompute() {
    let mandelbrot = Mandelbrot::default();
    let viewport = Viewport::default_mandelbrot(128, 96);
    let cancel = Arc::new(RenderCancel::new());

    let result = render(&mandelbrot, &viewport, &cancel, &opts(true));

    let palettes = mandelbrust_render::builtin_palettes();
    let params = ColorParams::from_smooth(true);
    let buf_a = palettes[0].colorize(&result.iterations, &params);
    let buf_b = palettes[1].colorize(&result.iterations, &params);

    assert_eq!(buf_a.pixels.len(), 128 * 96 * 4);
    assert_eq!(buf_b.pixels.len(), 128 * 96 * 4);

    assert_ne!(
        buf_a.pixels, buf_b.pixels,
        "different palettes should produce different images"
    );
}
