# MandelbRust — Development Roadmap v2

Phases 0–7 are complete. This roadmap covers everything from Phase 8 onward.

**Next development focus:** Phases 8–10 implement the planned features from **`Features_to_add.md`** (Display/color settings and profiles, Minimap, Julia C Explorer). Each phase is written so an AI agent can execute the tasks in order without ambiguity. Full behaviour and edge cases are specified in `Features_to_add.md`; the roadmap breaks implementation into digestible steps.

Each phase is a self-contained unit of work that produces a testable, working state.

> **Rules for every phase**
>
> - All work **must respect** [overview.md](overview.md). If a change alters the project's scope or architecture, update overview.md first and ask for confirmation.
> - Run `cargo clippy --workspace` after every task. Zero warnings.
> - Run `cargo test --workspace` after every task. All tests pass.
> - Keep functions small and pure. Prefer early returns over nesting. Add type hints on public functions. Add docstrings only when behaviour is not obvious.
> - Only modify files directly relevant to the task. Summarise multi-file changes and ask before implementing.
> - Before adding a dependency, justify it and ask for approval.
> - **Keep files from getting too long.** When a file would grow large, split it into smaller modules or extract logic into new files (e.g. separate UI panels, helpers, or submodules). Prefer many focused files over a few very long ones.

---

## Completed Phases

| Phase | Description | Status |
|---|---|---|
| 0 | Foundations & Project Setup | Done |
| 1 | Core Fractal Engine | Done |
| 2 | Multithreaded Tiled Renderer | Done |
| 3 | UI & Interaction Layer | Done |
| 4 | Progressive Rendering & UX | Done |
| 5 | Coloring System & Display Options | Done |
| 6 | Bookmarks System | Done |
| 7 | Quick Performance Wins | Done |
| 8 | Display/color settings model and profiles | Done |
| 9 | Minimap | Done |

---

## Phase 7 — Quick Performance Wins ✅

**Objective:** Improve render speed with low-risk, high-reward changes that require no architectural modifications.

**Reference:** [optimization-report.md](optimization-report.md) sections 2, 6, 13.

### Task 7.1 — Release profile optimization ✅

**File:** `Cargo.toml` (workspace root)

Add a `[profile.release]` section:

```toml
[profile.release]
lto = "fat"
codegen-units = 1
```

**Verify:** `cargo build --release` completes. Run `cargo bench -p mandelbrust-render` before and after. Log the speedup.

---

### Task 7.2 — Cache `escape_radius_sq` in `FractalParams` ✅

**File:** `mandelbrust-core/src/fractal.rs`

Currently `escape_radius_sq()` recomputes `escape_radius * escape_radius` on every call. Instead:

1. Add a private field `escape_radius_sq: f64` to `FractalParams`.
2. Compute it once in `FractalParams::new()` and in the `Default` impl.
3. Replace the `escape_radius_sq()` method body with a field read.
4. Update any code that mutates `escape_radius` to also update `escape_radius_sq`.

**Verify:** All existing tests pass. `FractalParams::escape_radius_sq()` returns the same value as before.

---

### Task 7.3 — Reduce periodicity check frequency ✅

**Files:** `mandelbrust-core/src/mandelbrot.rs`, `mandelbrust-core/src/julia.rs`

In both `iterate()` methods, the Brent's cycle detection comparison runs every iteration. Change it to:
1. Skip the cycle check entirely for the first 32 iterations.
2. After that, only check every 4th iteration (use `n & 3 == 0`).

**Verify:** All existing tests pass. Run `cargo bench -p mandelbrust-render` and log the difference for deep-zoom benchmarks.

---

### Task 7.4 — Parallelize colorization ✅

**File:** `mandelbrust-render/src/palette.rs`

The `colorize()` and `colorize_aa()` methods iterate sequentially over every pixel. Convert them to use Rayon:

1. In `colorize()`: replace the `for idx in 0..len` loop with `par_chunks_mut(4)` on the pixel buffer zipped with `par_iter()` on the iteration data.
2. In `colorize_aa()`: same approach — use `par_chunks_mut(4)` zipped with enumerated indices.
3. Add `rayon` as a dependency of `mandelbrust-render` (it is already in the workspace dependencies).

**Verify:** All existing tests pass. `cargo clippy --workspace` has zero warnings. The rendered image is byte-identical to the sequential version for a fixed test viewport.

---

### Task 7.5 — HashMap for symmetry tile matching ✅

**File:** `mandelbrust-render/src/tile.rs`

`find_tile_at()` does a linear scan of all tiles. Replace it with a `HashMap` lookup:

1. In `classify_tiles_for_symmetry()`, before the classification loop, build a `HashMap<(u32, u32), usize>` mapping `(tile.x, tile.y)` to tile index.
2. Replace calls to `find_tile_at()` with a `.get(&(mirror_x, mirror_y))` lookup.
3. Remove the `find_tile_at()` function if it's no longer used.

**Verify:** All existing tests pass. Render output unchanged.

---

### Deliverables — Phase 7

- [x] Release profile has `lto = "fat"` and `codegen-units = 1`
- [x] `escape_radius_sq` is cached, not recomputed
- [x] Periodicity check runs less frequently (skip first 32, then every 4th)
- [x] `colorize()` and `colorize_aa()` use Rayon
- [x] Symmetry matching uses `HashMap` instead of linear search
- [ ] All benchmarks re-run; results logged in a comment or benchmark output

---

## Phase 8 — Display/color settings model and profiles ✅

**Objective:** Introduce a single, serializable display/color settings model; replace the palette icon with a Display/color settings panel; add color profiles (one file per profile in `color_profiles/`); extend bookmarks to store the full display/color snapshot.

**Reference:** [Features_to_add.md](../Features_to_add.md) §3; [overview.md](overview.md) §9, §10, §13 (Planned features).

### Task 8.1 — Define `DisplayColorSettings` and use it in app state ✅

**Files:** new file `mandelbrust-app/src/display_color.rs` (or `mandelbrust-render` if the struct is used headlessly), `mandelbrust-app/src/main.rs`, `mandelbrust-app/src/preferences.rs`

1. Define a struct `DisplayColorSettings` with `serde::Serialize` and `serde::Deserialize` containing:
   - `palette_index: usize` (or palette id/name for built-in palettes)
   - `palette_mode: PaletteMode` (enum: `ByCycles(u32)` = number of cycles, `ByCycleLength(u32)` = cycle length in iterations)
   - `start_from: StartFrom` (enum: `None`, `Black`, `White`)
   - `low_threshold_start: u32`, `low_threshold_end: u32` (only used when `start_from != None`)
   - `smooth_coloring: bool` (log-log / continuous iteration toggle)
   - Optional later: `fade_to_black`, etc.
