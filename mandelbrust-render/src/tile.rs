/// Default tile size in pixels. 64×64 × 8 bytes = 32 KB, fits in L1 cache.
pub const TILE_SIZE: u32 = 64;

/// A rectangular tile within the viewport.
#[derive(Debug, Clone, Copy)]
pub struct Tile {
    /// Pixel x of the top-left corner.
    pub x: u32,
    /// Pixel y of the top-left corner.
    pub y: u32,
    /// Tile width in pixels (may be smaller at the right edge).
    pub width: u32,
    /// Tile height in pixels (may be smaller at the bottom edge).
    pub height: u32,
}

impl Tile {
    /// Number of pixels in this tile.
    pub fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }
}

/// How a tile relates to the real-axis symmetry optimization.
#[derive(Debug, Clone, Copy)]
pub enum TileKind {
    /// No symmetry applies — render normally.
    Normal,
    /// This tile is above (or on) the real axis and has a mirror below.
    Primary { mirror_index: usize },
    /// This tile mirrors a primary tile — copy its data with rows flipped.
    Mirror { primary_index: usize },
}

/// A tile paired with its symmetry classification.
#[derive(Debug, Clone, Copy)]
pub struct ClassifiedTile {
    pub tile: Tile,
    pub kind: TileKind,
}

/// Build a grid of tiles for the given viewport dimensions.
pub fn build_tile_grid(width: u32, height: u32) -> Vec<Tile> {
    let mut tiles = Vec::new();
    let mut y = 0;
    while y < height {
        let th = TILE_SIZE.min(height - y);
        let mut x = 0;
        while x < width {
            let tw = TILE_SIZE.min(width - x);
            tiles.push(Tile {
                x,
                y,
                width: tw,
                height: th,
            });
            x += tw;
        }
        y += th;
    }
    tiles
}

/// Classify tiles for real-axis symmetry exploitation.
///
/// When the viewport is centred on `im = 0`, tiles in the upper half
/// can be mirrored to the lower half, roughly halving the computation.
/// Returns `None` if symmetry doesn't apply (viewport not centred on real axis).
pub fn classify_tiles_for_symmetry(
    tiles: &[Tile],
    viewport_height: u32,
    center_im: f64,
) -> Option<Vec<ClassifiedTile>> {
    // Only apply when the viewport is exactly centred on the real axis.
    if center_im.abs() > f64::EPSILON {
        return None;
    }

    let half_h = viewport_height as f64 / 2.0;
    let mut classified: Vec<ClassifiedTile> = tiles
        .iter()
        .map(|&tile| ClassifiedTile {
            tile,
            kind: TileKind::Normal,
        })
        .collect();

    // Build a lookup from (x, y) → index for mirror matching.
    let tile_count = classified.len();
    for i in 0..tile_count {
        let tile = classified[i].tile;
        let tile_top = tile.y as f64;
        let tile_bottom = (tile.y + tile.height) as f64;

        // Skip tiles that cross the centre line — render normally.
        if tile_top < half_h && tile_bottom > half_h {
            continue;
        }

        // Tile is entirely in the upper half.
        if tile_bottom <= half_h {
            // Find the mirror tile in the lower half.
            let mirror_y = viewport_height - tile.y - tile.height;
            if let Some(j) = find_tile_at(&classified, tile.x, mirror_y, tile.width, tile.height) {
                if i != j {
                    classified[i].kind = TileKind::Primary { mirror_index: j };
                    classified[j].kind = TileKind::Mirror { primary_index: i };
                }
            }
        }
    }

    Some(classified)
}

fn find_tile_at(
    tiles: &[ClassifiedTile],
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Option<usize> {
    tiles.iter().position(|ct| {
        ct.tile.x == x && ct.tile.y == y && ct.tile.width == width && ct.tile.height == height
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_grid_covers_viewport() {
        let tiles = build_tile_grid(200, 150);
        let total_pixels: usize = tiles.iter().map(|t| t.pixel_count()).sum();
        assert_eq!(total_pixels, 200 * 150);
    }

    #[test]
    fn tile_grid_no_overlap() {
        let tiles = build_tile_grid(200, 150);
        let mut covered = vec![false; 200 * 150];
        for tile in &tiles {
            for py in tile.y..tile.y + tile.height {
                for px in tile.x..tile.x + tile.width {
                    let idx = py as usize * 200 + px as usize;
                    assert!(!covered[idx], "pixel ({px}, {py}) covered twice");
                    covered[idx] = true;
                }
            }
        }
        assert!(covered.iter().all(|&c| c), "all pixels must be covered");
    }

    #[test]
    fn tile_size_respects_constant() {
        let tiles = build_tile_grid(256, 256);
        for tile in &tiles {
            assert!(tile.width <= TILE_SIZE);
            assert!(tile.height <= TILE_SIZE);
        }
    }

    #[test]
    fn symmetry_classification_on_real_axis() {
        let tiles = build_tile_grid(128, 128);
        let classified = classify_tiles_for_symmetry(&tiles, 128, 0.0).unwrap();

        let primaries = classified
            .iter()
            .filter(|ct| matches!(ct.kind, TileKind::Primary { .. }))
            .count();
        let mirrors = classified
            .iter()
            .filter(|ct| matches!(ct.kind, TileKind::Mirror { .. }))
            .count();

        assert_eq!(primaries, mirrors, "each primary must have a mirror");
        assert!(
            primaries > 0,
            "128×128 centred at im=0 should have mirror pairs"
        );
    }

    #[test]
    fn symmetry_not_applied_off_axis() {
        let tiles = build_tile_grid(128, 128);
        assert!(classify_tiles_for_symmetry(&tiles, 128, 0.5).is_none());
    }
}
