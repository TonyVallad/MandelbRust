# Phase 11 — Deep Zoom: Double-Double Arithmetic

**Objective:** Extend the zoom ceiling from ~10^13× to ~10^28× by representing coordinates with pairs of `f64` values (~31 significant decimal digits). No external dependencies required.

**Background:** [deep-zoom-analysis.md](../deep-zoom-analysis.md), Option 2.

---

## Overview

Standard `f64` provides ~15–16 significant decimal digits, which limits useful zoom to roughly 10^13×. Beyond that, adjacent pixels map to the same `f64` value and the image breaks down into blocky artifacts.

Double-double arithmetic represents each value as `hi + lo` using two `f64` components, roughly doubling the significant digits to ~31. This pushes the zoom ceiling to ~10^28× with no external dependencies — the implementation is pure Rust using only IEEE 754 `f64` operations.

The approach integrates transparently: at normal zoom (`scale >= 1e-13`), the standard `f64` path runs unchanged. When the user zooms past the threshold, the app automatically switches to the double-double path. All navigation operations (zoom, pan, undo/redo, bookmarks) preserve full DD precision.

---

## Key algorithms

### Error-free transformations

The foundation is three building blocks that compute exact results from `f64` arithmetic:

- **TwoSum** (Knuth): Given `a, b : f64`, returns `(s, e)` where `s + e = a + b` exactly. No magnitude restriction on inputs.
- **QuickTwoSum**: Same as TwoSum but requires `|a| >= |b|`. Fewer operations.
- **TwoProd** (FMA-based): Given `a, b : f64`, returns `(p, e)` where `p + e = a * b` exactly. Uses hardware FMA via `f64::mul_add`.

### DD arithmetic

- **Addition**: TwoSum on the `hi` parts, TwoSum on the `lo` parts, then two QuickTwoSum passes to renormalize.
- **Multiplication**: TwoProd on the `hi` parts, accumulate cross terms (`hi × lo + lo × hi`), then QuickTwoSum to renormalize.
- **Subtraction**: Negation + addition.

Reference: Hida, Li, Bailey — *"Library for Double-Double and Quad-Double Arithmetic"* (2001).

---

## Design decisions

### Delta-coordinate protocol

The central challenge is that the `Fractal` trait's `iterate()` method accepts `Complex` (f64). If the renderer passes absolute coordinates, precision is already lost before the fractal sees the values.

**Solution:** DD fractals declare `uses_delta_coordinates() = true` on the `Fractal` trait. The renderer checks this flag and calls `Viewport::pixel_to_delta()` (returning just the pixel offset from center in f64) instead of `pixel_to_complex()` (which adds the f64 center). The DD fractal stores the high-precision center internally and reconstructs `c = center_dd + delta` in DD.

This adds a single well-predicted branch per pixel to the renderer, with zero overhead for the existing f64 path.

### Viewport dual center

`Viewport` now stores both `center: Complex` (f64 approximation) and `center_dd: ComplexDD` (authoritative). The DD center is always the source of truth; the f64 center is derived via `to_complex()` and kept in sync by helper methods (`set_center_dd`, `offset_center`).

This avoids changing the dozens of read-sites that use `viewport.center` for display, HUD text, minimap drawing, etc.

### Bookmark serialization

Rather than implementing a DD-to-decimal-string converter (non-trivial), the hi/lo components are stored as separate `f64` fields in JSON: `center_re` + `center_re_lo`, `center_im` + `center_im_lo`. The `_lo` fields use `#[serde(default)]`, so bookmarks created before Phase 11 load naturally with `lo = 0.0` — no migration code needed, no backward-compatibility shims.

### Periodicity detection scaling

The f64 Mandelbrot uses `1e-13` as the periodicity detection threshold (matching ~15-digit precision). DD paths use `1e-28` (~31-digit precision). Without this scaling, periodicity detection would falsely flag non-periodic orbits as periodic at deep zoom.

### Cardioid/bulb checks remain f64

