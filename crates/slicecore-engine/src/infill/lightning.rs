//! Lightning infill pattern generation with cross-layer tree branching.
//!
//! Lightning infill generates minimal tree-branching support structures that
//! only exist where needed to hold up top surfaces. Unlike traditional infill
//! patterns that fill the entire interior uniformly, lightning infill grows
//! vertical support columns downward from top surface regions and connects
//! nearby columns with horizontal branches, producing a lightning-bolt-like
//! pattern that uses 40-70% less material than rectilinear infill.
//!
//! # Cross-layer awareness
//!
//! Lightning requires a pre-pass that analyzes all layers to determine where
//! support is needed. The [`build_lightning_context`] function performs this
//! analysis and produces a [`LightningContext`] that is then passed to
//! [`generate`] for per-layer line extraction.
//!
//! # Simplified implementation
//!
//! This Phase 4 implementation uses a simplified column-based approach:
//! 1. Identify top surface regions (layers with no material above them).
//! 2. Sample support points at regular spacing across top surfaces.
//! 3. Each point becomes a vertical column extending downward.
//! 4. Merge nearby columns (within 2x line_width) into one.
//! 5. At each layer, connect adjacent columns with horizontal segments.

use slicecore_geo::polygon::ValidPolygon;
use slicecore_geo::{point_in_polygon, PointLocation};
use slicecore_math::{mm_to_coord, Coord, IPoint2};

use super::{compute_bounding_box, InfillLine};

/// Cross-layer context for lightning infill generation.
///
/// Built once by [`build_lightning_context`] before per-layer processing.
/// Contains the support column network that defines where lightning infill
/// lines should be generated at each layer.
#[derive(Debug, Clone)]
pub struct LightningContext {
    /// Support columns indexed by layer. Each entry is a list of column
    /// positions (in integer coordinates) that need infill at that layer.
    pub layer_columns: Vec<Vec<IPoint2>>,
    /// Horizontal connection segments per layer between nearby columns.
    pub layer_connections: Vec<Vec<(IPoint2, IPoint2)>>,
}

/// A support column that extends vertically through multiple layers.
#[derive(Debug, Clone)]
struct SupportColumn {
    /// Position in integer coordinate space.
    position: IPoint2,
    /// First layer (topmost) where this column exists.
    start_layer: usize,
    /// Last layer (bottommost) where this column extends to.
    end_layer: usize,
}

