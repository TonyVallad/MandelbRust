# MandelbRust — Development Roadmap

This document describes the **planned development phases** for MandelbRust, from initial scaffolding to a fully-featured high-performance fractal explorer.  
Each phase produces a usable, testable state of the application.

---

## Phase 0 — Foundations & Project Setup

**Objective:** Establish a clean, scalable Rust project structure and development environment.

### Tasks
- [ ] Create Git repository and Rust workspace
- [ ] Define crate layout:
  - [ ] `mandelbrust-core` — math, fractals, iteration
  - [ ] `mandelbrust-render` — tiled renderer, coloring, multithreading
  - [ ] `mandelbrust-app` — UI and user interaction
- [ ] Configure:
  - [ ] `cargo fmt`
  - [ ] `clippy`
  - [ ] basic CI workflow (build + lint + cross-platform matrix)
- [ ] Add logging infrastructure (`tracing`)
- [ ] Define crate-level error types (`CoreError`, `RenderError`) and `Result` conventions
- [ ] Define coding conventions and module boundaries

### Deliverables
- [ ] Compiling empty application window
- [ ] CI passing on main branch

---

## Phase 1 — Core Fractal Engine

**Objective:** Implement a correct, fast fractal iteration engine independent of UI.

### Tasks
- [ ] Implement complex plane math utilities
- [ ] Implement camera model (pixel ↔ complex plane mapping)
- [ ] Define `Fractal` trait for extensibility (Mandelbrot, Julia, future fractals)
  - [ ] Use static dispatch (generics) to allow compiler inlining of the hot loop
- [ ] Implement Mandelbrot iteration:
  - [ ] escape-time algorithm
  - [ ] cardioid and period-2 bulb check (skip iteration for known-interior points)
  - [ ] periodicity detection via Brent's algorithm (early exit for interior orbits)
  - [ ] store raw `(n, |z|²)` at escape; defer smooth formula to coloring pass
  - [ ] configurable max iterations and escape radius
- [ ] Define core data structures:
  - [ ] `Complex`
  - [ ] `Viewport` (center, scale, aspect ratio)
  - [ ] `FractalParams`
- [ ] Implement Julia set support
- [ ] Parameter validation and defaults

### Deliverables
- [ ] Unit tests for iteration correctness
- [ ] Deterministic iteration results
- [ ] Headless render of a small Mandelbrot image

---

## Phase 2 — Multithreaded Tiled Renderer

**Objective:** Achieve high CPU utilization and fast renders using parallelism.

### Tasks
- [ ] Design tile abstraction (64×64 for L1 cache locality)
- [ ] Pre-allocate per-thread tile buffers (zero allocation in render loop)
- [ ] Implement image buffer (RGBA)
- [ ] Implement tiled rendering pipeline
- [ ] Exploit real-axis symmetry for Mandelbrot (compute top half, mirror bottom)
- [ ] Integrate `rayon` for parallel tile execution
- [ ] Add render cancellation mechanism:
  - [ ] generation counter
  - [ ] early exit on invalidation
- [ ] Measure and log render timings
- [ ] Set up `criterion` benchmarks (iterations/sec, tiles/sec, full-frame render time)

### Deliverables
- [ ] Full-frame Mandelbrot render
- [ ] Near-linear CPU scaling
- [ ] Cancelable render jobs
- [ ] End-to-end integration test across `core` and `render` crates

---

## Phase 3 — UI & Interaction Layer

**Objective:** Enable real-time exploration with Google Maps–style controls.

### Tasks
- [ ] Integrate `egui` / `eframe`
- [ ] Display rendered image buffer
- [ ] Implement camera controls:
  - [ ] mouse wheel zoom (cursor-centered)
  - [ ] click + drag pan
  - [ ] click to select Julia parameter (in Julia mode)
- [ ] Implement keyboard shortcuts:
  - [ ] arrow keys for pan
  - [ ] `+` / `-` for zoom
  - [ ] `R` to reset view
  - [ ] `Escape` to cancel render
- [ ] Handle window resize (aspect ratio recalculation, re-render)
- [ ] Display HUD:
  - [ ] coordinates
  - [ ] zoom level
  - [ ] iteration count
  - [ ] render progress
  - [ ] render timing metrics
- [ ] Add parameter controls (max iterations, escape radius)
- [ ] Default startup view: Mandelbrot set
- [ ] Implement view navigation history (back / forward)

### Deliverables
- [ ] Interactive Mandelbrot explorer
- [ ] Smooth zoom and pan experience
- [ ] Instant visual feedback

---

## Phase 4 — Progressive Rendering & UX Optimization

