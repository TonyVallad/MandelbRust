# Deep Zoom Analysis for MandelbRust

> How far can we zoom, and what would it take to go further?

## Current State

MandelbRust uses **`f64`** (IEEE 754 double-precision) everywhere:

- `Complex { re: f64, im: f64 }` — the core number type (`mandelbrust-core/src/complex.rs`)
- `Viewport { center: Complex, scale: f64, … }` — the camera (`mandelbrust-core/src/viewport.rs`)
- All iteration arithmetic: `z = z² + c` is computed in `f64` (`mandelbrust-core/src/mandelbrot.rs`, `julia.rs`)
- Bookmarks, preferences, serialization — all `f64`

### Current zoom limit

`f64` provides **~15–16 significant decimal digits**. The Mandelbrot set lives in roughly `[-2, 0.5] × [-1.1, 1.1]`, so when the viewport `scale` (complex-plane units per pixel) drops below about `1e-13`, the difference between adjacent pixels becomes smaller than what `f64` can represent. The app already warns at `PRECISION_WARN_SCALE = 1e-13`.

**Practical zoom ceiling: ~10^13 to 10^14×** relative to the initial view.

Beyond this, adjacent pixels map to the same `f64` value and the image degenerates into blocky rectangles.

---

## Options for Deeper Zoom

The options below are ordered roughly by **impact-to-effort ratio** — the most effective techniques for the Mandelbrot set specifically are listed first.

---

### Option 1: Perturbation Theory with Series Approximation

**Zoom ceiling: effectively unlimited (10^100+ demonstrated by other renderers)**

This is the gold-standard technique used by every serious deep-zoom Mandelbrot renderer (Kalles Fraktaler, Mandel Machine, Fractal eXtreme). It is by far the most impactful change for deep zoom.

#### How it works

Instead of computing every pixel in high precision, you compute a **single reference orbit** `Z_n` in arbitrary precision, then express each pixel as a small perturbation `δ_n = z_n - Z_n`. The iteration formula becomes:

```
δ_{n+1} = 2·Z_n·δ_n + δ_n² + δ_c
```

Because `δ_n` and `δ_c` are *tiny* relative to `Z_n`, they fit comfortably in `f64` even at extreme zoom levels. Only the single reference orbit needs arbitrary/extended precision; all per-pixel work stays in `f64`.

**Series Approximation (SA)** further accelerates this: at the start of the orbit, when `δ_n` is extremely small, its evolution can be approximated by a polynomial in `δ_c`. This lets you skip the first N iterations for all pixels simultaneously, computing them from a small number of polynomial coefficients instead.

#### How to implement it in MandelbRust