2. Implement `Default` for `DisplayColorSettings` (e.g. palette 0, one cycle, no start-from, smooth on).
3. In the app state, replace the existing `palette_index` and `smooth_coloring` (and any other scattered display/color fields) with a single `display_color: DisplayColorSettings` field. Wire the UI and render path to read from `display_color`.
4. Ensure the render pipeline (and any code that chooses palette or smooth flag) receives these settings (e.g. pass `&DisplayColorSettings` into the colorize path).

**Verify:** `cargo test --workspace` passes. Application behaves as before: palette selection and smooth-coloring toggle still work, now backed by `DisplayColorSettings`.

---

### Task 8.2 — Extend coloring pipeline for cycle mode and start-from black/white ✅

**Files:** `mandelbrust-render/src/palette.rs`, `mandelbrust-core` (if `IterationResult` is used)

1. **Cycle mode:** When mapping iteration result to palette index, support:
   - `ByCycles(n)`: effective cycle length = `max_iterations / n`; position in cycle = (smooth or raw) iteration modulo cycle length, normalized to `[0, 1)`.
   - `ByCycleLength(len)`: position in cycle = iteration modulo `len`, normalized to `[0, 1)`.
   The existing LUT sampling stays; only the mapping from iteration value to `t` (index into the gradient) changes. Add a parameter or config to `Palette::color()` / `colorize()` so the render crate can pass cycle mode and max_iterations.
2. **Start-from black/white:** For iterations below `low_threshold_start`, output solid black (if StartFrom::Black) or solid white (if StartFrom::White). Between `low_threshold_start` and `low_threshold_end`, linearly blend from that solid color to the palette color. Above `low_threshold_end`, use only the palette (and existing interior = black). Add these parameters to the coloring path; they can live on `DisplayColorSettings` and be passed through.
3. Keep the existing smooth iteration formula `ν = n + 1 − log₂(ln|zₙ|)`; the `smooth_coloring` flag only toggles whether to use it or raw integer count.

**Verify:** Unit tests: (1) cycle mode changes repeat frequency of the palette; (2) start-from black/white produces solid band then gradient then normal palette. Manual check: changing cycles and thresholds in the UI updates the image without re-render (re-colorize only).

---

### Task 8.3 — Replace palette icon with Display/color settings panel ✅

**Files:** `mandelbrust-app/src/main.rs` (or `ui/toolbar.rs`, `ui/palette_popup.rs` if split in Phase 12)

1. Replace the current **palette** toolbar icon with a **Display/color settings** icon (choose a Material Symbol that suggests “palette + settings”, e.g. `palette` or `tune`; keep it in the same toolbar position).
2. Clicking the icon opens a **panel** (floating window or side panel) instead of the current palette popup. The panel contains:
   - **Palette:** dropdown or list of built-in palettes (same as current behaviour; selection updates `display_color.palette_index`).
   - **Palette mode:** radio or dropdown “By number of cycles” / “By cycle length” plus a number input (cycles count or cycle length in iterations).
   - **Start from:** None / Black / White; when not None, show numeric inputs for `low_threshold_start` and `low_threshold_end`.
   - **Smooth coloring (log-log):** checkbox, same as current smooth toggle (writes to `display_color.smooth_coloring`).
3. All edits apply immediately to the current session (re-colorize from current `IterationBuffer` where possible; no full re-render unless a future option requires it).
4. Do **not** add profile load/save in this task (Task 8.4). The panel only edits the current `DisplayColorSettings`.

**Verify:** User can change every display/color setting from the new panel. Toolbar no longer shows the old palette-only popup. Behaviour matches Features_to_add.md for the “Display/color settings icon and panel” bullet.

---

### Task 8.4 — Color profiles: one file per profile in `color_profiles/` ✅

**Files:** `mandelbrust-app/src/display_color.rs`, new or existing module for profile I/O, `mandelbrust-app/src/main.rs`

1. **Profile directory:** Resolve a `color_profiles` folder at the **program root** (directory of the executable, or workspace root in dev). Use `std::env::current_exe()` and strip the filename to get the directory, or use an existing app-base-path helper. Create the folder if it does not exist.
2. **One file per profile:** Each profile is a JSON file (human-readable). Filename = profile name sanitized for the filesystem (e.g. replace invalid chars with `_`) + `.json`. Content = serialized `DisplayColorSettings` (and optionally a `name` field for display). No subfolders; flat list.
3. **Panel integration:** In the Display/color settings panel, add:
   - **Profile list:** List or dropdown of existing profiles (scan `color_profiles/` for `*.json`; display name = filename without `.json` or a `name` field inside the file).
   - **Load:** Selecting a profile (or “Load” button) reads the file and applies the stored `DisplayColorSettings` to the current session.
   - **Save:** “Save as profile” button: prompt for a name (or use “Default” if none), serialize current `DisplayColorSettings` to `color_profiles/{name}.json`. Overwrite if exists.
4. Ensure at least one default profile exists on first run (e.g. create `Default.json` from current defaults if the folder is empty).

**Verify:** Saving a profile creates a file in `color_profiles/`. Loading a profile applies its settings. Copying a profile file to another machine and loading it works (shareable profiles).

---

### Task 8.5 — Bookmarks store and restore full `DisplayColorSettings` ✅

**Files:** `mandelbrust-app/src/bookmarks.rs`, `mandelbrust-app/src/main.rs`

1. Extend the `Bookmark` struct to include a **full snapshot** of display/color settings: add a field `display_color: DisplayColorSettings` (or embed all fields that `DisplayColorSettings` contains). Remove or deprecate storing only a “profile name” for color; the bookmark must be self-contained.
2. **On save bookmark:** Serialize the current `display_color` (from app state) into the bookmark file. Optionally also store `active_profile_name: Option<String>` for reference.
3. **On load bookmark (jump to bookmark):** Apply the bookmark’s `display_color` to the app state (overwriting current display/color settings for the session). Re-colorize the view from the current `IterationBuffer`; no full re-render unless needed.
4. **Backward compatibility:** When loading an old bookmark JSON that does not have `display_color`, use a default `DisplayColorSettings` (or infer from old `palette_index` / `smooth_coloring` if present).

**Verify:** Save a bookmark with custom cycle count and start-from white; reopen app, load the bookmark; display/color state matches. No dependency on a profile file for bookmark restore.

---

### Deliverables — Phase 8

- [x] `DisplayColorSettings` struct defined, serializable, used everywhere display/color is decided
- [x] Coloring pipeline supports palette mode (by cycles / by cycle length) and start-from black/white with thresholds
- [x] Palette icon replaced by Display/color settings icon; panel edits all display/color options
- [x] `color_profiles/` folder; one JSON file per profile; load/save/list in panel
- [x] Bookmarks contain and restore full `DisplayColorSettings` snapshot
- [x] Overview and Features_to_add.md behaviour for §3 (Display and color settings) satisfied for these items