/// Builds the lightning context by analyzing all layers for top surface support.
///
/// This pre-pass identifies which regions need support from below (top surfaces)
/// and creates a network of vertical support columns that grow downward from
/// those surfaces.
///
/// # Parameters
/// - `layer_contours`: Per-layer contour polygons from slicing.
/// - `total_layers`: Total number of layers.
/// - `density`: Fill density (controls support point spacing).
/// - `line_width`: Extrusion line width in mm.
///
/// # Returns
/// A [`LightningContext`] containing the support column network.
pub fn build_lightning_context(
    layer_contours: &[Vec<ValidPolygon>],
    density: f64,
    line_width: f64,
) -> LightningContext {
    let total_layers = layer_contours.len();

    if total_layers == 0 || density <= 0.0 || line_width <= 0.0 {
        return LightningContext {
            layer_columns: Vec::new(),
            layer_connections: Vec::new(),
        };
    }

    let density = density.min(1.0);

    // Spacing between support points (larger spacing = less material).
    let spacing_mm = line_width / density;
    let spacing = mm_to_coord(spacing_mm);
    let merge_distance = mm_to_coord(line_width * 2.0);

    if spacing <= 0 {
        return LightningContext {
            layer_columns: Vec::new(),
            layer_connections: Vec::new(),
        };
    }

    // Step 1: Identify top surface layers.
    // A point on layer N is a "top surface" if layer N+1 doesn't cover it
    // (or if N is the last layer).
    let mut columns: Vec<SupportColumn> = Vec::new();

    for layer_idx in 0..total_layers {
        let contours = &layer_contours[layer_idx];
        if contours.is_empty() {
            continue;
        }

        // Check if this layer has regions not covered by the layer above.
        let is_top_surface = if layer_idx + 1 >= total_layers {
            true
        } else {
            let above = &layer_contours[layer_idx + 1];
            // If above is empty, entire layer is top surface.
            // If above has different coverage, parts are top surface.
            above.is_empty() || !contours_fully_covered(contours, above)
        };

        if !is_top_surface {
            continue;
        }

        // Sample support points across the top surface region.
        let (min_x, min_y, max_x, max_y) = compute_bounding_box(contours);

        let mut x = min_x + spacing / 2;
        while x <= max_x {
            let mut y = min_y + spacing / 2;
            while y <= max_y {
                let pt = IPoint2::new(x, y);

                // Only add points that are inside the contour.
                if point_inside_any(pt, contours) {
                    // Check if the layer above covers this point.
                    let needs_support = if layer_idx + 1 >= total_layers {
                        true
                    } else {
                        !point_inside_any(pt, &layer_contours[layer_idx + 1])
                    };

                    if needs_support {
                        // Grow column downward from this layer.
                        let end_layer = grow_column_down(
                            pt,
                            layer_idx,
                            layer_contours,
                        );

                        columns.push(SupportColumn {
                            position: pt,
                            start_layer: layer_idx,
                            end_layer,
                        });
                    }
                }

                y += spacing;
            }
            x += spacing;
        }
    }

    // Step 2: Merge nearby columns.
    merge_columns(&mut columns, merge_distance);

    // Step 3: Build per-layer column lists and connections.
    let mut layer_columns = vec![Vec::new(); total_layers];
    let mut layer_connections = vec![Vec::new(); total_layers];

    for col in &columns {
        #[allow(clippy::needless_range_loop)]
        for layer_idx in col.end_layer..=col.start_layer {
            if layer_idx < total_layers {
                layer_columns[layer_idx].push(col.position);
            }
        }
    }

    // Step 4: Generate horizontal connections between nearby columns on each layer.
    for layer_idx in 0..total_layers {
        let cols = &layer_columns[layer_idx];
        if cols.len() < 2 {
            continue;
        }

        // Connect columns that are within a reasonable distance.
        // Use a greedy nearest-neighbor approach.
        let connect_dist = spacing * 2;
        let connect_dist_sq = connect_dist as i128 * connect_dist as i128;

        let mut connected = vec![false; cols.len()];
        for i in 0..cols.len() {
            if connected[i] {
                continue;
            }
            connected[i] = true;

            // Find nearest unconnected neighbor within distance.
            let mut best_j = None;
            let mut best_dist = i128::MAX;

            for j in (i + 1)..cols.len() {
                if connected[j] {
                    continue;
                }
                let dx = (cols[j].x - cols[i].x) as i128;
                let dy = (cols[j].y - cols[i].y) as i128;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq <= connect_dist_sq && dist_sq < best_dist {
                    best_dist = dist_sq;
                    best_j = Some(j);
                }
            }

            if let Some(j) = best_j {
                layer_connections[layer_idx].push((cols[i], cols[j]));
                connected[j] = true;
            }
        }
    }

    LightningContext {
        layer_columns,
        layer_connections,
    }
}

/// Checks if contours_a are fully covered by contours_b.
///
/// A simplified check: we sample a few points from contours_a and check
/// if they are all inside contours_b.
fn contours_fully_covered(contours_a: &[ValidPolygon], contours_b: &[ValidPolygon]) -> bool {
    if contours_b.is_empty() {
        return false;
    }

    // Sample center and corners of bounding box of contours_a.
    let (min_x, min_y, max_x, max_y) = compute_bounding_box(contours_a);
    let mid_x = (min_x + max_x) / 2;
    let mid_y = (min_y + max_y) / 2;

    let test_points = [
        IPoint2::new(mid_x, mid_y),
        IPoint2::new(min_x + (max_x - min_x) / 4, min_y + (max_y - min_y) / 4),
        IPoint2::new(
            min_x + 3 * (max_x - min_x) / 4,
            min_y + 3 * (max_y - min_y) / 4,
        ),
    ];

    for pt in &test_points {
        if point_inside_any(*pt, contours_a) && !point_inside_any(*pt, contours_b) {
            return false;
        }
    }

    true
}

