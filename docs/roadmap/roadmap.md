# MandelbRust — Roadmap

**Next development focus: Phase 17 — HUD Modifications.**

Each phase is a self-contained unit of work that produces a testable, working state.

> **Rules for every phase**
>
> - All work **must respect** [overview.md](../overview.md). If a change alters the project's scope or architecture, update overview.md first and ask for confirmation.
> - Run `cargo clippy --workspace` after every task. Zero warnings.
> - Run `cargo test --workspace` after every task. All tests pass.
> - Keep functions small and pure. Prefer early returns over nesting. Add type hints on public functions. Add docstrings only when behaviour is not obvious.
> - Only modify files directly relevant to the task. Summarise multi-file changes and ask before implementing.
> - Before adding a dependency, justify it and ask for approval.
> - **Keep files from getting too long.** When a file would grow large, split it into smaller modules or extract logic into new files. Prefer many focused files over a few very long ones.
> - **No backwards compatibility.** Never add legacy shims, fallback deserialization, or dead code to support old formats. When a data format changes (structs, serialization, file layout), replace it cleanly. The only exception is **bookmarks**: when a change would break existing bookmark files, document a migration procedure (a script, CLI command, or step-by-step instructions) so existing bookmarks can be converted to the new format — but do not keep old-format support in the application code itself.
> - **When a phase is completed:**
>   1. Remove the completed phase from the Phase Overview table in this file.
>   2. Move the concise summary of what was implemented to [roadmap-completed.md](roadmap-completed.md) and remove the detailed tasks from this file.
>   3. Create a dedicated `phase-<N>.md` file in this folder (e.g. `phase-11.md`) documenting **what** was implemented and **how** — covering design decisions, key algorithms, new/modified files, API surface, and anything a future contributor would need to understand the work.
>   4. Update project documentation (README, overview, etc.) to reflect the new capabilities. Don't reference phase numbers in user-facing documents like the README.

---

## Phase Overview

| Phase | Description |
|-------|-------------|
| 17 | HUD Modifications |
| 18 | Minimap Size Controls |
| 19 | Deep Zoom: Perturbation Theory |
| 20 | Deep Zoom: Series Approximation |
| 21 | Memory Layout & Buffer Management |
| 22 | SIMD Vectorization |
| 23 | Animation & Video Export |
| 24 | GPU Compute Backend |
| 25 | Polish & v1.0 Release |

Feature specifications for Phases 17–18: [Features_to_add.md](../Features_to_add.md).

Deep zoom background and analysis: [deep-zoom-analysis.md](../deep-zoom-analysis.md).

---

## Phase 17 — HUD Modifications

**Objective:** Rework the top-left and bottom-left HUD panels for clearer display, editable fields, and a simplified iterations/escape-radius block.

**Reference:** [Features_to_add.md](../Features_to_add.md) §4.

### Task 17.1 — Top-left HUD rework

**Files:** `mandelbrust-app/src/ui/hud.rs`

1. **Fractal name**: replace "Mode: …" with just the fractal name (e.g. "Mandelbrot" or "Julia"), **centered horizontally**, in **cyan**.
2. **Coordinates**: display real and imaginary on **two separate lines**.
3. **Editable coordinates and zoom**: make the coordinate and zoom fields editable text inputs. Pressing Enter or losing focus applies the new value and triggers a re-render.
4. **Julia C coordinates**: in Julia mode, display C coordinates in the same block and make them editable.
5. **Iterations display**: show the **actual** iteration count when using adaptive iterations. Format with **thousands separators** (e.g. `1.000.000`).

**Verify:** Fields are editable. Typing a new coordinate and pressing Enter navigates the view. Thousands separators display correctly.

---

### Task 17.2 — Bottom-left HUD rework

**Files:** `mandelbrust-app/src/ui/params.rs`

1. **Remove** the iterations slider. Keep only the numeric input for max iterations.
2. Allow input values **up to 1.000.000** (or a configurable limit set in the Settings menu).
3. **Remove** the "×10" and "/10" buttons.
4. Move the **Escape R** slider **below** all iterations-related controls.