---

## Phase 9 — Minimap ✅

**Objective:** Show a zoomed-out overview of the fractal with a viewport indicator; toggle with M key or toolbar icon; hide when HUD is off; size and styling configurable.

**Reference:** [Features_to_add.md](../Features_to_add.md) §1.

### Task 9.1 — Render and cache zoomed-out overview image ✅

**Files:** `mandelbrust-app` (minimap state and render trigger), `mandelbrust-render` (optional helper for fixed-iteration render)

1. **Minimap image:** Render a **square** image (aspect ratio **1:1**). The complex-plane range is **-2 to 2** on both axes by default; **zoom** (scale / range shown) is **configurable in the settings menu**. Use a **fixed max iteration count** (default **500**; configurable in settings). Resolution is square (e.g. 256×256) to match the 1:1 aspect ratio; size follows the minimap widget side length (small/medium/large).
2. **Cache:** Store the resulting pixel buffer (and optionally the iteration buffer) in app state. **Invalidate** the cache whenever any parameter that would change the image changes: fractal mode, Julia C, max iterations for minimap, palette/display color settings, or any other setting that affects the zoomed-out look. Do not invalidate on viewport pan/zoom of the main view.
3. Render the minimap image on a background thread (reuse the existing render channel or a one-off) so the UI stays responsive. When cache is invalid, show a placeholder (e.g. “Updating…” or previous frame) until the new minimap is ready.

**Verify:** After changing Julia C or color settings, the minimap updates. After only panning/zooming the main view, the minimap does not re-render (cache hit).

---

### Task 9.2 — Draw minimap with viewport rectangle and crosshair ✅

**Files:** `mandelbrust-app` (UI code that draws the minimap overlay)

1. **Placement:** Draw the minimap in the **bottom-right corner** of the viewport. Use the **same margin** as the other HUD boxes (see Task 9.4). The minimap **aspect ratio is always square (1:1)** — it shows an equal complex-plane range on both axes (e.g. -2 to 2; zoom configurable in settings).
2. **Viewport indicator:** Draw a **rectangle** in **cyan** on the minimap representing the current main viewport’s position and size in the complex plane (map main viewport bounds to minimap pixel coordinates).
3. **Crosshair:** Draw a **white vertical line** and a **white horizontal line** through the **centre** of that rectangle. Lines extend to the **edges of the minimap** but not outside. **Opacity** of the crosshair lines: **50%** by default; the minimap panel uses the global HUD box opacity (65% default; Task 9.4). Crosshair line opacity is configurable in the settings menu (e.g. 0–100%).
4. Draw the minimap so it does not obscure the centre of the view. Use egui’s layering or a dedicated panel.

**Verify:** The minimap always appears square (1:1). The cyan rectangle moves when panning/zooming; the crosshair stays centered on it. Changing crosshair opacity in settings updates the line transparency.

---

### Task 9.3 — Toggle, visibility, and size settings ✅

**Files:** `mandelbrust-app/src/main.rs`, settings/preferences

1. **Toggle:** Pressing **M** key toggles minimap visibility. Add a **toolbar icon** (e.g. Material Symbol `map` or `crop_free`) that also toggles the minimap. State is persisted in preferences (e.g. `show_minimap: bool`).
2. **HUD off:** When the user hides the HUD (e.g. **H** key), **hide the minimap** as well. When HUD is shown again, restore the minimap visibility from the saved state (so if it was on before HUD off, it is on again).
3. **Size and zoom:** Add settings in the settings menu: **Minimap size** (Small, Medium, Large — side length in pixels, e.g. 128, 256, 384). **Minimap zoom** — complex-plane range shown (default -2 to 2; configurable so the user can zoom the minimap in or out). Persist both. Minimap stays **square (1:1)**.

**Verify:** M and toolbar icon toggle minimap. Hiding HUD hides minimap; showing HUD restores minimap state. Changing minimap size in settings resizes the (square) minimap. Minimap zoom (-2..2 or custom range) and default iteration count (500) are configurable in settings.

---

### Task 9.4 — HUD layout and box styling (unify all boxes) ✅

**Files:** `mandelbrust-app` (HUD layout and panel drawing code)

1. **Move render stats to bottom-centre:** The box that is currently in the **bottom-right** (render stats: phase, timing, tile counts, AA status) must be **moved to the bottom centre** of the viewport. The **bottom-right** is reserved for the minimap (Task 9.2).
2. **Margins:** All HUD boxes (top-left, bottom-left, bottom-centre render stats, bottom-right minimap) use the **same margins** as the two top elements (top-left panel and top-right toolbar area). Apply consistently so all boxes align to the same inset from the viewport edges.
3. **Rounded corners:** All HUD boxes use **rounded corners** like the current top-left and bottom-left panels. Ensure top-left, bottom-left, bottom-centre, and minimap panel all share the same corner radius.
4. **No border:** All HUD boxes have **no border** (same as the current bottom-left panel). **Exception:** the **top-right toolbar** stays exactly as it is — do not change its border or style.
5. **Opacity:** All HUD boxes (viewport info, fractal parameters, render stats, minimap panel) use the **same background opacity**: **65% by default**, **configurable in the settings menu**. Add a single setting (e.g. “HUD panel opacity” or “Overlay opacity”) that controls all of them. The **toolbar is excluded** — it keeps its current appearance.
6. Implement this so that when the minimap is added (Tasks 9.1–9.3), it automatically fits into this layout (bottom-right, same margin, rounded corners, no border, same opacity).

**Verify:** Render stats appear in the bottom centre. All four box areas (top-left, bottom-left, bottom-centre, bottom-right when minimap on) have identical margins, rounded corners, no border, and the same configurable opacity (65% default). Toolbar looks unchanged.

---

### Deliverables — Phase 9

- [x] Zoomed-out overview image rendered and cached; cache invalidated only when image-affecting params change
- [x] Minimap drawn in **bottom-right** corner with cyan viewport rectangle and white crosshair (50% opacity default, configurable)
- [x] Toggle via M key and toolbar icon; minimap hidden when HUD off; minimap square (1:1), range -2..2 default with zoom configurable; size (small/medium/large) and iteration count (500 default) configurable in settings
- [x] HUD layout: render stats moved to **bottom-centre**; minimap in bottom-right; all boxes share same margins (as top boxes), rounded corners, no border, 65% opacity default (configurable in settings); toolbar unchanged
- [x] Behaviour matches Features_to_add.md §1 and §1b

---

## Phase 10 — Julia C Explorer

**Objective:** For Julia mode, replace “pick C from cursor” with a grid of small Julia set previews; clicking a cell sets the Julia constant. Support configurable grid size, coordinate range, and color settings in the explorer.

