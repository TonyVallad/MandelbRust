use mandelbrust_core::IterationResult;

use crate::tile::Tile;

/// Stores per-pixel `IterationResult` data for a full frame.
///
/// This is the raw output of the renderer before coloring.  Keeping iteration
/// data separate from colored pixels enables instant palette switching without
/// re-computing iterations.
#[derive(Clone)]
pub struct IterationBuffer {
    pub width: u32,
    pub height: u32,
    pub max_iterations: u32,
    pub data: Vec<IterationResult>,
}

impl IterationBuffer {
    pub fn new(width: u32, height: u32, max_iterations: u32) -> Self {
        let size = width as usize * height as usize;
        Self {
            width,
            height,
            max_iterations,
            data: vec![IterationResult::Interior; size],
        }
    }

    /// Copy tile iteration data into the correct region of the buffer.
    pub fn blit_tile(&mut self, tile: &Tile, tile_data: &[IterationResult]) {
        for py in 0..tile.height {
            let buf_y = tile.y + py;
            if buf_y >= self.height {
                break;
            }
            let dst_start = (buf_y * self.width + tile.x) as usize;
            let src_start = (py * tile.width) as usize;
            let copy_w = tile.width.min(self.width - tile.x) as usize;
            self.data[dst_start..dst_start + copy_w]
                .copy_from_slice(&tile_data[src_start..src_start + copy_w]);
        }
    }

    /// Shift the buffer by a pixel offset, preserving overlapping data.
    ///
    /// `dx > 0` means content moves right (left edge exposed).
    /// `dy > 0` means content moves down (top edge exposed).
    /// Exposed regions are filled with `Interior`.
    pub fn shift(&mut self, dx: i32, dy: i32) {
        if dx == 0 && dy == 0 {
            return;
        }
        let w = self.width as i32;
        let h = self.height as i32;
        let mut new_data = vec![IterationResult::Interior; self.data.len()];

        let x_start = dx.max(0) as usize;
        let x_end = (w + dx).min(w).max(0) as usize;
        if x_start >= x_end {
            self.data = new_data;
            return;
        }
        let count = x_end - x_start;
        let src_x_start = (x_start as i32 - dx) as usize;

        for dst_y in 0..h as usize {
            let src_y = dst_y as i32 - dy;
            if src_y < 0 || src_y >= h {
                continue;
            }
            let dst_row = dst_y * self.width as usize;
            let src_row = src_y as usize * self.width as usize;
            new_data[dst_row + x_start..dst_row + x_end]
                .copy_from_slice(&self.data[src_row + src_x_start..src_row + src_x_start + count]);
        }

        self.data = new_data;
    }

    /// Copy tile iteration data with vertical flip (for real-axis symmetry).
    pub fn blit_tile_mirrored(&mut self, tile: &Tile, tile_data: &[IterationResult]) {
        for py in 0..tile.height {
            let buf_y = tile.y + py;
            if buf_y >= self.height {
                break;
            }
            let src_py = tile.height - 1 - py;
            let dst_start = (buf_y * self.width + tile.x) as usize;
            let src_start = (src_py * tile.width) as usize;
            let copy_w = tile.width.min(self.width - tile.x) as usize;
            self.data[dst_start..dst_start + copy_w]
                .copy_from_slice(&tile_data[src_start..src_start + copy_w]);
        }
    }
}
