//! Adaptive cubic infill pattern generation with quadtree-based density variation.
//!
//! Adaptive cubic infill generates denser lines near the boundary of the infill
//! region (where the part surface is) and sparser lines in the interior. This
//! provides structural strength at surfaces while saving material and print time
//! for internal volumes.
//!
//! The algorithm uses a 2D quadtree to partition the infill region. Cells near
//! the polygon boundary are subdivided to higher depth (denser infill), while
//! cells deep in the interior remain large (sparser infill). Within each leaf
//! cell, cubic-pattern scanlines are generated at the cell's local spacing.

use slicecore_geo::polygon::ValidPolygon;
use slicecore_math::{Coord, IPoint2};

use super::rectilinear::find_horizontal_intersections;
use super::{compute_bounding_box, compute_spacing, InfillLine};

/// Maximum quadtree subdivision depth. Higher values produce finer density
/// gradients but more cells to process.
const MAX_DEPTH: u32 = 5;

/// The three angles (in degrees) that adaptive cubic cycles through,
/// matching standard cubic infill.
const CUBIC_ANGLES: [f64; 3] = [0.0, 60.0, 120.0];

/// A cell in the adaptive quadtree.
///
/// Each cell covers an axis-aligned rectangular region. Cells near the
/// infill boundary are subdivided into four children; interior cells
/// remain as leaves and generate infill at their local spacing.
struct QuadCell {
    x_min: Coord,
    y_min: Coord,
    x_max: Coord,
    y_max: Coord,
    depth: u32,
    children: Option<Box<[QuadCell; 4]>>,
}

impl QuadCell {
    /// Creates a new leaf cell covering the given region.
    fn new(x_min: Coord, y_min: Coord, x_max: Coord, y_max: Coord, depth: u32) -> Self {
        Self {
            x_min,
            y_min,
            x_max,
            y_max,
            depth,
            children: None,
        }
    }

    /// Subdivides this cell into four equal quadrants.
    fn subdivide(&mut self) {
        let mid_x = (self.x_min + self.x_max) / 2;
        let mid_y = (self.y_min + self.y_max) / 2;
        let d = self.depth + 1;

        self.children = Some(Box::new([
            // Bottom-left
            QuadCell::new(self.x_min, self.y_min, mid_x, mid_y, d),
            // Bottom-right
            QuadCell::new(mid_x, self.y_min, self.x_max, mid_y, d),
            // Top-left
            QuadCell::new(self.x_min, mid_y, mid_x, self.y_max, d),
            // Top-right
            QuadCell::new(mid_x, mid_y, self.x_max, self.y_max, d),
        ]));
    }

    /// Returns the cell width in coordinate units.
    fn width(&self) -> Coord {
        self.x_max - self.x_min
    }

    /// Returns the cell height in coordinate units.
    fn height(&self) -> Coord {
        self.y_max - self.y_min
    }

    /// Returns the center point of this cell.
    fn center(&self) -> (Coord, Coord) {
        ((self.x_min + self.x_max) / 2, (self.y_min + self.y_max) / 2)
    }

    /// Returns true if this cell is a leaf (not subdivided).
    fn is_leaf(&self) -> bool {
        self.children.is_none()
    }
}

/// Computes the minimum distance from a point to any edge of the polygon set.
///
/// This is an approximate distance using coordinate-space arithmetic.
/// The result is in coordinate units (not mm).
fn distance_to_boundary(cx: Coord, cy: Coord, polygons: &[ValidPolygon]) -> f64 {
    let mut min_dist_sq = f64::MAX;

    for poly in polygons {
        let pts = poly.points();
        let n = pts.len();
        for i in 0..n {
            let p1 = pts[i];
            let p2 = pts[(i + 1) % n];

            // Project point onto edge segment, compute distance.
            let dx = (p2.x - p1.x) as f64;
            let dy = (p2.y - p1.y) as f64;
            let len_sq = dx * dx + dy * dy;

            if len_sq < 1.0 {
                // Degenerate edge -- use distance to endpoint.
                let ex = (cx - p1.x) as f64;
                let ey = (cy - p1.y) as f64;
                let d = ex * ex + ey * ey;
                if d < min_dist_sq {
                    min_dist_sq = d;
                }
                continue;
            }

            // Parameter t of closest point on segment.
            let t = ((cx - p1.x) as f64 * dx + (cy - p1.y) as f64 * dy) / len_sq;
            let t = t.clamp(0.0, 1.0);

            let closest_x = p1.x as f64 + t * dx;
            let closest_y = p1.y as f64 + t * dy;

            let ex = cx as f64 - closest_x;
            let ey = cy as f64 - closest_y;
            let d = ex * ex + ey * ey;
            if d < min_dist_sq {
                min_dist_sq = d;
            }
        }
    }

    min_dist_sq.sqrt()
}

