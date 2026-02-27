# MandelbRust — Completed Phases

A concise record of everything that has been implemented. For the plan going forward, see [**roadmap.md**](roadmap.md).

---

## Phase 0 — Foundations & Project Setup

**Objective:** Establish a clean, scalable Rust workspace and development environment.

- [x] Git repository and Cargo workspace (`mandelbrust-core`, `mandelbrust-render`, `mandelbrust-app`)
- [x] `cargo fmt` and `clippy` configured
- [x] Logging infrastructure (`tracing`)
- [x] Crate-level error types (`CoreError`, `RenderError`) and `Result` conventions

---

## Phase 1 — Core Fractal Engine

**Objective:** Implement a correct, fast fractal iteration engine independent of UI.

- [x] `Complex` type with arithmetic operators (`complex.rs`)
- [x] `Viewport` camera model — pixel-to-complex mapping (`viewport.rs`)
- [x] `Fractal` trait with static dispatch for compiler inlining (`fractal.rs`)
- [x] Mandelbrot iteration: escape-time, cardioid/period-2 rejection, Brent's periodicity detection (`mandelbrot.rs`)
- [x] Julia set iteration (`julia.rs`)
- [x] Configurable `FractalParams` (max iterations, escape radius)
- [x] Unit tests for iteration correctness and determinism

---

## Phase 2 — Multithreaded Tiled Renderer

**Objective:** Achieve high CPU utilization using Rayon-based tile parallelism.

- [x] 64×64 tile abstraction for L1 cache locality (`tile.rs`)
- [x] Tiled rendering pipeline with cancellation via generation counter (`renderer.rs`)
- [x] Real-axis symmetry for Mandelbrot (compute top half, mirror bottom)
- [x] Rayon integration for parallel tile execution
- [x] `criterion` benchmarks (iterations/sec, tiles/sec, full-frame time)

---

## Phase 3 — UI & Interaction Layer

**Objective:** Enable real-time exploration with Google Maps-style controls.

- [x] `egui` / `eframe` integration with rendered image display
- [x] Mouse wheel zoom (cursor-centred), click-drag pan, keyboard shortcuts
- [x] HUD: coordinates, zoom level, iteration count, render progress/timing
- [x] Parameter controls (max iterations, escape radius)
- [x] View navigation history (back / forward)

---

## Phase 4 — Progressive Rendering & UX

**Objective:** Make exploration feel instantaneous while maintaining quality.

- [x] Two-pass rendering: low-res preview → full-quality refinement
- [x] Adaptive iteration scaling based on zoom depth
- [x] Border tracing: flood-fill uniform tiles in O(border) instead of O(area)
- [x] `f64` precision warning at scale < 1e-13

---

## Phase 5 — Coloring System & Display Options

**Objective:** Provide flexible, visually appealing color rendering.

- [x] LUT-based palette system with 5 built-in palettes
- [x] Palette switching without recomputing iterations (re-colorize from `IterationBuffer`)
- [x] Smooth coloring via continuous iteration renormalization

---

## Phase 6 — Bookmarks System

**Objective:** Allow persistent saving and restoration of exploration states.

- [x] `Bookmark` struct with fractal type, viewport, parameters, palette, Julia c, metadata
- [x] JSON serialization, one file per bookmark in `bookmarks/` directory
- [x] Bookmark UI: add, delete, rename, jump-to, search, sort
- [x] Application preferences with last-view restore on startup

---

## Phase 7 — Quick Performance Wins

**Objective:** Improve render speed with low-risk, high-reward changes.

Reference: [optimization-report.md](../optimization-report.md) sections 2, 6, 13.

- [x] Release profile: `lto = "fat"`, `codegen-units = 1`
- [x] Cached `escape_radius_sq` in `FractalParams` (field instead of recomputation)
- [x] Reduced periodicity check frequency (skip first 32 iterations, then every 4th)
- [x] Parallel colorization via Rayon (`colorize()`, `colorize_aa()`)
- [x] HashMap for symmetry tile matching (O(1) instead of linear scan)

---

## Phase 8 — Display/Color Settings Model and Profiles