**Reference:** [Features_to_add.md](../Features_to_add.md) §2.

### Task 10.1 — Grid of small Julia previews (squares, −2..2 default)

**Files:** `mandelbrust-app` (new UI state and screen for “Julia C explorer”), `mandelbrust-render` (render many small Julia images)

1. **Trigger:** When the user presses **C** in Julia mode (or uses the existing “set C” action), open the **Julia C Explorer** instead of (or in addition to) picking C from the cursor. Decide UX: either C always opens the grid, or a modifier/key switches between “cursor pick” and “grid explorer”. Per Features_to_add.md, C opens the grid.
2. **Grid:** Display a grid of **square** images. Each cell = one Julia set with a fixed C. Map grid cell index `(i, j)` to complex C: the viewport for the grid is a fixed complex rectangle; default **−2 to 2** on both axes. So cell `(i, j)` corresponds to C = (re, im) where re and im are linearly mapped from cell indices. Number of rows/columns is **configurable** (e.g. from settings or in-explorer control); e.g. 12×16 or 14×16.
3. **Per-cell render:** Each cell is a small **square** image (e.g. 64×64 or 80×80). For that cell, set Julia constant C from the cell’s position in the grid; render the Julia set with **coordinate range −2..2** (or the configurable range) in the complex plane. Use the current display/color settings (from `DisplayColorSettings`) and a **max iteration** default of **100** (configurable from settings). Render cells in the background (parallel or batched) so the grid fills in progressively.
4. **Coordinate range:** Allow the user to change the “zoom” of **all** cells from within the explorer (e.g. “Range: −2..2” default; change to −1..1 for a zoomed-in grid). This affects the mapping from cell index to C.

**Verify:** Opening Julia C Explorer shows a grid of square Julia previews. Each preview is correct for its C. Changing the range updates all cells. Grid size (number of cells) is configurable.

---

### Task 10.2 — Click to set C; hover shows coordinates; color settings in explorer

**Files:** `mandelbrust-app` (Julia C Explorer UI)

1. **Click:** Clicking a cell sets the **Julia constant C** to that cell’s (re, im) and **closes** the Julia C Explorer (or returns to the main view with the new C). Main view re-renders with the new C.
2. **Hover:** When hovering over a cell, **display the C coordinates** (real and imaginary) for that cell (tooltip or overlay text).
3. **Color settings:** From within the Julia C Explorer view, the user can **change display/color settings** (e.g. open the Display/color settings panel, or a minimal subset: palette, smooth, cycles). Previews in the grid **update** to reflect the new settings (re-colorize or re-render as needed). Changes apply to the current session so when the user closes the explorer, the main view uses the same settings.

**Verify:** Clicking a cell sets C and closes explorer. Hover shows C. Changing palette or other display options in the explorer updates the grid previews and the main view after closing.

---

### Task 10.3 — Configurable grid size and default iterations

**Files:** Settings (preferences), Julia C Explorer UI

1. **Grid size:** The number of cells (e.g. 12×16, 14×16) is **configurable** (settings menu or in-explorer). Store in preferences.
2. **Default max iterations for grid:** Default **100** for the small previews; **configurable from the settings menu**. Use this value when rendering the explorer grid (not the main view’s max iterations).

**Verify:** Changing “Julia explorer grid size” and “Julia explorer max iterations” in settings affects the next time the user opens the Julia C Explorer.

---

### Deliverables — Phase 10

- [ ] C key (Julia mode) opens grid of small Julia previews; each cell is a square; coordinate range −2..2 default, configurable in explorer
- [ ] Click cell to set C and close explorer; hover shows C coordinates; display/color settings changeable from explorer and apply to grid and session
- [ ] Grid size and default iterations (100) configurable in settings
- [ ] Behaviour matches Features_to_add.md §2

---

## Phase 11 — Image Export

**Objective:** Support high-quality still image exports independent of screen resolution. Export must use the current **DisplayColorSettings** (Phase 8) for coloring.

### Task 11.1 — Offscreen renderer

**File:** new function in `mandelbrust-render/src/renderer.rs`

Create a function `render_offscreen()` that:
1. Accepts `viewport: Viewport`, `fractal: &F`, `cancel: &RenderCancel`, `aa_level: u32`, `smooth: bool`, `palette: &Palette`.
2. Renders the full iteration buffer at the given viewport dimensions (not tied to the window size).
3. Runs AA if `aa_level > 0`.
4. Colorizes the result using the given palette and smooth flag.
5. Returns `Result<Vec<u8>>` (RGBA pixel buffer) or an error if cancelled.

This function must be usable without any UI dependencies — it lives in the `mandelbrust-render` crate.

**Verify:** Unit test that renders a 256x256 image and checks the output buffer length is `256 * 256 * 4`.

---

### Task 11.2 — PNG export utility

**File:** new function in `mandelbrust-render/src/lib.rs` or a new `export.rs` module

Create a function `export_png()` that:
1. Accepts `pixels: &[u8]`, `width: u32`, `height: u32`, `path: &Path`.
2. Writes the RGBA buffer as a PNG file using the `image` crate (already a workspace dependency).
3. Returns `Result<()>`.

**Verify:** Unit test that writes a small test image to a temp file and reads it back.

---

### Task 11.3 — Export UI in the app

**File:** `mandelbrust-app/src/main.rs`

Add an export button and dialog:
1. Add a new toolbar icon (Material Symbols `ICON_PHOTO_CAMERA` or similar) that opens an export dialog.
2. The dialog offers: width and height inputs (default: current viewport size x2), AA level selector, a "Export" button, and a file save dialog (via `rfd::FileDialog::new().save_file()`).
3. On "Export", spawn the render on the existing render thread (or a new background thread). Show a progress indicator. When complete, write the PNG to the chosen path.
4. The export must not block the UI. Use the same `mpsc` channel pattern as the main render pipeline, or a dedicated channel.
5. Add keyboard shortcut `E` to open the export dialog.

**Verify:** User can export a PNG. The exported file opens in an image viewer. The export can be cancelled.

---

### Task 11.4 — Update documentation

**Files:** `docs/overview.md`, `README.md`

1. Update the "Export System" section in overview.md: remove "(Planned)" from "Image Export", describe the offscreen renderer, export dialog, and supported options.
2. Update the "Planned Features" section in README.md: move image export to "Current Features".
3. Add the `E` key to the keyboard shortcuts table in both files.

**Verify:** Documentation matches implemented behaviour.

---

### Deliverables — Phase 11

- [ ] `render_offscreen()` function in the render crate (no UI dependency); uses DisplayColorSettings for coloring
- [ ] `export_png()` utility function
- [ ] Export dialog in the app with resolution, AA, and file picker
- [ ] Non-blocking export with progress
- [ ] `E` keyboard shortcut
- [ ] Documentation updated