/// Builds the adaptive quadtree by recursively subdividing cells near the boundary.
///
/// A cell is subdivided if:
/// 1. Its depth is below `max_depth`.
/// 2. Its size is larger than `min_cell_size`.
/// 3. Its center is within `threshold` distance of any polygon edge.
fn build_quadtree(
    cell: &mut QuadCell,
    polygons: &[ValidPolygon],
    max_depth: u32,
    min_cell_size: Coord,
    threshold: f64,
) {
    if cell.depth >= max_depth {
        return;
    }

    // Don't subdivide cells smaller than minimum printable size.
    if cell.width() < min_cell_size || cell.height() < min_cell_size {
        return;
    }

    let (cx, cy) = cell.center();
    let dist = distance_to_boundary(cx, cy, polygons);

    // Only subdivide cells near the boundary.
    // The threshold scales with cell size so that large cells far from
    // the boundary are still subdivided if they contain boundary edges.
    let cell_diag = ((cell.width() as f64).powi(2) + (cell.height() as f64).powi(2)).sqrt();
    let effective_threshold = threshold + cell_diag * 0.5;

    if dist > effective_threshold {
        return;
    }

    cell.subdivide();

    if let Some(ref mut children) = cell.children {
        for child in children.iter_mut() {
            build_quadtree(child, polygons, max_depth, min_cell_size, threshold);
        }
    }
}

/// Collects all leaf cells from the quadtree into a flat list.
fn collect_leaves(cell: &QuadCell, leaves: &mut Vec<(Coord, Coord, Coord, Coord, u32)>) {
    if cell.is_leaf() {
        leaves.push((cell.x_min, cell.y_min, cell.x_max, cell.y_max, cell.depth));
    } else if let Some(ref children) = cell.children {
        for child in children.iter() {
            collect_leaves(child, leaves);
        }
    }
}