**Objective:** Make exploration feel instantaneous while maintaining quality.

### Tasks
- [ ] Implement progressive rendering passes:
  - [ ] preview pass (low resolution / low iterations)
  - [ ] refinement pass (full quality)
- [ ] Add adaptive iteration logic
- [ ] Prioritize visible tiles
- [ ] Border tracing: flood-fill tile interiors when all border pixels share the same iteration count
- [ ] Improve cancellation responsiveness
- [ ] Add render status indicators
- [ ] Detect and warn when approaching `f64` precision limits (~10^15 zoom)

### Deliverables
- [ ] No visible lag during navigation
- [ ] High-quality final image convergence
- [ ] Responsive UI under heavy load

---

## Phase 5 — Coloring System & Display Options

**Objective:** Provide flexible and visually appealing color rendering.

### Tasks
- [ ] Implement palette system using LUTs
- [ ] Add predefined palettes
- [ ] Palette switching without recomputing iterations
- [ ] Implement smooth coloring
- [ ] Design palette API for future extensibility (custom palettes, editor)
- [ ] Add display toggles:
  - [ ] grayscale
  - [ ] raw iteration count
  - [ ] palette preview

### Deliverables
- [ ] Multiple selectable palettes
- [ ] Visually smooth gradients
- [ ] Instant palette switching

---

## Phase 6 — Bookmarks System

**Objective:** Allow persistent saving and restoration of exploration states.

### Tasks
- [ ] Define `Bookmark` structure:
  - [ ] fractal type
  - [ ] viewport
  - [ ] parameters
  - [ ] palette
  - [ ] Julia constant (if applicable)
  - [ ] metadata (name, tags, notes)
- [ ] Serialize bookmarks as JSON using `serde` / `serde_json`
- [ ] Store bookmarks in OS-appropriate config directories (`directories` crate)
- [ ] Implement bookmark UI:
  - [ ] add / delete / rename
  - [ ] jump to bookmark
- [ ] Bookmark search and ordering
- [ ] Implement application preferences:
  - [ ] default window size
  - [ ] default max iterations and palette
  - [ ] restore last view on startup
  - [ ] persist as JSON via `serde_json` + `directories`

### Deliverables
- [ ] Persistent bookmarks across sessions
- [ ] Instant state restoration
- [ ] Bookmark-driven exploration

---

## Phase 7 — Image Export

**Objective:** Support high-quality still image exports.

### Tasks
- [ ] Implement offscreen renderer
- [ ] Allow arbitrary export resolution
- [ ] Optional supersampling
- [ ] PNG export via `image` crate
- [ ] Export progress and cancellation

### Deliverables
- [ ] High-resolution PNG exports
- [ ] Deterministic output independent of screen resolution

---

> **v1.0 release boundary** — Phases 0–7 constitute the initial stable release. Phases 8+ are post-v1.0 enhancements.

---

## Phase 8 — Animation & Video Export

**Objective:** Enable smooth fractal zoom animations.

### Tasks
- [ ] Keyframe system based on bookmarks
- [ ] Camera interpolation (logarithmic scale interpolation for smooth zoom feel)
- [ ] Frame-by-frame renderer
- [ ] PNG sequence export
- [ ] Optional `ffmpeg` integration for MP4 generation

### Deliverables
- [ ] Reproducible animations
- [ ] High-quality video exports
- [ ] Bookmark-to-bookmark zooms

---

## Phase 9 — Advanced Performance Techniques (Optional)

**Objective:** Push rendering speed and zoom depth beyond baseline.

### Possible Enhancements
- [ ] Perturbation rendering for deep zooms
- [ ] Reference orbit caching
- [ ] Arbitrary precision math for reference points
- [ ] GPU compute backend (`wgpu`)
- [ ] SIMD optimization

### Deliverables
- [ ] Deep zoom capability
- [ ] Order-of-magnitude performance improvements

---

## Phase 10 — Polish & Release

**Objective:** Prepare MandelbRust for public use.

### Tasks
- [ ] Error handling and stability testing
- [ ] Performance profiling and optimization
- [ ] Cross-platform build verification (Windows, macOS, Linux)
- [ ] Documentation:
  - [ ] README
  - [ ] usage guide
- [ ] Prebuilt binaries
- [ ] Versioned releases

### Deliverables
- [ ] Stable v1.0 release
- [ ] Reproducible builds
- [ ] Clean user experience

---

## Long-Term Vision

MandelbRust is designed as:
- A **reference-quality fractal explorer**
- A **high-performance Rust application**
- A platform for experimentation in numerical rendering

The roadmap is intentionally modular: each phase is valuable on its own and can be reordered or extended without architectural rewrites.
