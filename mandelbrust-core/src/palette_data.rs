//! Palette data model: user-defined color palettes with gradient interpolation.
//!
//! Each palette is a sequence of [`ColorStop`]s at normalized positions along
//! `[0.0, 1.0]`. The gradient between stops is computed per-pixel via linear
//! interpolation — no fixed-resolution LUT is used.

use serde::{Deserialize, Serialize};

/// An RGB color (0–255 per channel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parse a hex string like `#3A120D` or `3A120D`.
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Self { r, g, b })
    }

    /// Format as `#RRGGBB`.
    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

/// A color stop: a color at a normalized position on the palette bar.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColorStop {
    /// Position in `[0.0, 1.0]`.
    pub position: f64,
    pub color: Rgb,
}

/// A user-defined color palette stored as an individual JSON file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaletteDefinition {
    pub name: String,
    /// Color stops sorted by position. Must contain at least two stops
    /// (at positions 0.0 and 1.0).
    pub colors: Vec<ColorStop>,
    /// When true, the last stop's color is always kept in sync with the
    /// first stop's color, producing seamless multi-cycle tiling.
    #[serde(default)]
    pub lock_end_to_start: bool,
}

impl PaletteDefinition {
    /// Create a new palette with the given stops. Stops are sorted by position.
    /// Endpoint stops at 0.0 and 1.0 are guaranteed to exist.
    pub fn new(name: impl Into<String>, mut colors: Vec<ColorStop>) -> Self {
        colors.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
        ensure_endpoints(&mut colors);
        let mut def = Self {
            name: name.into(),
            colors,
            lock_end_to_start: true,
        };
        def.enforce_lock();
        def
    }

    /// Ensure stops are sorted by position (call after mutation).
    pub fn sort_stops(&mut self) {
        self.colors
            .sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
    }

    /// If `lock_end_to_start` is on, copy the first stop's color to the last.
    pub fn enforce_lock(&mut self) {
        if self.lock_end_to_start && self.colors.len() >= 2 {
            let first_color = self.colors[0].color;
            let last = self.colors.len() - 1;
            self.colors[last].color = first_color;
        }
    }

    /// Sample the gradient at a normalized position `t` in `[0.0, 1.0]`.
    /// Returns an RGBA color (alpha always 255).
    pub fn sample(&self, t: f64) -> [u8; 4] {
        if self.colors.is_empty() {
            return [0, 0, 0, 255];
        }
        if self.colors.len() == 1 {
            let c = self.colors[0].color;
            return [c.r, c.g, c.b, 255];
        }

        let t = t.clamp(0.0, 1.0);

        let mut lo = 0;
        for (i, stop) in self.colors.iter().enumerate() {
            if stop.position <= t {
                lo = i;
            }
        }
        let hi = (lo + 1).min(self.colors.len() - 1);

        let lo_stop = &self.colors[lo];
        let hi_stop = &self.colors[hi];

        let frac = if (hi_stop.position - lo_stop.position).abs() < 1e-10 {
            0.0
        } else {
            ((t - lo_stop.position) / (hi_stop.position - lo_stop.position)).clamp(0.0, 1.0)
        };

        let base = lerp_rgb(lo_stop.color, hi_stop.color, frac);
        [base.r, base.g, base.b, 255]
    }

    /// Sample the gradient at an arbitrary float position, wrapping via
    /// `t.rem_euclid(1.0)` so the palette tiles seamlessly.
    pub fn sample_wrapped(&self, t: f64) -> [u8; 4] {
        self.sample(t.rem_euclid(1.0))
    }
}

fn lerp_rgb(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let inv = 1.0 - t;
    Rgb {
        r: (a.r as f64 * inv + b.r as f64 * t).round() as u8,
        g: (a.g as f64 * inv + b.g as f64 * t).round() as u8,
        b: (a.b as f64 * inv + b.b as f64 * t).round() as u8,
    }
}

