//! Sequential (by-object) printing support.
//!
//! Handles gantry clearance zone expansion, overlap validation, and
//! back-to-front print ordering for sequential printing mode.

use slicecore_math::IPoint2;

use crate::config::GantryModel;
use crate::error::ArrangeError;
use crate::footprint::{expand_footprint, footprints_overlap};
use crate::result::PartPlacement;

/// Expands a footprint polygon to account for gantry clearance.
///
/// The expansion depends on the [`GantryModel`]:
/// - [`Cylinder`](GantryModel::Cylinder): Offset polygon outward by radius
/// - [`Rectangular`](GantryModel::Rectangular): Expand bounding box by half-width/half-depth
/// - [`CustomPolygon`](GantryModel::CustomPolygon): Offset by max radius of custom polygon
/// - [`None`](GantryModel::None): Return footprint unchanged
///
/// # Examples
///
/// ```
/// use slicecore_math::IPoint2;
/// use slicecore_arrange::config::GantryModel;
/// use slicecore_arrange::sequential::expand_for_gantry;
///
/// let footprint = vec![
///     IPoint2::from_mm(0.0, 0.0),
///     IPoint2::from_mm(10.0, 0.0),
///     IPoint2::from_mm(10.0, 10.0),
///     IPoint2::from_mm(0.0, 10.0),
/// ];
/// let expanded = expand_for_gantry(&footprint, &GantryModel::Cylinder { radius: 5.0 });
/// assert!(expanded.len() >= 4);
/// ```
#[must_use]
pub fn expand_for_gantry(footprint: &[IPoint2], model: &GantryModel) -> Vec<IPoint2> {
    match model {
        GantryModel::Cylinder { radius } => {
            // Reuse expand_footprint: spacing=radius*2 gives offset of radius
            // Actually expand_footprint applies spacing/2, so pass radius*2
            expand_footprint(footprint, radius * 2.0, 0.0, 0.0)
        }
        GantryModel::Rectangular { width, depth } => {
            // Expand bounding box by half-width in X, half-depth in Y
            // Use a simple approach: expand by the larger dimension uniformly
            // since polygon offset is uniform. For a more precise approach
            // we'd need directional expansion.
            let max_expansion = (width / 2.0).max(depth / 2.0);
            expand_footprint(footprint, max_expansion * 2.0, 0.0, 0.0)
        }
        GantryModel::CustomPolygon { vertices } => {
            // Approximate: offset by the max radius of the custom polygon
            let max_radius = vertices
                .iter()
                .map(|(x, y)| (x * x + y * y).sqrt())
                .fold(0.0_f64, f64::max);
            expand_footprint(footprint, max_radius * 2.0, 0.0, 0.0)
        }
        GantryModel::None => footprint.to_vec(),
    }
}

/// Validates that gantry-expanded footprints do not overlap in sequential mode.
///
/// For each pair of placements, expands their footprints by the gantry model
/// and checks for intersection. Returns an error with the overlapping part IDs
/// if any collision is found.
///
/// # Errors
///
/// Returns [`ArrangeError::SequentialOverlap`] if any pair of expanded
/// footprints overlap.
pub fn validate_sequential(
    placements: &[PartPlacement],
    footprints: &[Vec<IPoint2>],
    gantry_model: &GantryModel,
) -> Result<(), ArrangeError> {
    let expanded: Vec<Vec<IPoint2>> = footprints
        .iter()
        .map(|fp| expand_for_gantry(fp, gantry_model))
        .collect();

    for i in 0..expanded.len() {
        for j in (i + 1)..expanded.len() {
            if footprints_overlap(&expanded[i], &expanded[j]) {
                return Err(ArrangeError::SequentialOverlap {
                    part_a: placements[i].part_id.clone(),
                    part_b: placements[j].part_id.clone(),
                });
            }
        }
    }

    Ok(())
}

