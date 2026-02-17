//! Tree support node data structures and algorithms.
//!
//! Provides the arena-based tree data structure for tree support generation,
//! including:
//! - [`TreeNode`]: Individual node with position, radius, and connectivity.
//! - [`TreeSupportArena`]: Flat arena holding all nodes for a tree support structure.
//! - [`compute_taper`]: Radius computation using linear, exponential, or load-based tapering.
//! - [`merge_nearby_branches`]: Greedy nearest-neighbor merging of nearby root nodes.
//! - [`compute_branch_angle`]: Angle between trunk vertical and branch direction.

use super::config::TaperMethod;

// ---------------------------------------------------------------------------
// TreeNode
// ---------------------------------------------------------------------------

/// A single node in the tree support structure.
///
/// Nodes are stored in a flat arena ([`TreeSupportArena`]) and reference each
/// other by index, following the project's arena-based pattern (no recursive
/// pointers).
#[derive(Clone, Debug)]
pub struct TreeNode {
    /// XY position in mm.
    pub position: (f64, f64),
    /// Z height in mm.
    pub z: f64,
    /// Radius at this node in mm.
    pub radius: f64,
    /// Indices of child nodes in the arena.
    pub children: Vec<usize>,
    /// Index of the parent node in the arena, if any.
    pub parent: Option<usize>,
    /// True if this node touches the overhang surface (top of branch).
    pub is_contact: bool,
    /// True if this node is on the build plate (z=0).
    pub is_root: bool,
}

// ---------------------------------------------------------------------------
// TreeSupportArena
// ---------------------------------------------------------------------------

/// Flat arena of tree support nodes.
///
/// All nodes are stored in a single `Vec` and reference each other by index.
/// This avoids recursive pointer structures and makes serialization and
/// traversal straightforward.
#[derive(Clone, Debug)]
pub struct TreeSupportArena {
    /// All nodes in the arena.
    nodes: Vec<TreeNode>,
}

impl TreeSupportArena {
    /// Creates an empty arena.
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Adds a node to the arena and returns its index.
    pub fn add_node(&mut self, node: TreeNode) -> usize {
        let idx = self.nodes.len();
        self.nodes.push(node);
        idx
    }

    /// Returns a reference to the node at the given index.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is out of bounds.
    pub fn get_node(&self, idx: usize) -> &TreeNode {
        &self.nodes[idx]
    }

    /// Returns a mutable reference to the node at the given index.
    ///
    /// # Panics
    ///
    /// Panics if `idx` is out of bounds.
    pub fn get_node_mut(&mut self, idx: usize) -> &mut TreeNode {
        &mut self.nodes[idx]
    }

    /// Returns the children indices of the node at `idx`.
    pub fn children_of(&self, idx: usize) -> &[usize] {
        &self.nodes[idx].children
    }

    /// Returns the total number of nodes in the arena.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns true if the arena contains no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns indices of all root nodes (nodes on the build plate).
    pub fn root_indices(&self) -> Vec<usize> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.is_root)
            .map(|(i, _)| i)
            .collect()
    }

    /// Returns indices of all contact nodes (nodes touching overhang surface).
    pub fn contact_indices(&self) -> Vec<usize> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.is_contact)
            .map(|(i, _)| i)
            .collect()
    }
}