**Verify:** Iterations input accepts large values. No slider or ×10/÷10 buttons. Escape radius slider is at the bottom of the panel.

---

### Deliverables — Phase 17

- [ ] Fractal name centred in cyan, no "Mode:" prefix
- [ ] Coordinates on separate lines, editable
- [ ] Zoom editable
- [ ] Julia C coordinates displayed and editable
- [ ] Iterations show actual count with thousands separators
- [ ] Iterations slider and ×10/÷10 buttons removed
- [ ] Max iterations limit configurable in Settings
- [ ] Escape R slider below iterations controls

---

## Phase 18 — Minimap Size Controls

**Objective:** Allow the user to change minimap size from the UI and keyboard, complementing the existing settings menu option.

**Reference:** [Features_to_add.md](../Features_to_add.md) §2.

### Task 18.1 — Add +/− buttons on the minimap

**Files:** `mandelbrust-app/src/ui/minimap.rs`

1. Draw **"−"** and **"+"** buttons on or next to the minimap panel.
2. Clicking them cycles through the size options (small → medium → large and back, or continuous scaling — whichever fits the existing `MinimapSize` model).
3. Persist the new size to preferences.

**Verify:** Clicking + increases size, − decreases. Size persists across restarts.

---

### Task 18.2 — Page Up / Page Down keyboard shortcuts

**Files:** `mandelbrust-app/src/input.rs`

1. **Page Up**: increase minimap size.
2. **Page Down**: decrease minimap size.
3. Same behaviour as the +/− buttons.

**Verify:** Page Up / Page Down change the minimap size. Update the help/shortcuts reference.

---

### Deliverables — Phase 18

- [ ] +/− buttons on the minimap
- [ ] Page Up / Page Down change minimap size
- [ ] Preferences updated on change
- [ ] Keyboard shortcuts documented

---

## Phase 19 — Deep Zoom: Perturbation Theory

**Objective:** Enable zoom depths of 10^50+ by computing a single arbitrary-precision reference orbit and iterating per-pixel `f64` deltas. This is the transformational change for deep zoom.

**Reference:** [deep-zoom-analysis.md](../deep-zoom-analysis.md), Option 1.

### Task 19.1 — Add arbitrary-precision dependency

**Files:** `mandelbrust-core/Cargo.toml`, new file `mandelbrust-core/src/arb.rs`

