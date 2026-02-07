use mandelbrust_core::{Complex, Fractal, FractalParams, IterationResult, Mandelbrot, Viewport};

/// Render every pixel of a viewport and collect results into a flat Vec.
fn render_grid<F: Fractal>(fractal: &F, viewport: &Viewport) -> Vec<IterationResult> {
    let mut results = Vec::with_capacity((viewport.width * viewport.height) as usize);
    for py in 0..viewport.height {
        for px in 0..viewport.width {
            let c = viewport.pixel_to_complex(px, py);
            results.push(fractal.iterate(c));
        }
    }
    results
}

#[test]
fn headless_mandelbrot_render() {
    let params = FractalParams::new(256, 2.0).unwrap();
    let mandelbrot = Mandelbrot::new(params);
    let viewport = Viewport::default_mandelbrot(100, 100);

    let results = render_grid(&mandelbrot, &viewport);

    assert_eq!(results.len(), 100 * 100);

    // The render should contain both escaped and interior points.
    let escaped = results
        .iter()
        .filter(|r| matches!(r, IterationResult::Escaped { .. }))
        .count();
    let interior = results
        .iter()
        .filter(|r| matches!(r, IterationResult::Interior))
        .count();

    assert!(escaped > 0, "should have some escaped points");
    assert!(interior > 0, "should have some interior points");
    assert_eq!(escaped + interior, 10_000);
}

#[test]
fn headless_render_is_deterministic() {
    let mandelbrot = Mandelbrot::default();
    let viewport = Viewport::default_mandelbrot(80, 60);

    let run1 = render_grid(&mandelbrot, &viewport);
    let run2 = render_grid(&mandelbrot, &viewport);

    assert_eq!(
        run1, run2,
        "two identical renders must produce identical results"
    );
}

#[test]
fn headless_julia_render() {
    let julia = mandelbrust_core::Julia::default();
    let viewport = Viewport::new(Complex::new(0.0, 0.0), 0.03, 100, 100).unwrap();

    let results = render_grid(&julia, &viewport);

    assert_eq!(results.len(), 10_000);

    let escaped = results
        .iter()
        .filter(|r| matches!(r, IterationResult::Escaped { .. }))
        .count();
    let interior = results
        .iter()
        .filter(|r| matches!(r, IterationResult::Interior))
        .count();

    assert!(escaped > 0, "should have some escaped points");
    assert!(interior > 0, "should have some interior points");
}
