# MandelbRust — Development Roadmap

> **Note:** This is the original roadmap covering Phases 0–6 (all complete). For Phase 7 onward, see [**roadmap-v2.md**](roadmap-v2.md). Roadmap v2 includes Phase 7 (Quick Performance Wins, done), Phases 8–10 (Display/color settings and profiles, Minimap, Julia C Explorer from `Features_to_add.md`), then Image Export, Architecture Cleanup, and later phases through v1.0 and beyond.

This document describes the **planned development phases** for MandelbRust, from initial scaffolding to a fully-featured high-performance fractal explorer.  
Each phase produces a usable, testable state of the application.

> **Important guidelines**
>
> - All implementation work **must respect the project vision** as described in [overview.md](overview.md). The overview is the single source of truth for architecture, principles, and scope.
> - If a change would alter the project's vision, scope, or architectural decisions, the [overview.md](overview.md) document **must be updated first** — always ask for confirmation before doing so.
> - When a smarter or more efficient approach is identified during implementation, **suggest it proactively** — even if it diverges from the overview. Trade-offs and alternative benefits are worth discussing; the user may choose to compromise if the advantages justify it.
> - **Keep code as simple as possible** while respecting current Rust best practices. Prefer clarity over cleverness: small pure functions, minimal nesting, straightforward control flow. Only add abstraction when it eliminates real duplication or is required by the architecture.
> - **Keep files from getting too long.** Split modules and extract logic into separate files when a file would otherwise grow large; prefer many focused files over a few very long ones (see [overview.md](overview.md) §2–3).

---

## Phase 0 — Foundations & Project Setup

**Objective:** Establish a clean, scalable Rust project structure and development environment.

### Tasks
- [x] Create Git repository and Rust workspace
- [x] Define crate layout:
  - [x] `mandelbrust-core` — math, fractals, iteration
  - [x] `mandelbrust-render` — tiled renderer, coloring, multithreading
  - [x] `mandelbrust-app` — UI and user interaction
- [x] Configure:
  - [x] `cargo fmt`
  - [x] `clippy`
  - [x] basic CI workflow (build + lint + cross-platform matrix)
- [x] Add logging infrastructure (`tracing`)
- [x] Define crate-level error types (`CoreError`, `RenderError`) and `Result` conventions
- [x] Define coding conventions and module boundaries

### Deliverables
- [x] Compiling empty application window
- [ ] CI passing on main branch

---

## Phase 1 — Core Fractal Engine

**Objective:** Implement a correct, fast fractal iteration engine independent of UI.

### Tasks
- [x] Implement complex plane math utilities
- [x] Implement camera model (pixel ↔ complex plane mapping)
- [x] Define `Fractal` trait for extensibility (Mandelbrot, Julia, future fractals)
  - [x] Use static dispatch (generics) to allow compiler inlining of the hot loop
- [x] Implement Mandelbrot iteration:
  - [x] escape-time algorithm
  - [x] cardioid and period-2 bulb check (skip iteration for known-interior points)
  - [x] periodicity detection via Brent's algorithm (early exit for interior orbits)
  - [x] store raw `(n, |z|²)` at escape; defer smooth formula to coloring pass
  - [x] configurable max iterations and escape radius
- [x] Define core data structures:
  - [x] `Complex`
  - [x] `Viewport` (center, scale, aspect ratio)
  - [x] `FractalParams`
- [x] Implement Julia set support
- [x] Parameter validation and defaults

### Deliverables
- [x] Unit tests for iteration correctness
- [x] Deterministic iteration results
- [x] Headless render of a small Mandelbrot image

---

## Phase 2 — Multithreaded Tiled Renderer

**Objective:** Achieve high CPU utilization and fast renders using parallelism.

### Tasks
- [x] Design tile abstraction (64×64 for L1 cache locality)
- [x] Pre-allocate per-thread tile buffers (zero allocation in render loop)
- [x] Implement image buffer (RGBA)
- [x] Implement tiled rendering pipeline
- [x] Exploit real-axis symmetry for Mandelbrot (compute top half, mirror bottom)
- [x] Integrate `rayon` for parallel tile execution
- [x] Add render cancellation mechanism:
  - [x] generation counter
  - [x] early exit on invalidation