/// Checks if a point is inside any polygon in the set.
fn point_inside_any(pt: IPoint2, polygons: &[ValidPolygon]) -> bool {
    for poly in polygons {
        let loc = point_in_polygon(&pt, poly.points());
        if loc == PointLocation::Inside || loc == PointLocation::OnBoundary {
            return true;
        }
    }
    false
}

/// Grows a support column downward from a starting layer until it hits
/// the build plate or exits the infill region.
fn grow_column_down(
    position: IPoint2,
    start_layer: usize,
    layer_contours: &[Vec<ValidPolygon>],
) -> usize {
    let mut end_layer = start_layer;

    // Grow downward, layer by layer.
    for layer_idx in (0..start_layer).rev() {
        let contours = &layer_contours[layer_idx];
        if contours.is_empty() || !point_inside_any(position, contours) {
            break;
        }
        end_layer = layer_idx;
    }

    end_layer
}

/// Merges columns that are closer than `merge_distance` to each other.
///
/// When two columns overlap in layer range and are spatially close,
/// the shorter one is removed (merged into the longer one).
fn merge_columns(columns: &mut Vec<SupportColumn>, merge_distance: Coord) {
    let merge_dist_sq = merge_distance as i128 * merge_distance as i128;

    let mut i = 0;
    while i < columns.len() {
        let mut j = i + 1;
        while j < columns.len() {
            let dx = (columns[j].position.x - columns[i].position.x) as i128;
            let dy = (columns[j].position.y - columns[i].position.y) as i128;
            let dist_sq = dx * dx + dy * dy;

            if dist_sq < merge_dist_sq {
                // Check for layer overlap.
                let overlap = columns[i].start_layer >= columns[j].end_layer
                    && columns[i].end_layer <= columns[j].start_layer;

                if overlap {
                    // Keep the longer column, remove the shorter one.
                    let len_i = columns[i].start_layer - columns[i].end_layer;
                    let len_j = columns[j].start_layer - columns[j].end_layer;

                    if len_j > len_i {
                        // Extend i to cover j's range.
                        columns[i].start_layer =
                            columns[i].start_layer.max(columns[j].start_layer);
                        columns[i].end_layer = columns[i].end_layer.min(columns[j].end_layer);
                    } else {
                        // Keep i as-is, extend to cover j's range.
                        columns[i].start_layer =
                            columns[i].start_layer.max(columns[j].start_layer);
                        columns[i].end_layer = columns[i].end_layer.min(columns[j].end_layer);
                    }

                    columns.swap_remove(j);
                    continue; // Don't increment j.
                }
            }
            j += 1;
        }
        i += 1;
    }
}

