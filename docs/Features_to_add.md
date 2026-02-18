# Features to Add

This document lists planned features for the MandelbRust project and describes how each should behave.

---

## 1. Minimap

### Overview
A minimap shows a zoomed-out view of the fractal and indicates the current viewport position.

### Behaviour

- **Image source**
  - Load and keep in memory a zoomed-out image of the fractal. The minimap shows a **square** region of the complex plane: **coordinates from -2 to 2** on both axes by default. **Zoom** (scale of this region, i.e. how much of the plane is visible) is **configurable from the settings menu** (e.g. zoom out beyond -2..2 or zoom in to a smaller range).
  - Use **500 iterations by default** for this image; this value must be **configurable in the settings menu**.
  - Keep the image in memory as long as no parameter that changes the image is modified (e.g. color settings, C coordinates for Julia sets, minimap zoom, or any other setting that would alter the zoomed-out view).
  - When such a parameter changes, the minimap image must be regenerated/refreshed.

- **Viewport indicator**
  - Draw a **rectangle** on the minimap in **cyan** representing the current viewport.
  - Draw a **white vertical line** and a **white horizontal line** that pass through the **centre** of that rectangle.
  - Both lines must have **50% opacity by default**, **configurable in the settings menu**.
  - These white lines must extend from **one side of the minimap to the other**, but **must not extend outside** the minimap.

- **Placement**
  - The minimap is drawn in the **bottom-right corner** of the viewport, with the same margin as the other HUD boxes (see “HUD layout and box styling” below).

- **Visibility**
  - Toggle the minimap with the **"M" key** or via a **new icon in the top-right toolbar**.
  - When the HUD is turned off, the minimap must also be **hidden**.

- **Size and aspect**
  - The minimap **aspect ratio is always square (1:1)** — it displays an equal range on both axes (e.g. -2 to 2). The **zoom** of the minimap (complex-plane range shown) is **configurable from the settings menu**.
  - Minimap **size** (side length in pixels) must be **configurable in the settings menu** with options: **small**, **medium**, **large**.

---

## 1b. HUD layout and box styling (global)

These rules apply to all HUD overlay boxes (viewport info, fractal parameters, render stats, minimap). The **top-right toolbar** is unchanged (no border/opacity changes).

- **Layout**
  - The box currently in the **bottom-right** (render stats: phase, timing, tile counts, AA status) is **moved to the bottom centre** of the viewport.
  - The **minimap** occupies the **bottom-right** corner (see §1).
  - **Top-left:** viewport info. **Top-right:** toolbar (unchanged) and, below it, cursor coordinates (no box). **Bottom-left:** fractal parameters. **Bottom-centre:** render stats. **Bottom-right:** minimap (when enabled).

- **Margins**
  - All HUD boxes use the **same margins** as the two top boxes (top-left and top-right toolbar area).

- **Shape and border**
  - All boxes use **rounded corners** like the current top-left and bottom-left boxes.
  - All boxes have **no border** (same as the current bottom-left box). The **toolbar at top-right stays exactly as it is** (no change to its border or style).

- **Opacity**
  - All HUD boxes (viewport info, fractal parameters, render stats, minimap panel) use the **same background opacity**: **65% by default**, **configurable in the settings menu**. The toolbar at top-right is not excluded for the opacity change.

---

## 2. Julia C Explorer

### Overview
For Julia sets, replace the current “cursor position = C” behaviour with a grid of small Julia set previews; clicking one sets the C coordinates.

### Behaviour

- **Replacing C key behaviour**
  - When the user presses the **"C" key** to choose new C coordinates, instead of using only the cursor position, show a **grid of small Julia set images**.
  - Each cell in the grid corresponds to C coordinates derived from that cell’s position on the viewport (i.e. the same mapping as “cursor position → C” today, but sampled over a grid).
  - **Clicking on one of those images** selects the C coordinates for that cell and closes/exits the explorer (or applies the new C and returns to the main view as per current flow).

- **Grid configuration**
  - The **number (and thus size) of images** in the grid must be **configurable** (e.g. via settings or in-explorer controls).
  - Each preview image must be a **square**, not a rectangle.

