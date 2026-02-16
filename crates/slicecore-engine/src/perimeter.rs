//! Perimeter shell generation via polygon offsetting.
//!
//! Perimeters define the walls of a printed object. Given slice contour
//! polygons and a [`PrintConfig`], this module generates inward-offset shells
//! (perimeter walls) and computes the innermost boundary for infill.
//!
//! The algorithm uses [`slicecore_geo::offset_polygons`] with
//! [`JoinType::Miter`] for crisp corners, offsetting all contours together
//! at each level so that adjacent boundaries interact correctly.

use slicecore_geo::offset::{offset_polygons, JoinType};
use slicecore_geo::polygon::ValidPolygon;
use slicecore_math::mm_to_coord;

use crate::config::{PrintConfig, WallOrder};

/// A single perimeter shell (one closed polygon path or set of paths).
#[derive(Clone, Debug)]
pub struct PerimeterShell {
    /// The offset contour(s) for this shell.
    pub polygons: Vec<ValidPolygon>,
    /// True for the outermost (visible) shell.
    pub is_outer: bool,
}

/// Perimeters for a set of contours (processed together for proper interaction).
#[derive(Clone, Debug)]
pub struct ContourPerimeters {
    /// Ordered per print order (respects `wall_order` in config).
    pub shells: Vec<PerimeterShell>,
    /// Innermost offset result = infill boundary.
    pub inner_contour: Vec<ValidPolygon>,
}

