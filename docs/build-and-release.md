# Build and release guide

This guide explains how to **test the app** (run it during development) and how to **create a new release** (standalone executable to distribute).

**Prerequisites:** Rust toolchain installed ([rustup](https://rustup.rs)). All commands are run from the **repository root** (the folder containing `Cargo.toml` and the `mandelbrust-app` folder).

---

## Testing the app

Use this when you want to run the app on your machine to try changes or use it normally.

### 1. Run from the repo (simplest)

From the repository root:

```bash
cargo run -p mandelbrust-app
```

- Builds the app in **debug** mode (faster compile, slower runtime) and runs it.
- No files to copy. The app uses `mandelbrust-app/icon.ico` at build time (embedded in the binary).
- **Preferences** and **bookmarks** are stored in the same folder as the executable: `preferences.json` and a `bookmarks/` folder (each bookmark as a `.json` file). You can change the bookmarks folder in **Settings** in the app.

### 2. Run the built executable (optional)

If you already built the app and only want to run it without recompiling:

```bash
# Build (debug)
cargo build -p mandelbrust-app

# Run the executable
.\target\debug\MandelbRust.exe
```

- **Windows:** `target\debug\MandelbRust.exe`
- No need to copy anything else; the icon is embedded.

---

## Creating a new release

Use this when you want a **standalone executable** to run on any Windows PC (no Rust, no Visual C++ Redistributable).

### 1. Build the release

From the repository root:

```bash
cargo build --release -p mandelbrust-app
```

- Builds in **release** mode (optimized, LTO). Takes longer than debug.
- The project is configured (`.cargo/config.toml`) to **statically link the C runtime**, so the resulting `.exe` does **not** require the Visual C++ Redistributable on the target machine.

### 2. Where is the executable?

After a successful build:

| Platform | Path |
|----------|------|
| Windows  | `target\release\MandelbRust.exe` |

### 3. What to copy for distribution

- **Copy only:** `target\release\MandelbRust.exe`

That single file is enough. You can put it in a folder or zip and share it. No need to copy `icon.ico` or any DLLs; the icon is embedded and the runtime is statically linked. When users run it, the app will create `preferences.json` and a `bookmarks/` folder next to the exe (unless they choose a different bookmarks folder in Settings).

### 4. Optional: run the release build locally

To run the release build on your machine:

```bash
.\target\release\MandelbRust.exe
```

Or double-click the exe in Explorer.

---

## Summary

| Goal | Command | What to use / copy |
|------|---------|--------------------|
| **Test the app** | `cargo run -p mandelbrust-app` | Nothing to copy. |
| **New release** | `cargo build --release -p mandelbrust-app` | Copy `target\release\MandelbRust.exe` only. |