**Objective:** Unified serializable display/color settings, color profiles (one file per profile), and bookmark integration.

Reference: [Features_to_add.md](../Features_to_add.md) §4.

- [x] `DisplayColorSettings` struct: palette index, palette mode (by cycles / by cycle length), start-from (none/black/white with thresholds), smooth coloring
- [x] Coloring pipeline extended for cycle mode and start-from black/white
- [x] Display/color settings panel replacing old palette popup
- [x] `color_profiles/` directory with one JSON file per profile (load/save/list)
- [x] Bookmarks store and restore full `DisplayColorSettings` snapshot

---

## Phase 9 — Minimap

**Objective:** Zoomed-out overview with viewport indicator, toggleable and configurable.

Reference: [Features_to_add.md](../Features_to_add.md) §1.

- [x] Zoomed-out overview image rendered and cached (invalidated only on image-affecting param changes)
- [x] Minimap in bottom-right corner: cyan viewport rectangle, white crosshair (configurable opacity)
- [x] Toggle via M key and toolbar icon; hidden when HUD off
- [x] Square (1:1), range -2..2 default (zoom configurable), size S/M/L, iteration count configurable
- [x] HUD layout unified: render stats moved to bottom-centre; all boxes share margins, rounded corners, no border, configurable opacity (65% default); toolbar unchanged
- [x] Minimap rendered with 4×4 AA; 1px white border (75% opacity)

---

## Phase 10 — Julia C Explorer

**Objective:** Grid of small Julia set previews for choosing the Julia constant c.

Reference: [Features_to_add.md](../Features_to_add.md) §2.

- [x] J key opens grid of square Julia previews; coordinate range -2..2 default, configurable
- [x] Click cell to set c and close explorer; hover shows c coordinates
- [x] Display/color settings changeable from within the explorer
- [x] Grid size and default iterations (100) configurable in settings

---

## Phase 10.5 — J Preview Panel and Julia C Explorer UX

**Objective:** J-key-toggled preview panel and improved Julia C Explorer access.

Reference: [Features_to_add.md](../Features_to_add.md) §3.

- [x] Clicking "Julia" in the bottom-left opens the Julia C Explorer (instead of J key)
- [x] J toggles a preview panel above the minimap (same size/shape/opacity as minimap, 4×4 AA)
- [x] Mandelbrot mode: panel shows live Julia at cursor c (250 iter default, configurable); left-click loads that Julia
- [x] Julia mode: panel shows Mandelbrot with white crosshair at c; updates only when c or display/color change
- [x] Documentation and shortcuts updated

---

## Phase 11 — Deep Zoom: Double-Double Arithmetic

**Objective:** Extend the zoom ceiling from ~10^13× to ~10^28× by representing coordinates with pairs of `f64` values (~31 significant decimal digits). No external dependencies required.

- [x] `DoubleDouble` type (`double_double.rs`): Dekker/Knuth error-free transformations (TwoSum, FMA-based TwoProd), full arithmetic (`Add`, `Sub`, `Mul`, `Neg`, assign variants, scalar `Mul<f64>`), `PartialOrd`, `Display`, helpers (`abs`, `is_positive`, `is_negative`, `to_f64`). 20 unit tests including precision retention.
- [x] `ComplexDD` type (`complex_dd.rs`): mirrors `Complex` with `DoubleDouble` components, full complex arithmetic, `norm_sq()`, `From<Complex>`, `to_complex()`, scalar multiply. 10 unit tests.
- [x] `MandelbrotDD` iteration path (`mandelbrot_dd.rs`): stores viewport center in DD, `iterate()` receives pixel deltas and reconstructs `c = center + delta` in DD precision, periodicity tolerance scaled to `1e-28`. Implements `Fractal` trait with `uses_delta_coordinates() = true`. 6 unit tests.
- [x] `JuliaDD` iteration path (`julia_dd.rs`): same pattern as `MandelbrotDD`. 5 unit tests.
- [x] High-precision viewport center: `Viewport` gains `center_dd: ComplexDD` as authoritative center, with `set_center_dd()`, `offset_center()`, `pixel_to_delta()`, `subpixel_to_delta()` helpers. `center: Complex` kept in sync as f64 approximation.
- [x] Bookmark and preference serialization: `center_re_lo` / `center_im_lo` fields added for lossless DD center round-trip.
- [x] Renderer and AA code respect `uses_delta_coordinates()` to pass pixel deltas instead of absolute coordinates to DD fractals.
- [x] `render_for_mode()` auto-selects DD path when `scale < 1e-13`.
- [x] HUD shows "Precision: f64" or "Precision: f64×2"; precision warning moved to `scale < 1e-28`.
- [x] All zoom/pan operations (scroll zoom, drag pan, arrow-key pan, zoom-rect) are DD-aware.