/// Orders placements back-to-front for sequential printing.
///
/// Sorts by Y coordinate descending (larger Y = back of bed = printed first).
/// Assigns sequential `print_order` starting from 0.
pub fn order_back_to_front(placements: &mut [PartPlacement]) {
    placements.sort_by(|a, b| {
        b.position
            .1
            .partial_cmp(&a.position.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for (i, p) in placements.iter_mut().enumerate() {
        p.print_order = Some(i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::footprint::footprint_area;

    fn square_footprint(size: f64) -> Vec<IPoint2> {
        vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(size, 0.0),
            IPoint2::from_mm(size, size),
            IPoint2::from_mm(0.0, size),
        ]
    }

    #[test]
    fn cylinder_expansion_increases_area() {
        let fp = square_footprint(10.0);
        let expanded = expand_for_gantry(&fp, &GantryModel::Cylinder { radius: 5.0 });
        let orig_area = footprint_area(&fp);
        let exp_area = footprint_area(&expanded);
        assert!(
            exp_area > orig_area,
            "Expanded ({exp_area}) should be > original ({orig_area})"
        );
    }

    #[test]
    fn rectangular_expansion_increases_area() {
        let fp = square_footprint(10.0);
        let expanded = expand_for_gantry(
            &fp,
            &GantryModel::Rectangular {
                width: 10.0,
                depth: 8.0,
            },
        );
        let orig_area = footprint_area(&fp);
        let exp_area = footprint_area(&expanded);
        assert!(
            exp_area > orig_area,
            "Expanded ({exp_area}) should be > original ({orig_area})"
        );
    }

    #[test]
    fn none_model_no_change() {
        let fp = square_footprint(10.0);
        let expanded = expand_for_gantry(&fp, &GantryModel::None);
        assert_eq!(expanded, fp);
    }

    #[test]
    fn well_separated_parts_pass_validation() {
        let fp_a = vec![
            IPoint2::from_mm(0.0, 0.0),
            IPoint2::from_mm(10.0, 0.0),
            IPoint2::from_mm(10.0, 10.0),
            IPoint2::from_mm(0.0, 10.0),
        ];
        let fp_b = vec![
            IPoint2::from_mm(100.0, 100.0),
            IPoint2::from_mm(110.0, 100.0),
            IPoint2::from_mm(110.0, 110.0),
            IPoint2::from_mm(100.0, 110.0),
        ];
        let placements = vec![
            PartPlacement {
                part_id: "a".into(),
                position: (5.0, 5.0),
                rotation_deg: 0.0,
                orientation: None,
                plate_index: 0,
                print_order: None,
            },
            PartPlacement {
                part_id: "b".into(),
                position: (105.0, 105.0),
                rotation_deg: 0.0,
                orientation: None,
                plate_index: 0,
                print_order: None,
            },
        ];
        let footprints = vec![fp_a, fp_b];
        let result = validate_sequential(
            &placements,
            &footprints,
            &GantryModel::Cylinder { radius: 5.0 },
        );
        assert!(result.is_ok());
    }

    #[test]
    fn overlapping_parts_fail_validation() {
        let fp_a = square_footprint(10.0);
        let fp_b = vec![
            IPoint2::from_mm(8.0, 8.0),
            IPoint2::from_mm(18.0, 8.0),
            IPoint2::from_mm(18.0, 18.0),
            IPoint2::from_mm(8.0, 18.0),
        ];
        let placements = vec![
            PartPlacement {
                part_id: "a".into(),
                position: (5.0, 5.0),
                rotation_deg: 0.0,
                orientation: None,
                plate_index: 0,
                print_order: None,
            },
            PartPlacement {
                part_id: "b".into(),
                position: (13.0, 13.0),
                rotation_deg: 0.0,
                orientation: None,
                plate_index: 0,
                print_order: None,
            },
        ];
        let footprints = vec![fp_a, fp_b];
        let result = validate_sequential(
            &placements,
            &footprints,
            &GantryModel::Cylinder { radius: 5.0 },
        );
        assert!(result.is_err());
    }

    #[test]
    fn back_to_front_ordering() {
        let mut placements = vec![
            PartPlacement {
                part_id: "front".into(),
                position: (100.0, 10.0), // front (low Y)
                rotation_deg: 0.0,
                orientation: None,
                plate_index: 0,
                print_order: None,
            },
            PartPlacement {
                part_id: "back".into(),
                position: (100.0, 200.0), // back (high Y)
                rotation_deg: 0.0,
                orientation: None,
                plate_index: 0,
                print_order: None,
            },
            PartPlacement {
                part_id: "middle".into(),
                position: (100.0, 100.0), // middle
                rotation_deg: 0.0,
                orientation: None,
                plate_index: 0,
                print_order: None,
            },
        ];

        order_back_to_front(&mut placements);

        assert_eq!(placements[0].part_id, "back");
        assert_eq!(placements[0].print_order, Some(0));
        assert_eq!(placements[1].part_id, "middle");
        assert_eq!(placements[1].print_order, Some(1));
        assert_eq!(placements[2].part_id, "front");
        assert_eq!(placements[2].print_order, Some(2));
    }
}
