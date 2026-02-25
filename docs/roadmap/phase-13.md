# Phase 13 — Menu Bar

## Overview

Phase 13 adds a persistent menu bar to the top of the application window. The menu bar is visible in every screen (fractal explorer, bookmark browser, Julia C Explorer, and the future main menu) and remains visible even when the HUD is hidden.

---

## Design Decisions

### Panel type

The menu bar uses `egui::TopBottomPanel::top` so that it reserves vertical space before the `CentralPanel` is laid out. This prevents the fractal viewport from rendering behind the menu.

### Dynamic height offset

Rather than hard-coding a pixel offset, the menu bar's actual rendered height is captured from the `egui::InnerResponse` returned by `TopBottomPanel::show()` and stored in `MandelbRustApp::menu_bar_height`. All top-anchored HUD elements (viewport info, toolbar, display/color panel, cursor coordinates) use this value as a Y-offset in their `egui::Area::anchor()` calls. This adapts automatically to DPI scaling and font size changes.

### Menu structure

Five menus provide quick access to all major features:

| Menu | Items |
|------|-------|
| **File** | Save Bookmark (S), Open Bookmarks (B), Export Image… (disabled placeholder), Quit |
| **Edit** | Copy Coordinates, Reset View (R) |
| **Fractal** | Switch to Mandelbrot, Switch to Julia, Julia C Explorer |
| **View** | Toggle HUD (H), Toggle Minimap (M), Toggle J Preview (J), Toggle Crosshair (C), Cycle Anti-Aliasing (A), Settings… |
| **Help** | Keyboard Shortcuts, About MandelbRust |

Context-sensitive labels (e.g. "Hide HUD" vs "Show HUD") and disabled states (e.g. "Switch to Mandelbrot" greyed out when already in Mandelbrot mode) improve clarity.

### Shortcut hints

A `shortcut_item` helper function formats a `Button` with both the label and the keyboard shortcut on a single line using `RichText`. Items that don't have a shortcut use a plain `ui.button()`.

### About window

A simple centered `egui::Window` dialog shows the project name, description, and GitHub URL. Controlled by the `show_about: bool` field.

---

## New / Modified Files

| File | Changes |
|------|---------|
| `mandelbrust-app/src/ui/menu_bar.rs` | **New.** `draw_menu_bar`, five private menu methods, `shortcut_item` helper, `format_coordinates_for_clipboard`, `switch_to_mandelbrot`, `switch_to_julia`, `cycle_aa`, `draw_about_window` |
| `mandelbrust-app/src/ui/mod.rs` | Added `pub(crate) mod menu_bar;` |
| `mandelbrust-app/src/app.rs` | Added `show_about: bool` and `menu_bar_height: f32` fields. Calls `draw_menu_bar(ctx)` at the start of `update()` and `draw_about_window(ctx)` at the end |
| `mandelbrust-app/src/ui/hud.rs` | Top-left HUD anchor offset by `self.menu_bar_height` |
| `mandelbrust-app/src/ui/toolbar.rs` | Top-right toolbar, display/color panel, and cursor coordinates anchors offset by `self.menu_bar_height`. Added `TOOLBAR_MARGIN` constant |
| `mandelbrust-app/src/input.rs` | `A` key shortcut now calls `self.cycle_aa()` (defined in `menu_bar.rs`) instead of inline AA logic |

---

## API Surface

### `MandelbRustApp` (new public(crate) items)

| Item | Kind | Description |
|------|------|-------------|
| `menu_bar_height` | `f32` field | Rendered height of the menu bar in logical pixels |
| `show_about` | `bool` field | Controls visibility of the About dialog |
| `draw_menu_bar(&mut self, ctx)` | method | Renders the menu bar and captures its height |
| `draw_about_window(&mut self, ctx)` | method | Renders the About dialog if `show_about` is true |
| `cycle_aa(&mut self)` | method | Cycles AA level (off → 2×2 → 4×4 → off) and triggers re-render |