---

## Phase 12 — Code Reorganization

**Objective:** Restructure the application codebase so that each concern lives in its own module or file, preparing for the upcoming main menu, menu bar, and HUD rework.

- [x] App-level state machine (`AppScreen` enum in `app_state.rs`) ready for future screens (main menu, bookmark browser, Julia C Explorer)
- [x] `main.rs` reduced to ~20 lines (module declarations + `main()`); all logic split into focused modules: `app.rs` (core struct, constructor, enums, `eframe::App` impl), `render_bridge.rs` (background render workers), `navigation.rs` (pan, zoom, history), `input.rs` (mouse/keyboard), `io_worker.rs` (file I/O thread), and a `ui/` subdirectory (`toolbar`, `hud`, `minimap`, `settings`, `help`, `bookmarks`, `julia_explorer`)
- [x] `ActiveDialog` enum replaces mutually exclusive boolean flags (`show_save_dialog`, `show_update_or_save_dialog`); independent floating panels kept as separate booleans to preserve co-existence behaviour
- [x] Dedicated I/O worker thread (`io_worker.rs`) handles all file writes, deletes, and directory scans via `mpsc` channels; `BookmarkStore` and `AppPreferences` dispatch file operations off the UI thread, falling back to synchronous I/O at startup
- [x] Thumbnail cache and failed-thumbnails set keyed by stable string IDs (bookmark filename) instead of vector indices; bounded LRU eviction (max 64 entries); unnecessary `thumbnail_cache.clear()` calls removed after sort operations
- [x] `BookmarkSnap` refactored from a tuple type alias to a proper struct for clarity
- [x] Application behaves identically to before

---

## Phase 13 — Menu Bar

**Objective:** Add a persistent menu bar at the top of the window, visible in every screen and when the HUD is hidden.

- [x] Menu bar implemented via `egui::TopBottomPanel::top` with `egui::MenuBar`, drawn before the central panel to reserve vertical space
- [x] Five menus: **File** (Save Bookmark, Open Bookmarks, Export Image placeholder, Quit), **Edit** (Copy Coordinates, Reset View), **Fractal** (Switch to Mandelbrot/Julia, Julia C Explorer), **View** (Toggle HUD/Minimap/J Preview/Crosshair, Cycle AA, Settings), **Help** (Keyboard Shortcuts, About MandelbRust)
- [x] All menu items wired to existing app actions; keyboard shortcut hints displayed next to items; Export Image greyed out with "Coming in a future update" tooltip
- [x] Menu bar height captured at render time and used to offset all top-anchored HUD elements (top-left viewport info, top-right toolbar, display/color panel, cursor coordinates)
- [x] Menu bar stays visible when HUD is hidden; HUD elements no longer overlap the menu bar
- [x] About MandelbRust dialog window with project name, description, and GitHub link

---

## Phase 14 — Main Menu at Launch

**Objective:** Show a full-window main menu when the app starts, letting the user choose how to begin: resume, Mandelbrot, Julia, or open a bookmark. The fractal explorer is not loaded until a choice is made.

