# MandelbRust — Project Overview

## 1. Purpose & Vision

**MandelbRust** is a high-performance, native fractal exploration and rendering application written in **Rust**.
Its goal is to provide **real-time, high-resolution exploration** of complex fractals (starting with the Mandelbrot set and Julia sets) using a **Google Maps–like interaction model**, while supporting **multithreaded rendering**, **bookmarks**, and **high-quality exports** (images and animations).

The project is a modern re-implementation of classic fractal explorers, designed with today's hardware, parallelism, and software architecture in mind. It is the spiritual successor to [MSZP](https://github.com/TonyVallad/MSZP), a QBasic fractal explorer by the same author.

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
- **Simplicity** — keep code as clear and straightforward as possible while following current Rust best practices
- **Native, portable executable**

---

## 3. Project Structure

MandelbRust is organized as a **Rust workspace** with three crates, each with a focused responsibility:

```
MandelbRust/
  mandelbrust-core/     # math, fractals, iteration engine
  mandelbrust-render/   # tiled renderer, coloring, multithreading
  mandelbrust-app/      # UI and user interaction
  docs/                 # project documentation
```

### `mandelbrust-core` — Math & Fractal Engine

Pure mathematical library with **no UI or rendering dependencies**. Contains:

| Module | Contents |
|---|---|
| `complex.rs` | `Complex` number type (`re`, `im` as `f64`) with arithmetic operators, `norm_sq()`, `norm()`, and serde support |
| `fractal.rs` | `Fractal` trait (static dispatch), `IterationResult` enum (`Escaped { iterations, norm_sq }` / `Interior`), `FractalParams` (max iterations, escape radius) |
| `mandelbrot.rs` | Mandelbrot iteration with cardioid check, period-2 bulb check, and Brent's periodicity detection |
| `julia.rs` | Julia set iteration with configurable constant `c` |
| `viewport.rs` | `Viewport` — camera model mapping pixels to the complex plane. Center, scale, pixel-to-complex conversion, subpixel sampling for AA, `shift()` for panning |
| `error.rs` | `CoreError` — validation errors for parameters and viewports |

### `mandelbrust-render` — Rendering Pipeline

Handles all pixel computation, coloring, and anti-aliasing. Depends on `mandelbrust-core` but has **no UI dependency**. Contains:

| Module | Contents |
|---|---|
| `renderer.rs` | `render()` — the main rendering pipeline. Tiled parallel rendering via Rayon, border tracing, symmetry mirroring. `RenderCancel` for generation-based cancellation with progress tracking. Returns `RenderResult` |
| `tile.rs` | `Tile` abstraction (64×64 pixels), `build_tile_grid()`, symmetry classification (`TileKind::Normal`, `Primary`, `Mirror`) |
| `buffer.rs` | `RenderBuffer` — RGBA pixel buffer with tile blitting and mirroring |
| `iteration_buffer.rs` | `IterationBuffer` — stores `IterationResult` per pixel, supports tile blitting, mirroring, and `shift()` for pan optimization |
| `palette.rs` | `Palette` — gradient LUT with 256 colors. Smooth coloring formula `ν = n + 1 − log₂(ln(\|zₙ\|))`. Five built-in palettes (Classic, Fire, Ocean, Neon, Grayscale). `colorize()`, `colorize_aa()`, `preview_colors()` |
| `aa.rs` | `AaSamples` — adaptive anti-aliasing. Sparse storage for boundary pixel supersamples. `compute_aa()` detects edges where iteration class differs between neighbors, then supersamples only those pixels (2×2 or 4×4) |
| `error.rs` | `RenderError` — rendering error types |

### `mandelbrust-app` — Application & UI

Desktop application using `egui` / `eframe`. Contains:

| Module | Contents |
|---|---|
| `main.rs` | `MandelbRustApp` — main application struct and all UI logic: rendering pipeline orchestration, keyboard/mouse input, HUD overlay (four-corner layout), top-right Material Symbols icon toolbar with state-aware dimming, palette popup picker, fractal parameters panel (bottom-left), controls/help window, settings panel, bookmark explorer with favorites toggle, save/update dialogs, thumbnail caching with automatic invalidation |
| `bookmarks.rs` | `Bookmark` data structure (self-contained with embedded base64 PNG thumbnail), `BookmarkStore` (one `.json` file per bookmark, immediate persistence, directory scanning), `LabelNode` for hierarchical label trees, `encode_thumbnail` / `decode_thumbnail` for inline image embedding, automatic legacy migration |
| `preferences.rs` | `AppPreferences` — persistent user settings (window size, defaults, restore-last-view, configurable bookmarks directory). `LastView` for capturing/restoring the last exploration state |

