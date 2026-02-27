<p align="center" style="margin-bottom: 0em;">
  <img src="docs/img/icon.png" alt="MandelbRust icon">
</p>
<h1 align="center">MandelbRust</h1>
<p align="center">A high-performance, native fractal explorer written in <strong>Rust</strong>.</p>

<p align="center">
  <img src="docs/img/Screenshot_Main.png" alt="MandelbRust — Julia set exploration" width="800">
</p>

MandelbRust provides real-time, interactive exploration of the Mandelbrot set and Julia sets using a Google Maps-like navigation model. It is built around heavy multithreading, progressive rendering, and a clean separation between math, rendering, and UI.

This project is the modern successor to [MSZP](https://github.com/TonyVallad/MSZP), a QBasic fractal explorer by the same author.

## Status

**Active development** — the core explorer is fully functional with real-time rendering, deep zoom (double-double precision up to ~10^28x), multiple color palettes, adaptive anti-aliasing, a persistent bookmark system, a main menu at launch, and an optimized release profile. See the [roadmap](docs/roadmap/roadmap.md) for what's coming next.

---

## Features

### Main menu

On launch, MandelbRust presents a full-window main menu with four choices: **Resume Exploration** (restore your last session with a live preview and full-precision coordinates), **Mandelbrot Set** (start fresh with defaults), **Julia's Sets** (open the Julia C Explorer to pick a constant), or **Open Bookmark** (browse saved locations). The fractal explorer only loads after you make a selection.

### Real-time exploration

Navigate the fractal plane with Google Maps-style controls: scroll to zoom at cursor, drag to pan, arrow keys for precise movement. Every interaction triggers an instant low-resolution preview that seamlessly refines to full quality in the background. Right-click and drag to draw a selection rectangle, then release to zoom into that exact region.

<p align="center">
  <img src="docs/img/Screenshot_Zoom_using_drag.png" alt="Selection-box zoom via right-click drag" width="800">
</p>

A view history stack (up to 200 entries) supports back/forward navigation so you can retrace your steps without needing bookmarks.

### Mandelbrot and Julia sets

Explore the Mandelbrot set (default) or Julia sets. Click **Julia** in the bottom-left parameters panel to open the **Julia C Explorer** — a grid of small Julia set previews where you can pick a constant `c` visually. In Julia mode, fine-tune `c` with drag-value inputs or Shift+Click anywhere on the main view.

Press **J** to toggle the **J preview panel** above the minimap. In Mandelbrot mode it shows a live Julia preview at your cursor — left-click to instantly load that Julia set. In Julia mode it shows a Mandelbrot overview with a crosshair marking your current `c`.

### Deep zoom

Standard `f64` arithmetic limits useful zoom to roughly 10^13x. MandelbRust automatically switches to **double-double precision** (two `f64` values per coordinate, ~31 significant digits) when you zoom past this threshold, extending the zoom ceiling to approximately **10^28x** with no loss of interactivity. The active precision mode is shown in the HUD ("f64" or "f64x2").

All navigation operations — scroll zoom, drag pan, arrow-key pan, zoom-rect, bookmarks, undo/redo — preserve full double-double precision so you never lose your position at deep zoom.

### Multithreaded tiled rendering

The viewport is divided into **64x64 pixel tiles** (sized to fit in L1 cache) and rendered in parallel across all CPU cores using Rayon's work-stealing thread pool. All rendering happens on a dedicated background thread communicating with the UI via channels, so the interface never freezes.

Additional optimizations reduce unnecessary work:
- **Border tracing** — if all border pixels of a tile share the same iteration count, the interior is flood-filled without computing individual pixels
- **Real-axis symmetry** — for the Mandelbrot set, when the viewport straddles the real axis, only the top half is computed and mirrored
- **Cardioid and period-2 bulb checks** — closed-form tests that skip iteration entirely for ~30-40% of visible points at default zoom
- **Periodicity detection** (Brent's algorithm) — detects orbital cycles to exit early for interior points
- **Pan preservation** — during drag, previously rendered pixels are shifted in-place; only newly exposed edges need rendering

### Adaptive iterations

Max iterations automatically increase with zoom depth to reveal finer fractal detail. The formula adds iterations proportional to log2(zoom). This is toggleable; you can also set a manual ceiling via the iteration slider.

### Color palettes and display settings

<p align="center">
  <img src="docs/img/Screenshot_Color_Palette.png" alt="Palette picker popup with Fire palette" width="800">
</p>

Five built-in color palettes — Classic, Fire, Ocean, Neon, Grayscale — stored as 256-entry gradient lookup tables. Switching palettes is instant: the iteration data is cached separately, so re-colorizing doesn't require re-rendering.

Smooth coloring uses the continuous iteration formula `v = n + 1 - log2(ln|z_n|)` to eliminate banding between iteration levels.

The display/color settings panel provides full control over:
- **Palette mode** — by number of cycles or by cycle length (same palette, different mapping)
- **Start-from** — fade from black or white for the first few iterations (MSZP-inspired)
- **Smooth coloring** — toggle continuous vs banded coloring
- **Color profiles** — save and load complete display/color configurations as shareable files

### Adaptive anti-aliasing

Boundary-aware supersampling that targets only edge pixels where the iteration count differs between neighbors. Interior regions are untouched. Choose between 2x2 and 4x4 sampling via the **A** key or the toolbar icon. AA data is preserved during panning so previously smoothed regions stay sharp.

### Minimap

<p align="center">
  <img src="docs/img/Screenshot_Minimap.png" alt="Minimap with viewport indicator" width="400">
</p>

A zoomed-out overview rendered at the bottom-right corner. In Mandelbrot mode it shows the full Mandelbrot set; in Julia mode it shows the current Julia set. A cyan rectangle marks your viewport, with white crosshair lines extending to the edges. The minimap is rendered with 4x4 AA and is configurable in size (Small / Medium / Large), opacity, and complex-plane range. Toggle it with **M** or the toolbar icon.

### Bookmark system

<p align="center">
  <img src="docs/img/Screenshot_Bookmark_Explorer.png" alt="Bookmark explorer open over a Mandelbrot render" width="800">
</p>

<p align="center">
  <img src="docs/img/Screenshot_Bookmark_Explorer_Window.png" alt="Bookmark explorer window close-up" width="550">
</p>

Every exploration state can be saved as a bookmark capturing the complete configuration: fractal type, viewport center and zoom, iteration parameters, palette and display/color settings, anti-aliasing level, Julia constant, and a PNG thumbnail. Each bookmark is a **single self-contained JSON file** with the thumbnail embedded as base64 — just copy a file to share a location.

The **bookmark explorer** (press **B**) provides:
- **Tabs** — All, Favorites, Mandelbrot, Julia (combinable with favorites filter)
- **Search** — filter across names and labels
- **Sort** — alphabetical or by date (newest first by default)
- **Hierarchical labels** — with whitelist/blacklist filtering and a collapsible label tree
- **Quick favorites** — star icon on each card for instant toggling
- **Thumbnail grid** — scrollable multi-column layout with cached GPU textures

Pressing **S** after navigating from a bookmark offers to **update it in-place** or **save as new**. Bookmarks are persisted immediately to disk — no "save on exit" step.

The bookmarks directory is configurable from Settings with a native folder picker.

### Menu bar

A persistent menu bar at the top of the window provides quick access to all major features: **File** (bookmarks, export, quit), **Edit** (copy coordinates, reset view), **Fractal** (switch mode, Julia C Explorer), **View** (toggle HUD/minimap/J preview/crosshair, cycle AA, settings), and **Help** (shortcuts, about). The menu bar is always visible, even when the HUD is hidden.

### HUD and toolbar

The heads-up display is distributed across the screen corners for minimal intrusion (all elements are positioned below the menu bar):

| Area | Content |
|------|---------|
| **Top-left** | Viewport info: mode, center, zoom, iterations, precision, palette, warnings |
| **Top-right** | Material Symbols icon toolbar with state-aware dimming (navigation, palette, AA, smooth coloring, bookmarks, minimap, help, settings) |
| **Top-right** (below toolbar) | Cursor complex coordinates (when crosshair is enabled) |
| **Bottom-left** | Fractal parameters: mode selector, Julia c inputs, iteration slider with x10/÷10 buttons, escape radius, adaptive iterations toggle |
| **Bottom-centre** | Render stats: phase, timing, tile counts, AA status |
| **Bottom-right** | J preview panel (when on) and minimap (when on) |

Press **H** to hide everything except the menu bar. All HUD panels share a unified style with configurable background opacity (65% default).

### Session persistence

Your complete state — display/color settings, last view (fractal mode, viewport position, zoom, palette, AA level) — is automatically saved on exit and restored on startup.

### Legacy import

Old save files from [MSZP](https://github.com/TonyVallad/MSZP) (the QBasic predecessor) can be imported as bookmarks, preserving coordinates, zoom, iterations, and Julia constants.

---

## Controls

### Mouse

| Action | Effect |
|--------|--------|
| Scroll wheel | Zoom at cursor |
| Left-drag | Pan |
| Left-click (Mandelbrot mode, J preview on) | Load Julia set at cursor c |
| Right-drag | Selection rectangle zoom |
| Shift + Click (Julia mode) | Set Julia constant c from cursor |

### Keyboard

| Key | Action |
|-----|--------|
| Arrow keys | Pan viewport |
| `+` / `-` | Zoom in / out (centred) |
| `R` | Reset view to default |
| `H` | Toggle entire HUD |
| `C` | Toggle crosshair |
| `A` | Cycle AA (Off / 2x2 / 4x4) |
| `S` | Save / update bookmark |
| `B` | Toggle bookmark explorer |
| `M` | Toggle minimap |
| `J` | Toggle J preview panel |
| `Backspace` | View history back |
| `Shift+Backspace` | View history forward |
| `Escape` | Close dialogs, cancel render |

<p align="center">
  <img src="docs/img/Screenshot_Controls_and_Shortcuts.png" alt="Controls and shortcuts window" width="280">
</p>

Press the help icon in the toolbar to see the full controls reference in-app.

---

## Technology Stack

| Component | Crate |
|-----------|-------|
| Language | Rust |
| UI | `egui` / `eframe`, `egui_material_icons` |
| Parallelism | `rayon` |
| Benchmarking | `criterion` |
| Image encoding | `image` |
| Base64 encoding | `base64` |
| Serialization | `serde`, `serde_json` |
| Config paths | `directories` |
| File dialogs | `rfd` |
| Logging | `tracing` |

No GPU required. Performance comes from CPU parallelism and careful architecture. The release profile uses full LTO (`lto = "fat"`) and a single codegen unit for maximum cross-crate inlining.

---

## Quick Start

```bash
# Build and run (requires Rust toolchain)
cargo run -p mandelbrust-app

# Run tests
cargo test --workspace

# Run benchmarks
cargo bench -p mandelbrust-render
```

## Project Structure

```
MandelbRust/
  mandelbrust-core/       # math, fractals, iteration engine
  mandelbrust-render/     # tiled renderer, coloring, multithreading
  mandelbrust-app/        # UI and user interaction
    src/
      main.rs             # entry point (~20 lines)
      app.rs              # core struct, constructor, shared types
      render_bridge.rs    # background render workers
      navigation.rs       # pan, zoom, view history
      input.rs            # mouse/keyboard event handling
      io_worker.rs        # file I/O worker thread
      bookmarks.rs        # bookmark data and persistence
      preferences.rs      # user preferences
      ui/                    # UI modules
        main_menu.rs         # main menu screen, tile layout, preview management
        menu_bar.rs          # persistent top menu bar
        bookmark_browser.rs  # full-window bookmark browser
        toolbar.rs           # icon toolbar
        hud.rs               # viewport info, render stats
        minimap.rs           # minimap panel
        settings.rs          # settings panel
        help.rs              # shortcuts window
        bookmarks.rs         # bookmark explorer overlay
        julia_explorer.rs    # Julia C Explorer grid
  docs/                   # project documentation
```

## Documentation

- [**Project Overview**](docs/overview.md) — architecture, design decisions, and full technical specification
- [**Roadmap**](docs/roadmap/roadmap.md) — phased development plan
- [**Completed Phases**](docs/roadmap/roadmap-completed.md) — record of all completed work
- [**Deep Zoom Analysis**](docs/deep-zoom-analysis.md) — precision options and techniques for extreme zoom depths
- [**Optimization Report**](docs/optimization-report.md) — technical analysis of performance opportunities

## License

This project is licensed under the [MIT License](LICENSE).