- [x] `AppScreen` enum (`MainMenu`, `FractalExplorer`, `BookmarkBrowser`, `JuliaCExplorer`) as top-level state machine; `MainMenu` is the default on launch
- [x] Main `update()` refactored into a screen dispatcher: menu bar always drawn first, then screen-specific logic, then global overlays (settings, help, about)
- [x] **Main menu screen** (`ui/main_menu.rs`): four horizontal tiles (Resume Exploration, Mandelbrot Set, Julia's Sets, Open Bookmark) on a black background with a vertical separator between the first and remaining tiles
- [x] Tiles have small rounded corners, dark backgrounds with hover effects, preview images (cover mode with aspect-ratio-preserving UV cropping), cyan titles, and centered rich-text descriptions (bold/normal via `**markup**` parsed into per-line `LayoutJob` galleys)
- [x] Tile descriptions show formulas (`Z = Z² + C`) and explain how each fractal works
- [x] **Resume Exploration** tile displays live state: fractal mode, double-double-precision center coordinates (30-digit `format_dd_trimmed`), Julia C coordinates (3-line format), zoom, and iterations — all with trailing zeros stripped
- [x] **Preview images** loaded from `images/previews/` directory: `resume_preview.png`, `mandelbrot_preview.png`, `julia_preview.png`, `bookmarks_preview.png`; lazy-loaded on first menu draw
- [x] Resume preview automatically captured on every final render completion (reuses the already-colorized pixel buffer from `apply_result`, no re-computation), saved as PNG (max 512px wide, Lanczos3 downscale), and loaded as an egui texture
- [x] Preview images stored on disk in `<exe_dir>/images/previews/` folder structure
- [x] Tile layout uses pixel-snapped rects (`snap_rect`) to eliminate sub-pixel gaps between preview images and tile borders
- [x] **Full-window bookmark browser** (`ui/bookmark_browser.rs`): back button, tab bar, search, sort, label filter, bookmark grid with single-click select / double-click open, "Open Bookmark" button
- [x] **Full-window Julia C Explorer** (`ui/julia_explorer.rs` `draw_julia_c_explorer_screen`): back button, grid of Julia previews, C selection transitions to fractal explorer in Julia mode
- [x] Menu bar updated: "Main Menu" item in File menu (saves exploration state when leaving fractal explorer), context-aware enable/disable of Save Bookmark and Open Bookmarks
- [x] Escape key handling is screen-aware: returns to main menu from BookmarkBrowser and JuliaCExplorer
- [x] `app_dir.rs` extended with `images_directory()` and `previews_directory()` helpers

---

## Phase 15 — Image Export

**Objective:** Support high-quality still image exports independent of screen resolution, with full control over color settings. Exported PNGs are saved to organised per-fractal subdirectories with fractal metadata embedded in the file.

- [x] `export_png()` function in `mandelbrust-render/src/export.rs` using the `png` crate for direct PNG encoding with custom tEXt chunk metadata (Software, Description, and MandelbRust.* keys for fractal type, center, zoom, iterations, escape radius, Julia C, AA level, palette, smooth coloring, resolution)
- [x] `ExportMetadata` struct capturing all relevant fractal and render parameters for embedding
- [x] Export dialog (`mandelbrust-app/src/ui/export.rs`): egui window with image name (auto-generated default `{Fractal}_{Iter}_{WxH}`), resolution presets (HD through 8K) plus custom option, max iterations, AA dropdown (Off / 2x2 / 4x4)
- [x] Full editable color settings in the export dialog: palette picker, palette mode (by cycles / by cycle length with numeric input), start-from (None / Black / White with threshold controls), smooth coloring checkbox — all initialized from the viewer's current display/color settings
- [x] Monitor resolution auto-detection for default resolution preset, falling back to 1920x1080
- [x] Export viewport preserves the viewer's visible complex-plane region at any export resolution (dynamic scale recalculation prevents zoomed-out exports at higher resolutions)
- [x] Color fidelity: export uses the same base `max_iterations` as the viewer for cycle length computation, ensuring identical palette mapping regardless of adaptive iterations
- [x] Output to `images/{fractal_name}/` with collision-safe filenames (numeric `_001`, `_002` suffixes)
- [x] Non-blocking background export on a dedicated thread with progress bar and cancel button
- [x] Success/error notification overlay with 5-second fade-out
- [x] `E` keyboard shortcut and File → Export Image menu item (only available from FractalExplorer screen)
- [x] Unit tests for PNG creation and metadata embedding