1. **Choose and add a library.** Preferred: [`dashu`](https://crates.io/crates/dashu) (pure Rust, easy Windows builds) or [`rug`](https://crates.io/crates/rug) (faster, requires GMP). Ask for approval before adding.
2. **Create a thin wrapper module** `arb.rs` that defines:
   - `type ArbFloat = ...` (the chosen library's float type).
   - `struct ComplexArb { pub re: ArbFloat, pub im: ArbFloat }` with `Add`, `Sub`, `Mul`, `norm_sq()`.
   - `fn required_precision_bits(scale: f64) -> u32` — computes the working precision from the viewport scale (rule of thumb: `bits ≈ -log2(scale) + 64`).
3. This module is the **only place** the arbitrary-precision crate is imported. All other code uses it through `ComplexArb`.

**Verify:** Unit tests for `ComplexArb` arithmetic at 256-bit and 1024-bit precision.

---

### Task 19.2 — Compute reference orbit

**File:** new file `mandelbrust-core/src/perturbation.rs`

1. Implement `fn compute_reference_orbit(center: &ComplexArb, max_iter: u32, escape_radius: f64, precision_bits: u32) -> ReferenceOrbit`.
2. `ReferenceOrbit` stores:
   - `orbit: Vec<Complex>` — the reference orbit points downcast to `f64` (needed for the delta recurrence).
   - `orbit_len: u32` — number of iterations before the reference escaped (or `max_iter` if it didn't).
   - `escape_iteration: Option<u32>` — `None` if the reference point is interior.
3. The computation uses `ComplexArb` at the specified precision. Each step `Z_{n+1} = Z_n² + C` is computed in arbitrary precision; the result is then truncated to `f64` and pushed to the orbit vector.
4. This function is serial (single-threaded) and may take seconds at very high precision. Design for cancellation: accept a `&AtomicBool` and check it periodically.

**Verify:** The reference orbit for center `(-0.75, 0.0)` at 256-bit precision matches the first N iterations computed by `Mandelbrot::iterate()` in `f64` (within `f64` tolerance). Test at zoom 10^20 and 10^40 with known coordinates from other renderers.

---

### Task 19.3 — Delta iteration (perturbation per-pixel)

**File:** `mandelbrust-core/src/perturbation.rs`

1. Implement `fn iterate_perturbed(ref_orbit: &ReferenceOrbit, delta_c: Complex, max_iter: u32, escape_radius_sq: f64) -> IterationResult`.
2. The delta recurrence: `δ_{n+1} = 2·Z_n·δ_n + δ_n² + δc`, where `Z_n` comes from the reference orbit and `δ_n`, `δc` are `f64`.
3. Escape check: `|Z_n + δ_n|² > escape_radius²`. Expand using `|Z_n|² + 2·Re(Z_n·conj(δ_n)) + |δ_n|²`.
4. If the reference orbit escapes before the pixel, handle gracefully (the pixel may still be iterating; this is a "rebasing" scenario — for now, fall back to marking the pixel for a secondary reference orbit in Task 19.4).
5. Return `IterationResult::Escaped { iterations, norm_sq }` or `IterationResult::Interior`.

**Verify:** A small test image at zoom 10^20 rendered via perturbation matches a brute-force arbitrary-precision reference (at tiny resolution, e.g. 16×16).

---

### Task 19.4 — Glitch detection and rebasing

**File:** `mandelbrust-core/src/perturbation.rs`

1. **Glitch detection**: during delta iteration, when `|δ_n|` becomes large relative to `|Z_n|` (e.g. `|δ_n|² > 1e-6 · |Z_n|²`), mark the pixel as "glitched."
2. **First pass**: render all pixels using the primary reference orbit. Collect the set of glitched pixels.
3. **Rebase / secondary reference**: pick a glitched pixel, compute a new reference orbit centred at that pixel's `c`, and re-render only the glitched pixels using the new reference.
4. Repeat until no glitches remain (in practice, 1–3 rebase passes suffice).
5. Wrap this into a `fn render_perturbed(...)` function that orchestrates the multi-pass rendering.

**Verify:** A test image at zoom 10^25 near a Misiurewicz point (known to produce glitches) renders without visible artifacts.

---

### Task 19.5 — Integrate perturbation into the render pipeline

**Files:** `mandelbrust-render/src/renderer.rs`, `mandelbrust-app/src/render_bridge.rs`

1. **Auto-detection**: when `viewport.scale < 1e-13`, engage the perturbation path instead of (or in addition to) the double-double path. The perturbation path is preferred at extreme depths because it keeps per-pixel work in `f64`.
2. **Render flow**:
   - Compute the reference orbit on the render thread (before tile dispatch). Show "Computing reference orbit…" in the HUD.
   - Dispatch tiles via Rayon. Each tile calls `iterate_perturbed()` per pixel with the shared reference orbit.
   - After the first pass, collect glitched tiles and re-render them with secondary references.
3. **Precision selection**: the precision bits for the reference orbit are derived from the viewport scale (`arb::required_precision_bits()`).
4. **Interaction with Phase 11 (DD)**: the double-double path becomes unnecessary once perturbation is active (since perturbation keeps per-pixel work in `f64`). DD can remain as a fallback for Julia sets (where perturbation is less effective) or for moderate zoom depths (10^13–10^15) where the reference orbit overhead isn't worth it.
5. **HUD indicator**: show "Perturbation (N-bit reference)" when active.

**Verify:** Zoom smoothly from f64 → DD → perturbation. Render times at 10^20+ zoom are comparable to f64 render times at moderate zoom (within 2–3×). No artifacts at transition points.

---

### Task 19.6 — Adaptive iteration scaling for deep zoom

**Files:** `mandelbrust-app/src/app.rs` (or relevant input/render module)

1. Review and tune `ADAPTIVE_ITER_RATE` for deep zoom. At 10^50 zoom, the Mandelbrot boundary requires much higher iteration counts.
2. Consider a two-segment curve: the current `30` iterations per zoom doubling up to 10^10, then a steeper rate (e.g. 50–80 per doubling) beyond that.
3. Make the adaptive rate configurable in settings.

**Verify:** Deep-zoom renders at 10^30+ show sufficient detail without requiring the user to manually set iteration counts.

---

### Deliverables — Phase 19

- [ ] Arbitrary-precision wrapper (`ComplexArb`) in `mandelbrust-core`
- [ ] Reference orbit computation with cancellation support
- [ ] Delta iteration with glitch detection and multi-pass rebasing
- [ ] Automatic perturbation activation based on zoom depth
- [ ] HUD shows active precision mode and reference orbit status
- [ ] Adaptive iteration scaling tuned for deep zoom
- [ ] Zoom to 10^30+ without artifacts

---

## Phase 20 — Deep Zoom: Series Approximation

**Objective:** Dramatically reduce per-frame cost at extreme zoom depths (10^20+) by skipping early iterations via a polynomial approximation of the perturbation orbit.

**Reference:** [deep-zoom-analysis.md](../deep-zoom-analysis.md), Option 1 (SA section).

### Task 20.1 — Taylor series coefficient computation

**File:** `mandelbrust-core/src/perturbation.rs` (extend)

Compute series approximation coefficients alongside the reference orbit:

1. During reference orbit computation, also compute `A_n`, `B_n`, `C_n` (and optionally higher-order terms):
   - `A_0 = 0`, `A_{n+1} = 2·Z_n·A_n + 1`
   - `B_0 = 0`, `B_{n+1} = 2·Z_n·B_n + A_n²`
   - `C_0 = 0`, `C_{n+1} = 2·Z_n·C_n + 2·A_n·B_n`
2. These are computed in `f64` (they are derived from the reference orbit which is already stored in `f64`).
3. Store them in the `ReferenceOrbit` struct: `sa_coeffs: Vec<(Complex, Complex, Complex)>` (A, B, C for each iteration).
4. At each step, estimate the approximation error: `|C_n·δc³|` for the worst-case pixel (the one with the largest `|δc|`, i.e. a corner of the viewport). When the error exceeds a threshold (e.g. the pixel spacing), record that iteration as the SA "validity limit" `K`.

**Verify:** For a test viewport at zoom 10^25, the SA skip count `K` should be in the hundreds or thousands. Verify that `δ_K ≈ A_K·δc + B_K·δc² + C_K·δc³` matches the result of iterating the delta recurrence K times (within the error estimate).

---

### Task 20.2 — Iteration skipping in the perturbation loop

**File:** `mandelbrust-core/src/perturbation.rs` (extend `iterate_perturbed`)

1. Before entering the delta iteration loop, compute the SA initial value: `δ_K = A_K·δc + B_K·δc² + C_K·δc³`.
2. Start the delta iteration from step `K` instead of step `0`.
3. This skips `K` iterations per pixel, which at zoom 10^30+ can save thousands of iterations per pixel.

**Verify:** Benchmark at zoom 10^30: render time with SA vs without SA. Expect 3–10× speedup depending on depth. Image output must be pixel-identical (within floating-point tolerance).

---

### Task 20.3 — Configurable SA order

**File:** `mandelbrust-core/src/perturbation.rs`

1. Allow configuring the SA polynomial order (2nd, 3rd, or 4th order). Higher order skips more iterations but costs more to compute coefficients.
2. Default to 3rd order (A, B, C). Add 4th order (D) as an option.
3. Expose the setting in the app (settings panel, advanced section) with a sensible default.

**Verify:** Switching between SA orders produces identical images. Higher orders skip more iterations (verify via HUD or debug log).

---

### Deliverables — Phase 20

- [ ] SA coefficient computation integrated into reference orbit
- [ ] Iteration skipping in the perturbation loop
- [ ] Configurable SA order (2nd–4th)
- [ ] Benchmark results showing speedup at zoom 10^30+
- [ ] Zoom to 10^100+ is practical (renders in seconds, not minutes)

---

## Phase 21 — Memory Layout & Buffer Management

**Objective:** Reduce memory footprint and allocation pressure for faster rendering.

**Reference:** [optimization-report.md](../optimization-report.md) section 5.

### Task 21.1 — Compact `IterationResult` to 8 bytes

**File:** `mandelbrust-core/src/fractal.rs`, `mandelbrust-render/src/iteration_buffer.rs`, `mandelbrust-render/src/palette.rs`, `mandelbrust-render/src/aa.rs`

Replace the `IterationResult` enum with a flat struct:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct IterationResult {
    pub iterations: u32,  // u32::MAX means interior
    pub norm_sq: f32,     // f32 is sufficient for smooth coloring
}
```

Update all code that pattern-matches on the old enum to use the new struct methods.

**Verify:** All tests pass. Rendered images are visually identical. `size_of::<IterationResult>() == 8`.

---

### Task 21.2 — Buffer pool for tile rendering

**Files:** `mandelbrust-render/src/renderer.rs`

Add a tile buffer pool using Rayon's `thread_local!` pattern to give each thread a reusable buffer, eliminating per-tile allocation.

**Verify:** Benchmark shows reduced allocation count. Rendered output unchanged.

---

### Task 21.3 — Avoid full buffer rebuild on `shift()`

**Files:** `mandelbrust-render/src/iteration_buffer.rs`, `mandelbrust-render/src/aa.rs`

Shift `IterationBuffer` and `AaSamples` in-place instead of allocating new buffers.

**Verify:** All tests pass. Pan-and-release produces the same visual result.

---

### Deliverables — Phase 21

- [ ] `IterationResult` is 8 bytes (down from 16)
- [ ] Tile buffers are pooled and reused
- [ ] `shift()` does not allocate a new buffer
- [ ] Benchmark results logged

---

## Phase 22 — SIMD Vectorization

**Objective:** Process 4 pixels simultaneously per CPU core using SIMD instructions.

**Reference:** [optimization-report.md](../optimization-report.md) section 3.

### Task 22.1 — Add batch iteration API

**File:** `mandelbrust-core/src/fractal.rs`

Add `fn iterate_batch()` default method on the `Fractal` trait. Update tile renderer to call it with groups of pixels.

**Verify:** All tests pass. Output identical. API preparation only — no SIMD yet.

---

### Task 22.2 — SIMD Mandelbrot iteration (AVX2)

**Files:** new file `mandelbrust-core/src/mandelbrot_simd.rs`

Implement `iterate_batch_simd()` processing 4 complex points per step using `f64x4` vectors. Handle lane-specific escape with masks. Scalar fallback on non-AVX2 targets.

**Verify:** Output identical to scalar. Benchmark: 3–4× speedup per core.

---

### Task 22.3 — SIMD Julia iteration

**File:** new file `mandelbrust-core/src/julia_simd.rs`

Same as 22.2 but for the Julia set.

**Verify:** Output identical to scalar. Similar speedup.

---

### Deliverables — Phase 22

- [ ] `iterate_batch()` API on the `Fractal` trait
- [ ] SIMD Mandelbrot and Julia iteration (4 pixels per step)
- [ ] Scalar fallback on non-x86-64 or non-AVX2 targets
- [ ] Benchmarks showing 3–4× per-core improvement

---

## Phase 23 — Animation & Video Export

**Objective:** Enable smooth fractal zoom animations between bookmarks.

### Task 23.1 — Keyframe system

**File:** new file `mandelbrust-app/src/animation.rs`

Define `Keyframe` and `AnimationPlan` structs. Implement camera interpolation: linear for center, logarithmic for scale. Compute `frame_viewport()` for each frame.

**Verify:** Unit test: two keyframes at different zoom levels produce smooth logarithmic interpolation.

---

### Task 23.2 — Frame-by-frame renderer

Render each frame via `render_offscreen()` (Phase 15), write PNG sequence to output directory. Background thread with progress callback and cancellation.

**Verify:** 10 frames produce 10 correctly named PNGs with smooth viewport transitions.

---

### Task 23.3 — Animation UI

Add an "Animation" panel with keyframe list (drag-to-reorder, add/remove), FPS/resolution options, render button, and progress bar.

**Verify:** User can create a 2-keyframe animation and render it.

---

### Task 23.4 — Optional ffmpeg integration

After PNG render, offer "Convert to MP4" if ffmpeg is on PATH. Handle not-found gracefully.

**Verify:** MP4 produced if ffmpeg present; helpful message if not.

---

### Deliverables — Phase 23

- [ ] Keyframe system with logarithmic zoom interpolation
- [ ] Frame-by-frame PNG export
- [ ] Animation UI with keyframe list and progress
- [ ] Optional ffmpeg MP4 conversion

---

## Phase 24 — GPU Compute Backend

**Objective:** Add an optional GPU rendering backend for 50–200× faster interactive exploration, including GPU-accelerated perturbation for real-time deep zoom.

**Reference:** [optimization-report.md](../optimization-report.md) section 4; [deep-zoom-analysis.md](../deep-zoom-analysis.md), Option 6.

### Task 24.1 — wgpu compute pipeline setup

Create the GPU compute infrastructure:
- Uniform buffer: viewport center, scale, dimensions, max_iterations, escape_radius.
- Storage buffer: output iteration data (u32 iteration + f32 norm_sq per pixel).
- WGSL compute shader for Mandelbrot/Julia iteration (f32 arithmetic).

**Verify:** Headless test renders 256×256 image via GPU; output matches CPU reference within f32 precision.

---

### Task 24.2 — GPU colorization shader

WGSL compute shader that reads the iteration buffer, applies a palette LUT, and writes RGBA pixels. Smooth coloring in shader.

**Verify:** GPU-colorized image matches CPU-colorized image.

---

### Task 24.3 — Integrate GPU backend into the app

Add "Renderer: CPU / GPU" toggle in settings. GPU writes directly to texture. Graceful CPU fallback if GPU init fails.

**Verify:** Switching between CPU and GPU produces similar results. GPU is noticeably faster.

---

### Task 24.4 — GPU perturbation (deep zoom on GPU)

Upload the reference orbit (from Phase 19) as a GPU storage buffer. Each GPU thread iterates deltas for one pixel. Emulated f64 in WGSL if needed, or Vulkan `shaderFloat64` on supported hardware.

**Verify:** GPU perturbation at zoom 10^25 matches CPU perturbation output. Render time is significantly faster.

---

### Deliverables — Phase 24

- [ ] WGSL compute shaders for iteration and colorization
- [ ] GPU/CPU toggle in settings with graceful fallback
- [ ] GPU perturbation for real-time deep zoom
- [ ] Benchmark comparison logged

---

## Phase 25 — Polish & v1.0 Release

**Objective:** Stabilise, document, and prepare for public release.

### Task 25.1 — Error handling audit

Audit all `unwrap()`, `expect()`, `panic!()` calls. Replace with proper error handling where recoverable. Keep `unwrap()` only where invariants are guaranteed (with comments).

---

### Task 25.2 — Cross-platform verification

Build and test on Windows (primary), macOS, and Linux. Fix platform-specific issues.

---

### Task 25.3 — Performance profiling

Profile with `cargo flamegraph` or `perf`. Fix top 3 bottlenecks. Log final benchmarks.

---

### Task 25.4 — Final documentation pass

Update `overview.md`, `README.md`, `optimization-report.md`, and this roadmap.

---

### Task 25.5 — Release packaging

GitHub Actions for prebuilt binaries (Windows, macOS, Linux). Versioned release (v1.0.0). Tag the commit.

---

### Deliverables — Phase 25

- [ ] No unhandled panics in normal operation
- [ ] Verified on at least 2 platforms
- [ ] Profiled and optimised
- [ ] Documentation complete and accurate
- [ ] v1.0.0 release published with binaries

---

## Long-Term (Post-v1.0)

Not scheduled but tracked as future possibilities:

- **Additional fractal types** — Multibrot, Burning Ship, Newton, Tricorn
- **Buddhabrot / Nebulabrot** rendering mode
- **Orbit trap coloring** (Pickover stalks, circles, crosses)
- **Fade to black** — MSZP-style fade near max iterations
- **WebAssembly build** — run MandelbRust in the browser
- **Plugin system** — user-defined fractal formulas
