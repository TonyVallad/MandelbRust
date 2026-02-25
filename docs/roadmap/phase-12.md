# Phase 12 — Code Reorganization

**Objective:** Restructure the application codebase so that each concern lives in its own module or file. This is a prerequisite for the upcoming main menu, menu bar, and HUD rework.

---

## Overview

Before Phase 12, nearly all application logic lived in a single `main.rs` file (~3,500+ lines). This monolithic structure made it difficult to reason about individual subsystems, and the planned features (main menu, menu bar, HUD rework) would have made the situation far worse.

Phase 12 broke `main.rs` into focused modules, introduced an application-level state machine, moved file I/O to a background thread, and stabilized the bookmark thumbnail cache. The application's external behaviour is unchanged — this was purely an internal restructuring.

---

## Design decisions

### Module split strategy

Rather than introducing new abstractions or traits, the split leverages Rust's ability to have multiple `impl` blocks for the same struct across different modules. The `MandelbRustApp` struct (with `pub(crate)` fields) is defined in `app.rs`, and each sibling module adds its own `impl MandelbRustApp` block with the methods relevant to that module's concern.

This avoids the complexity of passing partial state subsets or defining new trait boundaries while still achieving clean separation of concerns.

### Shared types in `app.rs`

Enums and constants used across multiple modules (`FractalMode`, `RenderPhase`, `BookmarkTab`, `LabelFilterMode`, `ActiveDialog`, `BookmarkSnap`, `THUMBNAIL_CACHE_CAPACITY`) are defined in `app.rs` to avoid circular dependencies. Other modules import them via `use crate::app::*`.

### ActiveDialog vs boolean panel flags

The original code used independent booleans for all UI panels. Analysis showed that most floating panels (Settings, Help, Bookmarks, Display/Color) can co-exist simultaneously — the user might have the help window and bookmark explorer open at the same time. Only the modal save/update dialogs are truly mutually exclusive.

To preserve existing behaviour:
- An `ActiveDialog` enum (`None`, `SaveBookmark`, `UpdateOrSave`) replaced the `show_save_dialog` and `show_update_or_save_dialog` booleans.
- Independent floating panel visibility flags (`show_controls`, `show_help`, `show_bookmarks`, etc.) remained as separate booleans.

### IO worker architecture

File operations (bookmark writes/deletes, preferences saves, directory scans) are dispatched to a dedicated `io-worker` thread via `mpsc` channels. The design:

- **Fire-and-forget writes/deletes:** The in-memory state is updated immediately on the UI thread; the file operation is serialized to JSON and sent to the worker. This keeps the UI responsive.
- **Async directory scans:** When bookmarks need to be reloaded (e.g. when the bookmark panel is opened), a `ScanBookmarkDir` request is sent to the worker. The worker reads all `.json` files and sends back a `BookmarksScanComplete` response, which is polled in the `update()` loop.
- **Synchronous fallback:** At startup (before the IO worker is connected), `BookmarkStore` and `AppPreferences` use direct synchronous I/O. The IO sender is injected after construction via `set_io_sender()`.
- **Ordering guarantee:** Since the worker processes requests sequentially via a single `mpsc` channel, a write followed by a scan will always see the written data.

### Stable bookmark IDs

The original thumbnail cache used vector indices (`usize`) as keys. This broke when bookmarks were sorted or deleted, because indices shifted. The fix uses the bookmark's on-disk filename (e.g. `"My Bookmark.json"`) as a stable string ID, exposed via `BookmarkStore::bookmark_id()`. The `BookmarkSnap` type was also refactored from a tuple alias into a proper struct to hold the new `id` field alongside display data.

### LRU thumbnail eviction

The thumbnail cache is bounded at 64 entries (`THUMBNAIL_CACHE_CAPACITY`). When full, the cache evicts an arbitrary entry (using `HashMap::keys().next()`) before inserting a new one. This is a simple approximation of LRU — not true LRU order, but sufficient for the use case since bookmarks are typically viewed in screen-order and the eviction rate is low.

---

## New files

