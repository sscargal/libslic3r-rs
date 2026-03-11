//! Material-aware and height-aware multi-plate grouping.
//!
//! Groups parts by material type and similar print heights, then splits
//! groups across multiple virtual plates when they exceed bed capacity.

use std::collections::HashMap;

use slicecore_math::IPoint2;

use crate::config::ArrangeConfig;
use crate::placer::{place_parts, PreparedPart};
use crate::result::PartPlacement;

/// Groups part indices by material string.
///
/// If `multi_head` is true or material grouping is disabled, returns a
/// single group containing all indices.
///
/// # Examples
///
/// ```
/// use slicecore_arrange::config::ArrangePart;
/// use slicecore_arrange::grouper::group_by_material;
///
/// let parts = vec![
///     ArrangePart { material: Some("PLA".into()), ..Default::default() },
///     ArrangePart { material: Some("ABS".into()), ..Default::default() },
///     ArrangePart { material: Some("PLA".into()), ..Default::default() },
/// ];
/// let groups = group_by_material(&parts, false);
/// assert_eq!(groups.len(), 2);
/// ```
#[must_use]
pub fn group_by_material(
    parts: &[crate::config::ArrangePart],
    multi_head: bool,
) -> Vec<Vec<usize>> {
    if multi_head || parts.is_empty() {
        return vec![(0..parts.len()).collect()];
    }

    let mut material_map: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, part) in parts.iter().enumerate() {
        let key = part.material.clone().unwrap_or_default();
        material_map.entry(key).or_default().push(i);
    }

    let mut groups: Vec<Vec<usize>> = material_map.into_values().collect();
    // Sort for deterministic output
    groups.sort_by(|a, b| a[0].cmp(&b[0]));
    groups
}

/// Groups part indices by similar mesh height.
///
/// Sorts indices by `mesh_height` and splits into groups where the ratio of
/// tallest to shortest within a group stays below 2.0. This heuristic
/// minimizes total print time per plate.
///
/// # Examples
///
/// ```
/// use slicecore_arrange::config::ArrangePart;
/// use slicecore_arrange::grouper::group_by_height;
///
/// let parts = vec![
///     ArrangePart { mesh_height: 10.0, ..Default::default() },
///     ArrangePart { mesh_height: 50.0, ..Default::default() },
///     ArrangePart { mesh_height: 12.0, ..Default::default() },
/// ];
/// let indices = vec![0, 1, 2];
/// let groups = group_by_height(&parts, &indices);
/// // 10 and 12 are similar; 50 is different
/// assert!(groups.len() >= 2);
/// ```
#[must_use]
pub fn group_by_height(parts: &[crate::config::ArrangePart], indices: &[usize]) -> Vec<Vec<usize>> {
    if indices.is_empty() {
        return Vec::new();
    }

    let mut sorted: Vec<usize> = indices.to_vec();
    sorted.sort_by(|&a, &b| {
        parts[a]
            .mesh_height
            .partial_cmp(&parts[b].mesh_height)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current_group: Vec<usize> = vec![sorted[0]];
    let mut group_min_height = parts[sorted[0]].mesh_height;

    for &idx in &sorted[1..] {
        let height = parts[idx].mesh_height;
        // If the tallest in current group is more than 2x the shortest, start new group
        if group_min_height > 0.0 && height / group_min_height >= 2.0 {
            groups.push(std::mem::take(&mut current_group));
            group_min_height = height;
        }
        current_group.push(idx);
    }
    if !current_group.is_empty() {
        groups.push(current_group);
    }

    groups
}

/// Splits parts into multiple plates when they don't all fit on one.
///
/// Runs the placement algorithm. Any unplaced parts go to the next plate.
/// Repeats until all parts are placed or genuinely too large for any plate.
///
/// Returns a vector of placement lists, one per plate.
#[must_use]
pub fn split_into_plates(
    parts: &[PreparedPart],
    bed: &[IPoint2],
    config: &ArrangeConfig,
) -> (Vec<Vec<PartPlacement>>, Vec<String>) {
    let mut plates: Vec<Vec<PartPlacement>> = Vec::new();
    let mut remaining: Vec<PreparedPart> = parts.to_vec();
    let mut truly_unplaced: Vec<String> = Vec::new();

    while !remaining.is_empty() {
        let (placed, _unplaced_ids) = place_parts(&remaining, bed, config);

        if placed.is_empty() {
            // Nothing could be placed -- all remaining are too large
            truly_unplaced.extend(remaining.iter().map(|p| p.id.clone()));
            break;
        }

        // Update plate indices for this plate
        let plate_idx = plates.len();
        let mut plate_placements: Vec<PartPlacement> = placed;
        for p in &mut plate_placements {
            p.plate_index = plate_idx;
        }

        // Find remaining parts (those not placed)
        let placed_ids: Vec<&str> = plate_placements
            .iter()
            .map(|p| p.part_id.as_str())
            .collect();
        remaining.retain(|p| !placed_ids.contains(&p.id.as_str()));

        plates.push(plate_placements);
    }

    (plates, truly_unplaced)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ArrangePart;

    #[test]
    fn group_by_material_two_materials() {
        let parts = vec![
            ArrangePart {
                material: Some("PLA".into()),
                ..Default::default()
            },
            ArrangePart {
                material: Some("ABS".into()),
                ..Default::default()
            },
            ArrangePart {
                material: Some("PLA".into()),
                ..Default::default()
            },
        ];
        let groups = group_by_material(&parts, false);
        assert_eq!(groups.len(), 2);
        // PLA group should have indices 0 and 2
        let pla_group = groups.iter().find(|g| g.contains(&0)).unwrap();
        assert!(pla_group.contains(&2));
        // ABS group should have index 1
        let abs_group = groups.iter().find(|g| g.contains(&1)).unwrap();
        assert_eq!(abs_group.len(), 1);
    }

    #[test]
    fn group_by_material_multi_head_single_group() {
        let parts = vec![
            ArrangePart {
                material: Some("PLA".into()),
                ..Default::default()
            },
            ArrangePart {
                material: Some("ABS".into()),
                ..Default::default()
            },
        ];
        let groups = group_by_material(&parts, true);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 2);
    }

    #[test]
    fn group_by_height_clusters_similar() {
        let parts = vec![
            ArrangePart {
                mesh_height: 10.0,
                ..Default::default()
            },
            ArrangePart {
                mesh_height: 50.0,
                ..Default::default()
            },
            ArrangePart {
                mesh_height: 12.0,
                ..Default::default()
            },
            ArrangePart {
                mesh_height: 15.0,
                ..Default::default()
            },
        ];
        let indices = vec![0, 1, 2, 3];
        let groups = group_by_height(&parts, &indices);
        // 10, 12, 15 are within 2x ratio; 50 is separate
        assert!(
            groups.len() >= 2,
            "Should have at least 2 height groups, got {}",
            groups.len()
        );
    }
}
