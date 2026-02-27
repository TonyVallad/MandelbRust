# Phase 14 — Main Menu at Launch

## Overview

Phase 14 introduces a full-window main menu that displays on application startup, replacing the previous behaviour of immediately loading the fractal explorer. The user now chooses how to begin — resume their previous session, start a fresh Mandelbrot exploration, pick a Julia set, or open a saved bookmark — before any fractal rendering takes place.

The implementation adds three new screens (`MainMenu`, `BookmarkBrowser`, `JuliaCExplorer`) managed by an `AppScreen` state machine, and a new `ui/main_menu.rs` module that handles tile layout, preview image management, and rich-text rendering.

---

## Design Decisions

### Screen dispatch architecture

The application's `update()` method was refactored into a screen dispatcher. The menu bar is always drawn first (via `egui::TopBottomPanel::top`), then the active screen is drawn in the central panel, followed by global overlays (settings, help, about). This keeps the menu bar visible on every screen.

```
update() → draw_menu_bar() → match screen {
    MainMenu        → draw_main_menu()
    FractalExplorer → draw_fractal_explorer()
    BookmarkBrowser → draw_bookmark_browser()
    JuliaCExplorer  → draw_julia_c_explorer_screen()
} → draw global overlays
```

### Tile layout and spacing

Four tiles are arranged horizontally, with a 1px vertical separator between the first tile ("Resume Exploration") and the remaining three. Layout uses manual spacing rather than egui's built-in `horizontal()` item spacing (set to 0) for precise pixel control:

- `tile_gap = 12.0` — spacing between tiles 2–4
- `sep_gap = 14.0` — spacing around the vertical separator
- `x_offset = (available - total_width).max(0) / 2` — centers the tile group; left tile touches the border if the window is too narrow

### Pixel-snapped rects

A `snap_rect()` helper rounds all `egui::Rect` coordinates to exact physical pixel boundaries (`(v * ppp).round() / ppp`). This eliminates sub-pixel rendering artifacts — particularly thin visible gaps between preview images and tile borders caused by fractional logical coordinates.

### Preview images

Each tile displays a preview image in "cover mode" at the top ~40% of the tile. The image's UV coordinates are adjusted to center-crop, preserving the original aspect ratio:

- If the texture is wider than the rect, horizontal margins are cropped (UV inset on X)
- If the texture is taller, vertical margins are cropped (UV inset on Y)
- If aspect ratios match, the full image is shown

Preview images are loaded lazily on the first `draw_main_menu()` frame from `<exe_dir>/images/previews/`. Static previews (Mandelbrot, Julia, bookmarks) are user-provided PNGs. The resume preview is generated automatically.

### Resume preview capture

Rather than capturing a separate render of the fractal, the resume preview reuses the already-colorized RGBA pixel buffer produced by `render_bridge::apply_result()` on every final render. This is zero-cost — no additional fractal computation or colorization is needed:

1. `apply_result()` detects `is_final: true` → calls `update_resume_preview(ctx, &buffer.pixels, width, height)`
2. `update_resume_preview()` calls `save_preview_png()` which downscales to max 512px wide using Lanczos3 filtering
3. The PNG is saved to `images/previews/resume_preview.png`
4. The texture handle is reloaded for immediate display

### Rich text with per-line centering

Tile descriptions use a `**bold**` markup syntax parsed into `egui::LayoutJob` objects. Each line is laid out independently (with `max_width: INFINITY` to prevent wrapping), measured, and painted at `center_x - line_w / 2.0`. This ensures correct horizontal centering for lines with mixed normal/bold text.

### Double-double precision formatting

The resume tile displays coordinates at full double-double precision (~30 digits). A `format_dd_trimmed()` function extracts decimal digits by repeated multiply-by-10 and truncation, then strips trailing zeros. The `format_f64_trimmed()` and `format_f64_signed_trimmed()` variants handle standard `f64` values (e.g. Julia C coordinates).

---

## New / Modified Files