1. **Add an arbitrary-precision complex type** for the reference orbit only. Options:
   - [`rug`](https://crates.io/crates/rug) (Rust bindings for GMP/MPFR) — fastest, but requires GMP C library at build time.
   - [`dashu`](https://crates.io/crates/dashu) — pure Rust arbitrary precision, easier to build on Windows.
   - [`astro-float`](https://crates.io/crates/astro-float) — pure Rust, designed for high-precision float math.

2. **Compute the reference orbit** in arbitrary precision and store it as a `Vec<Complex>` (downcast to `f64` for the deltas). This is a single serial computation.

3. **Iterate perturbation deltas per pixel** in `f64` using the stored reference orbit. This replaces the current per-pixel `Fractal::iterate()` call. The existing Rayon-based tiled pipeline (`renderer.rs`) can be reused almost unchanged — only the inner loop body changes.

4. **Handle glitches**: when `|δ_n|` approaches `|Z_n|` the perturbation becomes unreliable ("glitch"). The standard fix is to detect affected pixels and re-run them against a secondary reference orbit chosen from a glitched pixel.

5. **Add Series Approximation (SA)**: compute polynomial coefficients `A_n, B_n, C_n, …` alongside the reference orbit to skip early iterations. This dramatically speeds up renders at zoom depths above ~10^20.

6. **Wire up precision selection**: `Viewport::scale` tells you the required number of significant bits. A rule of thumb: `bits ≈ -log2(scale) + 64`. Below ~10^14 no perturbation is needed; above, engage the perturbation path.

#### Impact on existing code

- `Fractal` trait needs a new method or a parallel code path (`iterate_perturbed`).
- `Complex` stays as-is for the `f64` delta work; a new `ComplexArb` or `ComplexHP` type is added for the reference orbit only.
- `Viewport` and `Bookmark` serialization gains a precision field or stores center coordinates as decimal strings for lossless round-tripping at extreme zoom.
- Rendering pipeline in `renderer.rs` gains a branch: standard iteration below the `f64` threshold, perturbation above.

#### Pros

- Enables zoom depths of 10^50, 10^100, or more — the practical limit becomes patience, not precision.
- Per-pixel work remains in fast `f64`.
- The reference orbit is a small, bounded cost per frame.

#### Cons

- Significant implementation complexity (reference orbit, glitch detection, SA).
- New dependency for arbitrary-precision arithmetic.
- Julia sets benefit less (perturbation is possible but the reference orbit is less reusable since `z₀` varies per pixel, not `c`).

---

### Option 2: Double-Double Arithmetic (Emulated `f128`)

**Zoom ceiling: ~10^28 to 10^30×**

Uses a pair of `f64` values to represent a single number with ~31 decimal digits of precision, roughly doubling what `f64` provides.

#### How it works

A "double-double" number is stored as `(hi, lo)` where the value is `hi + lo` and `|lo| ≤ ε·|hi|`. Arithmetic operations use error-free transformations (Dekker/Knuth algorithms) to maintain the invariant. No external library needed — it's ~200 lines of Rust.

#### How to implement it in MandelbRust

1. **Create a `DoubleDouble` type** with `Add`, `Sub`, `Mul`, and comparison operators. This mirrors the existing `Complex` but uses `(f64, f64)` pairs.

2. **Create `ComplexDD`** wrapping two `DoubleDouble` components.

3. **Add a rendering mode** that switches the `Complex` type in the iteration loop. Since Rust doesn't have runtime type switching in the hot loop, you'd either:
   - Make `Complex` generic over a `Float` trait, or
   - Duplicate the iteration loop for the DD path (pragmatic; avoids generics overhead in the default `f64` path).

4. **Update `Viewport`**: store `center` as `DoubleDouble` values when zoom exceeds `f64` range.

#### Pros

- Pure Rust, no external dependencies.
- Relatively simple to implement (~200-300 LOC for the DD type).
- ~2× the zoom depth in terms of digits.
- Operations are still SIMD-friendly and vectorizable.

#### Cons

- Only doubles the zoom depth (10^13 → 10^28). Still finite and modest compared to perturbation theory.
- ~4-6× slower than native `f64` per operation (every multiply becomes ~10 `f64` ops).
- Every pixel pays the cost, unlike perturbation where only the reference is expensive.

---

### Option 3: Arbitrary-Precision Floating Point (Brute Force)

**Zoom ceiling: unlimited (but extremely slow)**

Replace `f64` with a software bigfloat type for *all* per-pixel computation.

#### How to implement it in MandelbRust

1. **Add a dependency** on `rug` (GMP/MPFR) or a pure-Rust alternative (`dashu`, `astro-float`).
2. **Parameterize the iteration loop** over a precision setting derived from `Viewport::scale`.
3. **Compute every pixel** in the chosen precision.

#### Pros

- Conceptually simple — the math is identical, just with wider numbers.
- Unlimited zoom.

#### Cons

- **Extremely slow**. At 1000-bit precision, each multiplication is ~100× slower than `f64`. At 10,000-bit, ~1000×. Renders that take 1 second in `f64` would take hours.
- This is why perturbation theory exists — it avoids per-pixel arbitrary precision.
- Only practical as the computation engine for the *reference orbit* in Option 1, not for per-pixel work.

---

### Option 4: `f128` / Quad Precision

**Zoom ceiling: ~10^30×**

Use 128-bit floating point, which provides ~33 significant decimal digits.

#### Current state in Rust

- Rust nightly has an experimental `f128` type (tracking issue [#116909](https://github.com/rust-lang/rust/issues/116909)), but it is not yet stable and platform support varies.
- The [`f128` crate](https://crates.io/crates/f128) wraps `__float128` from GCC's libquadmath, but has limited Windows support.
- The software-emulated approach (Option 2, double-double) gives similar precision with better portability.

#### How to implement it

Same approach as Option 2 but using a hardware or library `f128` type instead of double-double. The code changes would be nearly identical.

#### Pros

- More precise than double-double (~33 vs ~31 digits).
- If/when Rust stabilizes `f128` with hardware support, it could be faster than double-double on x86 (via SSE).

#### Cons

- Not stable in Rust yet; poor Windows support in existing crates.
- Same fundamental limitation as double-double: finite, modest improvement.
- Currently slower than double-double on most platforms because there's no hardware `f128` on x86.

---

### Option 5: Fixed-Point Arithmetic

**Zoom ceiling: configurable, proportional to bit width**

Use integers to represent fractional coordinates with a fixed binary point, scaling the number of bits to the required zoom depth.

#### How it works

Represent coordinates as, e.g., 128-bit or 256-bit signed integers where the binary point is at a fixed position. Multiplication requires widening to double the bit width and then truncating. Rust's `i128` gives you a native 128-bit path; beyond that, you'd use multi-word arithmetic.

#### How to implement it

1. Define a `FixedPoint<const BITS: usize>` type.
2. Implement complex arithmetic (`z² + c`) using integer ops.
3. The escape test (`|z|² > 4`) becomes an integer comparison.

#### Pros

- Deterministic, no floating-point quirks.
- Can be very cache-friendly (compact representation).
- 128-bit version uses native `i128` — no external dependency.

#### Cons

- Awkward to implement multiplication (widening multiply).
- Requires choosing bit width at compile time or using a dynamic multi-word type (which converges to Option 3).
- Not commonly used in Mandelbrot renderers; less community knowledge.
- Slower than perturbation theory for deep zooms.

---

### Option 6: GPU-Accelerated Perturbation

**Zoom ceiling: unlimited (same as Option 1, but faster)**

Move the perturbation delta iteration to the GPU while keeping the reference orbit on the CPU.

#### How it works

The reference orbit is computed once on the CPU in arbitrary precision and uploaded to the GPU as an `f64` (or `f32`) array. Each GPU thread iterates the perturbation delta `δ_n` for one pixel. Since deltas fit in `f64`, and the GPU has thousands of cores, this is massively parallel.

#### How to implement it

1. **Implement Option 1 first** (perturbation on CPU).
2. **Add a wgpu compute shader** that takes the reference orbit as a storage buffer and iterates deltas per pixel.
3. The shader would be written in WGSL. Note: WGSL only supports `f32` natively. Emulated `f64` in WGSL is possible (~4× slower than native `f32`) but still far faster than CPU due to parallelism. Alternatively, use Vulkan/SPIR-V with `shaderFloat64` for native `f64` on supported GPUs.

#### Pros

- Massive parallelism (thousands of pixels simultaneously).
- Real-time navigation at deep zoom levels.

#### Cons

- Large implementation effort (compute shaders, GPU buffer management).
- Depends on Option 1 being implemented first.
- `f64` on GPU is not universally supported; `f32`-only GPUs limit delta precision.
- Adds `wgpu` as a dependency (significant).

---

## Supporting Changes

Regardless of which precision option is chosen, several supporting changes help maximize zoom depth and quality:

### A. Store coordinates as decimal strings in bookmarks/preferences

At extreme zoom, `f64` cannot losslessly represent the center coordinate. Serializing coordinates as decimal strings (e.g. `"-0.7436438885706..."` with 50+ digits) avoids losing precision when saving and restoring deep-zoom positions.

**Where**: `Bookmark`, `LastView`, and `AppPreferences` serialization in `bookmarks.rs` and `preferences.rs`.

### B. Adaptive iteration count scaling

The current `ADAPTIVE_ITER_RATE` (30 extra iterations per zoom doubling) is reasonable for moderate zoom but may need tuning for extreme depths. At 10^50 zoom, the Mandelbrot boundary has detail at much higher iteration counts. A configurable or auto-tuned curve would help.

**Where**: `adaptive_max_iterations()` in `main.rs`.

### C. Periodicity detection threshold scaling

The hardcoded `1e-13` tolerance in Brent's cycle detection (`mandelbrot.rs:74`, `julia.rs:66`) should scale with the working precision. At double-double or higher precision, `1e-13` is far too loose and will falsely classify escaped points as interior.

**Where**: `mandelbrot.rs` and `julia.rs` — derive tolerance from the precision of the number type.

### D. Center-offset coordinate system

Even before implementing high-precision arithmetic, you can partially extend the zoom range by representing pixel coordinates as offsets from a high-precision center. The viewport center would be stored in higher precision, while per-pixel offsets (which are small) remain in `f64`. This is essentially "poor man's perturbation" and buys a few extra orders of magnitude.

**Where**: `Viewport::pixel_to_complex()` / `subpixel_to_complex()`.

---

## Recommended Implementation Order

| Phase | Technique | Zoom Ceiling | Effort |
|-------|-----------|-------------|--------|
| **1** | Double-double arithmetic (Option 2) | ~10^28× | Low–Medium |
| **2** | Perturbation theory (Option 1) | ~10^50+× | Medium–High |
| **3** | Series approximation (extension of Option 1) | 10^100+× | Medium |
| **4** | GPU perturbation (Option 6) | Same, but real-time | High |

**Phase 1** (double-double) gives a quick, self-contained win that roughly doubles the zoom depth. It requires no external dependencies and can be implemented in a few hundred lines.

**Phase 2** (perturbation theory) is the transformational change. It's more complex but is the only way to reach extreme depths without making every pixel expensive. Implemented on the CPU with Rayon, it leverages the existing tiled pipeline.

**Phase 3** (series approximation) is a natural extension of Phase 2 that dramatically reduces per-frame cost at very deep zooms by skipping early iterations.

**Phase 4** (GPU) is optional and depends on whether real-time deep-zoom navigation is a goal.

---

## Summary Table

| Technique | Max Zoom | Speed Impact | Complexity | Dependencies |
|-----------|----------|-------------|-----------|-------------|
| Current (`f64`) | ~10^13× | Baseline | — | — |
| Double-double | ~10^28× | ~4-6× slower | Low | None |
| Perturbation theory | 10^50+× | ~1.5-3× slower* | High | `rug` or `dashu` |
| Perturbation + SA | 10^100+× | ~1-2× slower* | High | `rug` or `dashu` |
| Arbitrary precision (brute) | Unlimited | 100-1000×+ slower | Low | `rug` or `dashu` |
| `f128` (when stable) | ~10^30× | ~5-10× slower | Low | Nightly Rust |
| Fixed-point (128-bit) | ~10^35× | ~3-5× slower | Medium | None |
| GPU perturbation | 10^100+× | Faster than CPU* | Very High | `wgpu` |

\* Per-pixel cost relative to current `f64`. Perturbation's total cost includes the reference orbit computation.