/// Generates perimeter shells from slice contour polygons.
///
/// For each offset level, all contours are offset together so adjacent
/// boundaries interact correctly (via `offset_polygons`).
///
/// - First shell offset = half extrusion width inward from the contour.
/// - Subsequent shells offset = full extrusion width from the previous shell.
/// - If an offset produces empty result, stop (polygon collapsed).
/// - The innermost shell result is the infill boundary.
///
/// Wall ordering is applied per `config.wall_order`:
/// - `OuterFirst`: outermost shell prints first (index 0).
/// - `InnerFirst`: innermost shell prints first (index 0), outer is last.
pub fn generate_perimeters(
    contours: &[ValidPolygon],
    config: &PrintConfig,
) -> Vec<ContourPerimeters> {
    if contours.is_empty() {
        return Vec::new();
    }

    let wall_count = config.wall_count as usize;
    let extrusion_width = config.extrusion_width();
    let half_width = extrusion_width / 2.0;

    // Generate shells by repeatedly offsetting inward.
    // All contours are processed together at each level.
    let mut shells_outside_in: Vec<PerimeterShell> = Vec::with_capacity(wall_count);
    let mut current_contours: Vec<ValidPolygon> = contours.to_vec();

    for i in 0..wall_count {
        // First shell: offset by half width (centers the extrusion on the contour edge).
        // Subsequent shells: offset by full width from previous shell.
        let offset_mm = if i == 0 { half_width } else { extrusion_width };
        let delta = mm_to_coord(-offset_mm); // negative = inward

        let offset_result = match offset_polygons(&current_contours, delta, JoinType::Miter) {
            Ok(result) => result,
            Err(_) => break, // offset failed, stop
        };

        if offset_result.is_empty() {
            break; // polygon collapsed, no more shells fit
        }

        shells_outside_in.push(PerimeterShell {
            polygons: offset_result.clone(),
            is_outer: false, // will be set below
        });

        current_contours = offset_result;
    }

    // Compute inner_contour: offset one more full width inward from the last shell.
    // This gives the infill boundary.
    let inner_contour = if shells_outside_in.is_empty() {
        // No shells were generated (wall_count=0 or collapsed immediately).
        // Offset by half width to get the infill boundary.
        offset_polygons(contours, mm_to_coord(-half_width), JoinType::Miter)
            .unwrap_or_default()
    } else {
        // Offset one more full width inward from the last shell.
        offset_polygons(
            &current_contours,
            mm_to_coord(-extrusion_width / 2.0),
            JoinType::Miter,
        )
        .unwrap_or_default()
    };

    // Mark the outermost shell.
    if !shells_outside_in.is_empty() {
        shells_outside_in[0].is_outer = true;
    }

    // Apply wall ordering.
    let shells = match config.wall_order {
        WallOrder::OuterFirst => {
            // Already in outside-in order: outer is first.
            shells_outside_in
        }
        WallOrder::InnerFirst => {
            // Reverse so inner is first, outer is last.
            shells_outside_in.into_iter().rev().collect()
        }
    };

    vec![ContourPerimeters {
        shells,
        inner_contour,
    }]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_geo::polygon::Polygon;

    /// Helper to create a validated CCW square at the origin with given size (mm).
    fn make_square(size: f64) -> ValidPolygon {
        Polygon::from_mm(&[
            (0.0, 0.0),
            (size, 0.0),
            (size, size),
            (0.0, size),
        ])
        .validate()
        .unwrap()
    }

    fn total_area_mm2(polys: &[ValidPolygon]) -> f64 {
        polys.iter().map(|p| p.area_mm2()).sum()
    }

    #[test]
    fn two_shells_from_20mm_square() {
        let square = make_square(20.0);
        let config = PrintConfig {
            wall_count: 2,
            ..Default::default()
        };

        let result = generate_perimeters(&[square], &config);
        assert_eq!(result.len(), 1, "Should produce one ContourPerimeters");

        let perimeters = &result[0];
        assert_eq!(
            perimeters.shells.len(),
            2,
            "20mm square with wall_count=2 should produce 2 shells"
        );

        // Each shell should have at least 1 polygon.
        for (i, shell) in perimeters.shells.iter().enumerate() {
            assert!(
                !shell.polygons.is_empty(),
                "Shell {} should have at least 1 polygon",
                i
            );
        }
    }

    #[test]
    fn outer_first_ordering() {
        let square = make_square(20.0);
        let config = PrintConfig {
            wall_count: 2,
            wall_order: WallOrder::OuterFirst,
            ..Default::default()
        };

        let result = generate_perimeters(&[square], &config);
        let perimeters = &result[0];
        assert!(
            perimeters.shells[0].is_outer,
            "OuterFirst: shells[0].is_outer should be true"
        );
        assert!(
            !perimeters.shells[1].is_outer,
            "OuterFirst: shells[1].is_outer should be false"
        );
    }

    #[test]
    fn inner_first_ordering() {
        let square = make_square(20.0);
        let config = PrintConfig {
            wall_count: 2,
            wall_order: WallOrder::InnerFirst,
            ..Default::default()
        };

        let result = generate_perimeters(&[square], &config);
        let perimeters = &result[0];

        // InnerFirst: outer shell is last.
        let last = perimeters.shells.last().unwrap();
        assert!(
            last.is_outer,
            "InnerFirst: last shell should be the outer shell"
        );

        // First shell should not be outer.
        assert!(
            !perimeters.shells[0].is_outer,
            "InnerFirst: first shell should not be outer"
        );
    }

    #[test]
    fn inner_contour_is_smaller() {
        let square = make_square(20.0);
        let original_area = square.area_mm2();
        let config = PrintConfig {
            wall_count: 2,
            ..Default::default()
        };

        let result = generate_perimeters(&[square], &config);
        let inner_area = total_area_mm2(&result[0].inner_contour);
        assert!(
            inner_area < original_area,
            "Inner contour area ({}) should be smaller than original ({})",
            inner_area,
            original_area
        );
        assert!(
            inner_area > 0.0,
            "Inner contour should have positive area for a 20mm square"
        );
    }

    #[test]
    fn wall_count_zero_no_shells_but_has_inner_contour() {
        let square = make_square(20.0);
        let config = PrintConfig {
            wall_count: 0,
            ..Default::default()
        };

        let result = generate_perimeters(&[square], &config);
        assert_eq!(result.len(), 1);
        assert!(
            result[0].shells.is_empty(),
            "wall_count=0 should produce no shells"
        );
        // inner_contour should still exist (offset by half-width).
        assert!(
            !result[0].inner_contour.is_empty(),
            "wall_count=0 should still compute inner_contour"
        );
    }

    #[test]
    fn small_polygon_collapses_early() {
        // 2mm square with nozzle_diameter=0.4, extrusion_width=0.44mm
        // Half width = 0.22mm, full width = 0.44mm
        // Shell 1: offset -0.22mm -> 1.56mm square
        // Shell 2: offset -0.44mm -> 0.68mm square
        // Shell 3: offset -0.44mm -> ~-0.2mm -> collapses
        // So requesting 5 shells should stop early.
        let square = make_square(2.0);
        let config = PrintConfig {
            wall_count: 5,
            ..Default::default()
        };

        let result = generate_perimeters(&[square], &config);
        let perimeters = &result[0];
        assert!(
            perimeters.shells.len() < 5,
            "2mm square with wall_count=5 should stop early (got {} shells)",
            perimeters.shells.len()
        );
        assert!(
            !perimeters.shells.is_empty(),
            "2mm square should still produce at least 1 shell"
        );
    }

    #[test]
    fn empty_contours_returns_empty() {
        let config = PrintConfig::default();
        let result = generate_perimeters(&[], &config);
        assert!(result.is_empty());
    }

    #[test]
    fn shells_are_nested_decreasing_area() {
        let square = make_square(20.0);
        let config = PrintConfig {
            wall_count: 3,
            wall_order: WallOrder::OuterFirst,
            ..Default::default()
        };

        let result = generate_perimeters(&[square], &config);
        let perimeters = &result[0];

        // In OuterFirst order, areas should decrease from shell 0 to shell N.
        let areas: Vec<f64> = perimeters
            .shells
            .iter()
            .map(|s| total_area_mm2(&s.polygons))
            .collect();

        for i in 1..areas.len() {
            assert!(
                areas[i] < areas[i - 1],
                "Shell {} area ({}) should be smaller than shell {} area ({})",
                i,
                areas[i],
                i - 1,
                areas[i - 1]
            );
        }
    }
}
