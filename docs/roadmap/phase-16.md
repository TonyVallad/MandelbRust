# Phase 16 — Advanced Coloring

## Overview

Phase 16 delivers the advanced coloring stack and palette authoring workflow. It adds histogram and distance-estimation coloring, stripe-average interior coloring, user-defined palettes persisted as JSON, and a tabbed Display/Color UI that unifies profiles, palette editing, coloring mode selection, and interior controls.

---

## Architecture

The phase spans core math data structures, render colorization, and app UI/persistence:

| Layer | File(s) | Responsibility |
|-------|---------|----------------|
| Core | `mandelbrust-core/src/palette_data.rs` | Palette data model (`PaletteDefinition`, `ColorStop`, RGB helpers), serializable and shareable |
| Render | `mandelbrust-render/src/palette.rs`, `mandelbrust-render/src/renderer.rs`, `mandelbrust-render/src/extras_buffer.rs` | Advanced coloring modes, interior coloring, and auxiliary buffers for derivative/stripe data |
| App | `mandelbrust-app/src/palette_io.rs`, `mandelbrust-app/src/ui/toolbar.rs`, `mandelbrust-app/src/ui/palette_editor.rs`, `mandelbrust-app/src/ui/color_picker.rs` | Palette persistence, tabbed Display/Color panel, palette editor, and color picker |
| Migration | `scripts/migrate_bookmarks.py` | Standalone bookmark migration for older display/color payloads |

---

## What Was Implemented

### 1) Advanced coloring modes

- Added **Histogram** coloring using iteration histogram + CDF remapping for more even visual distribution.
- Added **Distance Estimation** coloring using derivative-aware boundary distance.
- Kept **Standard** mode as baseline and made mode selection UI-accessible.

### 2) Interior coloring

- Added **Stripe Average** for interior regions, alongside existing **Black** interior mode.
- Added stripe density support and integrated it with rendering/colorization flow.

### 3) Palette model and JSON persistence

- Introduced a first-class palette model in core:
  - palette name
  - ordered color stops with normalized positions
  - start/end fade controls (black/white)
- Added app-side palette file operations:
  - list
  - load
  - save
  - rename
  - delete
- Palettes are stored as individual JSON files in `palettes/`.

### 4) Display/Color UI rework

- Reworked the Display/Color panel into tabs:
  - **Profiles**
  - **Palette**
  - **Coloring**
  - **Interior**
- Added a dedicated in-app palette editor:
  - gradient preview bar
  - draggable color stops
  - add/remove stops
  - helper for even spacing
- Added a color picker with synchronized RGB, HEX, and visual controls.

### 5) Bookmark compatibility and migration

- Extended bookmark payload handling to persist and restore all new color settings.
- Added `scripts/migrate_bookmarks.py` to upgrade older bookmark files when missing newer display/color fields.

---

## Key Design Decisions

- **Split responsibilities across crates:** palette schema in core, render algorithms in render crate, persistence/UI in app crate.
- **Keep colorization hot path separate from UI state:** render modes operate on compact buffers and optional extras.
- **Store palettes as one JSON per palette:** easy to share, edit, and version.
- **Provide migration as an external script:** application runtime stays clean while preserving user data continuity.

---

## New and Modified Files

| File | Change |
|------|--------|
| `mandelbrust-core/src/palette_data.rs` | **New.** Palette model and serialization types |
| `mandelbrust-core/src/lib.rs` | Exposed new `palette_data` module |
| `mandelbrust-render/src/palette.rs` | Added coloring/interior modes and advanced colorization paths |
| `mandelbrust-render/src/renderer.rs` | Wired render outputs needed by advanced color modes |
| `mandelbrust-render/src/extras_buffer.rs` | **New.** Storage for auxiliary per-pixel render extras |
| `mandelbrust-render/src/lib.rs` | Exported new modules/types |
| `mandelbrust-app/src/palette_io.rs` | **New.** Palette JSON file operations |
| `mandelbrust-app/src/ui/toolbar.rs` | Tabbed Display/Color integration and mode controls |
| `mandelbrust-app/src/ui/palette_editor.rs` | **New.** Palette editor UI |
| `mandelbrust-app/src/ui/color_picker.rs` | **New.** Color picker UI |
| `scripts/migrate_bookmarks.py` | **New.** Bookmark migration helper for Phase 16 fields |

---

## Verification Notes

- Coloring mode changes are instant (re-colorization paths do not require full UI reconfiguration).
- Palette create/edit/delete persists to disk and can be reloaded.
- Bookmarks retain and restore Display/Color state across sessions.
- Migration script upgrades old bookmark JSON files in place while writing `.bak` backups.