/// Ensure the stop list has entries at exactly 0.0 and 1.0.
fn ensure_endpoints(colors: &mut Vec<ColorStop>) {
    if colors.is_empty() {
        colors.push(ColorStop {
            position: 0.0,
            color: Rgb::BLACK,
        });
        colors.push(ColorStop {
            position: 1.0,
            color: Rgb::WHITE,
        });
        return;
    }
    if colors[0].position > 1e-9 {
        colors.insert(
            0,
            ColorStop {
                position: 0.0,
                color: colors[0].color,
            },
        );
    } else {
        colors[0].position = 0.0;
    }
    let last = colors.len() - 1;
    if colors[last].position < 1.0 - 1e-9 {
        colors.push(ColorStop {
            position: 1.0,
            color: colors[last].color,
        });
    } else {
        colors[last].position = 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_hex_round_trip() {
        let c = Rgb::new(58, 18, 13);
        let hex = c.to_hex();
        assert_eq!(hex, "#3A120D");
        assert_eq!(Rgb::from_hex(&hex), Some(c));
    }

    #[test]
    fn rgb_hex_with_and_without_hash() {
        assert_eq!(Rgb::from_hex("#FF0000"), Some(Rgb::new(255, 0, 0)));
        assert_eq!(Rgb::from_hex("FF0000"), Some(Rgb::new(255, 0, 0)));
    }

    #[test]
    fn rgb_hex_invalid() {
        assert_eq!(Rgb::from_hex("ZZZZZZ"), None);
        assert_eq!(Rgb::from_hex("FF00"), None);
    }

    #[test]
    fn two_stop_interpolation_locked() {
        let pal = PaletteDefinition::new(
            "test",
            vec![
                ColorStop {
                    position: 0.0,
                    color: Rgb::new(0, 0, 0),
                },
                ColorStop {
                    position: 1.0,
                    color: Rgb::new(255, 255, 255),
                },
            ],
        );
        // lock_end_to_start is true by default → end = start = black
        assert_eq!(pal.colors.last().unwrap().color, Rgb::new(0, 0, 0));
        let mid = pal.sample(0.5);
        assert_eq!(mid, [0, 0, 0, 255]);
    }

    #[test]
    fn two_stop_interpolation_unlocked() {
        let mut pal = PaletteDefinition::new(
            "test",
            vec![
                ColorStop {
                    position: 0.0,
                    color: Rgb::new(0, 0, 0),
                },
                ColorStop {
                    position: 1.0,
                    color: Rgb::new(255, 255, 255),
                },
            ],
        );
        pal.lock_end_to_start = false;
        pal.colors.last_mut().unwrap().color = Rgb::new(255, 255, 255);
        let mid = pal.sample(0.5);
        assert_eq!(mid, [128, 128, 128, 255]);
    }

    #[test]
    fn stops_are_sorted() {
        let pal = PaletteDefinition::new(
            "test",
            vec![
                ColorStop {
                    position: 0.8,
                    color: Rgb::new(255, 0, 0),
                },
                ColorStop {
                    position: 0.2,
                    color: Rgb::new(0, 0, 255),
                },
            ],
        );
        assert!(pal.colors[0].position < pal.colors[1].position);
    }

    #[test]
    fn sample_wrapped_tiles() {
        let mut pal = PaletteDefinition::new(
            "test",
            vec![
                ColorStop {
                    position: 0.0,
                    color: Rgb::new(0, 0, 0),
                },
                ColorStop {
                    position: 1.0,
                    color: Rgb::new(255, 255, 255),
                },
            ],
        );
        pal.lock_end_to_start = false;
        pal.colors.last_mut().unwrap().color = Rgb::new(255, 255, 255);
        let a = pal.sample(0.3);
        let b = pal.sample_wrapped(1.3);
        assert_eq!(a, b);
    }

    #[test]
    fn endpoints_guaranteed() {
        let pal = PaletteDefinition::new(
            "test",
            vec![ColorStop {
                position: 0.5,
                color: Rgb::new(100, 150, 200),
            }],
        );
        assert_eq!(pal.colors.len(), 3);
        assert!((pal.colors[0].position).abs() < 1e-9);
        assert!((pal.colors[2].position - 1.0).abs() < 1e-9);
    }

    #[test]
    fn lock_end_to_start_syncs() {
        let mut pal = PaletteDefinition::new(
            "test",
            vec![
                ColorStop {
                    position: 0.0,
                    color: Rgb::new(255, 0, 0),
                },
                ColorStop {
                    position: 1.0,
                    color: Rgb::new(0, 0, 255),
                },
            ],
        );
        pal.lock_end_to_start = true;
        pal.enforce_lock();
        assert_eq!(pal.colors.last().unwrap().color, pal.colors[0].color);
    }

    #[test]
    fn serde_round_trip() {
        let pal = PaletteDefinition {
            name: "My Palette".to_string(),
            colors: vec![
                ColorStop {
                    position: 0.0,
                    color: Rgb::new(0, 0, 0),
                },
                ColorStop {
                    position: 1.0,
                    color: Rgb::new(255, 255, 255),
                },
            ],
            lock_end_to_start: true,
        };
        let json = serde_json::to_string_pretty(&pal).unwrap();
        let loaded: PaletteDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.name, pal.name);
        assert_eq!(loaded.colors.len(), 2);
        assert!(loaded.lock_end_to_start);
    }
}