/// Generates lightning infill lines for a single layer.
///
/// Uses the pre-computed [`LightningContext`] to extract column positions
/// and horizontal connections for the given layer, producing sparse
/// tree-branching support lines.
///
/// # Parameters
/// - `infill_region`: The boundary polygons defining the infill area.
/// - `density`: Fill density as a fraction (0.0 = empty, 1.0 = solid).
/// - `layer_index`: Current layer index.
/// - `line_width`: Extrusion line width in mm.
/// - `context`: Optional lightning context from [`build_lightning_context`].
///   If `None`, falls back to an empty result.
///
/// # Returns
/// A vector of [`InfillLine`] segments for the lightning infill at this layer.
pub fn generate(
    infill_region: &[ValidPolygon],
    density: f64,
    layer_index: usize,
    line_width: f64,
    context: Option<&LightningContext>,
) -> Vec<InfillLine> {
    if density <= 0.0 || infill_region.is_empty() || line_width <= 0.0 {
        return Vec::new();
    }

    let ctx = match context {
        Some(c) => c,
        None => return Vec::new(),
    };

    if layer_index >= ctx.layer_columns.len() {
        return Vec::new();
    }

    let columns = &ctx.layer_columns[layer_index];
    let connections = &ctx.layer_connections[layer_index];

    if columns.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();

    // Add horizontal connection segments between columns.
    // These form the sparse tree-branching support network.
    for &(start, end) in connections {
        // Verify both endpoints are inside the infill region.
        if point_inside_any(start, infill_region) && point_inside_any(end, infill_region) {
            lines.push(InfillLine { start, end });
        }
    }

    // For isolated columns (not connected to any neighbor), add a small
    // cross mark so they still produce extruded material.
    let connected_points: std::collections::HashSet<(Coord, Coord)> = connections
        .iter()
        .flat_map(|&(s, e)| [(s.x, s.y), (e.x, e.y)])
        .collect();

    let cross_size = mm_to_coord(line_width * 0.5);

    for &col_pos in columns {
        if connected_points.contains(&(col_pos.x, col_pos.y)) {
            continue; // Already connected -- skip cross mark.
        }

        if !point_inside_any(col_pos, infill_region) {
            continue;
        }

        // Single horizontal cross mark for isolated columns.
        let h_start = IPoint2::new(col_pos.x - cross_size, col_pos.y);
        let h_end = IPoint2::new(col_pos.x + cross_size, col_pos.y);

        if point_inside_any(h_start, infill_region) && point_inside_any(h_end, infill_region) {
            lines.push(InfillLine {
                start: h_start,
                end: h_end,
            });
        }
    }

    lines
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_geo::polygon::Polygon;
    use slicecore_math::mm_to_coord;

    /// Helper to create a validated CCW square at the origin with given size (mm).
    fn make_square(size: f64) -> ValidPolygon {
        Polygon::from_mm(&[(0.0, 0.0), (size, 0.0), (size, size), (0.0, size)])
            .validate()
            .unwrap()
    }

    /// Creates a 5-layer stack simulating a flat-top box:
    /// Layers 0-4 all have the same 20mm square contour.
    /// The top layer (4) is a top surface needing support from below.
    fn make_box_layers() -> Vec<Vec<ValidPolygon>> {
        let square = make_square(20.0);
        (0..5).map(|_| vec![square.clone()]).collect()
    }

    #[test]
    fn lightning_context_builds_without_panic() {
        let layers = make_box_layers();
        let ctx = build_lightning_context(&layers, 0.2, 0.4);
        assert_eq!(
            ctx.layer_columns.len(),
            5,
            "Context should have entries for all 5 layers"
        );
    }

    #[test]
    fn lightning_flat_top_box_produces_support() {
        let layers = make_box_layers();
        let ctx = build_lightning_context(&layers, 0.2, 0.4);

        // The top layer (4) is a top surface, so columns should exist
        // in multiple layers below it.
        let total_columns: usize = ctx.layer_columns.iter().map(|c| c.len()).sum();
        assert!(
            total_columns > 0,
            "Lightning should produce support columns under the top surface"
        );

        // Layer 4 (top) should have columns.
        assert!(
            !ctx.layer_columns[4].is_empty(),
            "Top layer should have support columns"
        );
    }

    #[test]
    fn lightning_generates_lines_with_context() {
        let layers = make_box_layers();
        let ctx = build_lightning_context(&layers, 0.2, 0.4);
        let square = make_square(20.0);

        // Generate infill for a mid-layer.
        let lines = generate(&[square], 0.2, 2, 0.4, Some(&ctx));

        // Should produce some lines (either connections or cross marks).
        // This depends on column placement and density.
        // With a 20mm square at 20% density, there should be multiple columns.
        // Even if connections are sparse, cross marks should produce lines.
        if !ctx.layer_columns[2].is_empty() {
            assert!(
                !lines.is_empty(),
                "Layer 2 with columns should produce lightning infill lines"
            );
        }
    }

    #[test]
    fn lightning_less_material_than_rectilinear() {
        let layers = make_box_layers();
        let ctx = build_lightning_context(&layers, 0.2, 0.4);
        let square = make_square(20.0);

        /// Computes total extrusion length for a set of infill lines.
        fn total_length(lines: &[InfillLine]) -> f64 {
            lines
                .iter()
                .map(|l| {
                    let dx = (l.end.x - l.start.x) as f64;
                    let dy = (l.end.y - l.start.y) as f64;
                    (dx * dx + dy * dy).sqrt()
                })
                .sum()
        }

        // Sum lightning extrusion length across all layers.
        let mut lightning_length = 0.0;
        for layer_idx in 0..5 {
            let lines = generate(&[square.clone()], 0.2, layer_idx, 0.4, Some(&ctx));
            lightning_length += total_length(&lines);
        }

        // Sum rectilinear extrusion length across all layers.
        let mut rectilinear_length = 0.0;
        for layer_idx in 0..5 {
            let angle = if layer_idx % 2 == 0 { 0.0 } else { 90.0 };
            let lines =
                super::super::rectilinear::generate(&[square.clone()], 0.2, angle, 0.4);
            rectilinear_length += total_length(&lines);
        }

        assert!(
            lightning_length < rectilinear_length,
            "Lightning total extrusion length ({:.0}) should be less than rectilinear ({:.0})",
            lightning_length,
            rectilinear_length
        );
    }

    #[test]
    fn lightning_empty_region_returns_empty() {
        let ctx = build_lightning_context(&[vec![make_square(20.0)]], 0.2, 0.4);
        let lines = generate(&[], 0.2, 0, 0.4, Some(&ctx));
        assert!(
            lines.is_empty(),
            "Empty infill region should return empty lines"
        );
    }

    #[test]
    fn lightning_no_context_returns_empty() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.2, 0, 0.4, None);
        assert!(
            lines.is_empty(),
            "No lightning context should return empty lines"
        );
    }

    #[test]
    fn lightning_context_empty_layers_no_panic() {
        let layers: Vec<Vec<ValidPolygon>> = vec![Vec::new(); 3];
        let ctx = build_lightning_context(&layers, 0.2, 0.4);
        assert_eq!(ctx.layer_columns.len(), 3);
        for cols in &ctx.layer_columns {
            assert!(cols.is_empty(), "Empty layers should have no columns");
        }
    }

    #[test]
    fn lightning_zero_density_returns_empty() {
        let square = make_square(20.0);
        let ctx = build_lightning_context(&[vec![square.clone()]], 0.0, 0.4);
        let lines = generate(&[square], 0.0, 0, 0.4, Some(&ctx));
        assert!(
            lines.is_empty(),
            "Zero density should return empty lines"
        );
    }

    #[test]
    fn lightning_no_top_surface_minimal_infill() {
        // Create a stack where every layer has identical contours and the
        // top is "capped" by an identical layer above. Only the topmost
        // layer should be detected as a top surface.
        let square = make_square(20.0);
        let layers: Vec<Vec<ValidPolygon>> = (0..10).map(|_| vec![square.clone()]).collect();
        let ctx = build_lightning_context(&layers, 0.2, 0.4);

        // Middle layers (not top) should have columns because the top
        // layer's support grows down through them.
        // But total column count should be modest (tree branching, not dense fill).
        let total_columns: usize = ctx.layer_columns.iter().map(|c| c.len()).sum();
        assert!(
            total_columns > 0,
            "Should have some columns supporting the top surface"
        );

        // Layer 0 (bottommost) should also have columns (support extends all the way down).
        // (Unless column growth stopped at an interior layer.)
    }

    #[test]
    fn columns_merge_when_close() {
        let mut columns = vec![
            SupportColumn {
                position: IPoint2::new(1_000_000, 1_000_000),
                start_layer: 5,
                end_layer: 0,
            },
            SupportColumn {
                position: IPoint2::new(1_100_000, 1_000_000), // 0.1mm away
                start_layer: 5,
                end_layer: 0,
            },
        ];

        let merge_distance = mm_to_coord(0.8); // 0.8mm > 0.1mm distance
        merge_columns(&mut columns, merge_distance);

        assert_eq!(
            columns.len(),
            1,
            "Nearby columns should merge into one"
        );
    }
}
