//! PNG export with embedded metadata (tEXt chunks).

use std::io::BufWriter;
use std::path::Path;

use tracing::debug;

/// Metadata to embed in an exported PNG as tEXt chunks.
pub struct ExportMetadata {
    pub fractal_type: String,
    pub center_re: String,
    pub center_im: String,
    pub zoom: String,
    pub max_iterations: u32,
    pub escape_radius: f64,
    pub julia_c_re: Option<String>,
    pub julia_c_im: Option<String>,
    pub aa_level: u32,
    pub palette_name: String,
    pub smooth_coloring: bool,
    pub width: u32,
    pub height: u32,
}

/// Write an RGBA pixel buffer as a PNG file with embedded fractal metadata.
///
/// Uses the `png` crate directly (rather than `image`) to inject custom tEXt
/// chunks readable by exiftool, IrfanView, XnView, etc.
pub fn export_png(
    pixels: &[u8],
    width: u32,
    height: u32,
    path: &Path,
    metadata: &ExportMetadata,
) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("Failed to create file: {e}"))?;
    let writer = BufWriter::new(file);

    let mut encoder = png::Encoder::new(writer, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Default);

    encoder.add_text_chunk("Software".to_string(), "MandelbRust".to_string())
        .map_err(|e| format!("Failed to add text chunk: {e}"))?;

    let description = build_description(metadata);
    encoder.add_text_chunk("Description".to_string(), description)
        .map_err(|e| format!("Failed to add text chunk: {e}"))?;

    let meta_pairs = build_metadata_pairs(metadata);
    for (key, value) in &meta_pairs {
        encoder.add_text_chunk(key.clone(), value.clone())
            .map_err(|e| format!("Failed to add text chunk '{key}': {e}"))?;
    }

    let mut png_writer = encoder
        .write_header()
        .map_err(|e| format!("Failed to write PNG header: {e}"))?;

    png_writer
        .write_image_data(pixels)
        .map_err(|e| format!("Failed to write PNG image data: {e}"))?;

    debug!("Exported PNG {}x{} to {}", width, height, path.display());
    Ok(())
}

fn build_description(meta: &ExportMetadata) -> String {
    let mut desc = format!(
        "{} - Center: {} {}i, Zoom: {}, Iterations: {}",
        meta.fractal_type, meta.center_re, meta.center_im, meta.zoom, meta.max_iterations,
    );
    if let (Some(re), Some(im)) = (&meta.julia_c_re, &meta.julia_c_im) {
        desc.push_str(&format!(", Julia C: {} {}i", re, im));
    }
    desc
}

fn build_metadata_pairs(meta: &ExportMetadata) -> Vec<(String, String)> {
    let mut pairs = vec![
        ("MandelbRust.FractalType".into(), meta.fractal_type.clone()),
        ("MandelbRust.CenterRe".into(), meta.center_re.clone()),
        ("MandelbRust.CenterIm".into(), meta.center_im.clone()),
        ("MandelbRust.Zoom".into(), meta.zoom.clone()),
        ("MandelbRust.MaxIterations".into(), meta.max_iterations.to_string()),
        ("MandelbRust.EscapeRadius".into(), format!("{}", meta.escape_radius)),
        ("MandelbRust.AALevel".into(), meta.aa_level.to_string()),
        ("MandelbRust.Palette".into(), meta.palette_name.clone()),
        ("MandelbRust.SmoothColoring".into(), meta.smooth_coloring.to_string()),
        ("MandelbRust.Resolution".into(), format!("{}x{}", meta.width, meta.height)),
    ];
    if let Some(re) = &meta.julia_c_re {
        pairs.push(("MandelbRust.JuliaC_Re".into(), re.clone()));
    }
    if let Some(im) = &meta.julia_c_im {
        pairs.push(("MandelbRust.JuliaC_Im".into(), im.clone()));
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn export_creates_valid_png() {
        let w = 4u32;
        let h = 4u32;
        let pixels = vec![128u8; (w * h * 4) as usize];
        let meta = ExportMetadata {
            fractal_type: "Mandelbrot".into(),
            center_re: "-0.5".into(),
            center_im: "0.0".into(),
            zoom: "1.0".into(),
            max_iterations: 256,
            escape_radius: 2.0,
            julia_c_re: None,
            julia_c_im: None,
            aa_level: 0,
            palette_name: "Classic".into(),
            smooth_coloring: true,
            width: w,
            height: h,
        };
        let dir = std::env::temp_dir().join("mandelbrust_test_export");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_export.png");
        export_png(&pixels, w, h, &path, &meta).expect("export should succeed");

        let mut file = std::fs::File::open(&path).expect("file should exist");
        let mut header = [0u8; 8];
        file.read_exact(&mut header).expect("should read header");
        assert_eq!(&header, b"\x89PNG\r\n\x1a\n", "valid PNG signature");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn export_embeds_text_chunks() {
        let w = 2u32;
        let h = 2u32;
        let pixels = vec![0u8; (w * h * 4) as usize];
        let meta = ExportMetadata {
            fractal_type: "Julia".into(),
            center_re: "0.0".into(),
            center_im: "0.0".into(),
            zoom: "1.0".into(),
            max_iterations: 100,
            escape_radius: 2.0,
            julia_c_re: Some("-0.7".into()),
            julia_c_im: Some("0.27015".into()),
            aa_level: 4,
            palette_name: "Fire".into(),
            smooth_coloring: false,
            width: w,
            height: h,
        };
        let dir = std::env::temp_dir().join("mandelbrust_test_export_meta");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_meta.png");
        export_png(&pixels, w, h, &path, &meta).expect("export should succeed");

        let decoder = png::Decoder::new(
            std::fs::File::open(&path).expect("file should exist"),
        );
        let reader = decoder.read_info().expect("should read info");
        let info = reader.info();
        let texts: Vec<_> = info.uncompressed_latin1_text.iter().collect();
        assert!(
            texts.iter().any(|t| t.keyword == "Software" && t.text == "MandelbRust"),
            "Should contain Software text chunk"
        );
        assert!(
            texts.iter().any(|t| t.keyword == "MandelbRust.FractalType" && t.text == "Julia"),
            "Should contain fractal type chunk"
        );
        assert!(
            texts.iter().any(|t| t.keyword == "MandelbRust.JuliaC_Re"),
            "Should contain Julia C Re chunk"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
