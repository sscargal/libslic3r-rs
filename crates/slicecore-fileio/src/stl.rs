//! Unified STL loading interface.
//!
//! Provides a single entry point [`parse_stl`] that auto-detects whether the
//! data is binary or ASCII STL and dispatches to the appropriate parser.

use slicecore_mesh::TriangleMesh;

use crate::detect::{detect_format, MeshFormat};
use crate::error::FileIOError;
use crate::{stl_ascii, stl_binary};

/// Parse an STL file (binary or ASCII) from raw bytes.
///
/// Uses [`detect_format`] to determine the STL variant, then dispatches
/// to [`stl_binary::parse`] or [`stl_ascii::parse`].
///
/// # Errors
///
/// - [`FileIOError::UnrecognizedFormat`] if the data is not an STL file.
/// - Any error from the underlying binary or ASCII parser.
pub fn parse_stl(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    let format = detect_format(data)?;

    match format {
        MeshFormat::StlBinary => stl_binary::parse(data),
        MeshFormat::StlAscii => stl_ascii::parse(data),
        _ => Err(FileIOError::UnrecognizedFormat),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a binary STL byte buffer from raw triangle data.
    fn build_binary_stl(triangles: &[([f32; 3], [f32; 3], [f32; 3], [f32; 3])]) -> Vec<u8> {
        let mut data = Vec::new();

        // 80-byte header
        let mut header = b"binary STL test".to_vec();
        header.resize(80, 0u8);
        data.extend_from_slice(&header);

        // Triangle count
        data.extend_from_slice(&(triangles.len() as u32).to_le_bytes());

        for (normal, v0, v1, v2) in triangles {
            for c in normal {
                data.extend_from_slice(&c.to_le_bytes());
            }
            for c in v0 {
                data.extend_from_slice(&c.to_le_bytes());
            }
            for c in v1 {
                data.extend_from_slice(&c.to_le_bytes());
            }
            for c in v2 {
                data.extend_from_slice(&c.to_le_bytes());
            }
            data.extend_from_slice(&0u16.to_le_bytes());
        }

        data
    }

    fn unit_cube_binary() -> Vec<u8> {
        let v = [
            [0.0f32, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        let n0 = [0.0f32, 0.0, 0.0];

        build_binary_stl(&[
            (n0, v[4], v[5], v[6]),
            (n0, v[4], v[6], v[7]),
            (n0, v[1], v[0], v[3]),
            (n0, v[1], v[3], v[2]),
            (n0, v[1], v[2], v[6]),
            (n0, v[1], v[6], v[5]),
            (n0, v[0], v[4], v[7]),
            (n0, v[0], v[7], v[3]),
            (n0, v[3], v[7], v[6]),
            (n0, v[3], v[6], v[2]),
            (n0, v[0], v[1], v[5]),
            (n0, v[0], v[5], v[4]),
        ])
    }

    fn unit_cube_ascii() -> &'static [u8] {
        br#"solid cube
  facet normal 0 0 1
    outer loop
      vertex 0 0 1
      vertex 1 0 1
      vertex 1 1 1
    endloop
  endfacet
  facet normal 0 0 1
    outer loop
      vertex 0 0 1
      vertex 1 1 1
      vertex 0 1 1
    endloop
  endfacet
  facet normal 0 0 -1
    outer loop
      vertex 1 0 0
      vertex 0 0 0
      vertex 0 1 0
    endloop
  endfacet
  facet normal 0 0 -1
    outer loop
      vertex 1 0 0
      vertex 0 1 0
      vertex 1 1 0
    endloop
  endfacet
  facet normal 1 0 0
    outer loop
      vertex 1 0 0
      vertex 1 1 0
      vertex 1 1 1
    endloop
  endfacet
  facet normal 1 0 0
    outer loop
      vertex 1 0 0
      vertex 1 1 1
      vertex 1 0 1
    endloop
  endfacet
  facet normal -1 0 0
    outer loop
      vertex 0 0 0
      vertex 0 0 1
      vertex 0 1 1
    endloop
  endfacet
  facet normal -1 0 0
    outer loop
      vertex 0 0 0
      vertex 0 1 1
      vertex 0 1 0
    endloop
  endfacet
  facet normal 0 1 0
    outer loop
      vertex 0 1 0
      vertex 0 1 1
      vertex 1 1 1
    endloop
  endfacet
  facet normal 0 1 0
    outer loop
      vertex 0 1 0
      vertex 1 1 1
      vertex 1 1 0
    endloop
  endfacet
  facet normal 0 -1 0
    outer loop
      vertex 0 0 0
      vertex 1 0 0
      vertex 1 0 1
    endloop
  endfacet
  facet normal 0 -1 0
    outer loop
      vertex 0 0 0
      vertex 1 0 1
      vertex 0 0 1
    endloop
  endfacet
endsolid cube
"#
    }

    #[test]
    fn parse_binary_stl_via_unified_interface() {
        let data = unit_cube_binary();
        let mesh = parse_stl(&data).unwrap();
        assert_eq!(mesh.triangle_count(), 12);
        assert_eq!(mesh.vertex_count(), 8);
    }

    #[test]
    fn parse_ascii_stl_via_unified_interface() {
        let data = unit_cube_ascii();
        let mesh = parse_stl(data).unwrap();
        assert_eq!(mesh.triangle_count(), 12);
        assert_eq!(mesh.vertex_count(), 8);
    }

    #[test]
    fn non_stl_format_returns_unrecognized() {
        // 3MF (ZIP) data should be rejected by parse_stl.
        let mut data = vec![0x50, 0x4B, 0x03, 0x04];
        data.extend_from_slice(&[0u8; 100]);
        let result = parse_stl(&data);
        assert!(
            matches!(result, Err(FileIOError::UnrecognizedFormat)),
            "expected UnrecognizedFormat for 3MF data"
        );
    }

    #[test]
    fn binary_stl_with_solid_header_parses_as_binary() {
        // Binary STL with "solid" in the header -- parse_stl should detect it
        // as binary (not ASCII) because there's no "facet normal" text.
        let v = [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let n0 = [0.0f32, 0.0, 1.0];

        let mut data = Vec::new();
        // Header starting with "solid"
        let mut header = b"solid misleading header".to_vec();
        header.resize(80, 0u8);
        data.extend_from_slice(&header);
        // 1 triangle
        data.extend_from_slice(&1u32.to_le_bytes());
        // Normal
        for c in &n0 {
            data.extend_from_slice(&c.to_le_bytes());
        }
        // Vertices
        for vert in &v {
            for c in vert {
                data.extend_from_slice(&c.to_le_bytes());
            }
        }
        // Attribute
        data.extend_from_slice(&0u16.to_le_bytes());

        let mesh = parse_stl(&data).unwrap();
        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.vertex_count(), 3);
    }
}
