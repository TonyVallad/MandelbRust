# Phase 15 — Image Export

## Overview

Phase 15 adds a complete image export pipeline: an interactive export dialog with full render and color settings, a background export worker, and PNG output with embedded fractal metadata. Users can export at any resolution (up to 8K and beyond) while preserving the exact on-screen view and color appearance.

---

## Architecture

The feature spans two crates:

| Layer | File | Responsibility |
|-------|------|----------------|
| Render | `mandelbrust-render/src/export.rs` | PNG encoding with tEXt metadata via the `png` crate |
| App | `mandelbrust-app/src/ui/export.rs` | Export dialog UI, background worker, state management |

The export reuses the existing `render_for_mode()` function from `render_bridge.rs`, supporting both f64 and double-double precision, real-axis symmetry, border tracing, and adaptive anti-aliasing — the same pipeline as the interactive viewer.

---

## Export Dialog

The dialog opens via the **E** key or **File → Export Image** (only available from the FractalExplorer screen). It is a centered, non-resizable `egui::Window` with the following fields:

### Render settings

- **Image name** — text input. If left empty, a default is generated: `{Fractal}_{MaxIter}_{WxH}` (e.g. `Mandelbrot_256_3840x2160`).
- **Resolution** — dropdown with presets: 1280×720, 1920×1080, 2560×1440, 3840×2160, 5120×2880, 7680×4320, plus a "Custom" option that reveals editable width/height fields. The default selection matches the monitor resolution if detected, otherwise 1920×1080.
- **Max iterations** — text input, initialized to the viewer's base `params.max_iterations` (not the effective value from adaptive iterations, to ensure consistent palette mapping).
- **Anti-aliasing** — dropdown: Off, 2×2, 4×4.

### Color settings

A full copy of the viewer's `DisplayColorSettings` is cloned into the export state when the dialog opens, providing editable controls that default to the current viewer settings:

- **Palette** — dropdown listing all built-in palettes.
- **Palette mode** — toggle between "By cycles" and "By cycle length" with a numeric `DragValue` for the cycle count or cycle length.
- **Start from** — selectable: None, Black, White. When Black or White is selected, threshold start/end `DragValue` controls appear.
- **Smooth coloring** — checkbox.

These controls operate on the export state's own `DisplayColorSettings` instance, so changes do not affect the viewer.

---

## Viewport Scaling

A critical design decision: the export viewport must show the **same complex-plane region** as the viewer, regardless of export resolution. Simply reusing the viewer's `scale` (complex units per pixel) would cause higher resolutions to cover a larger area, appearing "zoomed out."

The fix computes a new scale for the export:

```rust
let viewer_complex_w = self.viewport.complex_width();
let viewer_complex_h = self.viewport.complex_height();
let export_scale = (viewer_complex_w / w as f64).max(viewer_complex_h / h as f64);
```

This preserves the visible extent: `export_scale * w ≈ viewer_complex_w`, so the exported image covers the same (or slightly larger, to avoid cropping due to aspect ratio differences) region of the complex plane.

---

## Color Fidelity

The viewer computes `cycle_length` from `self.params.max_iterations` (the base slider value). With adaptive iterations enabled, the effective iteration count can be higher, but the colorization always uses the base value.

The export dialog initializes its max_iterations field from the same base value, ensuring that `cycle_length = max_iterations / n` (for `ByCycles` mode) matches exactly between the viewer and the export. This eliminates palette offset or cycle count discrepancies.

---

## PNG Metadata

Exported PNGs embed fractal parameters as **tEXt chunks** (Latin-1 encoded key-value pairs), readable by exiftool, IrfanView, XnView, and similar tools:

| Key | Content |
|-----|---------|
| `Software` | "MandelbRust" |
| `Description` | Human-readable summary (fractal type, center, zoom, iterations, Julia C) |
| `MandelbRust.FractalType` | "Mandelbrot" or "Julia" |
| `MandelbRust.CenterRe` | Real part of center (15 decimal places) |
| `MandelbRust.CenterIm` | Imaginary part of center (15 decimal places) |
| `MandelbRust.Zoom` | Inverse scale in scientific notation |
| `MandelbRust.MaxIterations` | Max iteration count used |
| `MandelbRust.EscapeRadius` | Escape radius |
| `MandelbRust.AALevel` | 0, 2, or 4 |
| `MandelbRust.Palette` | Palette name |
| `MandelbRust.SmoothColoring` | "true" or "false" |
| `MandelbRust.Resolution` | "WxH" |
| `MandelbRust.JuliaC_Re` | Julia constant real part (if Julia) |
| `MandelbRust.JuliaC_Im` | Julia constant imaginary part (if Julia) |

The `png` crate is used directly (rather than through `image`) to access the `add_text_chunk` API. It is declared as a workspace dependency (`png = "0.17"`) and added to `mandelbrust-render/Cargo.toml`.

---

## Output Path

Exported files are saved to `<exe_dir>/images/{fractal_name}/` where `{fractal_name}` is the lowercase fractal type (`mandelbrot` or `julia`). The directory is created automatically.

Filename collisions are resolved by appending numeric suffixes: `name_001.png`, `name_002.png`, etc.

---

## Background Worker

The export runs on a dedicated thread (`export-worker`) to avoid blocking the UI:

1. `start_export()` collects all settings, builds the `ExportJob` struct, and spawns the thread.
2. The worker calls `render_for_mode()` (same function as the viewer), then `export_png()`.
3. Results are sent back via `mpsc::channel` as `ExportWorkerResult::Success(path)` or `ExportWorkerResult::Error(msg)`.
4. `poll_export_result()` checks the channel on each frame and updates the notification.

Progress is tracked via the existing `RenderCancel` atomic counters, displayed as a progress bar in the dialog. The cancel button calls `cancel()` on the shared `RenderCancel`, which the render loop checks between tiles.

---

## New and Modified Files

| File | Change |
|------|--------|
| `mandelbrust-render/src/export.rs` | **New.** `ExportMetadata` struct, `export_png()` function, metadata builder helpers, unit tests |
| `mandelbrust-render/src/lib.rs` | Added `pub mod export` and re-exports |
| `mandelbrust-render/Cargo.toml` | Added `png.workspace = true` dependency |
| `Cargo.toml` (workspace) | Added `png = "0.17"` to workspace dependencies |
| `mandelbrust-app/src/ui/export.rs` | **New.** `ExportState`, `ExportWorkerResult`, resolution presets, dialog UI, background worker, helpers |
| `mandelbrust-app/src/ui/mod.rs` | Added `pub(crate) mod export` |
| `mandelbrust-app/src/app.rs` | Added `export_state: ExportState` and `egui_ctx: egui::Context` fields; `draw_export_dialog()` call in `update()` |
| `mandelbrust-app/src/input.rs` | Added `E` key shortcut to open export dialog |
| `mandelbrust-app/src/ui/menu_bar.rs` | Enabled "Export Image..." menu item, wired to `open_export_dialog()` |

---

## Tests

- `export_creates_valid_png` — writes a small test image, verifies the PNG signature.
- `export_embeds_text_chunks` — writes a Julia export, reads it back, verifies Software, FractalType, and JuliaC_Re text chunks are present.
