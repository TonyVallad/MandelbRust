use crate::tile::Tile;

/// An RGBA pixel buffer representing a rendered image.
#[derive(Debug, Clone)]
pub struct RenderBuffer {
    pub width: u32,
    pub height: u32,
    /// RGBA pixel data, 4 bytes per pixel, row-major order.
    pub pixels: Vec<u8>,
}

impl RenderBuffer {
    /// Create a new buffer filled with black (opaque).
    pub fn new(width: u32, height: u32) -> Self {
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        // Set alpha to 255 for all pixels.
        for chunk in pixels.chunks_exact_mut(4) {
            chunk[3] = 255;
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    /// Copy a tile's RGBA data into the correct position in the buffer.
    pub fn blit_tile(&mut self, tile: &Tile, tile_pixels: &[u8]) {
        debug_assert_eq!(tile_pixels.len(), tile.pixel_count() * 4);
        let stride = self.width as usize * 4;
        for row in 0..tile.height as usize {
            let src_start = row * tile.width as usize * 4;
            let src_end = src_start + tile.width as usize * 4;
            let dst_start = (tile.y as usize + row) * stride + tile.x as usize * 4;
            let dst_end = dst_start + tile.width as usize * 4;
            self.pixels[dst_start..dst_end].copy_from_slice(&tile_pixels[src_start..src_end]);
        }
    }

    /// Copy a tile's data into a mirror position, flipping rows vertically.
    /// Used for real-axis symmetry: the primary tile's rows are written
    /// in reverse order into the mirror tile's location.
    pub fn blit_tile_mirrored(&mut self, mirror_tile: &Tile, primary_pixels: &[u8]) {
        debug_assert_eq!(primary_pixels.len(), mirror_tile.pixel_count() * 4);
        let stride = self.width as usize * 4;
        let th = mirror_tile.height as usize;
        let tw4 = mirror_tile.width as usize * 4;
        for row in 0..th {
            let src_row = th - 1 - row; // flip vertically
            let src_start = src_row * tw4;
            let src_end = src_start + tw4;
            let dst_start = (mirror_tile.y as usize + row) * stride + mirror_tile.x as usize * 4;
            let dst_end = dst_start + tw4;
            self.pixels[dst_start..dst_end].copy_from_slice(&primary_pixels[src_start..src_end]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_buffer_is_black_opaque() {
        let buf = RenderBuffer::new(4, 4);
        assert_eq!(buf.pixels.len(), 4 * 4 * 4);
        for chunk in buf.pixels.chunks_exact(4) {
            assert_eq!(chunk, &[0, 0, 0, 255]);
        }
    }

    #[test]
    fn blit_tile_writes_correct_region() {
        let mut buf = RenderBuffer::new(8, 8);
        let tile = Tile {
            x: 2,
            y: 1,
            width: 3,
            height: 2,
        };
        let red = vec![255, 0, 0, 255].repeat(tile.pixel_count());
        buf.blit_tile(&tile, &red);

        // Check a pixel inside the tile.
        let idx = ((1 * 8) + 2) * 4;
        assert_eq!(&buf.pixels[idx..idx + 4], &[255, 0, 0, 255]);

        // Check a pixel outside the tile is still black.
        let idx2 = 0;
        assert_eq!(&buf.pixels[idx2..idx2 + 4], &[0, 0, 0, 255]);
    }
}
