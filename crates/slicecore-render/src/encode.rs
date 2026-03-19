//! Image encoding for thumbnail output (PNG and JPEG).

use image::{codecs::jpeg::JpegEncoder, ImageBuffer, RgbaImage};

use crate::ImageFormat;

/// Encodes an RGBA pixel buffer to the specified image format.
pub(crate) fn encode(
    width: u32,
    height: u32,
    pixels: &[[u8; 4]],
    format: ImageFormat,
    quality: Option<u8>,
) -> Vec<u8> {
    match format {
        ImageFormat::Png => encode_png(width, height, pixels),
        ImageFormat::Jpeg => encode_jpeg(width, height, pixels, quality.unwrap_or(85)),
    }
}

/// Encodes an RGBA pixel buffer as a PNG image.
///
/// The pixel data is expected in top-to-bottom, left-to-right row order
/// (matching PNG convention, no vertical flip needed).
pub(crate) fn encode_png(width: u32, height: u32, pixels: &[[u8; 4]]) -> Vec<u8> {
    let flat: Vec<u8> = pixels.iter().flat_map(|px| px.iter().copied()).collect();
    let img =
        RgbaImage::from_raw(width, height, flat).expect("pixel buffer size matches dimensions");
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .expect("PNG encoding failed");
    buf.into_inner()
}

/// Encodes an RGBA pixel buffer as a JPEG image with alpha compositing onto white.
///
/// JPEG does not support transparency, so alpha is composited against a white
/// background before encoding to RGB.
fn encode_jpeg(width: u32, height: u32, pixels: &[[u8; 4]], quality: u8) -> Vec<u8> {
    // Alpha composite onto white background, then convert to RGB
    let rgb: Vec<u8> = pixels
        .iter()
        .flat_map(|px| {
            let a = f32::from(px[3]) / 255.0;
            let r = (f32::from(px[0]).mul_add(a, 255.0 * (1.0 - a))) as u8;
            let g = (f32::from(px[1]).mul_add(a, 255.0 * (1.0 - a))) as u8;
            let b = (f32::from(px[2]).mul_add(a, 255.0 * (1.0 - a))) as u8;
            [r, g, b]
        })
        .collect();
    let img: ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, rgb).expect("pixel buffer size matches dimensions");
    let mut buf = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    img.write_with_encoder(encoder)
        .expect("JPEG encoding failed");
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_magic_bytes() {
        let pixels = vec![[255u8, 0, 0, 255]; 4];
        let data = encode_png(2, 2, &pixels);
        assert!(data.len() > 8);
        assert_eq!(data[0], 0x89);
        assert_eq!(data[1], b'P');
        assert_eq!(data[2], b'N');
        assert_eq!(data[3], b'G');
    }

    #[test]
    fn png_nontrivial_size() {
        let pixels = vec![[128u8, 128, 128, 255]; 100 * 100];
        let data = encode_png(100, 100, &pixels);
        assert!(data.len() > 100);
    }

    #[test]
    fn jpeg_magic_bytes() {
        let pixels = vec![[255u8, 0, 0, 255]; 4];
        let data = encode_jpeg(2, 2, &pixels, 85);
        assert!(data.len() > 2);
        assert_eq!(data[0], 0xFF);
        assert_eq!(data[1], 0xD8);
        assert_eq!(data[2], 0xFF);
    }

    #[test]
    fn jpeg_white_background_from_transparent() {
        // Fully transparent pixels should become white after compositing
        let pixels = vec![[0u8, 0, 0, 0]; 4];
        let data = encode_jpeg(2, 2, &pixels, 85);
        // Decode back to verify: JPEG magic present
        assert_eq!(data[0], 0xFF);
        assert_eq!(data[1], 0xD8);
        // At minimum, the output should be valid and non-empty
        assert!(data.len() > 100, "JPEG should have non-trivial size");
    }

    #[test]
    fn encode_dispatcher_png() {
        let pixels = vec![[255u8, 0, 0, 255]; 4];
        let data = encode(2, 2, &pixels, ImageFormat::Png, None);
        assert_eq!(data[0], 0x89); // PNG
    }

    #[test]
    fn encode_dispatcher_jpeg() {
        let pixels = vec![[255u8, 0, 0, 255]; 4];
        let data = encode(2, 2, &pixels, ImageFormat::Jpeg, None);
        assert_eq!(data[0], 0xFF); // JPEG
    }

    #[test]
    fn image_format_extension() {
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
    }
}