---

## Phase 12 — Architecture Cleanup

**Objective:** Reduce complexity in `main.rs`, improve state management, and move I/O off the UI thread. This prepares the codebase for larger features (SIMD, GPU, perturbation).

**Reference:** [optimization-report.md](optimization-report.md) sections 10, 11, 12.

### Task 12.1 — Split `main.rs` into UI modules

**Current state:** `main.rs` contains all UI logic (~2500+ lines). Split it into focused modules.

Create the following files under `mandelbrust-app/src/`:

| New file | Responsibility | Functions to move |
|---|---|---|
| `app.rs` | `MandelbRustApp` struct definition, `new()`, `update()` orchestration | Struct + impl blocks for new/update |
| `render_bridge.rs` | `RenderRequest`, `RenderResponse`, `RenderPhase`, render thread setup, `request_render()`, `poll_responses()`, `apply_result()` | All render communication logic |
| `ui/mod.rs` | Module declarations | — |
| `ui/toolbar.rs` | `show_top_right_toolbar()` | Toolbar icon bar |
| `ui/hud.rs` | `show_hud()`, top-left info, bottom-right stats | HUD overlay drawing |
| `ui/params.rs` | Bottom-left fractal parameters panel | `show_fractal_params()` |
| `ui/bookmarks.rs` | Bookmark explorer, save/update dialogs | `show_bookmark_window()`, `show_save_dialog()`, `show_update_or_save_dialog()` |
| `ui/settings.rs` | Settings panel | `show_controls_panel()` (renamed to `show_settings()`) |
| `ui/help.rs` | Controls & shortcuts window | `show_help_window()` |
| `ui/palette_popup.rs` or `ui/display_color.rs` | Display/color settings panel (Phase 8) and/or palette picker | Display/color panel logic, palette popup if retained |
| `input.rs` | `handle_keyboard()`, mouse input processing | All input handling |

Rules:
- Each module receives `&mut MandelbRustApp` (or the relevant subset of state) and `&egui::Context`.
- `main.rs` only contains `fn main()` and the `eframe::run_native` call.
- No logic changes — just code reorganization. Behaviour must be identical.

**Verify:** `cargo build` succeeds. `cargo clippy --workspace` is clean. The application runs and behaves identically to before.

---

### Task 12.2 — Consolidate UI panel state into enums

**Files:** `mandelbrust-app/src/app.rs` (or wherever the struct lives after 9.1)

Replace mutually exclusive boolean flags with an enum:

1. Create `enum ActivePanel { None, Settings, Help, BookmarkExplorer, SaveDialog, UpdateOrSaveDialog, DisplayColorSettings, PalettePopup }` (include DisplayColorSettings from Phase 8).
2. Replace `show_controls`, `show_help`, `show_bookmarks`, `show_save_dialog`, `show_update_or_save_dialog`, `show_palette_popup` with a single `active_panel: ActivePanel` field.
3. Update all toggle logic: opening a panel sets `active_panel = X`, closing sets `active_panel = None`. Opening panel X while Y is open closes Y first.
4. Independent display flags (`show_hud`, `show_crosshair`, `smooth_coloring`, `adaptive_iterations`) remain as booleans — group them into a `DisplaySettings` struct.

**Verify:** Same behaviour as before. All panels open and close correctly. Escape closes the active panel.

---

### Task 12.3 — Move file I/O off the UI thread

**Files:** `mandelbrust-app/src/bookmarks.rs`, new file `mandelbrust-app/src/io_worker.rs`

Create a dedicated I/O worker thread:

1. Define `IoRequest` and `IoResponse` enums (see optimization-report.md section 11 for the full list).
2. Spawn the I/O thread in `MandelbRustApp::new()` with `mpsc` channels, just like the render thread.
3. Replace all synchronous `BookmarkStore` file operations called from UI code with messages sent to the I/O thread:
   - `reload()` → send `IoRequest::ReloadBookmarks`, receive `IoResponse::BookmarksLoaded(Vec<Bookmark>)`.
   - `add()` → send `IoRequest::SaveBookmark(bookmark)`.
   - `remove()` → send `IoRequest::DeleteBookmark(path)`.
   - `update_viewport()` → send `IoRequest::UpdateBookmark(...)`.
   - `preferences.save()` → send `IoRequest::SavePreferences(prefs)`.
4. Poll the I/O response channel in the `update()` loop, just like `poll_responses()` for renders.
5. Thumbnail encoding/decoding also moves to the I/O thread. The UI shows a placeholder until the decoded texture arrives.

**Verify:** Bookmarks save, load, delete, rename, and update correctly. No file I/O happens on the UI thread (verify by adding a log at DEBUG level to each I/O operation in the worker).

---

### Task 12.4 — Stable bookmark IDs and LRU thumbnail cache

**Files:** `mandelbrust-app/src/bookmarks.rs`, app state struct

1. Add a `pub id: String` field to `Bookmark`. Use the sanitized filename (without `.json` extension) as the ID. This is already unique and stable across sorts.
2. Change `thumbnail_cache` key from `usize` (positional index) to `String` (bookmark ID).
3. Change `failed_thumbnails` key from `usize` to `String`.
4. Replace the `HashMap` thumbnail cache with an LRU cache (max 64 entries). Implement a simple LRU using a `VecDeque<(String, TextureHandle)>` or add the `lru` crate (ask for approval first).
5. Remove all the manual `thumbnail_cache.clear()` calls that were needed because of positional indices — the stable ID makes them unnecessary.
6. Remove `last_jumped_bookmark_idx: Option<usize>` and replace with `last_jumped_bookmark_id: Option<String>`.

**Verify:** Bookmark thumbnails display correctly after sorting, deleting, and reloading. The cache does not grow beyond 64 entries.

---

### Deliverables — Phase 12

- [ ] `main.rs` is < 100 lines; all logic lives in focused modules
- [ ] UI panel state uses an enum (including DisplayColorSettings panel), not boolean flags
- [ ] All file I/O happens on a background thread
- [ ] Bookmark IDs are stable strings, not positional indices
- [ ] Thumbnail cache is bounded (LRU, max 64 entries)
- [ ] Application behaves identically to before

---

## Phase 13 — Memory Layout & Buffer Management

**Objective:** Reduce memory footprint and allocation pressure for faster rendering.

**Reference:** [optimization-report.md](optimization-report.md) section 5.

### Task 13.1 — Compact `IterationResult` to 8 bytes

**File:** `mandelbrust-core/src/fractal.rs`, `mandelbrust-render/src/iteration_buffer.rs`, `mandelbrust-render/src/palette.rs`, `mandelbrust-render/src/aa.rs`

Replace the `IterationResult` enum with a flat struct:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct IterationResult {
    pub iterations: u32,  // u32::MAX means interior
    pub norm_sq: f32,     // f32 is sufficient for smooth coloring (log2(ln(x)))
}