---

## 4. Interaction Model (Google Maps–like)

MandelbRust uses a continuous camera model over the complex plane.

### Mouse Controls
- **Scroll wheel**: zoom in / out centered on cursor
- **Left-click + drag**: pan the viewport
- **Right-click + drag**: selection rectangle — draws a cyan rectangle; on release, the viewport zooms to fit that region
- **Shift + Click** (Julia mode): set Julia constant `c` from the cursor position

### Keyboard Controls

| Key | Action |
|---|---|
| **Arrow keys** | Pan the viewport |
| **`+` / `-`** | Zoom in / out (centered) |
| **`R`** | Reset view to default |
| **`Escape`** | Close dialogs / help / settings, cancel current render |
| **`H`** | Toggle entire HUD (hides all overlays, toolbar, and panels) |
| **`C`** | Toggle crosshair overlay (shows cursor coords + viewport center marker) |
| **`A`** | Cycle anti-aliasing level (Off → 2×2 → 4×4 → Off) |
| **`S`** | Save bookmark (or update/save-new if currently viewing a bookmark) |
| **`B`** | Toggle bookmark explorer |
| **`Backspace`** | Navigate view history (back) |
| **`Shift+Backspace`** | Navigate view history (forward) |

Keyboard shortcuts are suppressed when a text input field is focused, so typing in search or name fields does not trigger fractal controls.

### Camera Representation
The view is defined by:
- Complex center `(re, im)`
- Scale (complex units per pixel)
- Aspect ratio (derived from viewport, updated on window resize)
- Active fractal parameters

A **view history stack** (up to 200 entries) supports back / forward navigation, allowing quick exploration without formal bookmarks.

---

## 5. Fractal System

### Architecture
Fractals are implemented behind a common **`Fractal` trait**, providing a uniform interface for iteration, parameter validation, and extensibility. New fractal types can be added by implementing this trait.

The trait is used via **static dispatch** (generics, not `dyn Fractal`), ensuring the compiler can inline and fully optimize the iteration hot loop for each fractal type.

### Supported Fractals
- **Mandelbrot set** (default at startup)
- **Julia sets** (parameter selectable interactively via Shift+Click or the fractal mode selector in the bottom-left panel)
- Extensible to additional fractals (Multibrot, Burning Ship, Newton, etc.)

### Iteration Model
- Escape-time algorithm
- Smooth iteration count (`ν`) for continuous coloring
- Configurable max iterations (slider in bottom-left fractal parameters panel) and escape radius

