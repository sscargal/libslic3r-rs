//! Magic-byte format detection for mesh files.
//!
//! Detects whether a byte buffer contains a binary STL, ASCII STL, 3MF (ZIP),
//! or OBJ file. The detection logic handles the well-known ambiguity where
//! binary STL files may begin with `"solid"` in their 80-byte header.

use serde::{Deserialize, Serialize};

use crate::error::FileIOError;

/// Recognized mesh file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeshFormat {
    /// Binary STL format.
    StlBinary,
    /// ASCII STL format.
    StlAscii,
    /// 3MF format (ZIP archive with 3D model data).
    ThreeMf,
    /// Wavefront OBJ format.
    Obj,
}

/// Detect the mesh file format from the raw byte content.
///
/// Detection order (first match wins):
/// 1. **3MF**: Starts with ZIP magic bytes `[0x50, 0x4B, 0x03, 0x04]`.
/// 2. **ASCII STL**: Starts with `"solid"` AND the first ~1000 bytes contain
///    `"facet normal"`. This handles the common pitfall of binary STL files
///    that happen to start with `"solid"` in their header.
/// 3. **Binary STL**: Length >= 84 AND file size matches
///    `84 + num_triangles * 50` (with 1-byte tolerance for files with trailing
///    newline or padding).
/// 4. **OBJ**: First non-empty, non-comment line starts with `"v "`.
///
/// # Errors
///
/// - [`FileIOError::FileTooSmall`] if the data is too small to identify.
/// - [`FileIOError::UnrecognizedFormat`] if no known format matches.
pub fn detect_format(data: &[u8]) -> Result<MeshFormat, FileIOError> {
    if data.len() < 4 {
        return Err(FileIOError::FileTooSmall(data.len()));
    }

    // 1. Check for 3MF (ZIP magic bytes).
    if data.len() >= 4 && data[..4] == [0x50, 0x4B, 0x03, 0x04] {
        return Ok(MeshFormat::ThreeMf);
    }

    // 2. Check for ASCII STL: starts with "solid" AND contains "facet normal".
    let starts_with_solid = data.len() >= 5
        && data[..5].eq_ignore_ascii_case(b"solid")
        && (data.len() == 5 || data[5].is_ascii_whitespace());

    if starts_with_solid {
        // Check the first ~1000 bytes for "facet normal" to confirm ASCII STL.
        let check_len = data.len().min(1000);
        let check_slice = &data[..check_len];
        if let Ok(text) = std::str::from_utf8(check_slice) {
            let lower = text.to_ascii_lowercase();
            if lower.contains("facet normal") {
                return Ok(MeshFormat::StlAscii);
            }
        }
        // If "solid" but no "facet normal", fall through to binary STL check.
    }

    // 3. Check for binary STL: >= 84 bytes, size matches 84 + tri_count * 50.
    if data.len() >= 84 {
        let num_triangles = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
        let expected_size = 84 + num_triangles * 50;
        // Allow +/- 1 byte tolerance for trailing newline or padding.
        if data.len() >= expected_size && data.len() <= expected_size + 1 {
            return Ok(MeshFormat::StlBinary);
        }
    }

    // 4. Check for OBJ: first non-empty, non-comment line starts with a known
    //    OBJ keyword. Valid OBJ files may begin with group lines ("g ") before
    //    vertex data ("v "), so we check the first few significant lines.
    if let Ok(text) = std::str::from_utf8(data) {
        let mut checked = 0;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with("v ") || trimmed.starts_with("g ") || trimmed.starts_with("o ") {
                return Ok(MeshFormat::Obj);
            }
            checked += 1;
            if checked >= 3 {
                break;
            }
        }
    }

    Err(FileIOError::UnrecognizedFormat)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_3mf_zip_magic() {
        let mut data = vec![0x50, 0x4B, 0x03, 0x04];
        data.extend_from_slice(&[0u8; 100]);
        assert_eq!(detect_format(&data).unwrap(), MeshFormat::ThreeMf);
    }

    #[test]
    fn detect_ascii_stl() {
        let data = b"solid cube\n  facet normal 0 0 1\n    outer loop\n      vertex 0 0 0\n      vertex 1 0 0\n      vertex 0 1 0\n    endloop\n  endfacet\nendsolid cube\n";
        assert_eq!(detect_format(data).unwrap(), MeshFormat::StlAscii);
    }

    #[test]
    fn detect_binary_stl_with_solid_header() {
        // Binary STL whose 80-byte header starts with "solid" but has no
        // "facet normal" text -- should be detected as binary, not ASCII.
        let mut data = Vec::new();
        // 80-byte header starting with "solid"
        let mut header = b"solid misleading header".to_vec();
        header.resize(80, 0u8);
        data.extend_from_slice(&header);
        // 1 triangle
        data.extend_from_slice(&1u32.to_le_bytes());
        // Triangle: 12 bytes normal + 36 bytes vertices + 2 bytes attribute = 50
        data.extend_from_slice(&[0u8; 50]);
        assert_eq!(detect_format(&data).unwrap(), MeshFormat::StlBinary);
    }

    #[test]
    fn detect_binary_stl_normal() {
        let mut data = Vec::new();
        // 80-byte header (no "solid" prefix)
        let mut header = b"binary STL header".to_vec();
        header.resize(80, 0u8);
        data.extend_from_slice(&header);
        // 2 triangles
        data.extend_from_slice(&2u32.to_le_bytes());
        data.extend_from_slice(&[0u8; 100]); // 2 * 50 bytes
        assert_eq!(detect_format(&data).unwrap(), MeshFormat::StlBinary);
    }

    #[test]
    fn detect_obj_format() {
        let data = b"# OBJ file\nv 0.0 0.0 0.0\nv 1.0 0.0 0.0\nv 0.0 1.0 0.0\nf 1 2 3\n";
        assert_eq!(detect_format(data).unwrap(), MeshFormat::Obj);
    }

    #[test]
    fn file_too_small_error() {
        let data = b"ab";
        assert!(matches!(
            detect_format(data),
            Err(FileIOError::FileTooSmall(2))
        ));
    }

    #[test]
    fn unrecognized_format() {
        let data = b"this is just random text that doesn't match any format really at all and is long enough";
        assert!(matches!(
            detect_format(data),
            Err(FileIOError::UnrecognizedFormat)
        ));
    }
}