impl IterationResult {
    pub const INTERIOR: Self = Self { iterations: u32::MAX, norm_sq: 0.0 };
    pub fn escaped(iterations: u32, norm_sq: f64) -> Self {
        Self { iterations, norm_sq: norm_sq as f32 }
    }
    pub fn is_interior(&self) -> bool { self.iterations == u32::MAX }
}
```

Update all code that pattern-matches on `IterationResult::Escaped { .. }` / `IterationResult::Interior` to use the new struct methods instead.

Update `class()` to return `iterations` directly (or `u32::MAX` for interior).

**Verify:** All tests pass. Rendered images are visually identical (minor floating-point differences in smooth coloring are acceptable). `std::mem::size_of::<IterationResult>() == 8`.

---

### Task 13.2 — Buffer pool for tile rendering

**Files:** `mandelbrust-render/src/renderer.rs`

Currently each tile allocates a new `Vec<IterationResult>`. Add a simple buffer pool:

1. Create a `struct TileBufferPool` using `crossbeam` channel or `std::sync::Mutex<Vec<Vec<IterationResult>>>`.
2. Before rendering a tile, pop a buffer from the pool (or allocate a new one if empty). Clear and resize it.
3. After blitting the tile's data into the `IterationBuffer`, push the buffer back to the pool.
4. The pool lives in the render thread (no sharing with the UI thread).

If a simpler approach is preferred: use Rayon's `thread_local!` pattern to give each thread a reusable buffer.

**Verify:** `cargo bench -p mandelbrust-render` shows reduced allocation count. Rendered output unchanged.

---

### Task 13.3 — Avoid full buffer rebuild on `shift()`

**Files:** `mandelbrust-render/src/iteration_buffer.rs`, `mandelbrust-render/src/aa.rs`

`IterationBuffer::shift()` and `AaSamples::shift()` both allocate a brand-new buffer and copy the overlapping region. Instead, shift in-place:

1. For `IterationBuffer::shift()`: if shifting right/down, iterate backwards to avoid overwriting source data. If shifting left/up, iterate forwards. Fill exposed regions with `IterationResult::INTERIOR`.
2. For `AaSamples::shift()`: rebuild the offset array in-place using the same directional iteration trick. Or, if the sparse storage makes in-place shifting too complex, keep the current approach but reuse the allocated vector (clear + resize instead of new allocation).

**Verify:** All tests pass. Pan-and-release produces the same visual result as before.

---

### Deliverables — Phase 13

- [ ] `IterationResult` is 8 bytes (down from 16)
- [ ] Tile buffers are pooled and reused across renders
- [ ] `shift()` does not allocate a new buffer
- [ ] Benchmark results logged showing improvement

---

## Phase 14 — Advanced Coloring

**Objective:** Add coloring techniques that dramatically improve visual quality.

**Reference:** [optimization-report.md](optimization-report.md) section 7.

### Task 14.1 — Histogram equalization coloring

**File:** `mandelbrust-render/src/palette.rs`

Add a new method `colorize_histogram()`:

1. Build a histogram of smooth iteration values from the `IterationBuffer` (exclude interior points).
2. Compute the cumulative distribution function (CDF).
3. Map each pixel's smooth value through the CDF to get an evenly distributed `[0, 1]` index.
4. Look up the palette color using this equalized index.

Add a toggle in the app (toolbar icon or checkbox in the fractal parameters panel) to switch between linear and histogram-equalized coloring.

**Verify:** Histogram coloring produces visibly more even color distribution than linear. Toggle switches instantly (no re-render needed — just re-colorize from the stored `IterationBuffer`).

---

### Task 14.2 — Distance estimation

**Files:** `mandelbrust-core/src/fractal.rs`, `mandelbrust-core/src/mandelbrot.rs`, `mandelbrust-core/src/julia.rs`, `mandelbrust-render/src/palette.rs`

1. Extend the `Fractal` trait with an optional method `iterate_with_derivative()` that returns `IterationResult` plus the derivative `dz` at escape.
2. Implement it for `Mandelbrot` and `Julia`: track `dz_{n+1} = 2*z_n*dz_n + 1` alongside the main iteration.
3. Compute distance: `d = |z| * ln|z| / |dz|`.
4. Store the distance in `IterationResult` (add an optional `distance: f32` field, or return it separately).
5. Add a "Distance estimation" coloring mode in `Palette` that maps `−log(distance)` to the palette.

**Verify:** Filament structures near the set boundary are visibly sharper and more detailed than with standard escape-time coloring.

---

### Task 14.3 — Stripe average coloring for interior points

**Files:** `mandelbrust-core/src/mandelbrot.rs`, `mandelbrust-core/src/julia.rs`, `mandelbrust-render/src/palette.rs`

Interior points (currently solid black) can be colored using the orbit's angular distribution:

1. During the iteration loop, accumulate `stripe_sum += 0.5 + 0.5 * sin(stripe_density * atan2(z.im, z.re))`.
2. At the end (when the point is determined to be interior), return `stripe_avg = stripe_sum / iterations`.
3. Add a `stripe_avg: f32` field to `IterationResult` (or a separate buffer).
4. Add a coloring mode that maps interior `stripe_avg` to the palette.
5. Add a toggle in the UI. Default: off (black interior, as currently).

**Verify:** Interior regions show smooth, colorful orbital structure instead of solid black.

---

### Deliverables — Phase 14

- [ ] Histogram equalization toggle (instant re-colorize, no re-render)
- [ ] Distance estimation coloring mode
- [ ] Interior stripe average coloring mode (optional, default off)
- [ ] All new coloring modes accessible from the UI (and compatible with DisplayColorSettings)

---

## Phase 15 — SIMD Vectorization

**Objective:** Process 4 pixels simultaneously per CPU core using SIMD instructions.

**Reference:** [optimization-report.md](optimization-report.md) section 3.

### Task 15.1 — Add batch iteration API

**File:** `mandelbrust-core/src/fractal.rs`

Add a default method to the `Fractal` trait:

```rust
fn iterate_batch(&self, points: &[Complex], results: &mut [IterationResult]) {
    for (p, r) in points.iter().zip(results.iter_mut()) {
        *r = self.iterate(*p);
    }
}
```

This default implementation is scalar. SIMD-optimized fractals will override it.

Update the tile renderer in `mandelbrust-render/src/renderer.rs` to call `iterate_batch()` with groups of pixels instead of calling `iterate()` one-at-a-time.

**Verify:** All tests pass. Output is identical. This task is purely an API preparation — no SIMD yet.

---

### Task 15.2 — SIMD Mandelbrot iteration (AVX2)

**Files:** new file `mandelbrust-core/src/mandelbrot_simd.rs`, `mandelbrust-core/src/mandelbrot.rs`

Using either the `wide` crate (portable, stable Rust) or `std::arch::x86_64` intrinsics (with `#[cfg(target_arch = "x86_64")]`):