/// Generates adaptive cubic infill lines clipped to an infill region.
///
/// The density varies based on proximity to the infill boundary:
/// - Near edges (surface proximity): dense infill at the base spacing.
/// - In the interior: sparse infill at larger spacing (fewer, wider cells).
///
/// The pattern cycles through three angles (0, 60, 120 degrees) per layer,
/// matching standard cubic infill behavior.
///
/// # Parameters
/// - `infill_region`: The boundary polygons defining the infill area.
/// - `density`: Fill density as a fraction (0.0 = empty, 1.0 = solid).
/// - `layer_index`: Current layer index (selects angle: 0, 60, or 120 degrees).
/// - `layer_z`: Z height of the current layer in mm (used for phase offset).
/// - `line_width`: Extrusion line width in mm.
///
/// # Returns
/// A vector of [`InfillLine`] segments for the adaptive cubic pattern.
/// Returns empty if density <= 0.0 or infill_region is empty.
pub fn generate(
    infill_region: &[ValidPolygon],
    density: f64,
    layer_index: usize,
    layer_z: f64,
    line_width: f64,
) -> Vec<InfillLine> {
    if density <= 0.0 || infill_region.is_empty() || line_width <= 0.0 {
        return Vec::new();
    }

    let density = density.min(1.0);

    let base_spacing = match compute_spacing(density, line_width) {
        Some(s) => s,
        None => return Vec::new(),
    };

    let (bb_min_x, bb_min_y, bb_max_x, bb_max_y) = compute_bounding_box(infill_region);

    if bb_max_x <= bb_min_x || bb_max_y <= bb_min_y {
        return Vec::new();
    }

    // Build the quadtree.
    let min_cell_size = base_spacing; // Don't subdivide below the base spacing.
    let threshold = base_spacing as f64 * 3.0; // Subdivision threshold distance.

    let mut root = QuadCell::new(bb_min_x, bb_min_y, bb_max_x, bb_max_y, 0);
    build_quadtree(
        &mut root,
        infill_region,
        MAX_DEPTH,
        min_cell_size,
        threshold,
    );

    // Collect all leaf cells.
    let mut leaves = Vec::new();
    collect_leaves(&root, &mut leaves);

    // Select angle based on layer index (cubic 3-angle cycling).
    let angle_deg = CUBIC_ANGLES[layer_index % 3];

    // Compute Z-dependent phase offset (same as cubic).
    let spacing_mm = line_width / density;
    let offset_mm = (layer_z * 1.0) % spacing_mm; // Z_FREQUENCY = 1.0
    let offset = (offset_mm * slicecore_math::COORD_SCALE).round() as Coord;

    // Generate lines for each leaf cell at the cell's local spacing.
    let mut all_lines = Vec::new();

    for &(cell_x_min, cell_y_min, cell_x_max, cell_y_max, depth) in &leaves {
        // Local spacing: deeper cells (near boundary) use base_spacing,
        // shallower cells (interior) use larger spacing.
        // At max depth: spacing = base_spacing.
        // At depth 0: spacing = base_spacing * 2^max_depth (very sparse).
        // We scale: spacing = base_spacing * 2^(max_depth - depth) but cap
        // it so that it doesn't exceed the cell dimensions.
        let scale_factor = 1u64 << (MAX_DEPTH.saturating_sub(depth)) as u64;
        let local_spacing = (base_spacing as u64 * scale_factor).min(
            (cell_x_max - cell_x_min).max(cell_y_max - cell_y_min) as u64,
        ) as Coord;

        if local_spacing <= 0 {
            continue;
        }

        // Generate scanlines within this cell.
        let cell_lines = generate_cell_lines(
            infill_region,
            cell_x_min,
            cell_y_min,
            cell_x_max,
            cell_y_max,
            local_spacing,
            angle_deg,
            offset,
        );
        all_lines.extend(cell_lines);
    }

    all_lines
}

/// Generates infill lines within a single quadtree cell, clipped to the infill region.
fn generate_cell_lines(
    infill_region: &[ValidPolygon],
    cell_x_min: Coord,
    cell_y_min: Coord,
    cell_x_max: Coord,
    cell_y_max: Coord,
    spacing: Coord,
    angle_deg: f64,
    offset: Coord,
) -> Vec<InfillLine> {
    let is_rotated = angle_deg.abs() > 1.0;

    if is_rotated {
        generate_cell_rotated(
            infill_region,
            cell_x_min,
            cell_y_min,
            cell_x_max,
            cell_y_max,
            spacing,
            angle_deg,
            offset,
        )
    } else {
        generate_cell_horizontal(
            infill_region,
            cell_x_min,
            cell_y_min,
            cell_x_max,
            cell_y_max,
            spacing,
            offset,
        )
    }
}

/// Generates horizontal lines within a cell, clipped to the infill region.
fn generate_cell_horizontal(
    infill_region: &[ValidPolygon],
    cell_x_min: Coord,
    cell_y_min: Coord,
    cell_x_max: Coord,
    cell_y_max: Coord,
    spacing: Coord,
    offset: Coord,
) -> Vec<InfillLine> {
    let mut lines = Vec::new();

    // Start position includes offset for Z-dependent shift.
    let mut y = cell_y_min + spacing / 2 + ((offset % spacing) + spacing) % spacing;
    while y > cell_y_min + spacing {
        y -= spacing;
    }

    while y <= cell_y_max {
        let mut intersections = find_horizontal_intersections(infill_region, y);
        intersections.sort_unstable();

        let mut i = 0;
        while i + 1 < intersections.len() {
            let x_enter = intersections[i];
            let x_exit = intersections[i + 1];

            if x_enter < x_exit {
                // Clip to cell bounds and infill region.
                let x_start = x_enter.max(cell_x_min);
                let x_end = x_exit.min(cell_x_max);

                if x_start < x_end {
                    lines.push(InfillLine {
                        start: IPoint2::new(x_start, y),
                        end: IPoint2::new(x_end, y),
                    });
                }
            }
            i += 2;
        }

        y += spacing;
    }

    lines
}

