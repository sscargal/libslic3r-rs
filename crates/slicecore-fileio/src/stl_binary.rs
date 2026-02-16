//! Binary STL parser.
//!
//! Parses binary STL files into [`TriangleMesh`]. Binary STL is the most
//! common 3D model interchange format -- virtually all CAD tools can export it.
//!
//! The parser deduplicates vertices using a quantized integer key hash map,
//! since binary STL stores 3 independent vertices per triangle (no shared
//! vertex buffer).

use std::collections::HashMap;
use std::io::Cursor;

use byteorder::{LittleEndian, ReadBytesExt};
use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

use crate::error::FileIOError;

/// Quantization scale for vertex deduplication.
/// Coordinates are multiplied by this value and rounded to i64, so that
/// vertices within 1e-5 mm (10 nm) are considered identical.
const QUANTIZE_SCALE: f64 = 1e5;

/// Parse a binary STL file from raw bytes into a [`TriangleMesh`].
///
/// # Binary STL format
///
/// - 80-byte header (ignored)
/// - 4-byte `u32` triangle count (little-endian)
/// - For each triangle (50 bytes):
///   - 12 bytes: normal vector (3 x `f32`, ignored -- we recompute normals)
///   - 36 bytes: 3 vertices, each 3 x `f32` (little-endian)
///   - 2 bytes: attribute byte count (ignored)
///
/// # Vertex deduplication
///
/// Binary STL stores 3 independent vertices per triangle. Shared vertices
/// are deduplicated using a `HashMap<[i64; 3], u32>` with quantized keys:
/// each coordinate is mapped to `(coord * 1e5).round() as i64`.
///
/// # Errors
///
/// - [`FileIOError::FileTooSmall`] if data is shorter than 84 bytes.
/// - [`FileIOError::UnexpectedEof`] if data is truncated.
/// - [`FileIOError::EmptyModel`] if the triangle count is zero.
/// - [`FileIOError::MeshError`] if mesh construction fails.
pub fn parse(data: &[u8]) -> Result<TriangleMesh, FileIOError> {
    if data.len() < 84 {
        return Err(FileIOError::FileTooSmall(data.len()));
    }

    let num_triangles =
        u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;

    if num_triangles == 0 {
        return Err(FileIOError::EmptyModel);
    }

    let expected_size = 84 + num_triangles * 50;
    if data.len() < expected_size {
        return Err(FileIOError::UnexpectedEof(format!(
            "binary STL claims {} triangles ({} bytes needed) but file is only {} bytes",
            num_triangles, expected_size, data.len()
        )));
    }

    let mut vertices: Vec<Point3> = Vec::new();
    let mut indices: Vec<[u32; 3]> = Vec::with_capacity(num_triangles);
    let mut vertex_map: HashMap<[i64; 3], u32> = HashMap::new();

    let mut cursor = Cursor::new(&data[84..]);

    for _ in 0..num_triangles {
        // Skip normal (3 x f32 = 12 bytes).
        cursor.read_f32::<LittleEndian>()?;
        cursor.read_f32::<LittleEndian>()?;
        cursor.read_f32::<LittleEndian>()?;

        let mut tri_indices = [0u32; 3];

        for idx in &mut tri_indices {
            let x = cursor.read_f32::<LittleEndian>()? as f64;
            let y = cursor.read_f32::<LittleEndian>()? as f64;
            let z = cursor.read_f32::<LittleEndian>()? as f64;

            let key = quantize_vertex(x, y, z);

            let vertex_idx = match vertex_map.get(&key) {
                Some(&existing) => existing,
                None => {
                    let new_idx = vertices.len() as u32;
                    vertices.push(Point3::new(x, y, z));
                    vertex_map.insert(key, new_idx);
                    new_idx
                }
            };

            *idx = vertex_idx;
        }

        // Skip attribute byte count (2 bytes).
        cursor.read_u16::<LittleEndian>()?;

        indices.push(tri_indices);
    }

    let mesh = TriangleMesh::new(vertices, indices)?;
    Ok(mesh)
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

    /// Build a binary STL byte buffer from raw triangle data.
    /// Each triangle is (normal, v0, v1, v2) with all values as f32.
    fn build_binary_stl(triangles: &[([f32; 3], [f32; 3], [f32; 3], [f32; 3])]) -> Vec<u8> {
        let mut data = Vec::new();

        // 80-byte header
        let mut header = b"binary STL test file".to_vec();
        header.resize(80, 0u8);
        data.extend_from_slice(&header);

        // Triangle count
        data.extend_from_slice(&(triangles.len() as u32).to_le_bytes());

        for (normal, v0, v1, v2) in triangles {
            // Normal
            for c in normal {
                data.extend_from_slice(&c.to_le_bytes());
            }
            // Vertex 0
            for c in v0 {
                data.extend_from_slice(&c.to_le_bytes());
            }
            // Vertex 1
            for c in v1 {
                data.extend_from_slice(&c.to_le_bytes());
            }
            // Vertex 2
            for c in v2 {
                data.extend_from_slice(&c.to_le_bytes());
            }
            // Attribute byte count
            data.extend_from_slice(&0u16.to_le_bytes());
        }

        data
    }

    /// Build a unit cube binary STL (12 triangles, 8 unique vertices).
    fn unit_cube_binary_stl() -> Vec<u8> {
        // Cube vertices: (0,0,0) to (1,1,1)
        let v = [
            [0.0f32, 0.0, 0.0], // 0: left-bottom-back
            [1.0, 0.0, 0.0],    // 1: right-bottom-back
            [1.0, 1.0, 0.0],    // 2: right-top-back
            [0.0, 1.0, 0.0],    // 3: left-top-back
            [0.0, 0.0, 1.0],    // 4: left-bottom-front
            [1.0, 0.0, 1.0],    // 5: right-bottom-front
            [1.0, 1.0, 1.0],    // 6: right-top-front
            [0.0, 1.0, 1.0],    // 7: left-top-front
        ];
        let n0 = [0.0f32, 0.0, 0.0]; // normals ignored by parser

        let triangles = vec![
            // Front face (z=1)
            (n0, v[4], v[5], v[6]),
            (n0, v[4], v[6], v[7]),
            // Back face (z=0)
            (n0, v[1], v[0], v[3]),
            (n0, v[1], v[3], v[2]),
            // Right face (x=1)
            (n0, v[1], v[2], v[6]),
            (n0, v[1], v[6], v[5]),
            // Left face (x=0)
            (n0, v[0], v[4], v[7]),
            (n0, v[0], v[7], v[3]),
            // Top face (y=1)
            (n0, v[3], v[7], v[6]),
            (n0, v[3], v[6], v[2]),
            // Bottom face (y=0)
            (n0, v[0], v[1], v[5]),
            (n0, v[0], v[5], v[4]),
        ];

        build_binary_stl(&triangles)
    }

    #[test]
    fn parse_unit_cube_binary_stl() {
        let data = unit_cube_binary_stl();
        let mesh = parse(&data).unwrap();

        // A cube has 12 triangles and 8 unique vertices.
        assert_eq!(mesh.triangle_count(), 12);
        assert_eq!(mesh.vertex_count(), 8);
    }

    #[test]
    fn vertex_deduplication_reduces_count() {
        // A single triangle has 3 unique vertices. With no dedup, 12 triangles
        // would have 36 vertices. With dedup, a cube has only 8.
        let data = unit_cube_binary_stl();
        let mesh = parse(&data).unwrap();

        // Without dedup: 12 * 3 = 36 vertices
        // With dedup: 8 vertices
        assert!(
            mesh.vertex_count() < 12 * 3,
            "vertex count {} should be less than {} (no dedup)",
            mesh.vertex_count(),
            12 * 3
        );
        assert_eq!(mesh.vertex_count(), 8);
    }

    #[test]
    fn truncated_file_produces_unexpected_eof() {
        let data = unit_cube_binary_stl();
        // Truncate to 200 bytes (should need 84 + 12*50 = 684 bytes).
        let truncated = &data[..200];
        let result = parse(truncated);
        assert!(
            matches!(result, Err(FileIOError::UnexpectedEof(_))),
            "expected UnexpectedEof, got Err or Ok variant"
        );
    }

    #[test]
    fn empty_file_produces_file_too_small() {
        let result = parse(&[0u8; 10]);
        assert!(
            matches!(result, Err(FileIOError::FileTooSmall(10))),
            "expected FileTooSmall(10)"
        );
    }

    #[test]
    fn zero_triangles_produces_empty_model() {
        let mut data = vec![0u8; 80]; // header
        data.extend_from_slice(&0u32.to_le_bytes()); // 0 triangles
        let result = parse(&data);
        assert!(
            matches!(result, Err(FileIOError::EmptyModel)),
            "expected EmptyModel"
        );
    }

    #[test]
    fn single_triangle_parses_correctly() {
        let triangles = vec![(
            [0.0f32, 0.0, 1.0],  // normal
            [0.0, 0.0, 0.0],     // v0
            [1.0, 0.0, 0.0],     // v1
            [0.0, 1.0, 0.0],     // v2
        )];
        let data = build_binary_stl(&triangles);
        let mesh = parse(&data).unwrap();

        assert_eq!(mesh.triangle_count(), 1);
        assert_eq!(mesh.vertex_count(), 3);
    }
}