1. Implement `iterate_batch_simd()` that processes 4 complex points per SIMD step using `f64x4` vectors.
2. The SIMD loop: perform `z = z² + c` on all 4 lanes, check escape on all 4 lanes using a mask, continue until all lanes have escaped or hit max_iter.
3. Handle lane-specific escape: when a lane escapes, record its iteration count and `norm_sq`, but keep iterating the remaining lanes (mask out escaped lanes).
4. Override `iterate_batch()` in the `Mandelbrot` impl to dispatch to the SIMD version when available, falling back to scalar otherwise.

If using `wide` crate: ask for approval before adding the dependency.
If using `std::arch`: wrap in `#[cfg(target_feature = "avx2")]` with a scalar fallback.

**Verify:** Output is identical to scalar for a test viewport. Benchmark shows 3–4x speedup per core at minimum.

---

### Task 15.3 — SIMD Julia iteration

**File:** `mandelbrust-core/src/julia.rs` (or new `julia_simd.rs`)

Same as task 12.2, but for the Julia set. The only difference is `z₀ = point` instead of `z₀ = 0` and `c` is fixed.

**Verify:** Output identical to scalar. Benchmark shows similar speedup to Mandelbrot.

---

### Deliverables — Phase 15

- [ ] `iterate_batch()` API on the `Fractal` trait
- [ ] SIMD Mandelbrot iteration (4 pixels per step)
- [ ] SIMD Julia iteration (4 pixels per step)
- [ ] Scalar fallback on non-x86-64 or non-AVX2 targets
- [ ] Benchmarks showing 3–4x per-core improvement

---

## Phase 16 — Animation & Video Export

**Objective:** Enable smooth fractal zoom animations between bookmarks.

### Task 16.1 — Keyframe system

**File:** new file `mandelbrust-app/src/animation.rs`

1. Define `struct Keyframe { bookmark_id: String, hold_seconds: f64 }`.
2. Define `struct AnimationPlan { keyframes: Vec<Keyframe>, fps: u32, transition_seconds: f64, width: u32, height: u32 }`.
3. Implement camera interpolation between keyframes: linear interpolation for center coordinates, logarithmic interpolation for scale (so zoom feels perceptually smooth).
4. Implement `fn frame_viewport(plan: &AnimationPlan, frame: usize) -> Viewport` that returns the viewport for a given frame number.
5. Compute `total_frames` from the plan.

**Verify:** Unit test: two keyframes at different zoom levels, verify that `frame_viewport()` produces a smooth sequence with logarithmic zoom interpolation.

---

### Task 16.2 — Frame-by-frame renderer

**File:** `mandelbrust-app/src/animation.rs` or `mandelbrust-render/src/export.rs`

1. Create `fn render_animation(plan: &AnimationPlan, fractal: ..., palette: ..., output_dir: &Path, cancel: &RenderCancel, progress_callback: impl Fn(usize, usize))`.
2. For each frame: compute the viewport, call `render_offscreen()` (from Phase 11), write PNG to `output_dir/frame_00001.png`.
3. Respect cancellation. Report progress via callback.
4. Run on a background thread.

**Verify:** Rendering 10 frames produces 10 sequentially named PNG files with smoothly changing viewports.

---

### Task 16.3 — Animation UI

**File:** `mandelbrust-app/src/ui/` (new submodule)

1. Add an "Animation" panel accessible from a new toolbar icon.
2. The panel shows a list of keyframes (drag-to-reorder, add from current bookmark, remove).
3. Options: FPS, transition duration, resolution.
4. "Render" button opens a folder picker and starts rendering in the background.
5. Progress bar shows current frame / total frames.
6. Optional: "Preview" button that plays the animation at low resolution in the viewport.

**Verify:** User can create a 2-keyframe animation and render it to PNG sequence.

---

### Task 16.4 — Optional ffmpeg integration

**File:** `mandelbrust-app/src/animation.rs`

1. After PNG sequence render, check if `ffmpeg` is available on the system PATH.
2. If available, offer a "Convert to MP4" button.
3. Run `ffmpeg -framerate {fps} -i frame_%05d.png -c:v libx264 -pix_fmt yuv420p output.mp4` as a subprocess.
4. Report progress. Handle ffmpeg not found gracefully (show message, not an error).

**Verify:** If ffmpeg is installed, an MP4 file is produced. If not, the user sees a helpful message and still has the PNG sequence.

---

### Deliverables — Phase 16

- [ ] Keyframe system with logarithmic zoom interpolation
- [ ] Frame-by-frame PNG export (using DisplayColorSettings for coloring)
- [ ] Animation UI with keyframe list, options, and progress
- [ ] Optional ffmpeg MP4 conversion
- [ ] Documentation updated

---

## Phase 17 — GPU Compute Backend

**Objective:** Add an optional GPU rendering backend for 50–200x faster interactive exploration.

**Reference:** [optimization-report.md](optimization-report.md) section 4.

### Task 17.1 — wgpu compute pipeline setup

**Files:** new crate `mandelbrust-gpu/` or new module in `mandelbrust-render`

1. Create the compute pipeline infrastructure:
   - Uniform buffer: viewport center, scale, dimensions, max_iterations, escape_radius.
   - Storage buffer: output iteration data (one `u32` iteration + one `f32` norm_sq per pixel).
   - Compute shader (WGSL) that performs Mandelbrot iteration per pixel.
2. The pipeline accepts viewport parameters and returns an iteration buffer.
3. Use `f32` arithmetic in the shader (standard GPU limitation).

Ask for approval before adding `wgpu` as a dependency (it may already be available via `eframe`).

**Verify:** A headless test renders a 256x256 Mandelbrot image via GPU compute and the output matches (within f32 precision) the CPU reference.

---

### Task 17.2 — GPU colorization shader

**File:** same module as 14.1

1. Write a second WGSL compute shader that reads the iteration buffer and writes RGBA pixels using a palette LUT.
2. Upload the 256-entry palette as a storage buffer.
3. Implement smooth coloring in the shader.

**Verify:** The GPU-colorized image matches the CPU-colorized image (within f32 precision).

---

### Task 17.3 — Integrate GPU backend into the app

**File:** `mandelbrust-app/src/render_bridge.rs` (or equivalent after Phase 9 refactor)

