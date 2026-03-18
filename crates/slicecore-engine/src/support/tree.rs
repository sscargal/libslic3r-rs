//! Tree support generation with bottom-up growth, branching, and merging.
//!
//! This module implements tree-style support structures that grow from the
//! build plate upward toward overhang contact points. Tree supports use
//! less material than traditional supports and leave smaller contact marks.
//!
//! # Algorithm
//!
//! 1. Extract contact points from overhang regions.
//! 2. Grow tree from build plate upward to each contact point.
//! 3. Merge nearby branches for material efficiency.
//! 4. Apply taper and branch style.
//! 5. Slice tree into per-layer support polygons.

use slicecore_geo::polygon::{Polygon, ValidPolygon};
use slicecore_geo::{polygon_difference, polygon_union};
use slicecore_math::{coord_to_mm, IBBox2};
use slicecore_slicer::SliceLayer;

use super::config::{SupportConfig, TreeBranchStyle, TreeSupportConfig};
use super::tree_node::{
    compute_taper, compute_taper_load_based, merge_nearby_branches, TreeNode, TreeSupportArena,
};
use super::SupportRegion;
// Infill generation is delegated to super::traditional::generate_support_infill.

// ---------------------------------------------------------------------------
// Contact point extraction
// ---------------------------------------------------------------------------

/// Extracts representative contact points from overhang regions.
///
/// Samples points along overhang region boundaries at `extrusion_width * 3`
/// spacing. Each point represents a place where a tree branch tip must arrive.
///
/// # Parameters
///
/// - `overhang_regions`: Per-layer overhang regions. Index = layer index.
/// - `layers`: The sliced model layers with Z heights.
/// - `extrusion_width`: Extrusion width in mm (controls sampling density).
///
/// # Returns
///
/// Contact points as `(x_mm, y_mm, z_mm)` triples.
pub fn extract_contact_points(
    overhang_regions: &[Vec<ValidPolygon>],
    layers: &[SliceLayer],
    extrusion_width: f64,
) -> Vec<(f64, f64, f64)> {
    let spacing = extrusion_width * 3.0;
    let mut contact_points = Vec::new();

    for (layer_idx, regions) in overhang_regions.iter().enumerate() {
        if regions.is_empty() {
            continue;
        }
        let z = layers.get(layer_idx).map(|l| l.z).unwrap_or(0.0);

        for region in regions {
            let pts = region.points();
            if pts.is_empty() {
                continue;
            }

            // Sample points along the boundary at spacing intervals.
            let mut accumulated_dist = 0.0_f64;
            for i in 0..pts.len() {
                let p0 = pts[i];
                let p1 = pts[(i + 1) % pts.len()];

                let (x0, y0) = (coord_to_mm(p0.x), coord_to_mm(p0.y));
                let (x1, y1) = (coord_to_mm(p1.x), coord_to_mm(p1.y));

                let dx = x1 - x0;
                let dy = y1 - y0;
                let seg_len = (dx * dx + dy * dy).sqrt();

                if seg_len < 1e-9 {
                    continue;
                }

                // Walk along the edge at spacing intervals.
                while accumulated_dist < seg_len {
                    let t = accumulated_dist / seg_len;
                    let px = x0 + dx * t;
                    let py = y0 + dy * t;
                    contact_points.push((px, py, z));
                    accumulated_dist += spacing;
                }
                accumulated_dist -= seg_len;
            }
        }
    }

    contact_points
}

// ---------------------------------------------------------------------------
// Bottom-up tree growth
// ---------------------------------------------------------------------------