The cardioid and period-2 bulb rejection tests are rough geometric filters. They work correctly in f64 at any zoom — false negatives just mean a few extra iterations for points near the boundary. No precision benefit from promoting these to DD.

---

## New files

| File | Description |
|------|-------------|
| `mandelbrust-core/src/double_double.rs` | `DoubleDouble` type: TwoSum, QuickTwoSum, TwoProd, full arithmetic, comparison, display. 20 tests. |
| `mandelbrust-core/src/complex_dd.rs` | `ComplexDD` type: complex arithmetic over `DoubleDouble`, `norm_sq`, `From<Complex>`, `to_complex`. 10 tests. |
| `mandelbrust-core/src/mandelbrot_dd.rs` | `MandelbrotDD`: DD Mandelbrot iteration with `Fractal` trait, delta-coordinate protocol. 6 tests. |
| `mandelbrust-core/src/julia_dd.rs` | `JuliaDD`: DD Julia iteration with `Fractal` trait, delta-coordinate protocol. 5 tests. |

## Modified files

| File | Changes |
|------|---------|
| `mandelbrust-core/src/lib.rs` | Registered new modules; re-exports `DoubleDouble`, `ComplexDD`, `MandelbrotDD`, `JuliaDD`. |
| `mandelbrust-core/src/fractal.rs` | Added `uses_delta_coordinates()` to `Fractal` trait (default `false`). |
| `mandelbrust-core/src/viewport.rs` | Added `center_dd: ComplexDD` field; `new_dd()`, `set_center_dd()`, `offset_center()`, `pixel_to_delta()`, `subpixel_to_delta()`. |
| `mandelbrust-render/src/renderer.rs` | Added `map_pixel()` helper respecting `uses_delta_coordinates()` in tile rendering and border tracing. |
| `mandelbrust-render/src/aa.rs` | AA sub-pixel sampling respects delta coordinates. |
| `mandelbrust-app/src/main.rs` | DD auto-selection in `render_for_mode()` at `scale < 1e-13`. All zoom/pan operations use DD helpers. HUD precision indicator. Warning at `scale < 1e-28`. |
| `mandelbrust-app/src/bookmarks.rs` | Added `center_re_lo`, `center_im_lo` fields. |
| `mandelbrust-app/src/preferences.rs` | Added `center_re_lo`, `center_im_lo` fields to `LastView`. |

---

## API surface

### `DoubleDouble`

```
struct DoubleDouble { pub hi: f64, pub lo: f64 }

ZERO, new(hi, lo), from(f64), to_f64()
abs(), is_positive(), is_negative()
Add, Sub, Mul, Neg, AddAssign, SubAssign, MulAssign
Mul<f64> (scalar)
PartialEq, PartialOrd, Display
```

### `ComplexDD`

```
struct ComplexDD { pub re: DoubleDouble, pub im: DoubleDouble }

ZERO, new(re, im), from(Complex), to_complex()
norm_sq() -> DoubleDouble
Add, Sub, Mul, Neg
Mul<DoubleDouble> (scalar)
Display
```

### `MandelbrotDD` / `JuliaDD`

```
MandelbrotDD::new(params: FractalParams, center: ComplexDD)
JuliaDD::new(c: ComplexDD, params: FractalParams, center: ComplexDD)

impl Fractal: iterate(delta: Complex), params(), uses_delta_coordinates() = true
```

### Viewport additions

```
new_dd(center_dd, scale, width, height) -> Result<Viewport>
set_center_dd(center_dd: ComplexDD)
offset_center(dre: f64, dim: f64)
pixel_to_delta(px, py) -> Complex
subpixel_to_delta(px, py) -> Complex
```

---

## Test coverage

41 new unit tests across the four new files, covering:

- Basic arithmetic correctness (matches f64 for small values)
- Precision retention (e.g. `1.0 + 1e-17 - 1.0 = 1e-17` in DD vs 0 in f64)
- Catastrophic cancellation survival
- Distributive property
- DD iteration count matching f64 at normal zoom
- Deep-zoom center offset correctness
- Determinism