- **Coordinate range (zoom per cell)**
  - Inside each square, the complex-plane coordinates must run from **-2 to 2** on both axes by default (i.e. the same range for real and imaginary parts).
  - This range (zoom level for all grid images) must be **configurable from within the explorer** (e.g. to zoom in or out across all previews).

- **Hover feedback**
  - When hovering over a small image in the grid, **display the C coordinates** (real and imaginary) for that cell (e.g. tooltip or overlay).

- **Color settings**
  - The user must be able to **change color settings from within the grid view** (so previews reflect the current color scheme and changes apply to the explorer previews).

- **Iterations**
  - **Default max iterations** for the grid previews must be **100**.
  - This value must be **configurable from the settings menu**.

---

## 3. J preview panel and Julia C Explorer access

### Overview
Change how the Julia C Explorer is opened (by **clicking “Julia”** in the bottom-left fractal selector instead of the J key). Repurpose the **J** key to toggle a **preview panel** above the minimap: in Mandelbrot mode it shows a live Julia preview at the cursor (with left-click to load Julia at that c); in Julia mode it shows the Mandelbrot set with a crosshair at the current c. Both previews use 4×4 AA.

### Behaviour

- **Opening the Julia C Explorer (Option B)**
  - **Clicking “Julia”** in the bottom-left fractal mode selector (Mandelbrot | Julia) **opens the Julia C Explorer** (grid of small Julia set previews). The app does **not** switch to Julia mode yet.
  - When the user **picks a cell** in the grid, set the Julia constant **c** to that cell’s coordinates, **switch to Julia mode**, and close the explorer. The user is then viewing that Julia set.

- **J key — toggle preview panel**
  - **J** toggles the visibility of a **preview panel** drawn **above the minimap**.
  - **Gap:** The space between this panel and the minimap is the **same size as the margin between HUD elements and the viewport** (e.g. 8 px).
  - **Appearance:** The panel has the **same size, shape, and opacity** as the minimap (same size setting, square, same HUD panel opacity). Same 1 px white border (75% opacity), no black margin, inset from the viewport edge.

- **J preview in Mandelbrot mode (Julia preview)**
  - Content: **Julia set** for **c = complex coordinate under the cursor**. Updates **as the cursor moves**.
  - **Iterations:** **250 by default**; must be **configurable in Settings** (e.g. “Julia preview iterations”).
  - **4×4 anti-aliasing** is applied to this preview.
  - **Left-click** on the main canvas (no drag) → set **c** to the clicked pixel’s complex coordinate and **switch to Julia mode** (i.e. “load” the Julia set that would be previewed at that pixel). Pan (left-drag) is unchanged.

- **J preview in Julia mode (Mandelbrot preview)**
  - Content: **Mandelbrot set** (default view, same as minimap) with a **white vertical and horizontal line** (crosshair) passing through the point that corresponds to the **current Julia c** (where that c lies in the Mandelbrot set).
  - This preview **does not** update on cursor move. It updates only when **c** changes (Re(c)/Im(c) in panel, Shift+Click, or after loading from Julia preview) or **display/color settings** change.
  - **Iterations:** Use the **same iteration count as the minimap** (minimap iterations setting).
  - **4×4 anti-aliasing** is applied.

---

## 4. Display and color settings (MSZP-inspired)

### Overview
Extend display and color options to support named color profiles, palette modes (cycles vs cycle length), start-from black/white, and optional log-log smoothing. The settings model must be designed so that new display/color features can be added later without reworking the architecture. Access to all of this is via a single **Display/color settings** entry point (replacing the current palette icon).

### Behaviour

- **Display/color settings icon and panel (replaces current palette icon)**
  - **Replace** the current color palette icon in the top-right of the viewport with a **Display/color settings** icon.
  - Clicking it opens a **panel** (or window) where the user can:
    - **Edit all display/color settings** (palette, palette mode, cycles, start from black/white, fade options, log-log/smooth coloring, etc.).
    - **Select** a color profile (apply it to current session).
    - **Save** the current display/color configuration as a new profile or overwrite an existing one.
    - **Load** a profile from the color profiles folder (see below).
  - All profile management (select, save, load, list) is done from this same place; no separate "palette only" icon.

