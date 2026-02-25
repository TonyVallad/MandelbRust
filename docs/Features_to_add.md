# Features to Add

This document lists planned features for the MandelbRust project and describes how each should behave.

**Note:** Previously planned features (minimap, Julia C Explorer, J preview panel, display/color settings, HUD layout) have been implemented and are no longer listed here.

---

## 1. Main menu at launch

### Overview
When launching the app, show a main menu instead of going straight to the fractal explorer. The menu offers a choice of how to start (resume, Mandelbrot, Julia, or open a bookmark). Layout follows the mockup: multiple horizontal panels with a preview image area, title in cyan, and details/description.

### Behaviour

- **Layout**
  - Full-window menu with **four options** arranged **horizontally**, each in a panel with:
    - A **preview image** area at the top (placeholder for now).
    - A **title in cyan** below the preview.
    - **Details or description** text below the title (e.g. fractal type, coordinates, zoom, iterations for “Continue”; short description/formula for Mandelbrot and Julia).

- **Options (for now)**

  1. **Resume Exploration**
     - Shows the last saved view state (fractal mode, center, zoom, iterations, C for Julia, etc.) with key parameters displayed (e.g. “Fractal: Julia”, “xC / yC”, “Center”, “Zoom”, “Iter”).
     - Selecting this option opens the fractal explorer with that state restored (same as current “restore last view” behaviour).

  2. **Mandelbrot Set**
     - Opens the **default view** of the Mandelbrot set with **default settings** (no restore of last view).

  3. **Julia’s Sets**
     - Opens the **Julia C Explorer** (grid menu) directly. The user picks a C from the grid; after selection (double-click or select + “Open” button), the **fractal explorer** opens in Julia mode at that C.

  4. **Open Bookmark**
     - Opens a **bookmark explorer** that uses the **whole window** (no fractal view behind it). Bookmarks are listed/browsed in this full-window view.
     - The **fractal explorer** is opened only **after** a bookmark is chosen: by **double-clicking** the bookmark or by **selecting it and clicking an “Open Bookmark” button**.
     - No fractal explorer until a bookmark is selected.

- **Styling**
  - Tiles must have **very small rounded corners** (subtle rounding, not large radii).
  - A **vertical line separator** must be drawn between the **"Resume Exploration"** tile and the other tiles, to visually distinguish the "resume" action from the "start fresh" options.
  - **Background**: plain **black** for now (may be changed to another color or a background image later).

- **Implementation note**
  - For now, the listed options and layout are sufficient; preview images can remain placeholders.

---

## 2. Minimap size controls

### Overview
Allow the user to change the minimap size from the UI and keyboard, in addition to the existing settings menu.

### Behaviour

- **Buttons**
  - Add a **“−”** and a **“+”** button on or next to the minimap to **decrease** and **increase** its size (e.g. cycle through small / medium / large or scale continuously, as fits the current design).

- **Keyboard**
  - **Page Up**: increase minimap size.
  - **Page Down**: decrease minimap size.
  - Behaviour should match or complement the +/- buttons (same size steps or scale).

---

## ~~3. Menu bar~~ *(completed — Phase 13)*

Implemented: persistent menu bar with File, Edit, Fractal, View, and Help menus. HUD elements offset below the menu bar dynamically. Menu bar stays visible when HUD is hidden and across all screens. See [phase-13.md](roadmap/phase-13.md) for details.

---

## 4. HUD modifications

### Overview
Change the content and behaviour of the top-left and bottom-left HUD areas: clearer fractal name, editable coordinates/zoom, and a simplified iterations/escape-radius block.

### Behaviour

- **Top-left HUD**
  - **Fractal name**: Instead of “Mode: …”, display **only the name of the fractal** (e.g. “Mandelbrot” or “Julia”), **centered horizontally** in the box, in **cyan**.
  - **Coordinates**: Show the current **coordinates on two separate lines** (e.g. real on one line, imaginary on the other).
  - **Editable fields**: Make **coordinates** and **zoom** **editable** (user can type values). For **Julia**, also show and make **C coordinates** editable in this area.
  - **Iterations**: Display the **actual iteration count** used when using **adaptive iterations** (not only the max). Format with **thousands separators** for readability (e.g. `1.000.000`).

- **Bottom-left HUD**
  - **Iterations**: **Remove the iterations slider**. Keep only the **numeric input** for max iterations.
  - **Max iterations**: Allow input **up to 1.000.000** (or another limit). The **maximum must be configurable in the Settings menu**.
  - **Remove** the “×10” and “/10” buttons; no longer needed.
  - **Escape radius**: Place the **Escape R** slider **below** all iterations-related controls (so the order is: iterations input, then escape radius slider).

---

## ~~5. Code reorganization~~ *(completed — Phase 12)*

Implemented: `main.rs` split into focused modules (`app.rs`, `render_bridge.rs`, `navigation.rs`, `input.rs`, `io_worker.rs`, `ui/` subdirectory). See [phase-12.md](roadmap/phase-12.md) for details.

---

*Add new items below or in new sections as needed.*