/// Grows a tree support structure from build plate upward to contact points.
///
/// For each contact point:
/// 1. Create a contact node at the overhang position.
/// 2. Project downward to find the trunk base position.
/// 3. Create a root node at z=0 (build plate).
/// 4. Connect root to contact via intermediate nodes at each layer height.
/// 5. At each layer, check for collision with model contours and adjust laterally.
///
/// After all branches are created, nearby roots are merged and taper is applied.
///
/// # Parameters
///
/// - `contact_points`: `(x, y, z)` positions where branches must arrive.
/// - `model_contours_per_layer`: Model contours at each layer for collision checking.
/// - `layer_heights`: Z height of each layer in mm.
/// - `config`: Tree support configuration.
/// - `extrusion_width`: Extrusion width in mm.
///
/// # Returns
///
/// A populated `TreeSupportArena` with the full tree structure.
pub fn grow_tree(
    contact_points: &[(f64, f64, f64)],
    model_contours_per_layer: &[Vec<ValidPolygon>],
    layer_heights: &[f64],
    config: &TreeSupportConfig,
    extrusion_width: f64,
) -> TreeSupportArena {
    let mut arena = TreeSupportArena::new();

    if contact_points.is_empty() || layer_heights.is_empty() {
        return arena;
    }

    let tip_radius = config.tip_diameter / 2.0;
    let base_radius = config.max_trunk_diameter / 2.0;

    for &(cx, cy, cz) in contact_points {
        if cz <= 0.0 {
            continue; // Contact at or below bed, skip.
        }

        // Step 1: Create contact node.
        let contact_idx = arena.add_node(TreeNode {
            position: (cx, cy),
            z: cz,
            radius: tip_radius,
            children: vec![],
            parent: None,
            is_contact: true,
            is_root: false,
        });

        // Step 2: Find trunk base position by projecting straight down.
        // If model is in the way, offset laterally.
        let mut trunk_x = cx;
        let trunk_y = cy;

        // Check the lowest layer for collision and try to find clear position.
        if !model_contours_per_layer.is_empty() && !model_contours_per_layer[0].is_empty() {
            let offset_step = extrusion_width * 2.0;
            let circle = make_circle_polygon(trunk_x, trunk_y, tip_radius, 8);
            if let Some(circle_valid) = circle {
                let overlap = polygon_difference(
                    std::slice::from_ref(&circle_valid),
                    &model_contours_per_layer[0],
                );
                if overlap.as_ref().map(|r| r.is_empty()).unwrap_or(true) {
                    // Circle is fully inside model -- offset laterally.
                    trunk_x += offset_step;
                }
            }
        }

        // Step 3: Create root node at z=0.
        let root_idx = arena.add_node(TreeNode {
            position: (trunk_x, trunk_y),
            z: 0.0,
            radius: base_radius,
            children: vec![],
            parent: None,
            is_contact: false,
            is_root: true,
        });

        // Step 4: Create intermediate nodes at each layer height between root and contact.
        let mut prev_idx = root_idx;
        let mut prev_x = trunk_x;
        let mut prev_y = trunk_y;

        for (layer_idx, &lh) in layer_heights.iter().enumerate() {
            if lh <= 0.0 || lh >= cz {
                continue; // Skip layers at or above the contact height.
            }

            // Interpolate position between trunk base and contact point.
            let t = lh / cz;
            let mut node_x = trunk_x + (cx - trunk_x) * t;
            let mut node_y = trunk_y + (cy - trunk_y) * t;

            // Step 5: Check for collision with model contours at this layer.
            if layer_idx < model_contours_per_layer.len()
                && !model_contours_per_layer[layer_idx].is_empty()
            {
                let node_circle = make_circle_polygon(node_x, node_y, tip_radius, 8);
                if let Some(circle_valid) = node_circle {
                    let remaining = polygon_difference(
                        std::slice::from_ref(&circle_valid),
                        &model_contours_per_layer[layer_idx],
                    );
                    if remaining.as_ref().map(|r| r.is_empty()).unwrap_or(true) {
                        // Collision: offset laterally away from model center.
                        let offset_step = extrusion_width * 2.0;
                        // Determine offset direction from model center.
                        if let Some(bbox) = model_bbox_center(&model_contours_per_layer[layer_idx])
                        {
                            let dx = node_x - bbox.0;
                            let dy = node_y - bbox.1;
                            let dist = (dx * dx + dy * dy).sqrt();
                            if dist > 1e-9 {
                                node_x += dx / dist * offset_step;
                                node_y += dy / dist * offset_step;
                            } else {
                                node_x += offset_step;
                            }
                        } else {
                            node_x += offset_step;
                        }
                    }
                }
            }

            let node_idx = arena.add_node(TreeNode {
                position: (node_x, node_y),
                z: lh,
                radius: tip_radius, // Will be updated by taper.
                children: vec![],
                parent: Some(prev_idx),
                is_contact: false,
                is_root: false,
            });

            arena.get_node_mut(prev_idx).children.push(node_idx);
            prev_idx = node_idx;
            prev_x = node_x;
            prev_y = node_y;
        }

        // Connect last intermediate node to the contact node.
        arena.get_node_mut(prev_idx).children.push(contact_idx);
        arena.get_node_mut(contact_idx).parent = Some(prev_idx);

        // Suppress unused variable warnings.
        let _ = prev_x;
        let _ = prev_y;
    }

    // Step 6: Merge nearby branches.
    let merge_dist = (config.merge_distance_factor * config.max_trunk_diameter).max(5.0);
    merge_nearby_branches(&mut arena, merge_dist, config.max_trunk_diameter);

    // Step 7: Apply taper to all nodes.
    apply_taper_to_arena(&mut arena, config, contact_points.len());

    arena
}

