# Features to Add

This document lists planned features for the MandelbRust project and describes how each should behave.

**Note:** Previously planned features (minimap, Julia C Explorer, J preview panel, display/color settings, HUD layout, main menu at launch) have been implemented and are no longer listed here.

---

## ~~1. Main menu at launch~~ *(completed — Phase 14)*

Implemented: full-window main menu at startup with four horizontal tiles (Resume Exploration, Mandelbrot Set, Julia's Sets, Open Bookmark). Tiles have preview images (cover mode), cyan titles, centered rich-text descriptions with bold markup, and a vertical separator. Resume tile auto-updates with live state and preview on every final render. Full-window bookmark browser and Julia C Explorer accessible from the menu. See [phase-14.md](roadmap/phase-14.md) for details.

---

## 2. Minimap size controls

### Overview
Allow the user to change the minimap size from the UI and keyboard, in addition to the existing settings menu.

### Behaviour

- **Buttons**
  - Add a **"−"** and a **"+"** button on or next to the minimap to **decrease** and **increase** its size (e.g. cycle through small / medium / large or scale continuously, as fits the current design).

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
  - **Fractal name**: Instead of "Mode: …", display **only the name of the fractal** (e.g. "Mandelbrot" or "Julia"), **centered horizontally** in the box, in **cyan**.
  - **Coordinates**: Show the current **coordinates on two separate lines** (e.g. real on one line, imaginary on the other).
  - **Editable fields**: Make **coordinates** and **zoom** **editable** (user can type values). For **Julia**, also show and make **C coordinates** editable in this area.
  - **Iterations**: Display the **actual iteration count** used when using **adaptive iterations** (not only the max). Format with **thousands separators** for readability (e.g. `1.000.000`).

- **Bottom-left HUD**
  - **Iterations**: **Remove the iterations slider**. Keep only the **numeric input** for max iterations.
  - **Max iterations**: Allow input **up to 1.000.000** (or another limit). The **maximum must be configurable in the Settings menu**.
  - **Remove** the "×10" and "/10" buttons; no longer needed.
  - **Escape radius**: Place the **Escape R** slider **below** all iterations-related controls (so the order is: iterations input, then escape radius slider).

---

## ~~5. Code reorganization~~ *(completed — Phase 12)*

Implemented: `main.rs` split into focused modules (`app.rs`, `render_bridge.rs`, `navigation.rs`, `input.rs`, `io_worker.rs`, `ui/` subdirectory). See [phase-12.md](roadmap/phase-12.md) for details.

---

*Add new items below or in new sections as needed.*
