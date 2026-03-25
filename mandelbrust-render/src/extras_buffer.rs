//! Per-pixel extras buffer for advanced coloring (distance estimation, stripe average).

use mandelbrust_core::IterationExtras;

use crate::tile::Tile;

/// Stores per-pixel [`IterationExtras`] for a full frame, parallel to the
/// [`IterationBuffer`](crate::iteration_buffer::IterationBuffer).
#[derive(Clone)]
pub struct ExtrasBuffer {
    pub width: u32,
    pub height: u32,
    pub distance: Vec<f64>,
    pub stripe_avg: Vec<f64>,
}

impl ExtrasBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let size = width as usize * height as usize;
        Self {
            width,
            height,
            distance: vec![0.0; size],
            stripe_avg: vec![0.0; size],
        }
    }

    pub fn blit_tile(&mut self, tile: &Tile, tile_extras: &[IterationExtras]) {
        for py in 0..tile.height {
            let buf_y = tile.y + py;
            if buf_y >= self.height {
                break;
            }
            let dst_start = (buf_y * self.width + tile.x) as usize;
            let src_start = (py * tile.width) as usize;
            let copy_w = tile.width.min(self.width - tile.x) as usize;
            for i in 0..copy_w {
                let ext = &tile_extras[src_start + i];
                self.distance[dst_start + i] = ext.distance;
                self.stripe_avg[dst_start + i] = ext.stripe_avg;
            }
        }
    }

    pub fn shift(&mut self, dx: i32, dy: i32) {
        if dx == 0 && dy == 0 {
            return;
        }
        let w = self.width as i32;
        let h = self.height as i32;
        let size = self.distance.len();
        let mut new_dist = vec![0.0f64; size];
        let mut new_stripe = vec![0.0f64; size];

        let x_start = dx.max(0) as usize;
        let x_end = (w + dx).min(w).max(0) as usize;
        if x_start >= x_end {
            self.distance = new_dist;
            self.stripe_avg = new_stripe;
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
            new_dist[dst_row + x_start..dst_row + x_start + count].copy_from_slice(
                &self.distance[src_row + src_x_start..src_row + src_x_start + count],
            );
            new_stripe[dst_row + x_start..dst_row + x_start + count].copy_from_slice(
                &self.stripe_avg[src_row + src_x_start..src_row + src_x_start + count],
            );
        }

        self.distance = new_dist;
        self.stripe_avg = new_stripe;
    }
}