- [x] Measure and log render timings
- [x] Set up `criterion` benchmarks (iterations/sec, tiles/sec, full-frame render time)

### Deliverables
- [x] Full-frame Mandelbrot render
- [x] Near-linear CPU scaling
- [x] Cancelable render jobs
- [x] End-to-end integration test across `core` and `render` crates

---

## Phase 3 — UI & Interaction Layer

**Objective:** Enable real-time exploration with Google Maps–style controls.

### Tasks
- [x] Integrate `egui` / `eframe`
- [x] Display rendered image buffer
- [x] Implement camera controls:
  - [x] mouse wheel zoom (cursor-centered)
  - [x] click + drag pan
  - [x] click to select Julia parameter (in Julia mode)
- [x] Implement keyboard shortcuts:
  - [x] arrow keys for pan
  - [x] `+` / `-` for zoom
  - [x] `R` to reset view
  - [x] `Escape` to cancel render
- [x] Handle window resize (aspect ratio recalculation, re-render)
- [x] Display HUD:
  - [x] coordinates
  - [x] zoom level
  - [x] iteration count
  - [x] render progress
  - [x] render timing metrics
- [x] Add parameter controls (max iterations, escape radius)
- [x] Default startup view: Mandelbrot set
- [x] Implement view navigation history (back / forward)

### Deliverables
- [x] Interactive Mandelbrot explorer
- [x] Smooth zoom and pan experience
- [x] Instant visual feedback

---

## Phase 4 — Progressive Rendering & UX Optimization

**Objective:** Make exploration feel instantaneous while maintaining quality.

### Tasks
- [x] Implement progressive rendering passes:
  - [x] preview pass (low resolution / low iterations)
  - [x] refinement pass (full quality)
- [x] Add adaptive iteration logic
- [x] Prioritize visible tiles
- [x] Border tracing: flood-fill tile interiors when all border pixels share the same iteration count
- [x] Improve cancellation responsiveness
- [x] Add render status indicators
- [x] Detect and warn when approaching `f64` precision limits (~10^15 zoom)

### Deliverables
- [x] No visible lag during navigation
- [x] High-quality final image convergence
- [x] Responsive UI under heavy load

---

## Phase 5 — Coloring System & Display Options ✅

**Objective:** Provide flexible and visually appealing color rendering.

### Tasks
- [x] Implement palette system using LUTs
- [x] Add predefined palettes (Classic, Fire, Ocean, Neon, Grayscale)
- [x] Palette switching without recomputing iterations (IterationBuffer stored separately)
- [x] Implement smooth coloring (continuous iteration renormalization)
- [x] Design palette API for future extensibility (custom palettes, editor)
- [x] Add display toggles:
  - [x] grayscale (dedicated Grayscale palette)
  - [x] raw iteration count (smooth coloring toggle)
  - [x] palette preview (gradient bar in controls panel)

### Deliverables
- [x] Multiple selectable palettes (5 built-in palettes with ComboBox selector)
- [x] Visually smooth gradients (LUT interpolation with smooth iteration formula)
- [x] Instant palette switching (re-colorize from stored IterationBuffer, no re-render)

---

## Phase 6 — Bookmarks System ✅

**Objective:** Allow persistent saving and restoration of exploration states.

### Tasks
- [x] Define `Bookmark` structure:
  - [x] fractal type
  - [x] viewport
  - [x] parameters
  - [x] palette
  - [x] Julia constant (if applicable)
  - [x] metadata (name, tags, notes)
- [x] Serialize bookmarks as JSON using `serde` / `serde_json`
- [x] Store bookmarks in OS-appropriate config directories (`directories` crate)
- [x] Implement bookmark UI:
  - [x] add / delete / rename
  - [x] jump to bookmark
- [x] Bookmark search and ordering
- [x] Implement application preferences:
  - [x] default window size
  - [x] default max iterations and palette
  - [x] restore last view on startup
  - [x] persist as JSON via `serde_json` + `directories`

### Deliverables
- [x] Persistent bookmarks across sessions
- [x] Instant state restoration
- [x] Bookmark-driven exploration

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
