## 🚀 MandelbRust v0.1.0-alpha

First public release of MandelbRust, a fast fractal explorer written in Rust.

---

## ✨ Features
- Mandelbrot and Julia rendering modes
- Multiple coloring modes: Standard, Histogram, Distance Estimation
- Interior coloring modes: Black, Stripe Average
- Built-in and custom palettes with an in-app palette editor
- Color profiles, bookmarks, minimap, and Julia preview panel
- Export image workflow with resolution, AA, and full color/display settings support
- Top toolbar export icon for faster image export access
- Anti-aliasing selector as a popup (Off, 2x2, 4x4)
- Display/color settings reorganized into logical tabs
- All tool windows are movable
- Version visible in the window title (`MandelbRust - Alpha ...`)

---

## ⚡ Performance
- Multi-threaded rendering pipeline
- Tile-based computation with progressive preview then full render
- Optional anti-aliasing (Off, 2x2, 4x4)
- Deep zoom support with DoubleDouble precision fallback

---

## 📦 Downloads
### Windows
- Download the Windows `.zip` release package
- Extract all files into the same folder
- Run `MandelbRust-v0.1.0-alpha-windows.exe` from the extracted folder

---

## ⚠️ Known limitations
- UI is still in active alpha iteration
- Some workflows are still being refined for ergonomics and discoverability

---

## 🛠️ Technical details
- Language: Rust
- UI: egui/eframe
- Precision: f64 + DoubleDouble for deep zoom scenarios
- Iterations and color parameters: fully configurable in-app

---

## 📸 Preview
(you can drag your screenshot here)

---

## 🧪 Status
Alpha release — expect bugs and incomplete features.