# MSZP Technical Report — How the Old Software Works

This document is a technical analysis of **MSZP** (Mandelbrot Set Zoomer Program), a QB64/BASIC fractal explorer written by Anthony Vallad. The source code lives at [GitHub - TonyVallad/MSZP](https://github.com/TonyVallad/MSZP). The report explains how BMP export, coloring, “AA”, color smoothing, and color profiles work in that codebase.

---

## 1. Overview of the pipeline

- **Mandelbrot**: For each pixel, the complex coordinate \(c\) is derived from the pixel position; the iteration count \(C\) is computed for \(z_{n+1} = z_n^2 + c\) with \(z_0 = 0\) until \(|z|^2 > 24\) or \(C = \text{precision}\).
- **Julia**: Same iteration with fixed \(c = (x_C, y_C)\); the pixel position gives the initial \(z_0\); escape when \(|z|^2 > 4\).
- **BMP export**: One pass over the image. For each pixel: compute \(C\) (optionally a continuous “smoothed” value), call `Get_pixel_color_BMP` to get RGB from the current color profile and palette, then write B, G, R to the BMP file (bottom-up rows, 24 bpp).

There is **no supersampling or multi-sample anti-aliasing** in MSZP. The only “smoothing” is **continuous iteration count** (see below), which affects color gradients, not edge sampling.

---

## 2. How colors are chosen when exporting BMP images

### 2.1 Iteration and escape

- **Mandelbrot** (`BMP_Creator`, `mode$ = "Mandelbrot"`):
  - \(x_C, y_C\) (written as `xC#`, `yC#` in code) are the complex coordinates for the current pixel, derived from `debutx#`, `debuty#` and step `pas# = view_size# / hauteur`.
  - Loop: \(z_{n+1} = z_n^2 + c\), with escape when \(x_2^2 + y_2^2 > 24\) (radius 2 in squared form uses 4 for Julia but 24 here — likely a tuning choice).
- **Julia** (`mode$ = "Julia"`):
  - \(c\) is fixed (`xC#`, `yC#`). The pixel determines initial \(z_0\) (`xcal#`, `ycal#`).
  - Escape when \(x_2^2 + y_2^2 > 4\).

After the loop, the code holds:
- Integer iteration count `C`.
- On escape: final \(z\) components `x2#`, `y2#` used for **color gradient smoothing** (see Section 4).

### 2.2 From iteration count to RGB

For **BMP export**, every pixel goes through `Get_pixel_color_BMP`. That subroutine:

1. Uses the (possibly smoothed) value `C` and the chosen **color_settings** (profile 1, 2, or 3–11).
2. For **profile 1 and 2** (palette-based):
   - Computes a **position in the color cycle** (see Section 5).
   - Calls `GetRGBValues(nb_colors_used, position_in_cycle#)` to get RGB by interpolating the palette.
3. Applies **global color features** when the profile supports them (profiles 1 and 2):
   - **Start from black/white** (low iteration band).
   - **Fade to black** near max iterations.
   - **Interior**: if `C = precision`, force RGB = (0,0,0).
4. For **profiles 3–11**, uses hard-coded formulas that map `C` and `precision` directly to R, G, B (no palette file).
5. Clamps R, G, B to [0, 255] and converts to character strings `r$`, `g$`, `b$` for writing to the BMP (as bytes in B, G, R order).

So: **each pixel’s color is determined solely by its (possibly smoothed) iteration value `C`, the chosen color profile, and the palette/options for that profile.**

---

## 3. “AA” (anti-aliasing) in MSZP

MSZP **does not implement** classic anti-aliasing (supersampling, multi-sample per pixel, or blur).

- There is **one sample per pixel** when building the BMP: one complex coordinate per pixel, one iteration run, one color.
- The only thing that softens the look of the image is **color gradient smoothing** (continuous iteration count), which removes hard “banding” between iteration bands. That is **not** geometric AA; it doesn’t average multiple samples to reduce jagged edges.

So in this report, “AA” in the sense of “supersampling or edge smoothing” is **absent**. The changelog’s “Improved …” and “color gradient smoothing” refer to the **continuous iteration formula** and palette usage, not to pixel-level anti-aliasing.

---

## 4. Color smoothing (continuous iteration count)

This is the main “smoothing” in MSZP and is applied **only when writing the BMP** (and when using the same logic for the explorer’s palette-based coloring). It does **not** add extra samples; it only makes the **iteration count** continuous so that the palette is indexed by a fractional value.

### 4.1 Where it happens

In `BMP_Creator`, right after the iteration loop, for **both** Mandelbrot and Julia:

```vb
'Color gradient smoothing - Log-log
If flag = 1 Then
  xx = Log(x2# * x2# + y2# * y2#) / 2
  yy = Log(xx / Log(2)) / Log(2)
  C = C + 1 - yy
End If
```

- `flag = 1` means the orbit escaped (exited the circle).
- \(x_2, y_2\) is the last point before exit; \(x_2^2 + y_2^2\) is \(|z|^2\) at escape.
- So \(\mathit{xx} = \frac{1}{2}\ln(|z|^2) = \ln|z|\), and \(\mathit{yy} = \log_2(\ln|z| / \ln 2)\).
- The new \(C\) is set to \(C + 1 - \mathit{yy}\), i.e. a **fractional iteration count** that depends on how far past the escape radius the orbit was. That gives a continuous value used later by the color routine.

So: **smoothing = one sample per pixel, but the iteration count is made continuous via a log–log style formula.** This removes sharp steps between integer iteration bands and makes gradients look smooth when combined with the palette.

### 4.2 How it interacts with the palette (profile 1 only)

For **color_settings = 1** (palette by number of cycles), the code uses the **decimal part** of \(C\) so that the gradient is smooth **within** each cycle:

- `C_int = int(C)`, `C_dec = C - int(C)`.
- `position_in_cycle# = (C_int MOD cycle_length) / cycle_length + C_dec / cycle_length`.

So the position in the cycle is continuous: integer part from \(\texttt{C\_int} \bmod \texttt{cycle\_length}\), plus a fractional part from `C_dec`. That way, small changes in the smoothed \(C\) produce smooth changes in the palette index (color smoothing **along** the gradient). Profile 2 uses `(C MOD cycle_length) / cycle_length` and does not add `C_dec`; it’s still smooth in \(C\) because \(C\) itself can be fractional after the log–log step.

---

## 5. Other color features (fade, start from black/white)

These are applied inside `Get_pixel_color_BMP` for **palette profiles 1 and 2** (and are driven by `color-settings.txt` for those).

### 5.1 Start from black or white (low-iteration band)

- **Settings**: `Start from = black` or `white`, and `low_threshold_start`, `low_threshold_end` in `color-settings.txt`.
- **Behavior**:
  - **White**: If `C < low_threshold_start`, pixel is forced to (255, 255, 255). Between `low_threshold_start` and `low_threshold_end`, RGB is linearly blended from the palette color toward white: e.g. `r = r + (255 - r) * (low_threshold_end - C) / (low_threshold_end - low_threshold_start)`.
  - **Black**: If `C < low_threshold_start`, pixel is (0, 0, 0). Between the two thresholds, RGB is scaled from black: `r = r * (C - low_threshold_start) / (low_threshold_end - low_threshold_start)` (and similarly g, b).

So the **first few iterations** fade in from either black or white over a configurable band.

### 5.2 Fade to black (high-iteration band)

- **Setting**: `Fade to black = 1` or `0` in `color-settings.txt`.
- **Behavior** when enabled:
  - If `precision >= cycle_length`: when `C > precision - 0.5 * color_length`, RGB is scaled down so that at \(C = \text{precision}\) the color goes to black (linear ramp over that band).
  - Otherwise: when `C > 0.85 * precision`, RGB is scaled by `(precision - C) / (precision * 0.15)` so the last 15% of the iteration range fades to black.

So the **top end** of the iteration range can be faded to black instead of ending at the last palette color.

### 5.3 Interior (max iterations)

- If `C = precision` (did not escape), the pixel is always set to **(0, 0, 0)** in `Get_pixel_color_BMP`, regardless of profile. So the “inside” of the set is always black in BMP export.

---

## 6. Color profiles and color palettes

### 6.1 Palette file (`color-settings.txt`)

Loaded by `Load_Color_Settings` and used by **color_settings 1 and 2** only.

- **Start from**: `black` or `white` (see above).
- **low_threshold_start**, **low_threshold_end**: iteration band for fade-in from black/white.
- **Fade to black**: 1 = yes, 0 = no.
- **Number of colors**: count of palette stops (max 15).
- **Palette entries**: lines like `[position, R, G, B]` with position in \([0, 1]\). The first color is duplicated at position 1 so the cycle wraps smoothly.

The loader duplicates the first stop to position 1 and increments `nb_colors_used`, so the effective palette has one extra wrap-around segment.

### 6.2 GetRGBValues (palette interpolation)

- **Input**: `nb_colors_used`, `position_in_cycle#` in \([0, 1)\) (or [0, 1] with the duplicate).
- **Logic**: Find the segment `i` such that `palette_pos#(i) <= position_in_cycle# < palette_pos#(i+1)`, then **linear interpolation** of R, G, B between `palette_color(i, 1..3)` and `palette_color(i+1, 1..3)` by position within the segment.
- **Output**: integer R, G, B. So **color smoothing** in the palette sense is linear interpolation between stops; the “smooth” iteration count (Section 4) feeds a continuous `position_in_cycle#`, which then gets interpolated here.

### 6.3 Color profile 1 — Palette by number of cycles

- User sets **number of cycles** (e.g. 1). Then `cycle_length = int(precision / cycles_nb#)`.
- Position in cycle:  
  `position_in_cycle# = (C_int MOD cycle_length) / cycle_length + C_dec / cycle_length`  
  so the palette is repeated `cycles_nb#` times over the iteration range, with **smooth** transition thanks to `C_dec`.
- Then: GetRGBValues, then start from black/white, fade to black, and interior to black as above.

### 6.4 Color profile 2 — Palette by cycle length

- User sets **cycle length** (in iterations). Position in cycle:  
  `position_in_cycle# = (C MOD cycle_length) / cycle_length`  
  (C can still be fractional from the log–log smoothing).
- Same application of palette, start from black/white, fade to black, and interior.

### 6.5 Color profiles 3–11 (fixed formulas)

These **ignore** the palette file and map `C` and `precision` directly to R, G, B with hard-coded formulas. Examples from the code:

- **3**: Green/blue on black (green ramp first half, then second half; blue from 255 down by C).
- **6**: Green/purple on black (B, G, R formulas).
- **7**: Red → green → blue gradient (two segments by C/precision).
- **8**: Yellow–purple style (R, G, B linear in C/precision).
- **9**: Black–white with different channel rates.
- **10**: White → blue/green → black (four segments).
- **11**: “Resonance” style: R, G, B each peak at a certain iteration (r_freq, g_freq, b_freq) with an amplitude (r_amp, etc.), so colors appear at specific iteration bands.

Profile 4 and 5 contain typos (`presicion`) and some logic that looks experimental (“To identify/fix”). They are still deterministic formulas from C.

---

## 7. Summary table

| Topic | In MSZP |
|-------|--------|
| **Per-pixel color** | One iteration run per pixel → integer or continuous \(C\) → color by profile (palette or fixed formula) → optional start from black/white, fade to black, interior = black. |
| **Anti-aliasing** | None. Single sample per pixel; no supersampling. |
| **Color smoothing** | (1) **Continuous iteration**: log–log formula so \(C\) is fractional. (2) **Profile 1**: fractional part of \(C\) used in cycle position for smooth palette gradient. (3) **Palette**: linear interpolation between stops in GetRGBValues. |
| **Fade from black/white** | Configurable band [low_threshold_start, low_threshold_end]; linear blend from black or white for palette profiles 1 and 2. |
| **Fade to black** | Optional; last 15% of iteration range or last 0.5×color_length band scaled to black for palette profiles. |
| **Palette** | From `color-settings.txt`: positions in [0,1], RGB per stop; first stop duplicated at 1; linear interpolation. |
| **Profiles** | 1 = palette + cycles, 2 = palette + cycle length; 3–11 = fixed R,G,B formulas, no file. |

---

*Report based on the MSZP source code at [https://github.com/TonyVallad/MSZP](https://github.com/TonyVallad/MSZP) (MSZP.bas, bmp-subs.bas, other-subs.bas, functions.bas, color-settings.txt).*