| File | Description |
|------|-------------|
| `mandelbrust-app/src/app.rs` | Core application struct (`MandelbRustApp`), shared enums/constants, constructor, palette/color helpers, `eframe::App` trait implementation, IO response polling. |
| `mandelbrust-app/src/app_state.rs` | `AppScreen` enum — top-level state machine for dispatching between screens (currently only `FractalExplorer`; placeholders for `MainMenu`, `BookmarkBrowser`, `JuliaCExplorer`). |
| `mandelbrust-app/src/render_bridge.rs` | Background render worker types (`RenderRequest`, `RenderResponse`, `RenderPhase`, `JuliaGridRequest`) and worker functions (`render_worker`, `julia_grid_worker`). Render dispatch and response polling methods. |
| `mandelbrust-app/src/navigation.rs` | Pan, zoom, view history (undo/redo), viewport resize, zoom-rect handling. |
| `mandelbrust-app/src/input.rs` | Mouse event handling (drag, click, scroll), keyboard shortcuts, zoom-rect drawing. |
| `mandelbrust-app/src/io_worker.rs` | `IoRequest`/`IoResponse` enums and `spawn_io_worker()`. Dedicated thread for file writes, deletes, and bookmark directory scans. |
| `mandelbrust-app/src/ui/mod.rs` | Module declarations for the `ui` subdirectory. |
| `mandelbrust-app/src/ui/toolbar.rs` | Top-right Material Symbols icon toolbar with state-aware dimming. |
| `mandelbrust-app/src/ui/hud.rs` | Top-left viewport info, bottom-centre render stats, J-preview panel drawing, minimap drawing delegation. |
| `mandelbrust-app/src/ui/minimap.rs` | Minimap viewport calculations, revision tracking, render request/response handling. |
| `mandelbrust-app/src/ui/settings.rs` | Settings panel (window size, bookmarks directory, minimap options, Julia explorer options, opacity controls). |
| `mandelbrust-app/src/ui/help.rs` | Controls & shortcuts window. |
| `mandelbrust-app/src/ui/bookmarks.rs` | Bookmark explorer window, save/update dialogs, thumbnail caching with LRU eviction, bookmark grid drawing, label tree. |
| `mandelbrust-app/src/ui/julia_explorer.rs` | Julia C Explorer grid (central panel mode). |

## Modified files

| File | Changes |
|------|---------|
| `mandelbrust-app/src/main.rs` | Reduced to ~20 lines: module declarations and `fn main()` calling `app::run()`. |
| `mandelbrust-app/src/bookmarks.rs` | Added `io_tx` field and `loading` flag to `BookmarkStore`. File operations dispatch through the IO channel when available. Added `set_io_sender()`, `is_loading()`, `apply_scan_result()`, `dispatch_write()`, `dispatch_delete()` methods. Added `bookmark_id()` for stable string IDs. |
| `mandelbrust-app/src/preferences.rs` | Added `io_tx` field (serde-skipped) to `AppPreferences`. `save()` dispatches to IO worker when available. Added `set_io_sender()`. |

---

## API surface

### `AppScreen`

```
enum AppScreen { FractalExplorer }
// Future: MainMenu, BookmarkBrowser, JuliaCExplorer
```

### `ActiveDialog`

```
enum ActiveDialog { None, SaveBookmark, UpdateOrSave }
```

### `BookmarkSnap`

```
struct BookmarkSnap {
    index: usize,
    id: String,         // stable filename-based ID
    name: String,
    summary: String,
    mode: String,
    labels: Vec<String>,
    thumbnail_png: String,
}
```

### `IoRequest` / `IoResponse`

```
enum IoRequest {
    WriteFile { path: PathBuf, content: String },
    DeleteFile { path: PathBuf },
    ScanBookmarkDir { dir: PathBuf },
}

enum IoResponse {
    BookmarksScanComplete { bookmarks: Vec<Bookmark>, filenames: Vec<String> },
}

fn spawn_io_worker() -> (Sender<IoRequest>, Receiver<IoResponse>)
```

### `BookmarkStore` additions

```
set_io_sender(tx: Sender<IoRequest>)
is_loading() -> bool
apply_scan_result(bookmarks: Vec<Bookmark>, filenames: Vec<String>)
bookmark_id(index: usize) -> &str
```

### `AppPreferences` additions

```
set_io_sender(tx: Sender<IoRequest>)
```