- **Color profiles: one file per profile, in a dedicated folder**
  - Color profiles are stored as **one file per profile** in a **color profiles** folder at the **root directory of the program** `color_profiles/` next to the executable or project root.
  - Each file contains the full serialized display/color settings for that profile (palette, mode, thresholds, log-log on/off, etc.) so that profiles can be **shared easily** (copy one file to another machine or to another user).
  - File format (e.g. JSON or TOML) should be human-readable and documented so power users can edit or share profiles. At least one default profile should exist (e.g. shipped with the app or created on first run).

- **Bookmarks: save display/color settings individually (not only profile name)**
  - When saving a **bookmark**, store **all display/color settings individually** (palette, palette mode, cycle length or number of cycles, start from black/white and thresholds, log-log on/off, etc.), **not** just a reference to a color profile.
  - This allows a bookmark to capture a **one-off tweak** (e.g. "this view with one extra cycle and start from white") without creating a whole new profile. Restoring the bookmark restores that exact display/color state.
  - Optionally, the bookmark can also record which profile was active at save time (for reference), but the applied state is the full snapshot of settings.

- **Palette mode: by number of cycles or by cycle length**
  - **By number of cycles**: User specifies how many times the palette repeats over the iteration range (e.g. 1 = one full cycle). Cycle length is derived as max_iterations / number_of_cycles. Palette is indexed by (smoothed) iteration position within each cycle.
  - **By cycle length**: User specifies the cycle length in iterations; the palette repeats every N iterations. Palette is indexed by (smoothed) iteration modulo cycle length, normalized to [0, 1).
  - Both modes use the same underlying palette (stops with positions and RGB); only the mapping from iteration count to palette position differs. The chosen mode is part of the color profile.

- **Start from black/white (MSZP-style)**
  - **Enable/disable**: Option to fade the **first few iterations** from solid black or solid white into the palette (e.g. “Start from black” or “Start from white”).
  - **Same settings as MSZP**: Two thresholds, **low_threshold_start** and **low_threshold_end** (in iteration count). Below low_threshold_start, pixel is full black or full white. Between the two thresholds, linear blend from that solid color to the palette color. Above low_threshold_end, only the normal palette (and other effects) apply.
  - These options and thresholds are stored in the color profile and must be editable in the Display/color settings panel.

- **Log-log (continuous iteration / smooth coloring)**
  - **Enable/disable** the **log-log style continuous iteration** used for coloring. When **enabled**, the renderer uses the existing smooth formula (see note below) so the effective iteration index is fractional and banding is reduced; the formula “C” is described in the note below.
  - When **disabled**, use the raw integer iteration count for coloring (banded look).
  - This is the same concept as the current **smooth coloring** toggle; it should be part of the unified display/color settings and of color profiles so users can save “smooth” vs “banded” presets.
  - **Current implementation (MandelbRust):** The codebase already uses a **log-log style** formula for smooth coloring. In `mandelbrust-render/src/palette.rs`, `smooth_iteration(iterations, norm_sq)` computes ν = n + 1 − log₂(ln|zₙ|) (with ln|z| from the stored norm_sq at escape). So the toggle for log-log is the existing **smooth coloring** toggle; keep it and ensure it is part of profiles and bookmarks.

- **Architecture for future features**
  - **Display/color settings** (palette, palette mode, cycles, start from black/white, fade to black, log-log, and any future options) must be implemented as a **single, coherent settings model** (e.g. a struct or config object) that:
    - Is used everywhere coloring or display is decided (main view, export, Julia C explorer, minimap if it uses the same palette).
    - Can be serialized/deserialized for **profiles** (one file per profile in the color profiles folder) and for **bookmarks** (full snapshot per bookmark).
    - Is easy to extend: adding a new option (e.g. “fade to black”, or a new palette type) should not require scattered changes across the codebase.
  - The **Display/color settings** UI (the panel opened by the new icon) should be the single place to edit these settings and to select/save/load profiles, organized so that new options can be added in one place without redesigning the whole screen.

---

*Document created from feature list as of today. Add new items below or in new sections as needed.*