1. Add a toggle in Settings: "Renderer: CPU / GPU".
2. When GPU is selected, the render pipeline sends requests to the GPU backend instead of the CPU render thread.
3. The GPU backend writes directly to a texture (no CPU round-trip for display).
4. Fall back to CPU if GPU initialization fails (log a WARNING).
5. CPU is always used for: AA (GPU doesn't support the sparse boundary approach easily), export at deep zoom (f64 precision needed), and perturbation rendering.

**Verify:** Switching between CPU and GPU produces visually similar results. GPU renders are noticeably faster (verify via the render timing display in the HUD).

---

### Deliverables — Phase 17

- [ ] WGSL compute shader for Mandelbrot/Julia iteration
- [ ] WGSL compute shader for palette colorization
- [ ] GPU/CPU toggle in settings
- [ ] Graceful fallback to CPU on GPU failure
- [ ] Benchmark comparison logged

---

## Phase 18 — Perturbation Theory (Deep Zoom)

**Objective:** Enable zoom depths beyond 10^15 by computing only deltas from a high-precision reference orbit.

**Reference:** [optimization-report.md](optimization-report.md) section 9.

### Task 18.1 — Arbitrary-precision reference orbit

**Files:** new module `mandelbrust-core/src/perturbation.rs`

1. Add an arbitrary-precision number library (e.g., `rug` for performance or `dashu` for pure Rust). Ask for approval.
2. Implement `compute_reference_orbit(center: &BigComplex, max_iter: u32, escape_radius: f64) -> Vec<Complex>`.
3. The reference orbit is computed at the viewport center using arbitrary precision, but the resulting orbit points are stored as `f64` pairs (sufficient for the delta recurrence).
4. This function runs on a background thread. It can take seconds for very deep zooms.

**Verify:** The reference orbit for the Mandelbrot center `(-0.75, 0.0)` at various zoom levels matches known values.

---

### Task 18.2 — Delta iteration (perturbation rendering)

**File:** `mandelbrust-core/src/perturbation.rs`

1. Implement `iterate_perturbation(ref_orbit: &[Complex], delta_c: Complex, max_iter: u32, escape_radius_sq: f64) -> IterationResult`.
2. The recurrence: `δ_{n+1} = 2*z_n*δ_n + δ_n² + δc`.
3. Implement glitch detection: if `|δ_n|` becomes comparable to `|z_n|`, the perturbation approximation breaks down. Mark the pixel as needing a new reference orbit (or fallback to full precision).
4. Implement a `PerturbationFractal` struct that wraps the reference orbit and exposes the `Fractal` trait (or a parallel API).

**Verify:** A test image at zoom 10^20 renders correctly without artifacts (compare to a reference rendered with full arbitrary precision at tiny resolution).

---

### Task 18.3 — Series approximation (skip initial iterations)

**File:** `mandelbrust-core/src/perturbation.rs`

1. Implement Taylor series coefficient computation: `A_{n+1} = 2*z_n*A_n + 1`, `B_{n+1} = 2*z_n*B_n + A_n²`, `C_{n+1} = 2*z_n*C_n + 2*A_n*B_n`.
2. For a given `δc`, estimate how many iterations K can be skipped by evaluating `δ_K ≈ A_K*δc + B_K*δc² + C_K*δc³` and checking if the approximation is still accurate.
3. Start the delta iteration from iteration K instead of 0.
4. This dramatically reduces per-pixel cost for deep zooms where K can be in the thousands.

**Verify:** Benchmark at zoom 10^30 comparing perturbation with and without series approximation.

---

### Task 18.4 — Integrate perturbation into the render pipeline

**Files:** `mandelbrust-render/src/renderer.rs`, `mandelbrust-app/src/render_bridge.rs`

1. When zoom depth exceeds a threshold (e.g., scale < 1e-13), automatically switch to perturbation rendering.
2. The reference orbit is computed once per render (on the render thread, before tile dispatch).
3. Each tile uses the delta iteration with the shared reference orbit.
4. If a tile detects glitches, it requests a re-render with a local reference orbit.
5. The HUD shows "Perturbation" as the render mode when active.

**Verify:** Zoom smoothly from standard to deep zoom (past 10^15). No visible artifacts at the transition point. The render timing in the HUD shows reasonable times even at extreme zoom.

---

### Deliverables — Phase 18

- [ ] Arbitrary-precision reference orbit computation
- [ ] Delta iteration with glitch detection
- [ ] Series approximation for initial iteration skip
- [ ] Automatic activation based on zoom depth
- [ ] Smooth transition from standard to perturbation rendering
- [ ] Deep zoom to 10^30+ without artifacts

---

## Phase 19 — Polish & v1.0 Release

**Objective:** Stabilize, document, and prepare for public release.

### Task 19.1 — Error handling audit

Audit all `unwrap()`, `expect()`, and `panic!()` calls across the workspace. Replace with proper error handling (`Result`, user-facing messages) where the error is recoverable. Keep `unwrap()` only where the invariant is guaranteed by construction (add a comment explaining why).

---

### Task 19.2 — Cross-platform verification

Build and test on:
- Windows (primary platform)
- macOS (if available)
- Linux

Fix any platform-specific issues (file paths, font rendering, window sizing).

---

### Task 19.3 — Performance profiling

1. Profile a standard exploration session using `cargo flamegraph` or `perf`.
2. Identify any remaining hot spots not addressed by earlier phases.
3. Fix the top 3 bottlenecks found.
4. Log final benchmark numbers.

---

### Task 19.4 — Final documentation pass

Update all documentation to reflect the final state:
- `overview.md` — full architecture and feature description
- `README.md` — user-facing feature list, screenshots, build instructions
- `optimization-report.md` — mark completed items, add actual benchmark results
- This roadmap — mark all completed phases

---

### Task 19.5 — Release packaging

1. Set up GitHub Actions to produce prebuilt binaries for Windows, macOS, Linux.
2. Create a versioned GitHub release (v1.0.0).
3. Tag the release commit.

---

### Deliverables — Phase 19

- [ ] No unhandled panics in normal operation
- [ ] Verified on at least 2 platforms
- [ ] Profiled and optimized
- [ ] Documentation complete and accurate (including Display/color, Minimap, Julia C Explorer, bookmarks)
- [ ] v1.0.0 release published with binaries

---

## Long-Term (Post-v1.0)

These are not scheduled but tracked as future possibilities:

- **Additional fractal types** — Multibrot, Burning Ship, Newton, Tricorn
- **Buddhabrot / Nebulabrot** rendering mode
- **Orbit trap coloring** (Pickover stalks, circles, crosses)
- **Palette editor** — custom gradient creation (Display/color profiles and panel from Phase 8 provide a foundation; editor would extend the palette definition within that model)
- **Fade to black** — MSZP-style fade near max iterations (specified in Features_to_add.md §3; fits in DisplayColorSettings when implemented)
- **GPU perturbation** — deep zoom on the GPU using emulated double precision
- **WebAssembly build** — run MandelbRust in the browser via wasm
- **Plugin system** — user-defined fractal formulas