| File | Changes |
|------|---------|
| `mandelbrust-app/src/ui/main_menu.rs` | **New.** `draw_main_menu`, `format_resume_details`, `save_exploration_state`, `update_resume_preview`, `ensure_menu_previews_loaded`, `apply_mandelbrot_defaults`, `draw_tile`, `paint_rich_lines`, `snap_rect`, DD/f64 formatting helpers, `save_preview_png`, `load_preview_texture` |
| `mandelbrust-app/src/ui/bookmark_browser.rs` | **New.** `draw_bookmark_browser` — full-window bookmark explorer with tab bar, search, sort, label filter, bookmark grid, "Open Bookmark" button, back navigation |
| `mandelbrust-app/src/app_state.rs` | **New.** `AppScreen` enum (`MainMenu`, `FractalExplorer`, `BookmarkBrowser`, `JuliaCExplorer`) with `Default → MainMenu` |
| `mandelbrust-app/src/app.rs` | Added `resume_thumbnail`, `mandelbrot_preview`, `julia_preview`, `bookmarks_preview` texture handle fields. `screen` field uses `AppScreen::default()`. `update()` refactored to dispatch by screen |
| `mandelbrust-app/src/app_dir.rs` | Added `images_directory()` and `previews_directory()` helpers |
| `mandelbrust-app/src/render_bridge.rs` | `apply_result` gains `is_final: bool` parameter; calls `update_resume_preview` on final renders |
| `mandelbrust-app/src/ui/menu_bar.rs` | "Main Menu" item in File menu; calls `save_exploration_state()` when leaving the fractal explorer. Context-aware enable/disable of bookmark items |
| `mandelbrust-app/src/ui/julia_explorer.rs` | `draw_julia_c_explorer_screen` for full-window mode (back button, Escape handling) |
| `mandelbrust-app/src/input.rs` | Escape key handling dispatches by `AppScreen` (returns to main menu from bookmark browser and Julia C Explorer) |

---

## API Surface

### `AppScreen` enum

| Variant | Description |
|---------|-------------|
| `MainMenu` | Main menu shown at startup (default) |
| `FractalExplorer` | Fractal rendering / exploration view |
| `BookmarkBrowser` | Full-window bookmark browser |
| `JuliaCExplorer` | Full-window Julia C Explorer |

### `MandelbRustApp` (new `pub(crate)` items)

| Item | Kind | Description |
|------|------|-------------|
| `screen` | `AppScreen` field | Current top-level screen |
| `resume_thumbnail` | `Option<TextureHandle>` | Resume tile preview texture |
| `mandelbrot_preview` | `Option<TextureHandle>` | Mandelbrot tile preview texture |
| `julia_preview` | `Option<TextureHandle>` | Julia tile preview texture |
| `bookmarks_preview` | `Option<TextureHandle>` | Bookmarks tile preview texture |
| `draw_main_menu(&mut self, ctx)` | method | Draws the main menu screen |
| `draw_bookmark_browser(&mut self, ctx)` | method | Draws the full-window bookmark browser |
| `save_exploration_state(&mut self)` | method | Persists viewport, mode, and display/color to preferences |
| `update_resume_preview(&mut self, ctx, pixels, w, h)` | method | Saves resume preview PNG and loads texture |
| `apply_mandelbrot_defaults(&mut self)` | method | Resets to default Mandelbrot state |

### Free functions in `main_menu.rs`

| Function | Description |
|----------|-------------|
| `save_preview_png(pixels, w, h, path)` | Resizes RGBA buffer to max 512px and saves as PNG |
| `load_preview_texture(ctx, path)` | Loads PNG from disk into an egui `TextureHandle` |
| `format_dd_trimmed(dd)` | Formats `DoubleDouble` with full precision, trimmed zeros |
| `format_f64_trimmed(v)` | Formats `f64` with 15 decimal places, trimmed zeros |
| `snap_rect(r, ppp)` | Rounds rect to physical pixel boundaries |

---

## Preview Image Storage

```
<exe_dir>/
  images/
    previews/
      resume_preview.png      # auto-generated on every final render
      mandelbrot_preview.png   # user-provided
      julia_preview.png        # user-provided
      bookmarks_preview.png    # user-provided
```

The `images/previews/` directory is created automatically by `update_resume_preview()`. Static preview PNGs are supplied by the user. The directory structure is prepared for future image export features (the `images/` parent can hold exported images).
