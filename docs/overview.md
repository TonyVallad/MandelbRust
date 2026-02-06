# MandelbRust — Project Overview

## 1. Purpose & Vision

**MandelbRust** is a high-performance, native fractal exploration and rendering application written in **Rust**.  
Its goal is to provide **real-time, high-resolution exploration** of complex fractals (starting with the Mandelbrot set) using a **Google Maps–like interaction model**, while supporting **multithreaded rendering**, **bookmarks**, and **high-quality exports** (images and animations).

The project is a modern re-implementation of classic fractal explorers, designed with today's hardware, parallelism, and software architecture in mind.

MandelbRust aims to be:
- A **serious fractal exploration tool**
- A **performance-oriented Rust showcase**
- A modern successor to classic BASIC and early C fractal programs

Fast, precise, extensible — and built to last.

---

## 2. Core Principles

- **Maximum responsiveness** during exploration  
- **Deterministic, reproducible renders**
- **Heavy multithreading** (mandatory)
- **Progressive rendering** (fast preview → refined result)
- **Clean separation** between UI, rendering, and math
- **Native, portable executable**

---

## 3. Interaction Model (Google Maps–like)

MandelbRust uses a continuous camera model over the complex plane.

### Mouse Controls
- **Scroll wheel**: zoom in / out centered on cursor
- **Click + drag**: pan the viewport
- **Click (optional)**: select Julia parameter (when in Julia mode)

### Keyboard Controls
- **Arrow keys**: pan the viewport
- **`+` / `-`**: zoom in / out
- **`R`**: reset view to default
- **`Escape`**: cancel current render

### Camera Representation
The view is defined by:
- Complex center `(re, im)`
- Scale (complex units per pixel)
- Aspect ratio (derived from viewport, updated on window resize)
- Active fractal parameters

A **view history stack** supports back / forward navigation, allowing quick exploration without formal bookmarks.

Any visible state can be bookmarked or exported.

---

## 4. Fractal System

### Architecture
Fractals are implemented behind a common **`Fractal` trait**, providing a uniform interface for iteration, parameter validation, and extensibility. New fractal types can be added by implementing this trait.

The trait is used via **static dispatch** (generics, not `dyn Fractal`), ensuring the compiler can inline and fully optimize the iteration hot loop for each fractal type.

### Supported Fractals
- **Mandelbrot set** (default at startup)
- **Julia sets** (parameter selectable interactively)
- Extensible to additional fractals (Multibrot, Burning Ship, Newton, etc.)

### Iteration Model
- Escape-time algorithm
- Smooth iteration count (`ν`) for continuous coloring
- Configurable max iterations and escape radius

### Computation Optimizations
The iteration engine applies several techniques to minimize unnecessary work:
- **Cardioid & period-2 bulb check** — closed-form test that skips iteration entirely for ~30–40% of points at default zoom
- **Periodicity detection** (Brent's algorithm) — detects orbital cycles to exit early for interior points, avoiding full `max_iter` cost
- **Deferred smooth formula** — the iteration loop stores only raw `(n, |z|²)` at escape; the expensive `ln(ln(...))` smooth coloring formula is computed once during the coloring pass, not inside the hot loop
- **Real-axis symmetry** — for the Mandelbrot set, only the top half is computed when the viewport straddles `im = 0`; results are mirrored for the bottom half

### Precision Limits
Standard `f64` arithmetic limits useful zoom depth to approximately 10^15. Beyond this, visual artifacts appear. The application detects and warns when approaching this limit. Deep zoom techniques (perturbation theory, arbitrary precision) are planned as future enhancements.

---

## 5. Rendering Pipeline

### Tiled CPU Renderer
- The viewport is divided into fixed-size **tiles** (64×64 pixels — 32 KB per tile at `f64`, fits in L1 cache)
- Each tile is rendered independently using **pre-allocated per-thread buffers** (no allocation in the render loop)
- Tiles are scheduled using **Rayon** for automatic load balancing
- **Border tracing**: if all border pixels of a tile share the same iteration count, the interior is flood-filled without computation

### Progressive Rendering
1. **Preview pass**  
   - Lower resolution or reduced iteration count  
   - Triggered immediately on camera movement
2. **Refinement pass**  
   - Full resolution and full iteration depth  
   - Runs asynchronously
3. **Cancellation**  
   - Any user interaction invalidates the current render pass  
   - Ongoing tile jobs terminate early

This ensures immediate feedback while converging to a sharp image.

---

## 6. Multithreading Strategy

- Uses **Rayon work-stealing thread pool**
- No shared mutable state inside pixel loops
- Atomic generation counter for render invalidation
- CPU cores are saturated efficiently, even for deep zooms

Multithreading is a **core requirement**, not an optimization.

---

## 7. Coloring & Display

### Color Palettes
- Predefined palettes (gradient LUTs)
- Smooth coloring using normalized iteration values
- Palette selection is instantaneous (no recomputation of iterations)
- Architecture allows future palette editor / histogram coloring

### Display Options
- Iteration overlays (optional)
- Coordinate & zoom HUD
- Render progress and timing metrics

---

## 8. Bookmarks & Configuration

### Bookmarks

Bookmarks capture the **entire render state**, including:
- Fractal type
- Camera center and scale
- Iteration parameters
- Palette selection
- Julia constant (if applicable)
- Optional user metadata (name, tags, notes)

#### Storage
- Serialized using `serde`
- Stored as JSON in OS-appropriate config directories
- Fully portable and human-readable

Bookmarks can be:
- Added from current view
- Renamed / deleted
- Recalled instantly
- Used as animation keyframes

### Application Preferences

User preferences are stored separately from bookmarks and include:
- Default window size
- Default max iterations and palette
- Restore last view on startup

Preferences use the same JSON + `directories` crate storage mechanism as bookmarks.

---

## 9. Export System

### Image Export
- High-resolution PNG export
- Offscreen render at arbitrary resolution
- Independent of viewport resolution
- Optional supersampling for final quality

### Animation Export
- Camera interpolation between bookmarks (logarithmic scale interpolation for perceptually smooth zoom)
- Frame-by-frame rendering
- PNG sequence output
- Optional integration with `ffmpeg` for MP4 generation

Animations are deterministic and reproducible.

---

## 10. Technology Stack

- **Language**: Rust
- **UI**: `egui` / `eframe`
- **Parallelism**: `rayon`
- **Benchmarking**: `criterion`
- **Image encoding**: `image`
- **Serialization**: `serde`, `serde_json`
- **Config paths**: `directories`
- **Logging / profiling**: `tracing`

No GPU is required; performance is achieved via CPU parallelism and careful architecture.  
GPU compute may be added later without redesigning the core.

---

## 11. Project Scope

### v1.0 (Initial Release)

- Native desktop application
- Mandelbrot and Julia set exploration
- Real-time interaction with mouse and keyboard
- Multithreaded tiled renderer with progressive rendering
- Coloring system with multiple palettes
- Bookmarks and application preferences
- High-resolution image export

### Post-v1.0

- Animation and video export
- Deep zoom techniques (perturbation theory, arbitrary precision)
- Additional fractal types
- GPU compute backend
- Advanced coloring (palette editor, histogram coloring)