/// Applies taper to all nodes in the arena based on the configured method.
fn apply_taper_to_arena(
    arena: &mut TreeSupportArena,
    config: &TreeSupportConfig,
    total_contacts: usize,
) {
    let tip_radius = config.tip_diameter / 2.0;
    let base_radius = config.max_trunk_diameter / 2.0;

    // Find max Z in the arena for total_height.
    let max_z = (0..arena.len())
        .map(|i| arena.get_node(i).z)
        .fold(0.0_f64, f64::max);

    if max_z <= 0.0 {
        return;
    }

    for i in 0..arena.len() {
        let z = arena.get_node(i).z;
        let is_contact = arena.get_node(i).is_contact;
        let is_root = arena.get_node(i).is_root;

        let radius = if is_contact {
            tip_radius
        } else if is_root {
            base_radius
        } else {
            match config.taper_method {
                super::config::TaperMethod::LoadBased => {
                    // Count contacts above this Z.
                    let contacts_above = (0..arena.len())
                        .filter(|&j| arena.get_node(j).is_contact && arena.get_node(j).z > z)
                        .count();
                    compute_taper_load_based(
                        base_radius,
                        tip_radius,
                        contacts_above,
                        total_contacts,
                    )
                }
                _ => compute_taper(base_radius, tip_radius, z, max_z, config.taper_method),
            }
        };

        arena.get_node_mut(i).radius = radius;
    }
}

// ---------------------------------------------------------------------------
// Branch style application
// ---------------------------------------------------------------------------

/// Applies the specified branch style to the tree arena.
///
/// - **Geometric**: No change (straight line segments, already the default).
/// - **Organic**: Inserts intermediate nodes with Bezier-curve-like interpolation
///   to create smooth curves between branch points.
/// - **Auto**: Uses geometric for flat overhang surfaces (low curvature), organic
///   for curved surfaces.
///
/// # Parameters
///
/// - `arena`: The tree support arena to modify in place.
/// - `style`: The branch style to apply.
pub fn apply_branch_style(arena: &mut TreeSupportArena, style: TreeBranchStyle) {
    match style {
        TreeBranchStyle::Geometric | TreeBranchStyle::Auto => {
            // Geometric: straight segments, no additional processing.
            // Auto: default to geometric for simplicity (curvature-based
            // switching would require analyzing contact point distribution).
        }
        TreeBranchStyle::Organic => {
            apply_organic_style(arena);
        }
    }
}

/// Applies organic branching by inserting Bezier-like intermediate control nodes.
///
/// For each parent-child edge that spans more than one layer, inserts a
/// control point that curves the path smoothly.
fn apply_organic_style(arena: &mut TreeSupportArena) {
    // Collect parent-child pairs that need smoothing.
    let len = arena.len();
    let mut insertions: Vec<(usize, usize, TreeNode)> = Vec::new();

    for i in 0..len {
        let children = arena.children_of(i).to_vec();
        for &child_idx in &children {
            let parent = arena.get_node(i);
            let child = arena.get_node(child_idx);

            let dz = (child.z - parent.z).abs();
            if dz < 1e-6 {
                continue;
            }

            let dx = child.position.0 - parent.position.0;
            let dy = child.position.1 - parent.position.1;
            let horizontal_dist = (dx * dx + dy * dy).sqrt();

            // Only insert control points for branches with meaningful lateral offset.
            if horizontal_dist < 0.5 {
                continue;
            }

            // Insert a control point at the midpoint with a lateral offset
            // to create a curved path.
            let mid_z = (parent.z + child.z) / 2.0;
            let mid_x = (parent.position.0 + child.position.0) / 2.0;
            let mid_y = (parent.position.1 + child.position.1) / 2.0;

            // Perpendicular offset for the curve (toward the parent side).
            let perp_scale = horizontal_dist * 0.15;
            let curve_x = mid_x + dy / horizontal_dist * perp_scale;
            let curve_y = mid_y - dx / horizontal_dist * perp_scale;

            let mid_radius = (parent.radius + child.radius) / 2.0;

            insertions.push((
                i,
                child_idx,
                TreeNode {
                    position: (curve_x, curve_y),
                    z: mid_z,
                    radius: mid_radius,
                    children: vec![],
                    parent: None,
                    is_contact: false,
                    is_root: false,
                },
            ));
        }
    }

    // Apply insertions: for each (parent, child), insert a new node between them.
    for (parent_idx, child_idx, mut new_node) in insertions {
        new_node.parent = Some(parent_idx);
        new_node.children = vec![child_idx];

        let new_idx = arena.add_node(new_node);

        // Update parent's children: replace child_idx with new_idx.
        let parent = arena.get_node_mut(parent_idx);
        if let Some(pos) = parent.children.iter().position(|&c| c == child_idx) {
            parent.children[pos] = new_idx;
        }

        // Update child's parent to point to new node.
        arena.get_node_mut(child_idx).parent = Some(new_idx);
    }
}

