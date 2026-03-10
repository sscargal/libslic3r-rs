//! PNG encoding from RGBA pixel buffer.

/// Encodes an RGBA pixel buffer as a PNG image.
///
/// The pixel data is expected in top-to-bottom, left-to-right row order
/// (matching PNG convention, no vertical flip needed).
pub(crate) fn encode_png(width: u32, height: u32, pixels: &[[u8; 4]]) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut buf, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);

        let mut writer = encoder.write_header().expect("PNG header write failed");

        // Flatten &[[u8; 4]] to &[u8]
        let flat: &[u8] =
            unsafe { std::slice::from_raw_parts(pixels.as_ptr() as *const u8, pixels.len() * 4) };

        writer.write_image_data(flat).expect("PNG data write failed");
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_magic_bytes() {
        let pixels = vec![[255u8, 0, 0, 255]; 4]; // 2x2 red
        let data = encode_png(2, 2, &pixels);

        // PNG magic: 0x89 P N G
        assert!(data.len() > 8, "PNG should have at least header bytes");
        assert_eq!(data[0], 0x89);
        assert_eq!(data[1], b'P');
        assert_eq!(data[2], b'N');
        assert_eq!(data[3], b'G');
    }

    #[test]
    fn png_nontrivial_size() {
        let pixels = vec![[128u8, 128, 128, 255]; 100 * 100]; // 100x100 gray
        let data = encode_png(100, 100, &pixels);
        // Should be larger than just headers: 100*100*4 = 40000 pixels, compressed but non-trivial
        assert!(
            data.len() > 100,
            "PNG should have non-trivial size, got {}",
            data.len()
        );
    }
}
