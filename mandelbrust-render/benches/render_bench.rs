use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};

use mandelbrust_core::{Complex, FractalParams, Mandelbrot, Viewport};
use mandelbrust_render::{render, Palette, RenderCancel};

fn bench_full_frame_render(c: &mut Criterion) {
    let mandelbrot = Mandelbrot::default();
    let viewport = Viewport::default_mandelbrot(640, 480);
    let cancel = Arc::new(RenderCancel::new());

    c.bench_function("full_frame_640x480", |b| {
        b.iter(|| render(&mandelbrot, &viewport, &cancel));
    });
}

fn bench_iteration_throughput(c: &mut Criterion) {
    let params = FractalParams::new(1000, 2.0).unwrap();
    let mandelbrot = Mandelbrot::new(params);
    let viewport = Viewport::new(Complex::new(-0.5, 0.0), 0.005, 256, 256).unwrap();
    let cancel = Arc::new(RenderCancel::new());

    c.bench_function("render_256x256_1000iter", |b| {
        b.iter(|| render(&mandelbrot, &viewport, &cancel));
    });
}

fn bench_colorize(c: &mut Criterion) {
    let mandelbrot = Mandelbrot::default();
    let viewport = Viewport::default_mandelbrot(640, 480);
    let cancel = Arc::new(RenderCancel::new());
    let result = render(&mandelbrot, &viewport, &cancel);
    let palette = Palette::default();

    c.bench_function("colorize_640x480", |b| {
        b.iter(|| palette.colorize(&result.iterations, true));
    });
}

criterion_group!(
    benches,
    bench_full_frame_render,
    bench_iteration_throughput,
    bench_colorize
);
criterion_main!(benches);
