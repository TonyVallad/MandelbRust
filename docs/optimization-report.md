# MandelbRust — Optimization & Improvement Report

A technical analysis of what could be improved, simplified, reorganized, or rebuilt for maximum performance and code quality — as if starting from scratch with everything we know now.

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Iteration Hot Loop](#2-iteration-hot-loop)
3. [SIMD Vectorization](#3-simd-vectorization)
4. [GPU Compute](#4-gpu-compute)
5. [Memory Layout & Allocation](#5-memory-layout--allocation)
6. [Rendering Pipeline](#6-rendering-pipeline)
7. [Coloring System](#7-coloring-system)
8. [Anti-Aliasing](#8-anti-aliasing)
9. [Deep Zoom — Perturbation Theory](#9-deep-zoom--perturbation-theory)
10. [Architecture & Code Organization](#10-architecture--code-organization)
11. [UI Thread & File I/O](#11-ui-thread--file-io)
12. [Bookmarks & Serialization](#12-bookmarks--serialization)
13. [Rust-Specific Improvements](#13-rust-specific-improvements)
14. [New Techniques & Algorithms](#14-new-techniques--algorithms)
15. [Priority Roadmap](#15-priority-roadmap)

---

## 1. Executive Summary

MandelbRust is well-structured: static dispatch for fractals, tiled parallel rendering, border tracing, symmetry mirroring, and sparse AA are all solid architectural choices. The codebase is clean, safe Rust with no `unsafe` blocks.

The largest gains — potentially **10–50x** — lie in three areas:

| Area | Estimated Speedup | Effort |
|---|---|---|
| SIMD vectorization of the iteration loop | 4–8x per core | Medium |
| GPU compute via wgpu | 50–200x for standard zooms | High |
| Perturbation theory for deep zoom | Enables zoom beyond 10^15 | High |

Medium gains (2–5x) come from:

| Area | Estimated Speedup | Effort |
|---|---|---|
| Parallel colorization | 2–4x on colorize pass | Low |
| Buffer pooling & reuse | ~1.5x (reduces GC/alloc pressure) | Low |
| `IterationResult` layout compaction | ~1.3x (cache efficiency) | Low |
| Precomputed viewport transform | ~1.1x (removes per-pixel division) | Low |

And there are architectural improvements that don't change raw speed but improve responsiveness, maintainability, and correctness:

- Moving all file I/O off the UI thread
- State machine for UI panels instead of boolean flags
- LRU thumbnail cache with bounded memory
- Histogram equalization coloring

---

## 2. Iteration Hot Loop

The iteration loop is the single most critical code path. Every pixel executes it, and deep zooms may run millions of iterations per pixel.

### Current Implementation

```rust
for n in 0..max_iter {
    z = Complex::new(
        z.re * z.re - z.im * z.im + c.re,
        2.0 * z.re * z.im + c.im,
    );
    let norm_sq = z.norm_sq();
    if norm_sq > escape_radius_sq { return Escaped { n, norm_sq }; }
    // Brent's cycle detection every iteration
    if (z.re - old_z.re).abs() < 1e-13 && (z.im - old_z.im).abs() < 1e-13 {
        return Interior;
    }
    period += 1;
    if period > check { old_z = z; period = 0; check = check.saturating_mul(2); }
}
```

### Improvements

**a) Cache `escape_radius_sq` at construction, not per-call.**
`FractalParams::escape_radius_sq()` recomputes `radius * radius` on every call. Store it as a field:

```rust
pub struct FractalParams {
    pub max_iterations: u32,
    pub escape_radius: f64,
    escape_radius_sq: f64,  // precomputed
}
```

This is a trivial win — one multiplication saved per `iterate()` call.

**b) Reduce periodicity check frequency.**
Currently, the comparison `(z.re - old_z.re).abs() < 1e-13` runs every iteration. For the first few hundred iterations, orbits rarely converge. A common trick: skip cycle detection for the first N iterations (e.g., 32), then check every K iterations (e.g., every 4th). This reduces branch overhead in the hot loop by ~75%.

```rust
for n in 0..max_iter {
    z = z * z + c;
    if z.norm_sq() > esc_sq { return Escaped { n, norm_sq }; }
    if n >= 32 && n & 3 == 0 {
        // cycle check only every 4th iteration after warmup
    }
}
```

**c) Use FMA (fused multiply-add).**
Modern CPUs have FMA instructions that compute `a * b + c` in a single operation with higher precision. Rust's `f64::mul_add` maps to hardware FMA when available:

```rust
let re_new = z.re.mul_add(z.re, -(z.im * z.im)) + c.re;
// equivalent to z.re*z.re - z.im*z.im + c.re but potentially faster and more precise
```

Whether this helps depends on the target CPU and LLVM's auto-vectorization. Worth benchmarking.

**d) Adaptive periodicity tolerance.**
The fixed tolerance `1e-13` is too strict at shallow zooms (wastes time on near-matches) and potentially too loose at extreme deep zooms. Scale tolerance with zoom level:

```rust
let tol = (1e-13 * scale_factor).max(1e-15);
```

---

## 3. SIMD Vectorization

This is the single biggest performance opportunity for CPU rendering. The Mandelbrot iteration is embarrassingly parallel at the pixel level — each pixel is independent.

### The Opportunity

With AVX2 (available on most x86-64 CPUs since 2013), you can process **4 complex numbers simultaneously** using 256-bit `f64x4` vectors. With AVX-512 (available on recent Intel/AMD), that doubles to 8.

Expected speedup: **4x** (AVX2) to **8x** (AVX-512) per core, multiplicative with Rayon's thread parallelism.

### How It Works

Instead of iterating one pixel at a time:

```
pixel_0: z = z² + c, check escape
pixel_1: z = z² + c, check escape
pixel_2: z = z² + c, check escape
pixel_3: z = z² + c, check escape
```

You iterate four pixels simultaneously:

```
[z0, z1, z2, z3] = [z0, z1, z2, z3]² + [c0, c1, c2, c3]
mask = [|z0|² > R, |z1|² > R, |z2|² > R, |z3|² > R]
if all(mask) { break }  // all escaped
```

The key subtlety: pixels escape at different iteration counts. You use a **mask** to track which lanes have escaped and continue iterating only the non-escaped lanes. When all lanes escape (or hit max_iter), you stop.

### Implementation Strategy

Since `std::simd` (portable SIMD) is still nightly-only, the practical options are:

1. **`std::arch` intrinsics** — stable, but x86-specific. Use `#[cfg(target_arch = "x86_64")]` with a scalar fallback.
2. **Auto-vectorization** — restructure the scalar code so LLVM can vectorize it. Process pixels in groups of 4, store `re` and `im` in separate arrays (SoA layout). This sometimes works but is fragile.
3. **`wide` crate** — stable, portable SIMD wrapper. Provides `f64x4` that maps to AVX2 or scalar fallback. Good compromise.

Recommended approach: use the `wide` crate for a first pass, then optionally add `std::arch` intrinsics for maximum control.

### Required Refactoring

The `Fractal` trait needs a batch method:

```rust
pub trait Fractal {
    fn iterate(&self, point: Complex) -> IterationResult;
    fn iterate_batch(&self, points: &[Complex], results: &mut [IterationResult]) {
        // default: scalar fallback
        for (p, r) in points.iter().zip(results.iter_mut()) {
            *r = self.iterate(*p);
        }
    }
}
```

The tile renderer would then call `iterate_batch` with groups of 4 (or 8) adjacent pixels.

---

## 4. GPU Compute

For maximum performance, the iteration loop should run on the GPU. A 1080p viewport has ~2 million pixels; a modern GPU has thousands of cores.

### Architecture with eframe/egui

`eframe` already uses `wgpu` as its rendering backend. The `egui-wgpu` crate exposes a `callback_resources` system that allows injecting custom GPU compute work into the rendering pipeline. The approach:

1. Write a WGSL compute shader that performs the Mandelbrot/Julia iteration
2. Upload viewport parameters as a uniform buffer
3. Dispatch the compute shader to fill an iteration buffer (GPU-side)
4. Run a second compute shader (or the same one) for colorization
5. Copy the result to a texture that egui can display

### Trade-offs

| Aspect | CPU (current) | GPU (wgpu) |
|---|---|---|
| Precision | Full `f64` | `f32` only on most GPUs (some support `f64` but slowly) |
| Deep zoom | Limited by `f64` (~10^15) | Limited by `f32` (~10^7) without perturbation |
| Parallelism | ~8–32 threads | ~2,000–10,000 threads |
| Iteration throughput | ~1 billion iter/sec | ~100 billion iter/sec |
| Complexity | Simple Rust code | WGSL shader + pipeline setup |

**Recommendation**: implement GPU compute as an optional backend. Use CPU for deep zooms (where `f64` precision matters) and GPU for interactive exploration at standard zoom levels. The existing architecture (clean separation between core/render/app) makes this feasible — the GPU backend would be a new `Renderer` implementation.

### Hybrid Approach

The most powerful approach: GPU for the initial render + CPU for deep-zoom refinement and AA. The GPU fills the screen in milliseconds, and the CPU refines boundary pixels at full `f64` precision.

---

## 5. Memory Layout & Allocation

### `IterationResult` Compaction

The current enum:

```rust
pub enum IterationResult {
    Escaped { iterations: u32, norm_sq: f64 },  // 12 bytes data
    Interior,                                     // 0 bytes data
}
```

Due to alignment, this is **16 bytes** per pixel. For a 1920x1080 viewport, that's ~32 MB for the iteration buffer alone.

Alternative: use a flat struct with a sentinel value:

```rust
#[repr(C)]
pub struct IterationData {
    pub iterations: u32,  // u32::MAX = interior
    pub norm_sq: f32,     // f32 is sufficient for smooth coloring (only used in log/ln)
}
// 8 bytes per pixel — 50% reduction
```

Using `f32` for `norm_sq` is safe because it's only used in the smooth coloring formula `log₂(ln(|z|))`, where the precision of the input doesn't significantly affect the visual result. The iteration count remains `u32` (full precision). This halves the iteration buffer from ~32 MB to ~16 MB, improving cache utilization.

If `f64` precision is needed for `norm_sq` (e.g., for future coloring techniques), use:

```rust
#[repr(C)]
pub struct IterationData {
    pub norm_sq: f64,     // 8 bytes
    pub iterations: u32,  // 4 bytes (u32::MAX = interior)
    _pad: u32,            // explicit padding
}
// 16 bytes — same size, but no enum discriminant overhead and better cache behavior
```

### Buffer Pooling

Currently, every render allocates fresh `Vec`s:
- One `Vec<IterationResult>` per tile (~32 KB each)
- One `Vec<u8>` for the pixel buffer (~8 MB at 1080p)
- One `Vec<IterationResult>` for the full iteration buffer (~32 MB)
- Intermediate `Vec<Vec<IterationResult>>` in AA

Since renders happen continuously during exploration, these allocations create significant pressure on the allocator. A buffer pool that reuses allocations across renders would reduce this:

```rust
struct BufferPool {
    tile_buffers: Vec<Vec<IterationResult>>,  // reusable per-tile buffers
    pixel_buffer: Vec<u8>,                     // reusable pixel output
}
```

Rayon's `par_iter` makes this slightly tricky (each thread needs its own buffer), but `thread_local!` or Rayon's `ThreadLocal` from the `thread_local` crate handles this cleanly.

### SoA (Structure-of-Arrays) for SIMD

For SIMD iteration, a SoA layout is far more efficient than the current AoS (Array-of-Structures):

```rust
// Current (AoS): [Complex, Complex, Complex, ...]
// Each Complex is {re: f64, im: f64}

// Better (SoA): 
struct ComplexBatch {
    re: Vec<f64>,  // [re0, re1, re2, ...]
    im: Vec<f64>,  // [im0, im1, im2, ...]
}
```

SoA allows loading 4 consecutive `re` values into one SIMD register and 4 `im` values into another, enabling full-width vector operations without gather/scatter.

---

## 6. Rendering Pipeline

### Symmetry Matching — O(n²) to O(1)

`find_tile_at()` in `tile.rs` performs a linear search over all tiles to find a mirror partner:

```rust
fn find_tile_at(tiles: &[Tile], x: u32, y: u32, w: u32, h: u32) -> Option<usize> {
    tiles.iter().position(|t| t.x == x && t.y == y && t.width == w && t.height == h)
}
```

This is called once per primary tile, making the total cost O(n²) where n is the number of tiles. For a 4K viewport, there are ~2,000 tiles.

Fix: build a `HashMap<(u32, u32), usize>` mapping `(x, y)` to tile index before the classification pass. This makes each lookup O(1).

### Tile Size Tuning

The current 64x64 tile size (32 KB at `f64`) is a good default. However, the optimal size depends on:
- L1 cache size (typically 32–48 KB per core)
- Iteration depth (deeper zooms mean more computation per pixel, making tile overhead negligible)
- SIMD width (tile width should be a multiple of the SIMD lane count)

Consider making tile size configurable or adaptive. For SIMD with AVX2 (4 lanes), tile widths of 64 or 128 work well. For GPU compute, much larger tiles (256x256 or even full-screen) are better.

### Parallel Colorization

`palette.rs` colorizes sequentially:

```rust
pub fn colorize(&self, iter_buf: &IterationBuffer, smooth: bool) -> Vec<u8> {
    let mut pixels = vec![0u8; len * 4];
    for idx in 0..len {
        let color = self.color(&iter_buf.data[idx], smooth);
        pixels[base..base+4].copy_from_slice(&color);
    }
    pixels
}
```

This should use `par_chunks` for parallel colorization:

```rust
pub fn colorize(&self, iter_buf: &IterationBuffer, smooth: bool) -> Vec<u8> {
    let mut pixels = vec![0u8; len * 4];
    pixels.par_chunks_mut(4)
        .zip(iter_buf.data.par_iter())
        .for_each(|(pixel, result)| {
            let color = self.color(result, smooth);
            pixel.copy_from_slice(&color);
        });
    pixels
}
```

For a 1080p image, this gives a ~2–4x speedup on the colorization pass (which is currently sequential and can take 5–15ms).

### Cancellation Granularity

Currently, cancellation is checked once per tile (or once per AA pixel). If a tile takes a long time (e.g., deep zoom with high max_iter), the render thread won't notice cancellation until the tile finishes. Adding a check every N iterations inside the hot loop would improve responsiveness:

```rust
for n in 0..max_iter {
    // ... iteration ...
    if n & 0xFF == 0 && cancel.is_cancelled(gen) { return Interior; }
}
```

The `& 0xFF` mask checks every 256 iterations (essentially free — a single AND + branch-not-taken).

---

## 7. Coloring System

### Histogram Equalization

The current smooth coloring formula distributes colors based on iteration count, but this often wastes most of the palette on a narrow band of iteration values while leaving large uniform areas with nearly identical colors.

Histogram equalization solves this by redistributing colors so that each color in the palette covers an equal number of pixels:

1. After rendering, build a histogram of smooth iteration values
2. Compute the cumulative distribution function (CDF)
3. Map each pixel's iteration value through the CDF to get an evenly distributed color index

This dramatically improves visual quality at any zoom level and is especially effective for deep zooms where iteration counts span a huge range.

Implementation cost: one extra pass over the iteration buffer (O(n) — negligible compared to rendering). The iteration buffer already stores everything needed.

### Distance Estimation Coloring

The distance estimator computes approximate distance from a point to the Mandelbrot set boundary. During iteration, track the derivative alongside `z`:

```
z_{n+1} = z_n² + c
dz_{n+1} = 2·z_n·dz_n + 1
distance ≈ |z| · ln|z| / |dz|
```

This requires storing one additional `Complex` per pixel during iteration (the derivative `dz`). The payoff:
- Filament structures become visible that escape-time misses entirely
- Combined with smooth coloring, it produces the highest-quality fractal images
- Can be used for adaptive AA — supersample only where distance is small (near boundary)

### Orbit Trap Coloring

Orbit traps measure how close the orbit comes to a geometric shape (point, line, circle) during iteration. This produces striking visual effects:
- **Point trap**: colors based on minimum distance to the origin
- **Cross trap** (Pickover stalks): two perpendicular lines produce stalk-like structures
- **Circle trap**: measures proximity to a circle of given radius

Implementation: track minimum distance during the iteration loop:

```rust
let mut min_dist = f64::MAX;
for n in 0..max_iter {
    z = z * z + c;
    min_dist = min_dist.min(z.im.abs());  // line trap at im=0
    // ...
}
```

This adds minimal cost to the iteration loop (one comparison per iteration) and produces a new coloring dimension.

---

## 8. Anti-Aliasing

### Current Approach

The adaptive AA implementation is solid — detecting boundaries and supersampling only edge pixels is the right strategy. Some improvements:

### Distance-Estimation-Guided AA

Instead of comparing iteration classes between neighbors (which misses smooth transitions), use the distance estimator to identify pixels near the set boundary. Pixels with small distance values are inherently more likely to need AA:

```rust
let needs_aa = distance_estimate < threshold * scale;
```

This is more accurate than neighbor comparison and avoids the 8-neighbor scan.

### Stratified Sampling

The current sub-pixel sampling uses a regular grid (2x2 or 4x4). Stratified (jittered) sampling produces better results with fewer samples:

```rust
// Instead of fixed grid:
let sx = (i as f64 + 0.5) / aa_level as f64;
// Use jittered positions:
let sx = (i as f64 + rng.gen::<f64>()) / aa_level as f64;
```

This breaks up aliasing patterns and can achieve 4x4 quality with 3x3 samples (44% fewer iterations).

### Progressive AA

Instead of computing all AA samples at once, compute them progressively:
1. First pass: 2x2 samples for all boundary pixels
2. If the user hasn't moved, refine to 4x4
3. Continue to 8x8 for the sharpest possible image

This gives quick initial AA and progressively improves while the user waits.

---

## 9. Deep Zoom — Perturbation Theory

This is the gateway to extreme zoom depths (10^15 and beyond). Without it, `f64` precision creates visible artifacts.

### How It Works

1. Pick a reference point (typically the viewport center)
2. Compute its full orbit `z_0, z_1, ..., z_N` at **arbitrary precision** (using a big-number library)
3. For every other pixel, compute only the **delta** from the reference orbit using standard `f64`:
   ```
   δ_{n+1} = 2·z_n·δ_n + δ_n² + δc
   ```
   where `δ_n = Z_n - z_n` is the difference between the pixel's orbit and the reference orbit

The key insight: even at zoom 10^100, the difference between neighboring pixels is tiny — small enough for `f64`.

### Series Approximation

For even more speed, compute the first K iterations analytically using a Taylor series:

```
δ_K = A_K·δc + B_K·δc² + C_K·δc³ + ...
```

The coefficients `A_K, B_K, C_K` are computed once from the reference orbit. This skips K iterations for every pixel — often thousands of iterations can be skipped.

### Implementation in MandelbRust

The current architecture supports this cleanly:
1. Add a `PerturbationFractal` that wraps a reference orbit + delta iteration
2. The reference orbit is computed once (arbitrary precision) on a background thread
3. The tile renderer calls `iterate_delta()` instead of `iterate()` for each pixel
4. Falls back to full precision when the delta becomes unreliable (glitch detection)

**Dependency**: a big-number library. `rug` (GMP wrapper) is the fastest but requires C linkage. `dashu` is pure Rust but slower. For initial implementation, `rug` is recommended.

---

## 10. Architecture & Code Organization

### State Machine for Render Pipeline

The render pipeline uses a mix of enums (`RenderPhase`), booleans (`needs_render`, `pan_completed`), and IDs (`render_id`, `skip_preview_id`). This works but is fragile — certain state combinations are invalid but not enforced by the type system.

A cleaner approach: encode the entire render state as a single enum:

```rust
enum RenderState {
    Idle,
    WaitingForPreview { request_id: u64 },
    PreviewReceived { request_id: u64, preview: TextureHandle },
    WaitingForFull { request_id: u64, preview: TextureHandle },
    Complete { texture: TextureHandle, iterations: IterationBuffer, aa: Option<AaSamples> },
    Panning { base_texture: TextureHandle, offset: Vec2 },
}
```

Invalid states become unrepresentable. Transitions are explicit match arms.

### UI Panel State

The current 13+ boolean flags (`show_hud`, `show_controls`, `show_palette_popup`, `show_help`, `show_bookmarks`, `show_save_dialog`, `show_update_or_save_dialog`, `favorites_only`, `drag_active`, `adaptive_iterations`, `pan_completed`, `show_crosshair`, `smooth_coloring`) make the state space enormous (2^13 = 8,192 combinations, most invalid).

For mutually exclusive panels, use an enum:

```rust
enum ActivePanel {
    None,
    Settings,
    Help,
    BookmarkExplorer,
    SaveDialog,
    UpdateOrSaveDialog,
}
```

Independent toggles (`show_hud`, `show_crosshair`, `smooth_coloring`, `adaptive_iterations`) remain as booleans but could be collected into a `DisplaySettings` struct.

### Separate Render Orchestration from UI

`main.rs` currently handles both render orchestration and all UI drawing. Extracting the render pipeline into its own module would improve testability:

```
mandelbrust-app/
  src/
    main.rs          # entry point, eframe setup
    app.rs           # MandelbRustApp: update loop, input handling
    render_bridge.rs # render thread communication, request/response
    ui/
      toolbar.rs     # top-right icon toolbar
      hud.rs         # four-corner HUD overlay
      bookmarks.rs   # bookmark explorer window
      dialogs.rs     # save/update dialogs
      settings.rs    # settings panel
      params.rs      # fractal parameters panel
```

Each UI module would take `&mut AppState` and draw its section. This makes individual panels testable and keeps `main.rs` focused on orchestration.

---

## 11. UI Thread & File I/O

### Current Problem

All file I/O is synchronous on the UI thread:
- `BookmarkStore::reload()` scans the bookmarks directory and reads every `.json` file
- `BookmarkStore::add()` / `remove()` / `update_viewport()` write/delete files
- `encode_thumbnail()` performs PNG encoding + base64 encoding
- `decode_thumbnail()` performs base64 decoding + PNG decoding
- `preferences.save()` writes preferences

With many bookmarks or slow storage (network drive, USB), these operations can cause visible UI freezes.

### Solution: Offload to a Dedicated I/O Thread

Spawn a lightweight I/O worker thread (or use `tokio::spawn_blocking`):

```rust
enum IoRequest {
    ReloadBookmarks,
    SaveBookmark(Bookmark),
    DeleteBookmark(PathBuf),
    SavePreferences(AppPreferences),
    EncodeThumbnail { pixels: Vec<u8>, width: u32, height: u32 },
}

enum IoResponse {
    BookmarksLoaded(Vec<Bookmark>),
    BookmarkSaved(Result<()>),
    ThumbnailEncoded(Result<String>),  // base64 string
}
```

The UI thread sends requests and polls for responses (same pattern as the render thread). This guarantees the UI never blocks on I/O.

### Thumbnail Decoding

Move thumbnail decoding to the I/O thread as well. The UI shows a placeholder (or the bookmark card without an image) until the decoded texture arrives. This is especially important when scrolling through many bookmarks.

### LRU Thumbnail Cache

Replace the unbounded `HashMap<usize, TextureHandle>` with an LRU cache capped at a reasonable size (e.g., 50 entries, ~50 MB of textures):

```rust
struct ThumbnailCache {
    entries: VecDeque<(BookmarkId, TextureHandle)>,
    max_size: usize,
}
```

Evict the least-recently-used entry when the cache is full. Use a stable bookmark ID (e.g., the filename hash) instead of a positional index, which changes on sort/delete.

---

## 12. Bookmarks & Serialization

### Stable Bookmark IDs

The current system uses positional indices as bookmark identifiers. This causes cache invalidation issues (already fixed with full cache clears) and makes cross-reference fragile.

Give each bookmark a stable UUID:

```rust
pub struct Bookmark {
    pub id: uuid::Uuid,  // stable across sorts, renames, moves
    // ... other fields
}
```

The thumbnail cache, "last jumped" bookmark, and any future cross-references would use this ID instead of a position.

### Lazy Thumbnail Loading

Currently, `BookmarkStore::load()` reads every bookmark file (including the base64 thumbnail) into memory. For a large collection (hundreds of bookmarks), this loads megabytes of base64 data that won't be displayed until the user scrolls to it.

Alternative: load bookmark metadata (name, labels, date, fractal type) eagerly, but load the thumbnail field **lazily** — only when the bookmark card is about to be displayed.

This could be done by splitting the JSON or by reading the file twice (first a quick scan for metadata, second for the thumbnail). Or store thumbnails as separate files again but alongside the JSON (e.g., `bookmark-name.json` + `bookmark-name.thumb.png`), which also shrinks the JSON and makes it easier to read by humans.

### Binary Format Option

JSON with embedded base64 PNGs is human-readable but inefficient:
- base64 encoding adds ~33% overhead
- JSON parsing is slower than binary deserialization

For users with large collections, offer an optional binary format (e.g., MessagePack or bincode) with the PNG embedded directly (no base64 overhead). Keep JSON as the default for shareability.

---

## 13. Rust-Specific Improvements

### Edition 2024

The workspace uses `edition = "2021"`. Rust edition 2024 (stable since Rust 1.85, February 2025) brings:
- `gen` blocks for iterators (useful for progressive rendering)
- Improved `unsafe` diagnostics
- Better lifetime elision in closures
- `#[diagnostic::on_unimplemented]` for better trait error messages

### `#[repr(C)]` for Hot Data

Adding `#[repr(C)]` to `Complex`, `IterationData`, and `Viewport` guarantees a predictable memory layout. This is important for:
- SIMD (predictable offsets for vector loads)
- GPU (matching WGSL struct layout)
- Cache prefetching (predictable stride)

### Profile-Guided Optimization (PGO)

Rust supports PGO via LLVM. The workflow:
1. Build with instrumentation: `RUSTFLAGS="-Cprofile-generate=/tmp/pgo"` cargo build
2. Run a representative workload (render several bookmarks at various zoom levels)
3. Rebuild with profile data: `RUSTFLAGS="-Cprofile-use=/tmp/pgo/merged.profdata"` cargo build

Typical gain: 5–15% across the board, with no code changes. The compiler optimizes branch prediction, inlining, and code layout based on actual execution patterns.

### Link-Time Optimization (LTO)

Add to `Cargo.toml`:

```toml
[profile.release]
lto = "fat"
codegen-units = 1
```

`lto = "fat"` enables cross-crate inlining (critical for the `Fractal::iterate` → `palette.color` → `viewport.pixel_to_complex` chain). `codegen-units = 1` gives LLVM the maximum optimization scope. Together, these can yield 10–20% speedup at the cost of longer compile times.

### Target-Specific Compilation

```toml
[build]
rustflags = ["-C", "target-cpu=native"]
```

This enables AVX2/AVX-512 and other CPU-specific features. For distributed binaries, use `target-cpu=x86-64-v3` (AVX2 baseline, covers ~95% of modern PCs).

---

## 14. New Techniques & Algorithms

### Buddhabrot / Nebulabrot

A fundamentally different rendering mode: instead of coloring pixels by escape time, count how many escape trajectories pass through each pixel. This produces density maps that reveal structures invisible to standard rendering.

- **Buddhabrot**: grayscale density map from escaping orbits
- **Nebulabrot**: color version using three different iteration limits for R, G, B channels

This doesn't require architectural changes — it's a new rendering mode alongside Mandelbrot and Julia. It does require:
- Random sampling of the complex plane (not grid-based)
- An accumulation buffer (atomic increment per pixel visit)
- Long render times (millions of random samples needed)

### Interior Detection via Multiplier

For points inside the Mandelbrot set, the current code runs to `max_iter` (even with periodicity detection, it still iterates significantly). A faster approach: detect the period of the attracting cycle and compute the orbit's multiplier. If `|multiplier| < 1`, the point is interior.

This is a well-known technique in the fractal community and can reduce interior detection time by orders of magnitude at high iteration counts.

### Mariani-Silver Algorithm

An alternative to tiled border tracing: recursively subdivide the viewport. If all four corners of a rectangle have the same iteration count, fill the rectangle. Otherwise, subdivide into 4 quadrants and recurse.

This is more aggressive than per-tile border tracing and can skip large interior regions entirely. The risk: it can miss thin filaments that pass through a rectangle without touching its corners. Mitigation: add midpoint checks on each edge.

### Stripe Average Coloring

A technique that produces smooth, banding-free coloring for interior points (which currently render as solid black):

```rust
let stripe = 0.5 * (1.0 + (z.im.atan2(z.re) * stripe_density).sin());
```

This reveals the internal structure of the Mandelbrot set, making interior regions visually interesting instead of uniformly black.

---

## 15. Priority Roadmap

If rebuilding for maximum efficiency, here's the recommended order:

### Phase A — Quick Wins (1–2 days each)

| # | Change | Speedup | Effort |
|---|---|---|---|
| A1 | Cache `escape_radius_sq` in `FractalParams` | ~1% | Trivial |
| A2 | Add `lto = "fat"` + `codegen-units = 1` to release profile | 10–20% | Trivial |
| A3 | Compile with `target-cpu=native` | 5–15% | Trivial |
| A4 | Parallelize `colorize()` and `colorize_aa()` with Rayon | 2–4x on color pass | Low |
| A5 | Replace `find_tile_at` linear search with `HashMap` | Negligible at 1080p, visible at 4K | Low |
| A6 | Reduce periodicity check frequency | 5–10% on deep zooms | Low |

### Phase B — Medium Effort, High Impact (1–2 weeks each)

| # | Change | Speedup | Effort |
|---|---|---|---|
| B1 | SIMD iteration (via `wide` crate or `std::arch`) | 4–8x per core | Medium |
| B2 | Histogram equalization coloring | Visual quality (not speed) | Medium |
| B3 | Buffer pooling (tile buffers, pixel buffers) | ~1.5x allocation reduction | Medium |
| B4 | Move all file I/O off UI thread | Responsiveness (not speed) | Medium |
| B5 | `IterationResult` compaction to 8 bytes | ~1.3x cache efficiency | Medium |
| B6 | Distance estimation coloring | Visual quality + smarter AA | Medium |

### Phase C — Large Features (1–2 months each)

| # | Change | Speedup | Effort |
|---|---|---|---|
| C1 | GPU compute via wgpu | 50–200x at standard zoom | High |
| C2 | Perturbation theory for deep zoom | Enables zoom beyond 10^15 | High |
| C3 | UI architecture refactor (state machines, module split) | Maintainability | High |
| C4 | Buddhabrot / Nebulabrot rendering mode | New capability | Medium-High |
| C5 | Series approximation (with perturbation) | 10–100x for deep zoom | High |

### Cumulative Impact Estimate

| Configuration | Relative Speed |
|---|---|
| Current (CPU, scalar, no LTO) | **1x** |
| + LTO + native CPU + parallel colorize | **~2x** |
| + SIMD (AVX2) | **~8–12x** |
| + GPU compute (standard zoom) | **~100–300x** |
| + Perturbation (deep zoom only) | Unlocks 10^15+ zoom |

---

## Final Notes

The current codebase is well-designed and clean. The biggest mistake would be premature optimization of the wrong things — the rendering hot loop and memory layout are where 95% of the time is spent. Focus there first.

The architectural improvements (state machines, UI thread I/O, module split) don't improve raw speed but prevent bugs and make the high-impact optimizations (SIMD, GPU, perturbation) easier to implement correctly.

The recommended first step is Phase A (quick wins) — these are near-zero-risk changes that compound: LTO + native CPU + parallel colorize can double overall throughput with a few lines of config changes.