### Computation Optimizations
The iteration engine applies several techniques to minimize unnecessary work:
- **Cardioid & period-2 bulb check** — closed-form test that skips iteration entirely for ~30–40% of points at default zoom
- **Periodicity detection** (Brent's algorithm) — detects orbital cycles to exit early for interior points, avoiding full `max_iter` cost
- **Deferred smooth formula** — the iteration loop stores only raw `(n, |z|²)` at escape; the expensive `ln(ln(...))` smooth coloring formula is computed once during the coloring pass, not inside the hot loop
- **Real-axis symmetry** — for the Mandelbrot set, only the top half is computed when the viewport straddles `im = 0`; results are mirrored for the bottom half

### Precision Limits
Standard `f64` arithmetic limits useful zoom depth to approximately 10^15. Beyond this, visual artifacts appear. The application detects and warns when approaching this limit (precision warning displayed in the HUD). Deep zoom techniques (perturbation theory, arbitrary precision) are planned as future enhancements.

---

## 6. Rendering Pipeline

### Tiled CPU Renderer
- The viewport is divided into fixed-size **tiles** (64×64 pixels — 32 KB per tile at `f64`, fits in L1 cache)
- Each tile is rendered independently using **pre-allocated per-thread buffers** (no allocation in the render loop)
- Tiles are scheduled using **Rayon** for automatic work-stealing load balancing
- **Border tracing**: if all border pixels of a tile share the same iteration count, the interior is flood-filled without computation

### Background Render Thread
All rendering runs on a dedicated background thread, communicating with the UI via `mpsc` channels (`RenderRequest` / `RenderResponse`). The UI thread never blocks, ensuring smooth interaction even during heavy renders.

### Progressive Rendering
1. **Preview pass**
   - 1/4 resolution with capped iteration count
   - Triggered immediately on camera movement
   - Displayed as a bilinear-filtered placeholder
2. **Refinement pass**
   - Full resolution and full iteration depth
   - Followed by adaptive anti-aliasing if enabled
   - Replaces the preview when complete
3. **Cancellation**
   - Any user interaction invalidates the current render pass via an atomic generation counter
   - Ongoing tile jobs terminate early
   - The render thread drains stale requests and always works on the latest

This ensures immediate feedback while converging to a sharp image.

### Pan Optimization
When the user drags the viewport:
- The existing iteration buffer and AA samples are **shifted** in-place — pixels that remain visible are preserved at full quality
- Only the newly exposed edges trigger a low-resolution **drag preview** render
- On drag release, a full render fills the exposed edges; the previously rendered area retains its quality (no flicker)

### Adaptive Iterations
Max iterations automatically increase with zoom depth to reveal finer detail. The formula adds iterations proportional to `log₂(zoom)`. This is toggleable via the checkbox in the bottom-left fractal parameters panel; the user can also set a manual ceiling via the iteration slider.

---

## 7. Anti-Aliasing

MandelbRust implements **adaptive supersampling** to reduce jagged edges without the cost of full-image AA:

1. **Boundary detection** — after the main render, neighboring pixels are compared by iteration class. Only pixels where the class differs from at least one neighbor are flagged as boundary pixels.
2. **Selective supersampling** — boundary pixels are re-sampled at sub-pixel positions (2×2 or 4×4 grid, configurable via `A` key or the deblur toolbar icon). Interior pixels are untouched.
3. **Sparse storage** — `AaSamples` uses a `HashMap<usize, Vec<IterationResult>>` to store sub-pixel data only for boundary pixels, keeping memory usage proportional to edge complexity rather than total pixel count.
4. **Shift-aware** — during panning, the AA data is shifted together with the iteration buffer so previously anti-aliased regions retain their quality.

---

## 8. Multithreading Strategy

- Uses **Rayon work-stealing thread pool**
- No shared mutable state inside pixel loops
- Atomic generation counter for render invalidation
- Progress tracking via atomic counters (`progress_done` / `progress_total`) for UI progress bar
- CPU cores are saturated efficiently, even for deep zooms

Multithreading is a **core requirement**, not an optimization.

---

## 9. Coloring & Display

### Color Palettes
- **5 built-in palettes**: Classic, Fire, Ocean, Neon, Grayscale
- Stored as 256-entry gradient lookup tables (LUTs)
- **Smooth coloring** using normalized iteration values: `ν = n + 1 − log₂(ln(|zₙ|))`
- Palette selection is **instantaneous** — the `IterationBuffer` is stored separately from the pixel buffer, so switching palettes re-colorizes without re-rendering
- **Palette popup picker** — clicking the palette icon in the toolbar opens a popup showing all palettes with color gradient preview swatches; clicking a palette selects it immediately
- Architecture allows future palette editor / histogram coloring

### HUD Layout
The HUD is distributed across four screen areas for minimal visual intrusion. Pressing **H** hides everything; all overlays, toolbar, panels, and floating windows disappear together.

| Area | Content |
|---|---|
| **Top-left** | Read-only viewport info: fractal mode, center coordinates, zoom level, iteration count, palette name, precision warning |
| **Top-right toolbar** | Icon bar using **Material Symbols** (embedded via `egui_material_icons`): arrow_back / arrow_forward (navigate back / forward), restart_alt (reset view), palette (palette picker popup), deblur (cycle anti-aliasing), gradient (smooth coloring), bookmark_add (save bookmark), bookmarks (open bookmark explorer), help_outline (controls & shortcuts), settings (settings — always last). Icons are evenly spaced in a fixed-width grid. Icons that represent a toggleable state (AA, smooth coloring, bookmarks explorer) are **dimmed when off** and bright when active. |
| **Top-right** (below toolbar) | Cursor complex coordinates (visible only when crosshair is enabled, no background) |
| **Bottom-left** | Fractal parameters panel: fractal mode selector (Mandelbrot / Julia), Julia `c` value, iteration slider with x10 / /10 buttons, escape radius slider, adaptive iterations checkbox |
| **Bottom-right** | Render stats: phase, timing, tile counts, AA status (semi-transparent background) |

A **progress bar** appears at the top of the viewport during rendering and AA passes.

### Controls & Shortcuts Window
Accessed via the **help_outline** icon in the toolbar. A floating, closable window that lists all keyboard shortcuts, mouse controls, and toolbar icon descriptions (shown as actual Material Symbol glyphs) in a clean grid layout. Closes with Escape or the window's X button.

### Display Options
- Crosshair overlay with viewport center indicator (toggle with `C`)
- Selection rectangle for zoom (semi-transparent cyan outline via right-click drag)
- Smooth coloring on/off toggle (toolbar gradient icon)

---

## 10. Bookmarks & Configuration

### Bookmarks

Bookmarks capture the **entire exploration state**, including:
- Fractal type (Mandelbrot / Julia)
- Camera center and scale
- Iteration parameters (max iterations, escape radius)
- Palette selection and smooth coloring state
- Anti-aliasing level
- Julia constant (if applicable)
- User metadata: name, hierarchical labels, notes
- Base64-encoded PNG thumbnail (160px max width, auto-generated on save)

#### Storage — One File Per Bookmark

Each bookmark is stored as a **single, self-contained `.json` file** in the `bookmarks/` directory. By default this is a `bookmarks/` subdirectory in the project's working directory. The path can be changed from the **Settings** panel (⚙ icon in the toolbar), which offers a native folder picker dialog, an Apply button, and a Reset-to-default button. The file contains all metadata **and** the preview image (embedded as a base64-encoded PNG), making each bookmark file independently shareable — just copy the file to another machine.

- **Serialized** using `serde` / `serde_json` + `base64`
- **Filenames** are derived from the bookmark name (sanitized for filesystem safety, with numeric suffixes to avoid collisions)
- **Fully portable** and human-readable JSON
- **Immediate persistence** — every add, remove, rename, label toggle, or viewport update writes or deletes the individual file right away; there is no deferred "save on exit" step
- **Live directory scanning** — the bookmark explorer re-scans the bookmarks folder every time it is opened, so externally added, removed, or modified `.json` files are immediately visible

#### Automatic Legacy Migration

On first launch after upgrading, the old single-file format (`bookmarks.json` + separate `thumbnails/` directory) is automatically migrated:
1. Each entry in `bookmarks.json` becomes its own `.json` file with the thumbnail embedded inline
2. The old `bookmarks.json` is renamed to `bookmarks.json.migrated` as a safety backup
3. The old `thumbnails/` directory is renamed to `thumbnails.migrated`

This migration runs once and is transparent to the user.

#### Bookmark Explorer (B key or bookmarks toolbar icon)
- **Tab bar**: `All` | `Fav` (favorites toggle) | `Mandelbrot` | `Julia`. The **favorites toggle** is independent — it can be combined with any fractal tab and any label filter to show only bookmarks labelled "Favorites".
- **Search**: text filter across bookmark names and labels
- **Sort**: alphabetical (A-Z) or by date. Default sort on startup is **date descending** (newest first).
- **Label filter**: collapsible section with Whitelist / Blacklist modes and a hierarchical label tree. "Favorites" checkbox is always pinned at the top of the tree.
- **Quick favorites toggle**: star icon on each bookmark card to add/remove the "Favorites" label instantly (persisted to disk immediately)
- **Grid layout**: bookmark cards (thumbnail + name + label chips) in a multi-column, scrollable grid
- **Actions per bookmark**: click to jump, rename (pencil icon), delete (trash icon), toggle favorite (star icon)
- **Semi-transparent dark background** for readability
- **Thumbnail cache**: decoded thumbnails are cached in GPU textures for fast display; the cache is automatically invalidated on sort, delete, or reload to prevent stale images

#### Save / Update (S key or bookmark_add toolbar icon)
- If the current view was reached by clicking a bookmark, pressing **S** (or the bookmark_add toolbar icon) shows a choice dialog: **"Update existing"** (overwrites viewport, params, and thumbnail of the source bookmark) or **"Save as new"** (opens the standard save dialog).
- If no bookmark was active, it opens the save dialog directly. The toolbar icon is dimmed when no bookmark is active as a visual cue.
- The save dialog offers: name input (auto-generated if blank, e.g. `Mandelbrot_000021`), label toggle buttons for all known labels, new-label input with `/`-nesting support, and smart default labels (fractal type, zoom depth, detail level).

#### Fractal Parameters Panel (Bottom-left)
- **Fractal mode selector** — switch between Mandelbrot and Julia sets
- **Julia `c` display** — shows current Julia constant; Shift+Click to pick from the viewport
- **Iteration slider** for quick adjustments within the current range
- **x10 / /10 magnitude buttons** below the slider for rapidly scaling the iteration count by orders of magnitude
- **Escape radius slider** for adjusting the bailout radius
- **Adaptive iterations checkbox** — automatically increases max iterations with zoom depth

#### Legacy Import
Old save files from [MSZP](https://github.com/TonyVallad/MSZP) (QBasic predecessor) can be imported as bookmarks with the `Legacy import` label, preserving center coordinates, zoom level, iteration count, and Julia constants.

### Settings & Application Preferences

User preferences are accessible via the **⚙** icon in the toolbar (always the rightmost icon). The settings window includes:

- **Restore last view on startup** — captures and restores fractal mode, viewport, palette, and AA level
- **Bookmarks folder** — text field with **Browse…** (native folder picker via `rfd`), **Apply**, and **Reset** buttons. The chosen path is persisted across sessions.

Preferences are stored as a JSON file in the OS config directory, using the `directories` crate for cross-platform path resolution.

---

## 11. Export System

### Image Export (Planned)
- High-resolution PNG export
- Offscreen render at arbitrary resolution
- Independent of viewport resolution
- Optional supersampling for final quality

### Animation Export (Planned)
- Camera interpolation between bookmarks (logarithmic scale interpolation for perceptually smooth zoom)
- Frame-by-frame rendering
- PNG sequence output
- Optional integration with `ffmpeg` for MP4 generation

Animations are deterministic and reproducible.

---

## 12. Technology Stack

| Component | Crate |
|---|---|
| Language | Rust |
| UI | `egui` / `eframe`, `egui_material_icons` (Material Symbols icon font) |
| Parallelism | `rayon` |
| Benchmarking | `criterion` |
| Image encoding | `image` (PNG feature) |
| Base64 encoding | `base64` (for embedding thumbnails in bookmark files) |
| Serialization | `serde`, `serde_json` |
| Config paths | `directories` |
| File dialogs | `rfd` (native folder picker for settings) |
| Logging | `tracing` |

No GPU is required; performance is achieved via CPU parallelism and careful architecture.
GPU compute may be added later without redesigning the core.

---

## 13. Project Scope

### v1.0 (Initial Release)

- Native desktop application
- Mandelbrot and Julia set exploration
- Real-time interaction with mouse and keyboard
- Multithreaded tiled renderer with progressive rendering
- Adaptive anti-aliasing (2×2 / 4×4)
- Coloring system with 5 palettes, smooth coloring, and palette popup picker
- Material Symbols icon toolbar with state-aware dimming
- Four-corner HUD layout (viewport info, toolbar, fractal parameters, render stats)
- Controls & shortcuts help window
- Self-contained bookmark files (one `.json` per bookmark with embedded thumbnail) for easy sharing, with labels, favorites, search, sorting, and persistent storage
- Bookmark explorer with independent favorites toggle, fractal tabs, label filtering, and thumbnail previews
- Configurable bookmarks directory with native folder picker
- Application preferences with last-view restoration
- High-resolution image export

### Post-v1.0

- Animation and video export
- Deep zoom techniques (perturbation theory, arbitrary precision)
- Additional fractal types
- GPU compute backend
- Advanced coloring (palette editor, histogram coloring)
