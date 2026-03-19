//! G-code thumbnail comment formatting.
//!
//! Formats thumbnail image data as G-code comment blocks compatible with
//! PrusaSlicer and Creality firmware thumbnail conventions.

use base64::Engine as _;

use crate::Thumbnail;

/// Thumbnail comment format for G-code embedding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailFormat {
    /// PrusaSlicer format: `; thumbnail begin WxH SIZE` / `; thumbnail end`
    PrusaSlicer,
    /// Creality format: `; png begin WxH SIZE` / `; png end`
    Creality,
}

/// Format a thumbnail as a G-code comment block.
///
/// The output contains:
/// - A begin tag with width, height, and byte size
/// - Base64-encoded PNG data split into lines of at most 78 characters,
///   each prefixed with `; `
/// - An end tag
///
/// # Example output (PrusaSlicer format)
/// ```text
/// ; thumbnail begin 300x300 12345
/// ; iVBORw0KGgoAAAANSUhEUgAA...
/// ; ...more base64 lines...
/// ; thumbnail end
/// ```
pub fn format_gcode_thumbnail_block(thumbnail: &Thumbnail, format: ThumbnailFormat) -> String {
    let (begin_tag, end_tag) = match format {
        ThumbnailFormat::PrusaSlicer => ("thumbnail", "thumbnail"),
        ThumbnailFormat::Creality => ("png", "png"),
    };

    let data_size = thumbnail.encoded_data.len();
    let b64 = base64::engine::general_purpose::STANDARD.encode(&thumbnail.encoded_data);

    let mut result = format!(
        "; {} begin {}x{} {}\n",
        begin_tag, thumbnail.width, thumbnail.height, data_size
    );

    // Split base64 into 78-char chunks
    for chunk in b64.as_bytes().chunks(78) {
        result.push_str("; ");
        result.push_str(std::str::from_utf8(chunk).unwrap());
        result.push('\n');
    }

    result.push_str(&format!("; {} end\n", end_tag));
    result
}

/// Determine the appropriate thumbnail format for a G-code dialect.
///
/// Returns `None` for Bambu (which uses 3MF thumbnails only, not G-code comments).
pub fn thumbnail_format_for_dialect(dialect_name: &str) -> Option<ThumbnailFormat> {
    match dialect_name.to_ascii_lowercase().as_str() {
        "marlin" | "klipper" | "reprap" | "reprapfirmware" => Some(ThumbnailFormat::PrusaSlicer),
        "creality" => Some(ThumbnailFormat::Creality),
        "bambu" => None,
        _ => Some(ThumbnailFormat::PrusaSlicer), // default to PrusaSlicer format
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CameraAngle;

    fn make_test_thumbnail() -> Thumbnail {
        // Create a small thumbnail with some fake PNG data
        Thumbnail {
            angle: CameraAngle::Isometric,
            width: 32,
            height: 32,
            rgba: vec![[128, 128, 128, 255]; 32 * 32],
            encoded_data: vec![
                0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4, 5,
            ],
            format: crate::ImageFormat::Png,
        }
    }

    #[test]
    fn format_prusaslicer_contains_begin_end() {
        let thumb = make_test_thumbnail();
        let block = format_gcode_thumbnail_block(&thumb, ThumbnailFormat::PrusaSlicer);
        assert!(block.contains("; thumbnail begin 32x32 13"));
        assert!(block.contains("; thumbnail end"));
    }

    #[test]
    fn format_creality_contains_png_begin_end() {
        let thumb = make_test_thumbnail();
        let block = format_gcode_thumbnail_block(&thumb, ThumbnailFormat::Creality);
        assert!(block.contains("; png begin 32x32 13"));
        assert!(block.contains("; png end"));
    }

    #[test]
    fn base64_lines_max_80_chars() {
        // Use larger PNG data to generate multiple base64 lines
        let thumb = Thumbnail {
            angle: CameraAngle::Isometric,
            width: 100,
            height: 100,
            rgba: vec![[0; 4]; 100],
            encoded_data: vec![42u8; 500], // 500 bytes -> ~668 base64 chars -> multiple lines
            format: crate::ImageFormat::Png,
        };
        let block = format_gcode_thumbnail_block(&thumb, ThumbnailFormat::PrusaSlicer);
        for line in block.lines() {
            assert!(
                line.len() <= 80,
                "Line too long ({} chars): '{}'",
                line.len(),
                line
            );
        }
    }

    #[test]
    fn format_for_dialect_marlin_prusaslicer() {
        assert_eq!(
            thumbnail_format_for_dialect("marlin"),
            Some(ThumbnailFormat::PrusaSlicer)
        );
        assert_eq!(
            thumbnail_format_for_dialect("klipper"),
            Some(ThumbnailFormat::PrusaSlicer)
        );
        assert_eq!(
            thumbnail_format_for_dialect("reprap"),
            Some(ThumbnailFormat::PrusaSlicer)
        );
    }

    #[test]
    fn format_for_dialect_bambu_none() {
        assert_eq!(thumbnail_format_for_dialect("bambu"), None);
    }
}