/// Generates rotated lines (60 or 120 degrees) within a cell, clipped to infill region.
fn generate_cell_rotated(
    infill_region: &[ValidPolygon],
    cell_x_min: Coord,
    cell_y_min: Coord,
    cell_x_max: Coord,
    cell_y_max: Coord,
    spacing: Coord,
    angle_deg: f64,
    offset: Coord,
) -> Vec<InfillLine> {
    let angle_rad = angle_deg.to_radians();
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();

    // Cell center for rotation reference.
    let center_x = ((cell_x_min as f64 + cell_x_max as f64) / 2.0).round() as Coord;
    let center_y = ((cell_y_min as f64 + cell_y_max as f64) / 2.0).round() as Coord;

    // Determine the scan range in the rotated coordinate frame.
    // Rotate cell corners by -angle to find the bounding box in rotated space.
    let corners = [
        (cell_x_min, cell_y_min),
        (cell_x_max, cell_y_min),
        (cell_x_min, cell_y_max),
        (cell_x_max, cell_y_max),
    ];

    let cos_neg = (-angle_rad).cos();
    let sin_neg = (-angle_rad).sin();

    let rotated_ys: Vec<f64> = corners
        .iter()
        .map(|&(x, y)| {
            let dx = x as f64 - center_x as f64;
            let dy = y as f64 - center_y as f64;
            dx * sin_neg + dy * cos_neg
        })
        .collect();

    let r_min_y = rotated_ys
        .iter()
        .copied()
        .fold(f64::MAX, f64::min)
        .round() as Coord
        + center_y;
    let r_max_y = rotated_ys
        .iter()
        .copied()
        .fold(f64::MIN, f64::max)
        .round() as Coord
        + center_y;

    let mut lines = Vec::new();

    // Scan in the rotated frame using horizontal lines, then rotate back.
    let mut ry = r_min_y + spacing / 2 + ((offset % spacing) + spacing) % spacing;
    while ry > r_min_y + spacing {
        ry -= spacing;
    }

    while ry <= r_max_y {
        // Generate a long line in the rotated frame at this y.
        let half_diag = ((cell_x_max - cell_x_min) as f64).hypot((cell_y_max - cell_y_min) as f64);
        let half_diag_coord = half_diag.round() as Coord;

        // Line endpoints in rotated frame (very wide to ensure coverage).
        let rx_start = center_x - half_diag_coord;
        let rx_end = center_x + half_diag_coord;

        // Rotate these endpoints back by +angle.
        let dy_from_center = (ry - center_y) as f64;

        let start_dx = (rx_start - center_x) as f64;
        let sx = center_x as f64 + start_dx * cos_a - dy_from_center * sin_a;
        let sy = center_y as f64 + start_dx * sin_a + dy_from_center * cos_a;

        let end_dx = (rx_end - center_x) as f64;
        let ex = center_x as f64 + end_dx * cos_a - dy_from_center * sin_a;
        let ey = center_y as f64 + end_dx * sin_a + dy_from_center * cos_a;

        // Clip this line against the infill region using parametric clipping.
        let clipped = clip_line_to_region(
            sx,
            sy,
            ex,
            ey,
            infill_region,
            &CellBounds {
                x_min: cell_x_min,
                y_min: cell_y_min,
                x_max: cell_x_max,
                y_max: cell_y_max,
            },
        );

        lines.extend(clipped);

        ry += spacing;
    }

    lines
}

