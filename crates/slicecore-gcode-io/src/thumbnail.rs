//! G-code thumbnail comment writing.
//!
//! Writes thumbnail PNG data as G-code comment blocks directly to a writer.
//! This module has no dependency on slicecore-render -- it accepts raw PNG bytes.

use base64::Engine as _;
use std::io::Write;

/// Write thumbnail PNG data as G-code comment lines to a writer.
///
/// The format parameter selects the comment style:
/// - `"prusaslicer"` -> `; thumbnail begin WxH SIZE` / `; thumbnail end`
/// - `"creality"` -> `; png begin WxH SIZE` / `; png end`
///
/// Base64 lines are at most 78 characters each, prefixed with `"; "`.
pub fn write_thumbnail_comments<W: Write>(
    writer: &mut W,
    png_data: &[u8],
    width: u32,
    height: u32,
    format: &str,
) -> Result<(), std::io::Error> {
    let (begin_tag, end_tag) = match format {
        "creality" => ("png", "png"),
        _ => ("thumbnail", "thumbnail"), // prusaslicer is the default
    };

    let png_size = png_data.len();
    let b64 = base64::engine::general_purpose::STANDARD.encode(png_data);

    writeln!(writer, "; {} begin {}x{} {}", begin_tag, width, height, png_size)?;

    for chunk in b64.as_bytes().chunks(78) {
        write!(writer, "; ")?;
        writer.write_all(chunk)?;
        writeln!(writer)?;
    }

    writeln!(writer, "; {} end", end_tag)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_thumbnail_prusaslicer_format() {
        let png_data = vec![0x89, b'P', b'N', b'G', 1, 2, 3, 4, 5, 6, 7, 8];
        let mut buf = Vec::new();
        write_thumbnail_comments(&mut buf, &png_data, 100, 100, "prusaslicer").unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("; thumbnail begin 100x100 12"));
        assert!(output.contains("; thumbnail end"));
    }

    #[test]
    fn write_thumbnail_creality_format() {
        let png_data = vec![0x89, b'P', b'N', b'G', 1, 2, 3];
        let mut buf = Vec::new();
        write_thumbnail_comments(&mut buf, &png_data, 220, 124, "creality").unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("; png begin 220x124 7"));
        assert!(output.contains("; png end"));
    }

    #[test]
    fn thumbnail_lines_max_80_chars() {
        let png_data = vec![42u8; 500]; // produces multi-line base64
        let mut buf = Vec::new();
        write_thumbnail_comments(&mut buf, &png_data, 300, 300, "prusaslicer").unwrap();
        let output = String::from_utf8(buf).unwrap();
        for line in output.lines() {
            assert!(
                line.len() <= 80,
                "Line too long ({} chars): '{}'",
                line.len(),
                line
            );
        }
    }
}
