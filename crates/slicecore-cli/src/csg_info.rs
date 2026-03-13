//! Comprehensive mesh information display for the CSG `info` subcommand.

use std::path::Path;

use serde::Serialize;
use slicecore_fileio::load_mesh;
use slicecore_mesh::TriangleMesh;
use slicecore_mesh::stats::compute_stats;

/// Comprehensive information about a mesh file.
#[derive(Clone, Debug, Serialize)]
pub struct MeshInfo {
    // -- Geometry --
    /// Number of triangles in the mesh.
    pub triangle_count: usize,
    /// Number of vertices in the mesh.
    pub vertex_count: usize,
    /// Signed volume in mm^3 (positive when normals face outward).
    pub volume: f64,
    /// Total surface area in mm^2.
    pub surface_area: f64,
    /// Bounding box dimensions (width, height, depth) in mm.
    pub bounding_box: [f64; 3],

    // -- Quality --
    /// Whether every edge is shared by exactly two triangles.
    pub is_manifold: bool,
    /// Count of non-manifold edges.
    pub non_manifold_edges: usize,
    /// Number of degenerate (zero-area) triangles.
    pub degenerate_triangles: usize,

    // -- File info --
    /// File size in bytes.
    pub file_size_bytes: u64,

    // -- Components --
    /// Number of disconnected mesh regions (shells).
    pub shell_count: usize,

    // -- Repair suggestions --
    /// Suggested repair actions based on detected issues.
    pub repair_suggestions: Vec<String>,
}

/// Computes comprehensive mesh information from a loaded mesh.
pub fn compute_mesh_info(
    mesh: &TriangleMesh,
    file_size: u64,
) -> MeshInfo {
    let stats = compute_stats(mesh);

    let bbox_dims = [
        stats.aabb.max.x - stats.aabb.min.x,
        stats.aabb.max.y - stats.aabb.min.y,
        stats.aabb.max.z - stats.aabb.min.z,
    ];

    // Count non-manifold edges by building edge map.
    let non_manifold_edges = count_non_manifold_edges(mesh);

    // Count shells (connected components via BFS).
    let shell_count = count_shells(mesh);

    // Build repair suggestions.
    let mut suggestions = Vec::new();
    if !stats.is_manifold {
        suggestions.push(format!(
            "Mesh has {non_manifold_edges} non-manifold edges; consider mesh repair"
        ));
    }
    if stats.degenerate_count > 0 {
        suggestions.push(format!(
            "{} degenerate (zero-area) triangles should be removed",
            stats.degenerate_count
        ));
    }
    if stats.volume < 0.0 {
        suggestions.push("Negative volume indicates inverted normals; consider flipping winding".to_string());
    }
    if !stats.has_consistent_winding {
        suggestions.push("Inconsistent triangle winding detected; normals may need fixing".to_string());
    }

    MeshInfo {
        triangle_count: stats.triangle_count,
        vertex_count: stats.vertex_count,
        volume: stats.volume,
        surface_area: stats.surface_area,
        bounding_box: bbox_dims,
        is_manifold: stats.is_manifold,
        non_manifold_edges,
        degenerate_triangles: stats.degenerate_count,
        file_size_bytes: file_size,
        shell_count,
        repair_suggestions: suggestions,
    }
}

/// Displays mesh info as a human-readable table.
pub fn display_mesh_info(info: &MeshInfo) {
    println!("=== Mesh Information ===");
    println!();
    println!("  Geometry:");
    println!("    Triangle count:    {}", info.triangle_count);
    println!("    Vertex count:      {}", info.vertex_count);
    println!("    Volume:            {:.2} mm^3", info.volume);
    println!("    Surface area:      {:.2} mm^2", info.surface_area);
    println!(
        "    Bounding box:      {:.2} x {:.2} x {:.2} mm",
        info.bounding_box[0], info.bounding_box[1], info.bounding_box[2]
    );
    println!();
    println!("  Quality:");
    println!(
        "    Manifold:          {}",
        if info.is_manifold { "yes" } else { "no" }
    );
    if !info.is_manifold {
        println!("    Non-manifold edges: {}", info.non_manifold_edges);
    }
    println!("    Degenerate tris:   {}", info.degenerate_triangles);
    println!();
    println!("  File:");
    println!("    File size:         {} bytes", info.file_size_bytes);
    println!("    Shells:            {}", info.shell_count);

    if !info.repair_suggestions.is_empty() {
        println!();
        println!("  Repair suggestions:");
        for s in &info.repair_suggestions {
            println!("    - {s}");
        }
    }
}

/// Runs the info subcommand.
pub fn run_info(input: &Path, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let data = std::fs::read(input)
        .map_err(|e| format!("failed to read '{}': {e}", input.display()))?;
    let file_size = data.len() as u64;
    let mesh = load_mesh(&data)
        .map_err(|e| format!("failed to parse mesh from '{}': {e}", input.display()))?;

    let info = compute_mesh_info(&mesh, file_size);

    if json {
        let json_str = serde_json::to_string_pretty(&info)?;
        println!("{json_str}");
    } else {
        display_mesh_info(&info);
    }

    Ok(())
}

/// Counts the number of non-manifold edges in a mesh.
///
/// An edge is non-manifold if it is shared by other than exactly 2 triangles.
fn count_non_manifold_edges(mesh: &TriangleMesh) -> usize {
    use std::collections::HashMap;

    let indices = mesh.indices();
    let mut edge_counts: HashMap<(u32, u32), usize> = HashMap::new();

    for tri in indices {
        for i in 0..3 {
            let a = tri[i];
            let b = tri[(i + 1) % 3];
            let edge = if a < b { (a, b) } else { (b, a) };
            *edge_counts.entry(edge).or_insert(0) += 1;
        }
    }

    edge_counts.values().filter(|&&count| count != 2).count()
}

/// Counts the number of disconnected shells (connected components) via BFS.
fn count_shells(mesh: &TriangleMesh) -> usize {
    use std::collections::HashMap;

    let indices = mesh.indices();
    let tri_count = indices.len();
    if tri_count == 0 {
        return 0;
    }

    // Build adjacency: vertex -> list of triangle indices.
    let mut vert_to_tris: HashMap<u32, Vec<usize>> = HashMap::new();
    for (ti, tri) in indices.iter().enumerate() {
        for &vi in tri {
            vert_to_tris.entry(vi).or_default().push(ti);
        }
    }

    let mut visited = vec![false; tri_count];
    let mut shells = 0;

    for start in 0..tri_count {
        if visited[start] {
            continue;
        }
        shells += 1;
        let mut stack = vec![start];
        while let Some(ti) = stack.pop() {
            if visited[ti] {
                continue;
            }
            visited[ti] = true;
            // Find neighbors via shared vertices.
            for &vi in &indices[ti] {
                if let Some(neighbors) = vert_to_tris.get(&vi) {
                    for &ni in neighbors {
                        if !visited[ni] {
                            stack.push(ni);
                        }
                    }
                }
            }
        }
    }

    shells
}