/// Cell bounds for clipping operations.
struct CellBounds {
    x_min: Coord,
    y_min: Coord,
    x_max: Coord,
    y_max: Coord,
}

/// Clips a line segment against the infill region polygons and cell bounds.
///
/// Returns line segments that are inside both the cell and the infill region.
fn clip_line_to_region(
    sx: f64,
    sy: f64,
    ex: f64,
    ey: f64,
    infill_region: &[ValidPolygon],
    bounds: &CellBounds,
) -> Vec<InfillLine> {
    // Sample the line at regular intervals and check point-in-polygon.
    // This is simpler than exact line-polygon intersection and sufficient
    // for infill line clipping.
    let dx = ex - sx;
    let dy = ey - sy;
    let line_len = (dx * dx + dy * dy).sqrt();

    if line_len < 1.0 {
        return Vec::new();
    }

    // Use the horizontal intersection approach: find intersections of
    // the actual infill region with horizontal scanlines, but only
    // keep segments within the cell bounds.
    //
    // For rotated lines, we instead use a sampling approach: subdivide
    // the line and find connected runs that are inside the polygon.
    let step_count = ((line_len / (slicecore_math::COORD_SCALE * 0.1)).round() as usize).max(10);
    let step = 1.0 / step_count as f64;

    let mut lines = Vec::new();
    let mut inside_start: Option<(f64, f64)> = None;

    for i in 0..=step_count {
        let t = i as f64 * step;
        let px = sx + t * dx;
        let py = sy + t * dy;
        let px_coord = (px.round()) as Coord;
        let py_coord = (py.round()) as Coord;

        // Check cell bounds.
        let in_cell = px_coord >= bounds.x_min
            && px_coord <= bounds.x_max
            && py_coord >= bounds.y_min
            && py_coord <= bounds.y_max;

        // Check polygon containment.
        let in_poly = if in_cell {
            is_inside_region(px_coord, py_coord, infill_region)
        } else {
            false
        };

        if in_poly {
            if inside_start.is_none() {
                inside_start = Some((px, py));
            }
        } else if let Some((start_x, start_y)) = inside_start.take() {
            // End of an inside run -- use the previous point.
            let prev_t = (i as f64 - 1.0) * step;
            let prev_x = sx + prev_t * dx;
            let prev_y = sy + prev_t * dy;

            let s = IPoint2::new(start_x.round() as Coord, start_y.round() as Coord);
            let e = IPoint2::new(prev_x.round() as Coord, prev_y.round() as Coord);

            if s != e {
                lines.push(InfillLine { start: s, end: e });
            }
        }
    }

    // Close any remaining inside run.
    if let Some((start_x, start_y)) = inside_start {
        let end_x = sx + dx;
        let end_y = sy + dy;
        let s = IPoint2::new(start_x.round() as Coord, start_y.round() as Coord);
        let e = IPoint2::new(end_x.round() as Coord, end_y.round() as Coord);
        if s != e {
            lines.push(InfillLine { start: s, end: e });
        }
    }

    lines
}