impl Default for TreeSupportArena {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Taper computation
// ---------------------------------------------------------------------------

/// Computes the radius at a given Z height using the specified taper method.
///
/// The radius varies from `base_radius` at z=0 (bottom) to `tip_radius` at
/// z=`total_height` (top). All methods produce `base_radius` at z=0 and
/// `tip_radius` at z=total_height.
///
/// # Parameters
///
/// - `base_radius`: Radius at the bottom (z=0) in mm.
/// - `tip_radius`: Radius at the top (z=total_height) in mm.
/// - `z`: Current Z height in mm.
/// - `total_height`: Total height from base to tip in mm.
/// - `method`: Taper method to use.
///
/// # Taper methods
///
/// - **Linear**: `tip + (base - tip) * (1 - z/h)` -- constant rate of change.
/// - **Exponential**: `tip + (base - tip) * (1 - z/h)^2` -- rapid widening near base.
/// - **LoadBased**: Takes `contacts_above` and `total_contacts` parameters.
///   For standalone calls, behaves like Linear.
/// - **Auto**: Defaults to Linear.
pub fn compute_taper(
    base_radius: f64,
    tip_radius: f64,
    z: f64,
    total_height: f64,
    method: TaperMethod,
) -> f64 {
    if total_height <= 0.0 {
        return tip_radius;
    }

    let t = (z / total_height).clamp(0.0, 1.0);

    match method {
        TaperMethod::Linear | TaperMethod::Auto => {
            // Linear interpolation: wider at bottom, narrower at top.
            tip_radius + (base_radius - tip_radius) * (1.0 - t)
        }
        TaperMethod::Exponential => {
            // Exponential: rapid widening near base, narrower near top.
            tip_radius + (base_radius - tip_radius) * (1.0 - t).powi(2)
        }
        TaperMethod::LoadBased => {
            // Load-based: for standalone use, default to linear.
            // The full load-based computation is done in compute_taper_load_based.
            tip_radius + (base_radius - tip_radius) * (1.0 - t)
        }
    }
}

/// Computes load-based taper radius at a given Z height.
///
/// Width is proportional to the estimated load above: at each Z, the radius
/// scales with the square root of the fraction of contact points above.
///
/// # Parameters
///
/// - `base_radius`: Radius at the bottom (z=0) in mm.
/// - `tip_radius`: Radius at the top in mm.
/// - `contacts_above`: Number of contact points above this Z.
/// - `total_contacts`: Total number of contact points in the tree.
pub fn compute_taper_load_based(
    base_radius: f64,
    tip_radius: f64,
    contacts_above: usize,
    total_contacts: usize,
) -> f64 {
    if total_contacts == 0 {
        return tip_radius;
    }
    let load_fraction = (contacts_above as f64 / total_contacts as f64).sqrt();
    tip_radius + (base_radius - tip_radius) * load_fraction
}

// ---------------------------------------------------------------------------
// Branch merging
// ---------------------------------------------------------------------------

/// Merges nearby root-level branches to save material.
///
/// Uses greedy nearest-neighbor: sorts all root pairs by distance, and
/// merges the closest first. Merged trunks are positioned at the midpoint
/// of the two original roots. Children of both original roots become children
/// of the merged trunk.
///
/// A merge is skipped if the resulting trunk radius would exceed
/// `max_diameter / 2`.
///
/// Per research: merge within `merge_distance` (typically
/// `merge_distance_factor * trunk_diameter`, minimum 5mm).
///
/// # Parameters
///
/// - `arena`: The tree support arena to modify in place.
/// - `merge_distance`: Maximum distance between two roots to consider merging (in mm).
/// - `max_diameter`: Maximum allowed trunk diameter in mm.
pub fn merge_nearby_branches(
    arena: &mut TreeSupportArena,
    merge_distance: f64,
    max_diameter: f64,
) {
    let max_radius = max_diameter / 2.0;

    loop {
        // Collect current root indices (recollect each iteration since merging
        // changes root status).
        let roots: Vec<usize> = arena.root_indices();
        if roots.len() < 2 {
            break;
        }

        // Build all pairs with distances, sorted by distance (ascending).
        let mut pairs: Vec<(usize, usize, f64)> = Vec::new();
        for i in 0..roots.len() {
            for j in (i + 1)..roots.len() {
                let a = &arena.nodes[roots[i]];
                let b = &arena.nodes[roots[j]];
                let dx = a.position.0 - b.position.0;
                let dy = a.position.1 - b.position.1;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < merge_distance {
                    pairs.push((roots[i], roots[j], dist));
                }
            }
        }

        if pairs.is_empty() {
            break;
        }

        // Sort by distance ascending -- merge closest first.
        pairs.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

        // Track which nodes have been merged this round to avoid double-merging.
        let mut merged_this_round = false;
        let mut consumed: Vec<bool> = vec![false; arena.nodes.len()];

        for (idx_a, idx_b, _dist) in &pairs {
            if consumed[*idx_a] || consumed[*idx_b] {
                continue;
            }

            let node_a = &arena.nodes[*idx_a];
            let node_b = &arena.nodes[*idx_b];

            // Check if merged radius would exceed max.
            let merged_radius = (node_a.radius + node_b.radius) / 2.0;
            if merged_radius > max_radius {
                continue;
            }

            // Compute midpoint position.
            let mid_x = (node_a.position.0 + node_b.position.0) / 2.0;
            let mid_y = (node_a.position.1 + node_b.position.1) / 2.0;

            // Collect children from both roots.
            let children_a = node_a.children.clone();
            let children_b = node_b.children.clone();
            let mut all_children = children_a;
            all_children.extend(children_b);

            // Create merged root node.
            let merged_idx = arena.add_node(TreeNode {
                position: (mid_x, mid_y),
                z: 0.0,
                radius: merged_radius,
                children: all_children.clone(),
                parent: None,
                is_contact: false,
                is_root: true,
            });

            // Update children to point to new parent.
            for &child_idx in &all_children {
                arena.nodes[child_idx].parent = Some(merged_idx);
            }

            // Mark original roots as non-root (effectively removed as roots).
            arena.nodes[*idx_a].is_root = false;
            arena.nodes[*idx_b].is_root = false;

            consumed[*idx_a] = true;
            consumed[*idx_b] = true;
            merged_this_round = true;
        }

        if !merged_this_round {
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// Branch angle computation
// ---------------------------------------------------------------------------

/// Computes the angle in degrees between the trunk vertical and a branch
/// direction defined by a parent node and a child node.
///
/// A vertical branch (child directly above parent in XY) returns 0 degrees.
/// A 45-degree lateral offset returns 45 degrees.
///
/// # Parameters
///
/// - `parent_pos`: XY position of the parent node.
/// - `child_pos`: XY position of the child node.
/// - `parent_z`: Z height of the parent node.
/// - `child_z`: Z height of the child node.
///
/// # Returns
///
/// Angle in degrees between the vertical axis and the branch direction.
pub fn compute_branch_angle(
    parent_pos: (f64, f64),
    child_pos: (f64, f64),
    parent_z: f64,
    child_z: f64,
) -> f64 {
    let dx = child_pos.0 - parent_pos.0;
    let dy = child_pos.1 - parent_pos.1;
    let dz = (child_z - parent_z).abs();
    let horizontal_dist = (dx * dx + dy * dy).sqrt();

    if dz < 1e-12 && horizontal_dist < 1e-12 {
        return 0.0;
    }

    // Angle from vertical: atan2(horizontal, vertical).
    horizontal_dist.atan2(dz).to_degrees()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_taper_base_and_tip() {
        let base = 5.0;
        let tip = 0.4;
        let height = 10.0;

        // At z=0, radius should equal base_radius.
        let r0 = compute_taper(base, tip, 0.0, height, TaperMethod::Linear);
        assert!(
            (r0 - base).abs() < 1e-9,
            "Linear taper at z=0 should be base_radius ({}), got {}",
            base,
            r0,
        );

        // At z=height, radius should equal tip_radius.
        let rh = compute_taper(base, tip, height, height, TaperMethod::Linear);
        assert!(
            (rh - tip).abs() < 1e-9,
            "Linear taper at z=height should be tip_radius ({}), got {}",
            tip,
            rh,
        );

        // At z=height/2, radius should be midpoint.
        let mid = compute_taper(base, tip, height / 2.0, height, TaperMethod::Linear);
        let expected_mid = tip + (base - tip) * 0.5;
        assert!(
            (mid - expected_mid).abs() < 1e-9,
            "Linear taper at z=h/2 should be {}, got {}",
            expected_mid,
            mid,
        );
    }

    #[test]
    fn exponential_taper_decreases_faster_near_top() {
        let base = 5.0;
        let tip = 0.4;
        let height = 10.0;

        // At z=0, should equal base.
        let r0 = compute_taper(base, tip, 0.0, height, TaperMethod::Exponential);
        assert!(
            (r0 - base).abs() < 1e-9,
            "Exponential taper at z=0 should be base_radius",
        );

        // At z=height, should equal tip.
        let rh = compute_taper(base, tip, height, height, TaperMethod::Exponential);
        assert!(
            (rh - tip).abs() < 1e-9,
            "Exponential taper at z=height should be tip_radius",
        );

        // At z=h/2, exponential should be smaller than linear (decreases faster near top).
        let exp_mid = compute_taper(base, tip, height / 2.0, height, TaperMethod::Exponential);
        let lin_mid = compute_taper(base, tip, height / 2.0, height, TaperMethod::Linear);
        assert!(
            exp_mid < lin_mid,
            "Exponential taper at z=h/2 ({}) should be less than linear ({})",
            exp_mid,
            lin_mid,
        );
    }

    #[test]
    fn load_based_taper() {
        let base = 5.0;
        let tip = 0.4;

        // All contacts above -> full load -> base_radius.
        let full = compute_taper_load_based(base, tip, 10, 10);
        assert!(
            (full - base).abs() < 1e-9,
            "Full load should equal base_radius, got {}",
            full,
        );

        // No contacts above -> zero load -> tip_radius.
        let none = compute_taper_load_based(base, tip, 0, 10);
        assert!(
            (none - tip).abs() < 1e-9,
            "Zero load should equal tip_radius, got {}",
            none,
        );

        // Half contacts -> sqrt(0.5) load factor.
        let half = compute_taper_load_based(base, tip, 5, 10);
        let expected = tip + (base - tip) * (0.5_f64).sqrt();
        assert!(
            (half - expected).abs() < 1e-9,
            "Half load should be {}, got {}",
            expected,
            half,
        );
    }

    #[test]
    fn merge_within_distance_combines_roots() {
        let mut arena = TreeSupportArena::new();

        // Two roots close together (1mm apart).
        let child_a = arena.add_node(TreeNode {
            position: (10.0, 10.0),
            z: 5.0,
            radius: 0.4,
            children: vec![],
            parent: None,
            is_contact: true,
            is_root: false,
        });

        let root_a = arena.add_node(TreeNode {
            position: (10.0, 10.0),
            z: 0.0,
            radius: 2.0,
            children: vec![child_a],
            parent: None,
            is_contact: false,
            is_root: true,
        });
        arena.nodes[child_a].parent = Some(root_a);

        let child_b = arena.add_node(TreeNode {
            position: (11.0, 10.0),
            z: 5.0,
            radius: 0.4,
            children: vec![],
            parent: None,
            is_contact: true,
            is_root: false,
        });

        let root_b = arena.add_node(TreeNode {
            position: (11.0, 10.0),
            z: 0.0,
            radius: 2.0,
            children: vec![child_b],
            parent: None,
            is_contact: false,
            is_root: true,
        });
        arena.nodes[child_b].parent = Some(root_b);

        // Merge with distance threshold of 5mm (roots are 1mm apart).
        merge_nearby_branches(&mut arena, 5.0, 10.0);

        let roots = arena.root_indices();
        assert_eq!(
            roots.len(),
            1,
            "Two nearby roots should merge into one, got {} roots",
            roots.len(),
        );

        // Merged root should be at midpoint.
        let merged = &arena.nodes[roots[0]];
        assert!(
            (merged.position.0 - 10.5).abs() < 1e-9,
            "Merged root x should be midpoint (10.5), got {}",
            merged.position.0,
        );
        assert_eq!(
            merged.children.len(),
            2,
            "Merged root should have 2 children",
        );
    }

    #[test]
    fn merge_beyond_distance_keeps_separate() {
        let mut arena = TreeSupportArena::new();

        // Two roots far apart (20mm).
        arena.add_node(TreeNode {
            position: (10.0, 10.0),
            z: 0.0,
            radius: 2.0,
            children: vec![],
            parent: None,
            is_contact: false,
            is_root: true,
        });

        arena.add_node(TreeNode {
            position: (30.0, 10.0),
            z: 0.0,
            radius: 2.0,
            children: vec![],
            parent: None,
            is_contact: false,
            is_root: true,
        });

        // Merge with distance threshold of 5mm (roots are 20mm apart).
        merge_nearby_branches(&mut arena, 5.0, 10.0);

        let roots = arena.root_indices();
        assert_eq!(
            roots.len(),
            2,
            "Distant roots should remain separate, got {} roots",
            roots.len(),
        );
    }

    #[test]
    fn branch_angle_vertical_is_zero() {
        // Child directly above parent in XY.
        let angle = compute_branch_angle((10.0, 10.0), (10.0, 10.0), 0.0, 5.0);
        assert!(
            angle.abs() < 1e-9,
            "Vertical branch should be 0 degrees, got {}",
            angle,
        );
    }

    #[test]
    fn branch_angle_45_degrees() {
        // Child offset by 5mm horizontally over 5mm vertically -> 45 degrees.
        let angle = compute_branch_angle((10.0, 10.0), (15.0, 10.0), 0.0, 5.0);
        assert!(
            (angle - 45.0).abs() < 0.1,
            "Branch with equal horizontal and vertical distance should be ~45 degrees, got {}",
            angle,
        );
    }

    #[test]
    fn branch_angle_horizontal_is_90() {
        // Child at same Z, offset horizontally -> 90 degrees.
        let angle = compute_branch_angle((10.0, 10.0), (20.0, 10.0), 5.0, 5.0);
        assert!(
            (angle - 90.0).abs() < 0.1,
            "Horizontal branch should be ~90 degrees, got {}",
            angle,
        );
    }

    #[test]
    fn merge_respects_max_diameter() {
        let mut arena = TreeSupportArena::new();

        // Two roots with large radii (4.9mm each). Max diameter is 10mm (radius 5mm).
        // Merged radius would be (4.9 + 4.9) / 2 = 4.9, which is under 5.0.
        arena.add_node(TreeNode {
            position: (10.0, 10.0),
            z: 0.0,
            radius: 4.9,
            children: vec![],
            parent: None,
            is_contact: false,
            is_root: true,
        });

        arena.add_node(TreeNode {
            position: (11.0, 10.0),
            z: 0.0,
            radius: 4.9,
            children: vec![],
            parent: None,
            is_contact: false,
            is_root: true,
        });

        // Merge with large distance but small max diameter.
        merge_nearby_branches(&mut arena, 5.0, 9.0); // max_radius = 4.5, merged would be 4.9

        let roots = arena.root_indices();
        assert_eq!(
            roots.len(),
            2,
            "Merge should be skipped when merged radius exceeds max: got {} roots",
            roots.len(),
        );
    }

    #[test]
    fn auto_taper_defaults_to_linear() {
        let base = 5.0;
        let tip = 0.4;
        let height = 10.0;

        let auto = compute_taper(base, tip, 5.0, height, TaperMethod::Auto);
        let linear = compute_taper(base, tip, 5.0, height, TaperMethod::Linear);

        assert!(
            (auto - linear).abs() < 1e-9,
            "Auto taper should equal linear: auto={}, linear={}",
            auto,
            linear,
        );
    }

    #[test]
    fn arena_basic_operations() {
        let mut arena = TreeSupportArena::new();
        assert!(arena.is_empty());
        assert_eq!(arena.len(), 0);

        let idx = arena.add_node(TreeNode {
            position: (1.0, 2.0),
            z: 3.0,
            radius: 0.5,
            children: vec![],
            parent: None,
            is_contact: false,
            is_root: true,
        });

        assert_eq!(idx, 0);
        assert_eq!(arena.len(), 1);
        assert!(!arena.is_empty());

        let node = arena.get_node(0);
        assert!((node.position.0 - 1.0).abs() < 1e-9);
        assert!(node.is_root);
        assert!(arena.children_of(0).is_empty());
    }
}
