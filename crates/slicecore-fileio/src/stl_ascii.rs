//! ASCII STL parser.
//!
//! Parses ASCII STL files into [`TriangleMesh`]. ASCII STL is a text-based
//! format where each facet lists its normal and three vertices as floating-point
//! numbers.
//!
//! Like the binary parser, vertices are deduplicated using quantized integer
//! key hashing to produce a shared vertex buffer.

use std::collections::HashMap;

use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

use crate::error::FileIOError;

/// Quantization scale for vertex deduplication (same as binary parser).
const QUANTIZE_SCALE: f64 = 1e5;

/// Parse an ASCII STL file from raw bytes into a [`TriangleMesh`].
///
/// # ASCII STL format
///
/// ```text
/// solid name
///   facet normal ni nj nk
///     outer loop
///       vertex x y z
///       vertex x y z
///       vertex x y z
///     endloop
///   endfacet
///   ...
/// endsolid name
/// ```
///
/// The parser ignores normals (they are recomputed by `TriangleMesh::new`)
/// and focuses on extracting `vertex` lines. Each `endfacet` emits one
/// triangle from the 3 most recently collected vertices.
///
/// # Errors
///
/// - [`FileIOError::InvalidUtf8`] if the data contains non-UTF-8 bytes.
/// - [`FileIOError::ParseError`] if a vertex line has invalid coordinates.
/// - [`FileIOError::EmptyModel`] if no triangles are found.
/// - [`FileIOError::MeshError`] if mesh construction fails.
pub fn parse(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    let text = std::str::from_utf8(data).map_err(|_| FileIOError::InvalidUtf8)?;

    let mut vertices: Vec<Point3> = Vec::new();
    let mut indices: Vec<[u32; 3]> = Vec::new();
    let mut vertex_map: HashMap<[i64; 3], u32> = HashMap::new();

    // Collect vertices for the current facet.
    let mut facet_verts: Vec<u32> = Vec::with_capacity(3);

    for line in text.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.starts_with("vertex") {
            let coords = parse_vertex_line(trimmed)?;
            let key = quantize_vertex(coords[0], coords[1], coords[2]);

            let vertex_idx = match vertex_map.get(&key) {
                Some(&existing) => existing,
                None => {
                    let new_idx = vertices.len() as u32;
                    vertices.push(Point3::new(coords[0], coords[1], coords[2]));
                    vertex_map.insert(key, new_idx);
                    new_idx
                }
            };

            facet_verts.push(vertex_idx);
        } else if lower.starts_with("endfacet") {
            if facet_verts.len() == 3 {
                indices.push([facet_verts[0], facet_verts[1], facet_verts[2]]);
            }
            facet_verts.clear();
        }
    }

    if indices.is_empty() {
        return Err(FileIOError::EmptyModel);
    }

    let mesh = TriangleMesh::new(vertices, indices)?;
    Ok(mesh)
}

/// Parse a "vertex X Y Z" line into [x, y, z] coordinates.
fn parse_vertex_line(line: &str) -> Result<[f64; 3], FileIOError> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 4 {
        return Err(FileIOError::ParseError(format!(
            "vertex line has fewer than 4 tokens: '{}'",
            line
        )));
    }

    let x = parts[1].parse::<f64>().map_err(|e| {
        FileIOError::ParseError(format!("invalid x coordinate '{}': {}", parts[1], e))
    })?;
    let y = parts[2].parse::<f64>().map_err(|e| {
        FileIOError::ParseError(format!("invalid y coordinate '{}': {}", parts[2], e))
    })?;
    let z = parts[3].parse::<f64>().map_err(|e| {
        FileIOError::ParseError(format!("invalid z coordinate '{}': {}", parts[3], e))
    })?;

    Ok([x, y, z])
}

/// Quantize a vertex coordinate to an integer key for deduplication.
#[inline]
fn quantize_vertex(x: f64, y: f64, z: f64) -> [i64; 3] {
    [
        (x * QUANTIZE_SCALE).round() as i64,
        (y * QUANTIZE_SCALE).round() as i64,
        (z * QUANTIZE_SCALE).round() as i64,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple ASCII STL unit cube (12 triangles, 8 unique vertices).
    fn unit_cube_ascii_stl() -> &'static [u8] {
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
    fn parse_unit_cube_ascii_stl() {
        let data = unit_cube_ascii_stl();
        let mesh = parse(data).unwrap();

        assert_eq!(mesh.triangle_count(), 12);
        assert_eq!(mesh.vertex_count(), 8);
    }

    #[test]
    fn vertex_deduplication_reduces_count() {
        let data = unit_cube_ascii_stl();
        let mesh = parse(data).unwrap();

        // Without dedup: 12 * 3 = 36 vertices. With dedup: 8.
        assert!(
            mesh.vertex_count() < 12 * 3,
            "vertex count {} should be less than {}",
            mesh.vertex_count(),
            12 * 3
        );
        assert_eq!(mesh.vertex_count(), 8);
    }

    #[test]
    fn invalid_utf8_produces_error() {
        // Create invalid UTF-8 bytes.
        let data: &[u8] = &[0xFF, 0xFE, 0x80, 0x81, 0x82];
        let result = parse(data);
        assert!(matches!(result, Err(FileIOError::InvalidUtf8)));
    }

    #[test]
    fn malformed_vertex_line_produces_parse_error() {
        let data = b"solid test\n  facet normal 0 0 1\n    outer loop\n      vertex 0.0\n    endloop\n  endfacet\nendsolid test\n";
        let result = parse(data);
        assert!(
            matches!(result, Err(FileIOError::ParseError(_))),
            "expected ParseError for malformed vertex"
        );
    }

    #[test]
    fn empty_stl_produces_empty_model() {
        let data = b"solid empty\nendsolid empty\n";
        let result = parse(data);
        assert!(
            matches!(result, Err(FileIOError::EmptyModel)),
            "expected EmptyModel for empty STL"
        );
    }

    #[test]
    fn single_triangle_ascii() {
        let data = b"solid single
  facet normal 0 0 1
    outer loop
      vertex 0 0 0
      vertex 1 0 0
      vertex 0 1 0
    endloop
  endfacet
endsolid single
";
        let mesh = parse(data).unwrap();
        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.vertex_count(), 3);
    }
}