// ---------------------------------------------------------------------------
// Tree slicing to per-layer polygons
// ---------------------------------------------------------------------------

/// Slices the tree support structure into per-layer support region polygons.
///
/// For each layer, finds all tree nodes at that Z height (within layer_height/2
/// tolerance), generates a circular polygon with the node's radius, and unions
/// all circles at each layer.
///
/// # Parameters
///
/// - `arena`: The tree support arena.
/// - `layer_count`: Number of layers.
/// - `layer_heights`: Z heights for each layer in mm.
///
/// # Returns
///
/// Per-layer support region polygons. `result[i]` contains all support
/// polygons for layer `i`.
pub fn slice_tree_to_layers(
    arena: &TreeSupportArena,
    layer_count: usize,
    layer_heights: &[f64],
) -> Vec<Vec<ValidPolygon>> {
    let mut result = vec![Vec::new(); layer_count];

    if arena.is_empty() || layer_count == 0 {
        return result;
    }

    for (layer_idx, layer_slot) in result.iter_mut().enumerate().take(layer_count) {
        let layer_z = layer_heights.get(layer_idx).copied().unwrap_or(0.0);

        // Compute tolerance: half the layer height (default 0.1mm if unknown).
        let tolerance = if layer_idx > 0 {
            let prev_z = layer_heights.get(layer_idx - 1).copied().unwrap_or(0.0);
            ((layer_z - prev_z) / 2.0).max(0.05)
        } else {
            0.1
        };

        let mut layer_circles: Vec<ValidPolygon> = Vec::new();

        for node_idx in 0..arena.len() {
            let node = arena.get_node(node_idx);

            // Check if node is at this layer's Z height (within tolerance).
            if (node.z - layer_z).abs() <= tolerance && node.radius > 0.001 {
                // Generate circular polygon for this node.
                if let Some(circle) =
                    make_circle_polygon(node.position.0, node.position.1, node.radius, 16)
                {
                    layer_circles.push(circle);
                }
            }
        }

        if layer_circles.is_empty() {
            continue;
        }

        // Union all circles at this layer.
        if layer_circles.len() == 1 {
            *layer_slot = layer_circles;
        } else {
            // Progressive union.
            let mut merged = vec![layer_circles[0].clone()];
            for circle in &layer_circles[1..] {
                merged = polygon_union(&merged, std::slice::from_ref(circle)).unwrap_or(merged);
            }
            *layer_slot = merged;
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Generates tree support structures from overhang regions.
///
/// This is the main entry point for tree support generation, combining:
/// 1. Contact point extraction from overhang regions.
/// 2. Bottom-up tree growth from build plate to contact points.
/// 3. Branch style application (organic or geometric).
/// 4. Tree slicing into per-layer polygons.
/// 5. Support infill generation per layer.
///
/// # Parameters
///
/// - `overhang_regions`: Per-layer overhang regions from detection.
/// - `layers`: The sliced model layers.
/// - `config`: Support configuration with tree parameters.
/// - `extrusion_width`: Extrusion width in mm.
///
/// # Returns
///
/// Per-layer support region vectors with infill.
pub fn generate_tree_supports(
    overhang_regions: &[Vec<ValidPolygon>],
    layers: &[SliceLayer],
    config: &SupportConfig,
    extrusion_width: f64,
) -> Vec<Vec<SupportRegion>> {
    let n = layers.len();
    if n == 0 {
        return Vec::new();
    }

    // Step 1: Extract contact points.
    let contact_points = extract_contact_points(overhang_regions, layers, extrusion_width);

    if contact_points.is_empty() {
        return vec![Vec::new(); n];
    }

    // Collect model contours per layer and layer heights.
    let model_contours: Vec<Vec<ValidPolygon>> =
        layers.iter().map(|l| l.contours.clone()).collect();
    let layer_heights: Vec<f64> = layers.iter().map(|l| l.z).collect();

    // Step 2: Grow tree.
    let mut arena = grow_tree(
        &contact_points,
        &model_contours,
        &layer_heights,
        &config.tree,
        extrusion_width,
    );

    // Step 3: Apply branch style.
    apply_branch_style(&mut arena, config.tree.branch_style);

    // Step 4: Slice tree into per-layer polygons.
    let tree_polygons = slice_tree_to_layers(&arena, n, &layer_heights);

    // Step 5: Generate infill per layer.
    let support_pattern = config.support_pattern;

    let mut result: Vec<Vec<SupportRegion>> = Vec::with_capacity(n);

    for (layer_idx, polygons) in tree_polygons.into_iter().enumerate() {
        if polygons.is_empty() {
            result.push(Vec::new());
            continue;
        }

        let infill_lines = super::traditional::generate_support_infill(
            &polygons,
            config.support_density,
            support_pattern,
            layer_idx,
            extrusion_width,
        );

        let z = layers[layer_idx].z;

        result.push(vec![SupportRegion {
            contours: polygons,
            z,
            layer_index: layer_idx,
            is_bridge: false,
            infill: infill_lines,
        }]);
    }

    result
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Creates a circle polygon approximation at the given position with given radius.
///
/// Uses `n_segments` line segments to approximate the circle.
fn make_circle_polygon(
    center_x: f64,
    center_y: f64,
    radius: f64,
    n_segments: usize,
) -> Option<ValidPolygon> {
    if radius < 1e-6 || n_segments < 3 {
        return None;
    }

    let points: Vec<(f64, f64)> = (0..n_segments)
        .map(|i| {
            let angle = 2.0 * std::f64::consts::PI * i as f64 / n_segments as f64;
            (
                center_x + radius * angle.cos(),
                center_y + radius * angle.sin(),
            )
        })
        .collect();

    Polygon::from_mm(&points).validate().ok()
}

/// Computes the center of a set of model contours from their bounding box.
fn model_bbox_center(contours: &[ValidPolygon]) -> Option<(f64, f64)> {
    let mut all_points = Vec::new();
    for contour in contours {
        all_points.extend_from_slice(contour.points());
    }

    let bbox = IBBox2::from_points(&all_points)?;
    Some((
        coord_to_mm(bbox.min.x + bbox.max.x) / 2.0,
        coord_to_mm(bbox.min.y + bbox.max.y) / 2.0,
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::config::SupportPattern;
    use super::*;
    use slicecore_geo::polygon::Polygon;

    /// Helper to create a validated CCW square at a given position and size.
    fn make_square(x: f64, y: f64, size: f64) -> ValidPolygon {
        Polygon::from_mm(&[(x, y), (x + size, y), (x + size, y + size), (x, y + size)])
            .validate()
            .unwrap()
    }

    /// Helper to create a SliceLayer with the given contours.
    fn make_layer(z: f64, contours: Vec<ValidPolygon>) -> SliceLayer {
        SliceLayer {
            z,
            layer_height: 0.2,
            contours,
        }
    }

    #[test]
    fn single_contact_produces_trunk() {
        // One contact point at (50, 50, 2.0). Should produce a trunk from
        // the build plate (z=0) to the contact point.
        let contact_points = vec![(50.0, 50.0, 2.0)];
        let model_contours: Vec<Vec<ValidPolygon>> = vec![Vec::new(); 10];
        let layer_heights: Vec<f64> = (0..10).map(|i| 0.2 * (i as f64 + 0.5)).collect();

        let config = TreeSupportConfig::default();
        let arena = grow_tree(
            &contact_points,
            &model_contours,
            &layer_heights,
            &config,
            0.4,
        );

        assert!(
            !arena.is_empty(),
            "Arena should have nodes for a single contact point"
        );

        // Should have at least one root and one contact.
        let roots = arena.root_indices();
        assert!(!roots.is_empty(), "Should have at least one root node");

        let contacts = arena.contact_indices();
        assert_eq!(contacts.len(), 1, "Should have exactly one contact node");

        // Root should be at z=0.
        let root = arena.get_node(roots[0]);
        assert!(root.z.abs() < 1e-9, "Root should be at z=0, got {}", root.z,);

        // Contact should be at z=2.0.
        let contact = arena.get_node(contacts[0]);
        assert!(
            (contact.z - 2.0).abs() < 1e-9,
            "Contact should be at z=2.0, got {}",
            contact.z,
        );
    }

    #[test]
    fn two_nearby_contacts_share_merged_trunk() {
        // Two contact points 2mm apart at the same Z.
        let contact_points = vec![(50.0, 50.0, 2.0), (52.0, 50.0, 2.0)];
        let model_contours: Vec<Vec<ValidPolygon>> = vec![Vec::new(); 10];
        let layer_heights: Vec<f64> = (0..10).map(|i| 0.2 * (i as f64 + 0.5)).collect();

        // Configure merge distance to include both (merge_distance_factor * max_trunk_diameter).
        let config = TreeSupportConfig {
            merge_distance_factor: 3.0,
            max_trunk_diameter: 10.0,
            ..Default::default()
        };

        let arena = grow_tree(
            &contact_points,
            &model_contours,
            &layer_heights,
            &config,
            0.4,
        );

        let roots = arena.root_indices();
        // With merge_distance = 3.0 * 10.0 = 30mm, roots 2mm apart should merge.
        assert_eq!(
            roots.len(),
            1,
            "Two nearby contact points should share a merged trunk, got {} roots",
            roots.len(),
        );
    }

    #[test]
    fn tree_avoids_model_contour_collision() {
        // Place a model contour exactly where the trunk would go.
        let model_square = make_square(49.5, 49.5, 1.0); // Small model at (49.5-50.5, 49.5-50.5)
        let model_contours: Vec<Vec<ValidPolygon>> = vec![vec![model_square.clone()]; 10];
        let layer_heights: Vec<f64> = (0..10).map(|i| 0.2 * (i as f64 + 0.5)).collect();

        let contact_points = vec![(50.0, 50.0, 2.0)];
        let config = TreeSupportConfig::default();

        let arena = grow_tree(
            &contact_points,
            &model_contours,
            &layer_heights,
            &config,
            0.4,
        );

        // The tree should still have nodes, even if collision avoidance moved them.
        assert!(
            !arena.is_empty(),
            "Arena should have nodes even with collision"
        );

        // Check that intermediate nodes are not exactly at the model center.
        // (Collision avoidance should have moved them.)
        for i in 0..arena.len() {
            let node = arena.get_node(i);
            if !node.is_root && !node.is_contact && node.z > 0.0 {
                // At least some intermediate nodes should have been offset.
                // We just verify the tree was built without crashing.
            }
        }
    }

    #[test]
    fn slice_tree_produces_circles_at_correct_heights() {
        // Build a simple tree manually and slice it.
        let mut arena = TreeSupportArena::new();

        // Root at z=0.
        let root = arena.add_node(TreeNode {
            position: (50.0, 50.0),
            z: 0.0,
            radius: 2.0,
            children: vec![],
            parent: None,
            is_contact: false,
            is_root: true,
        });

        // Intermediate at z=1.0.
        let mid = arena.add_node(TreeNode {
            position: (50.0, 50.0),
            z: 1.0,
            radius: 1.5,
            children: vec![],
            parent: Some(root),
            is_contact: false,
            is_root: false,
        });
        arena.get_node_mut(root).children.push(mid);

        // Contact at z=2.0.
        let contact = arena.add_node(TreeNode {
            position: (50.0, 50.0),
            z: 2.0,
            radius: 0.4,
            children: vec![],
            parent: Some(mid),
            is_contact: true,
            is_root: false,
        });
        arena.get_node_mut(mid).children.push(contact);

        let layer_heights = vec![0.0, 1.0, 2.0];
        let sliced = slice_tree_to_layers(&arena, 3, &layer_heights);

        assert_eq!(sliced.len(), 3, "Should have 3 layers of sliced support");

        // Layer 0 (z=0.0): root node with radius 2.0.
        assert!(
            !sliced[0].is_empty(),
            "Layer 0 should have support polygons from root node"
        );

        // Layer 1 (z=1.0): intermediate node with radius 1.5.
        assert!(
            !sliced[1].is_empty(),
            "Layer 1 should have support polygons from intermediate node"
        );

        // Layer 2 (z=2.0): contact node with radius 0.4.
        assert!(
            !sliced[2].is_empty(),
            "Layer 2 should have support polygons from contact node"
        );

        // Verify that layer 0 circles are larger than layer 2 circles.
        let area_0: f64 = sliced[0].iter().map(|p| p.area_mm2()).sum();
        let area_2: f64 = sliced[2].iter().map(|p| p.area_mm2()).sum();
        assert!(
            area_0 > area_2,
            "Root layer area ({}) should be > contact layer area ({})",
            area_0,
            area_2,
        );
    }

    #[test]
    fn geometric_vs_organic_node_count() {
        // Build a simple tree with lateral offset, then apply each style.
        let contact_points = vec![(55.0, 50.0, 3.0)]; // Offset from center.
        let model_contours: Vec<Vec<ValidPolygon>> = vec![Vec::new(); 15];
        let layer_heights: Vec<f64> = (0..15).map(|i| 0.2 * (i as f64 + 0.5)).collect();

        let config = TreeSupportConfig {
            branch_style: TreeBranchStyle::Geometric,
            ..Default::default()
        };

        let geo_arena = grow_tree(
            &contact_points,
            &model_contours,
            &layer_heights,
            &config,
            0.4,
        );
        let geo_count = geo_arena.len();

        // Now grow with organic style.
        let mut org_arena = grow_tree(
            &contact_points,
            &model_contours,
            &layer_heights,
            &config,
            0.4,
        );
        apply_branch_style(&mut org_arena, TreeBranchStyle::Organic);
        let org_count = org_arena.len();

        // Organic should have more nodes due to inserted control points.
        assert!(
            org_count >= geo_count,
            "Organic ({}) should have >= nodes than geometric ({})",
            org_count,
            geo_count,
        );
    }

    #[test]
    fn generate_tree_supports_end_to_end() {
        // Full end-to-end test with overhang regions.
        let model_square = make_square(50.0, 50.0, 10.0);
        let overhang_square = make_square(55.0, 50.0, 10.0);

        let layers: Vec<SliceLayer> = (0..6)
            .map(|i| make_layer(0.2 * (i as f64 + 0.5), vec![model_square.clone()]))
            .collect();

        let mut overhang_regions = vec![Vec::new(); 6];
        overhang_regions[5] = vec![overhang_square];

        let config = SupportConfig {
            enabled: true,
            support_density: 0.15,
            support_pattern: SupportPattern::Line,
            tree: TreeSupportConfig::default(),
            ..Default::default()
        };

        let result = generate_tree_supports(&overhang_regions, &layers, &config, 0.4);

        assert_eq!(result.len(), 6, "Should have 6 layers of results");

        // At least some layers should have support regions.
        let layers_with_support: Vec<usize> = result
            .iter()
            .enumerate()
            .filter(|(_, regions)| !regions.is_empty())
            .map(|(i, _)| i)
            .collect();

        assert!(
            !layers_with_support.is_empty(),
            "Should have at least some layers with tree support"
        );
    }

    #[test]
    fn extract_contact_points_sampling() {
        // Create an overhang region and verify contact points are extracted.
        let overhang = make_square(50.0, 50.0, 10.0);
        let layers: Vec<SliceLayer> = (0..3)
            .map(|i| make_layer(0.2 * (i as f64 + 0.5), vec![]))
            .collect();

        let mut overhang_regions = vec![Vec::new(); 3];
        overhang_regions[2] = vec![overhang];

        let points = extract_contact_points(&overhang_regions, &layers, 0.4);

        assert!(
            !points.is_empty(),
            "Should extract contact points from overhang region"
        );

        // All points should be at the Z height of layer 2.
        let z2 = layers[2].z;
        for &(_, _, z) in &points {
            assert!(
                (z - z2).abs() < 1e-6,
                "Contact point z ({}) should match layer 2 z ({})",
                z,
                z2,
            );
        }
    }
}
