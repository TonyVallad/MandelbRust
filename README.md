# MandelbRust

A high-performance, native fractal explorer written in **Rust**.

MandelbRust provides real-time, interactive exploration of the Mandelbrot set and Julia sets using a Google Maps-like navigation model. It is built around heavy multithreading, progressive rendering, and a clean separation between math, rendering, and UI.

## Status

**Early development** — the project is in the planning and scaffolding phase.

## Planned Features

- **Real-time exploration** — smooth pan and zoom with mouse and keyboard
- **Mandelbrot & Julia sets** — extensible architecture for additional fractal types
- **Multithreaded tiled renderer** — parallel CPU rendering via Rayon with automatic load balancing
- **Progressive rendering** — instant low-resolution preview, asynchronous refinement to full quality
- **Computation optimizations** — cardioid/bulb checks, periodicity detection, symmetry exploitation
- **Multiple color palettes** — smooth gradient coloring with instant palette switching
- **Bookmarks** — save and restore exploration states, including all parameters and metadata
- **High-resolution image export** — offscreen rendering at arbitrary resolution with optional supersampling
- **Animation export** (post-v1.0) — bookmark-to-bookmark zoom animations with video output

## Technology Stack

| Component | Crate |
|---|---|
| Language | Rust |
| UI | `egui` / `eframe` |
| Parallelism | `rayon` |
| Benchmarking | `criterion` |
| Image encoding | `image` |
| Serialization | `serde`, `serde_json` |
| Config paths | `directories` |
| Logging | `tracing` |

No GPU required. Performance is achieved through CPU parallelism and careful architecture.

## Project Structure

```
MandelbRust/
  mandelbrust-core/     # math, fractals, iteration engine
  mandelbrust-render/   # tiled renderer, coloring, multithreading
  mandelbrust-app/      # UI and user interaction
  docs/                 # project documentation
```

## Documentation

- [**Project Overview**](docs/overview.md) — architecture, design decisions, and full technical specification
- [**Development Roadmap**](docs/roadmap.md) — phased development plan with progress tracking

## License

This project is licensed under the [MIT License](LICENSE).