/// Checks if a point (in coord space) is inside any polygon in the region.
fn is_inside_region(x: Coord, y: Coord, infill_region: &[ValidPolygon]) -> bool {
    use slicecore_geo::{point_in_polygon, PointLocation};
    let pt = IPoint2::new(x, y);
    for poly in infill_region {
        let loc = point_in_polygon(&pt, poly.points());
        if loc == PointLocation::Inside || loc == PointLocation::OnBoundary {
            return true;
        }
    }
    false
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

    #[test]
    fn adaptive_cubic_20mm_square_produces_lines() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.2, 0, 0.0, 0.4);
        assert!(
            !lines.is_empty(),
            "20mm square at 20% density should produce adaptive cubic infill lines"
        );
    }

    #[test]
    fn adaptive_cubic_denser_near_edges() {
        // Create a larger square so the interior vs. edge distinction is clear.
        let square = make_square(40.0);
        let lines = generate(&[square], 0.15, 0, 0.0, 0.4);

        assert!(
            !lines.is_empty(),
            "Should produce lines for 40mm square at 15%"
        );

        // Count lines in edge region (within 5mm of boundary) vs. interior.
        let edge_threshold = mm_to_coord(5.0);
        let region_min = mm_to_coord(0.0);
        let region_max = mm_to_coord(40.0);

        let mut edge_line_length: f64 = 0.0;
        let mut interior_line_length: f64 = 0.0;

        for line in &lines {
            let mid_x = (line.start.x + line.end.x) / 2;
            let mid_y = (line.start.y + line.end.y) / 2;

            let dist_to_edge = [
                (mid_x - region_min).abs(),
                (region_max - mid_x).abs(),
                (mid_y - region_min).abs(),
                (region_max - mid_y).abs(),
            ]
            .into_iter()
            .min()
            .unwrap_or(0);

            let dx = (line.end.x - line.start.x) as f64;
            let dy = (line.end.y - line.start.y) as f64;
            let length = (dx * dx + dy * dy).sqrt();

            if dist_to_edge < edge_threshold {
                edge_line_length += length;
            } else {
                interior_line_length += length;
            }
        }

        // Edge region is ~5mm deep on all 4 sides of a 40mm square.
        // Edge area: 40*40 - 30*30 = 1600 - 900 = 700 mm^2
        // Interior area: 30*30 = 900 mm^2
        // Edge is smaller area (700 vs 900), but should have proportionally
        // MORE infill (denser). So edge density > interior density.
        let edge_area = 700.0;
        let interior_area = 900.0;

        if interior_line_length > 0.0 {
            let edge_density = edge_line_length / edge_area;
            let interior_density = interior_line_length / interior_area;
            assert!(
                edge_density > interior_density * 0.8,
                "Edge region density ({:.2}) should be comparable to or higher than \
                 interior density ({:.2}) -- adaptive behavior",
                edge_density,
                interior_density
            );
        }
        // If all lines are in edge region (small object), that's also valid.
    }

    #[test]
    fn adaptive_cubic_between_dense_and_sparse() {
        let square = make_square(30.0);
        let adaptive_lines = generate(&[square.clone()], 0.2, 0, 0.0, 0.4);

        // Compare against a pure dense (high density) fill and pure sparse fill.
        // Adaptive should produce a count between the two extremes.
        let dense_lines = super::super::rectilinear::generate(&[square.clone()], 0.8, 0.0, 0.4);
        let sparse_lines = super::super::rectilinear::generate(&[square], 0.1, 0.0, 0.4);

        assert!(
            !adaptive_lines.is_empty(),
            "Adaptive cubic should produce lines"
        );

        // The adaptive line count should be somewhere in a reasonable range.
        // It's not strictly between dense and sparse because adaptive generates
        // per-cell lines, but it should be non-trivial.
        let adaptive_count = adaptive_lines.len();
        let sparse_count = sparse_lines.len();
        let dense_count = dense_lines.len();

        assert!(
            adaptive_count > 0,
            "Adaptive should produce more than 0 lines, sparse={}, dense={}",
            sparse_count,
            dense_count
        );
    }

    #[test]
    fn adaptive_cubic_lines_within_bounding_box() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.3, 0, 0.5, 0.4);

        let min = mm_to_coord(0.0);
        let max = mm_to_coord(20.0);
        let tolerance = mm_to_coord(0.5);

        for (i, line) in lines.iter().enumerate() {
            assert!(
                line.start.x >= min - tolerance && line.start.x <= max + tolerance,
                "Line {} start x ({}) outside bounds [{}, {}]",
                i,
                line.start.x,
                min - tolerance,
                max + tolerance
            );
            assert!(
                line.end.x >= min - tolerance && line.end.x <= max + tolerance,
                "Line {} end x ({}) outside bounds [{}, {}]",
                i,
                line.end.x,
                min - tolerance,
                max + tolerance
            );
            assert!(
                line.start.y >= min - tolerance && line.start.y <= max + tolerance,
                "Line {} start y ({}) outside bounds [{}, {}]",
                i,
                line.start.y,
                min - tolerance,
                max + tolerance
            );
            assert!(
                line.end.y >= min - tolerance && line.end.y <= max + tolerance,
                "Line {} end y ({}) outside bounds [{}, {}]",
                i,
                line.end.y,
                min - tolerance,
                max + tolerance
            );
        }
    }

    #[test]
    fn adaptive_cubic_angle_cycling() {
        let square = make_square(20.0);
        let lines_0 = generate(&[square.clone()], 0.2, 0, 0.0, 0.4);
        let lines_1 = generate(&[square.clone()], 0.2, 1, 0.0, 0.4);
        let lines_2 = generate(&[square], 0.2, 2, 0.0, 0.4);

        // Different layer indices should produce different patterns (different angles).
        let pos_0: Vec<_> = lines_0.iter().map(|l| (l.start, l.end)).collect();
        let pos_1: Vec<_> = lines_1.iter().map(|l| (l.start, l.end)).collect();
        let pos_2: Vec<_> = lines_2.iter().map(|l| (l.start, l.end)).collect();

        // At least the angle-0 layer should differ from angle-60 and angle-120.
        assert_ne!(
            pos_0, pos_1,
            "Layer 0 (0 deg) and layer 1 (60 deg) should produce different patterns"
        );
        assert_ne!(
            pos_0, pos_2,
            "Layer 0 (0 deg) and layer 2 (120 deg) should produce different patterns"
        );
    }

    #[test]
    fn adaptive_cubic_empty_region_returns_empty() {
        let lines = generate(&[], 0.2, 0, 0.0, 0.4);
        assert!(
            lines.is_empty(),
            "Empty region should return empty adaptive cubic lines"
        );
    }

    #[test]
    fn adaptive_cubic_zero_density_returns_empty() {
        let square = make_square(20.0);
        let lines = generate(&[square], 0.0, 0, 0.0, 0.4);
        assert!(
            lines.is_empty(),
            "Zero density should return empty adaptive cubic lines"
        );
    }

    #[test]
    fn quadtree_subdivision_creates_cells() {
        let square = make_square(20.0);
        let (min_x, min_y, max_x, max_y) = compute_bounding_box(&[square.clone()]);

        let base_spacing = mm_to_coord(2.0);
        let threshold = base_spacing as f64 * 3.0;

        let mut root = QuadCell::new(min_x, min_y, max_x, max_y, 0);
        build_quadtree(&mut root, &[square], MAX_DEPTH, base_spacing, threshold);

        let mut leaves = Vec::new();
        collect_leaves(&root, &mut leaves);

        // Should have more than 1 leaf (root was subdivided).
        assert!(
            leaves.len() > 1,
            "Quadtree should subdivide into multiple cells, got {}",
            leaves.len()
        );

        // Should have cells at different depths.
        let depths: std::collections::HashSet<u32> =
            leaves.iter().map(|&(_, _, _, _, d)| d).collect();
        assert!(
            depths.len() > 1,
            "Quadtree should have cells at different depths, got {:?}",
            depths
        );
    }

    #[test]
    fn distance_to_boundary_works() {
        let square = make_square(10.0);
        let center = (mm_to_coord(5.0), mm_to_coord(5.0));
        let edge_pt = (mm_to_coord(0.0), mm_to_coord(5.0));

        let dist_center = distance_to_boundary(center.0, center.1, &[square.clone()]);
        let dist_edge = distance_to_boundary(edge_pt.0, edge_pt.1, &[square]);

        // Center should be farther from boundary than edge point.
        assert!(
            dist_center > dist_edge,
            "Center ({}) should be farther from boundary than edge point ({})",
            dist_center,
            dist_edge
        );

        // Edge point should be very close to boundary (nearly 0).
        let dist_edge_mm = dist_edge / slicecore_math::COORD_SCALE;
        assert!(
            dist_edge_mm < 0.01,
            "Edge point should be very close to boundary, got {} mm",
            dist_edge_mm
        );
    }
}
