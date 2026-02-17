//! Polyhole conversion for improved dimensional accuracy of circular holes.
//!
//! Circular holes printed on FDM printers tend to be undersized because the
//! extruded plastic rounds inward at corners. Replacing circular holes with
//! regular polygons (polyholes) produces more dimensionally accurate results.
//!
//! This implementation uses the **Nophead formula** (popularized by OrcaSlicer)
//! to compute the optimal number of polygon sides based on the hole diameter
//! and nozzle diameter.
//!
//! # Pipeline Integration
//!
//! When `polyhole_enabled` is true in [`PrintConfig`], the engine calls
//! [`convert_polyholes`] on each layer's contours after slicing and before
//! perimeter generation. Only hole contours (CW winding) are modified;
//! outer boundaries (CCW) are left unchanged.

use std::f64::consts::PI;

use slicecore_geo::polygon::{ValidPolygon, Winding};
use slicecore_geo::Polygon;
use slicecore_math::IPoint2;

/// Computes the optimal number of sides for a polyhole using the Nophead formula.
///
/// The formula ensures that the inscribed circle of the resulting regular polygon
/// matches the desired hole diameter, compensating for FDM corner rounding.
///
/// # Parameters
///
/// - `hole_diameter`: The desired hole diameter in mm.
/// - `nozzle_diameter`: The nozzle diameter in mm.
///
/// # Returns
///
/// The number of polygon sides (minimum 3).
pub fn polyhole_sides(hole_diameter: f64, nozzle_diameter: f64) -> u32 {
    if hole_diameter <= 0.0 || nozzle_diameter <= 0.0 {
        return 3;
    }
    let ratio = nozzle_diameter / hole_diameter;
    if ratio >= 1.0 {
        return 3;
    }
    let sides = PI / (1.0 - ratio).acos();
    (sides.ceil() as u32).max(3)
}

/// Computes the circumradius of a regular polygon whose inscribed circle has
/// the desired radius.
///
/// For a regular n-gon, the inscribed circle radius `r` relates to the
/// circumradius `R` by: `r = R * cos(PI/n)`. Therefore: `R = r / cos(PI/n)`.
///
/// # Parameters
///
/// - `desired_diameter`: The desired inscribed circle diameter in mm.
/// - `sides`: The number of polygon sides.
///
/// # Returns
///
/// The circumradius in mm that produces the desired inscribed circle diameter.
pub fn polyhole_radius(desired_diameter: f64, sides: u32) -> f64 {
    let desired_radius = desired_diameter / 2.0;
    let n = sides as f64;
    desired_radius / (PI / n).cos()
}

/// Checks if a polygon represents a circular hole.
///
/// A polygon is considered circular if:
/// 1. It has CW winding (it's a hole, not an outer boundary).
/// 2. All vertices are within a tolerance of the mean radius from the centroid.
/// 3. The fitted diameter is at least `min_diameter`.
///
/// # Parameters
///
/// - `polygon`: The polygon to check.
/// - `min_diameter`: Minimum hole diameter in mm (skip tiny holes).
///
/// # Returns
///
/// `Some(((cx, cy), diameter))` if the polygon is a circular hole, `None` otherwise.
pub fn is_circular_hole(
    polygon: &ValidPolygon,
    min_diameter: f64,
) -> Option<((f64, f64), f64)> {
    // Only consider holes (CW winding).
    if polygon.winding() != Winding::Clockwise {
        return None;
    }

    let points = polygon.points();
    // A circular hole needs at least 8 vertices to be a reasonable circle
    // approximation. Fewer vertices (3=triangle, 4=square, etc.) are clearly
    // not circles.
    if points.len() < 8 {
        return None;
    }

    // Compute centroid in mm.
    let n = points.len() as f64;
    let (cx, cy) = points.iter().fold((0.0, 0.0), |(ax, ay), p| {
        let (px, py) = p.to_mm();
        (ax + px, ay + py)
    });
    let cx = cx / n;
    let cy = cy / n;

    // Compute distances from centroid and mean radius.
    let radii: Vec<f64> = points
        .iter()
        .map(|p| {
            let (px, py) = p.to_mm();
            let dx = px - cx;
            let dy = py - cy;
            (dx * dx + dy * dy).sqrt()
        })
        .collect();

    let mean_radius = radii.iter().sum::<f64>() / n;
    if mean_radius < 1e-9 {
        return None;
    }

    // Check that all vertices are within 10% tolerance of mean radius.
    let tolerance = mean_radius * 0.1;
    let is_circular = radii.iter().all(|r| (r - mean_radius).abs() <= tolerance);

    if !is_circular {
        return None;
    }

    let diameter = mean_radius * 2.0;
    if diameter < min_diameter {
        return None;
    }

    Some(((cx, cy), diameter))
}

/// Creates a regular polygon (polyhole) with CW winding to replace a circular hole.
///
/// # Parameters
///
/// - `center`: The center of the hole in mm `(cx, cy)`.
/// - `diameter`: The desired hole diameter in mm.
/// - `nozzle_diameter`: The nozzle diameter in mm.
///
/// # Returns
///
/// A `ValidPolygon` with CW winding (hole convention) and the optimal number
/// of sides for dimensional accuracy.
pub fn convert_to_polyhole(
    center: (f64, f64),
    diameter: f64,
    nozzle_diameter: f64,
) -> ValidPolygon {
    let sides = polyhole_sides(diameter, nozzle_diameter);
    let circumradius = polyhole_radius(diameter, sides);

    // Generate vertices in CW order (clockwise for holes).
    // CW means we traverse the angle in the negative direction.
    let points: Vec<IPoint2> = (0..sides)
        .map(|i| {
            let angle = -2.0 * PI * (i as f64) / (sides as f64);
            let x = center.0 + circumradius * angle.cos();
            let y = center.1 + circumradius * angle.sin();
            IPoint2::from_mm(x, y)
        })
        .collect();

    // Validate to get proper winding and area caching.
    let poly = Polygon::new(points);
    match poly.validate() {
        Ok(vp) => {
            // Ensure CW winding.
            vp.ensure_cw()
        }
        Err(_) => {
            // Fallback: should not happen for a valid regular polygon,
            // but if it does, create a minimal triangle.
            let r = circumradius;
            let fallback = Polygon::from_mm(&[
                (center.0 + r, center.1),
                (center.0 - r * 0.5, center.1 + r * 0.866),
                (center.0 - r * 0.5, center.1 - r * 0.866),
            ]);
            fallback.validate().unwrap().ensure_cw()
        }
    }
}

/// Converts circular holes in a contour list to polyholes.
///
/// Iterates through contours, identifies circular holes (CW winding, circular
/// shape), and replaces them with regular polygon polyholes. Outer contours
/// (CCW winding) are left unchanged.
///
/// # Parameters
///
/// - `contours`: Mutable reference to the contour list.
/// - `nozzle_diameter`: Nozzle diameter in mm for side count computation.
/// - `min_diameter`: Minimum hole diameter to convert (skip very small holes).
pub fn convert_polyholes(
    contours: &mut [ValidPolygon],
    nozzle_diameter: f64,
    min_diameter: f64,
) {
    for contour in contours.iter_mut() {
        if contour.winding() != Winding::Clockwise {
            continue; // Skip outer boundaries (CCW).
        }

        if let Some((center, diameter)) = is_circular_hole(contour, min_diameter) {
            let polyhole = convert_to_polyhole(center, diameter, nozzle_diameter);
            *contour = polyhole;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polyhole_sides_5mm_hole_04_nozzle() {
        let sides = polyhole_sides(5.0, 0.4);
        // Expected: PI / acos(1.0 - 0.4/5.0) = PI / acos(0.92) ~ 7.8 -> 8
        // Typical range around 8-12 depending on formula interpretation.
        assert!(
            sides >= 7 && sides <= 15,
            "5mm hole with 0.4mm nozzle should produce reasonable side count, got {}",
            sides
        );
    }

    #[test]
    fn polyhole_sides_1mm_hole_04_nozzle() {
        let sides = polyhole_sides(1.0, 0.4);
        // Ratio = 0.4, acos(0.6) ~ 0.927, PI / 0.927 ~ 3.39 -> 4
        assert!(
            sides >= 3 && sides <= 8,
            "1mm hole should produce fewer sides, got {}",
            sides
        );
    }

    #[test]
    fn polyhole_sides_very_small_returns_minimum() {
        let sides = polyhole_sides(0.3, 0.4);
        // Ratio = 0.4/0.3 > 1.0, should return minimum 3.
        assert_eq!(sides, 3, "Very small hole should return minimum 3 sides");
    }

    #[test]
    fn polyhole_sides_zero_diameter_returns_minimum() {
        assert_eq!(polyhole_sides(0.0, 0.4), 3);
    }

    #[test]
    fn polyhole_radius_produces_correct_inscribed_diameter() {
        // For a hexagon (6 sides), cos(PI/6) = sqrt(3)/2 ~ 0.866.
        // If desired diameter = 10mm, desired radius = 5mm.
        // Circumradius R = 5 / cos(PI/6) = 5 / 0.866 ~ 5.774.
        // Inscribed circle diameter of hexagon with R=5.774: 2 * R * cos(PI/6) = 2 * 5.774 * 0.866 = 10.0.
        let sides = 6;
        let desired_diameter = 10.0;
        let r = polyhole_radius(desired_diameter, sides);

        let inscribed_diameter = 2.0 * r * (PI / sides as f64).cos();
        assert!(
            (inscribed_diameter - desired_diameter).abs() < 1e-9,
            "Inscribed diameter should match desired: expected {}, got {}",
            desired_diameter,
            inscribed_diameter
        );
    }

    #[test]
    fn is_circular_hole_identifies_circle() {
        // Create a 16-vertex circle hole (CW winding) of diameter 5mm.
        let n = 16;
        let radius = 2.5; // 5mm diameter
        let center = (50.0, 50.0);
        let points: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                // CW direction (negative angle)
                let angle = -2.0 * PI * (i as f64) / (n as f64);
                (center.0 + radius * angle.cos(), center.1 + radius * angle.sin())
            })
            .collect();

        let poly = Polygon::from_mm(&points).validate().unwrap();
        assert_eq!(poly.winding(), Winding::Clockwise, "Should be CW hole");

        let result = is_circular_hole(&poly, 1.0);
        assert!(result.is_some(), "16-vertex circle should be identified as circular");
        let ((cx, cy), diam) = result.unwrap();
        assert!(
            (cx - center.0).abs() < 0.1,
            "Center X should be ~{}, got {}",
            center.0,
            cx
        );
        assert!(
            (cy - center.1).abs() < 0.1,
            "Center Y should be ~{}, got {}",
            center.1,
            cy
        );
        assert!(
            (diam - 5.0).abs() < 0.1,
            "Diameter should be ~5.0, got {}",
            diam
        );
    }

    #[test]
    fn is_circular_hole_rejects_square() {
        // A CW square is not circular.
        let square = Polygon::from_mm(&[
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
            (0.0, 0.0),
        ])
        .validate()
        .unwrap()
        .ensure_cw();

        let result = is_circular_hole(&square, 1.0);
        assert!(
            result.is_none(),
            "Square should not be identified as circular"
        );
    }

    #[test]
    fn is_circular_hole_rejects_ccw() {
        // CCW polygon (outer boundary) should not be considered a hole.
        let n = 16;
        let radius = 2.5;
        let points: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let angle = 2.0 * PI * (i as f64) / (n as f64);
                (50.0 + radius * angle.cos(), 50.0 + radius * angle.sin())
            })
            .collect();

        let poly = Polygon::from_mm(&points).validate().unwrap();
        assert_eq!(poly.winding(), Winding::CounterClockwise);

        let result = is_circular_hole(&poly, 1.0);
        assert!(result.is_none(), "CCW polygon should not be identified as hole");
    }

    #[test]
    fn convert_to_polyhole_correct_vertex_count_and_winding() {
        let sides = polyhole_sides(5.0, 0.4);
        let polyhole = convert_to_polyhole((50.0, 50.0), 5.0, 0.4);

        assert_eq!(
            polyhole.winding(),
            Winding::Clockwise,
            "Polyhole should have CW winding (hole)"
        );
        assert_eq!(
            polyhole.len(),
            sides as usize,
            "Polyhole should have {} vertices, got {}",
            sides,
            polyhole.len()
        );
    }

    #[test]
    fn convert_polyholes_replaces_circular_leaves_others() {
        // Create contours: one CCW outer, one CW circular hole, one CW square hole.
        let outer = Polygon::from_mm(&[
            (0.0, 0.0),
            (100.0, 0.0),
            (100.0, 100.0),
            (0.0, 100.0),
        ])
        .validate()
        .unwrap();
        assert_eq!(outer.winding(), Winding::CounterClockwise);

        // Circular hole: 16 vertices, 5mm diameter at center (50, 50).
        let n = 16;
        let radius = 2.5;
        let circle_points: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let angle = -2.0 * PI * (i as f64) / (n as f64);
                (50.0 + radius * angle.cos(), 50.0 + radius * angle.sin())
            })
            .collect();
        let circle_hole = Polygon::from_mm(&circle_points).validate().unwrap();
        assert_eq!(circle_hole.winding(), Winding::Clockwise);

        // Square hole: not circular.
        let square_hole = Polygon::from_mm(&[
            (80.0, 10.0),
            (80.0, 20.0),
            (70.0, 20.0),
            (70.0, 10.0),
        ])
        .validate()
        .unwrap()
        .ensure_cw();

        let original_circle_len = circle_hole.len();
        let original_square_len = square_hole.len();

        let mut contours = vec![outer.clone(), circle_hole, square_hole];
        convert_polyholes(&mut contours, 0.4, 1.0);

        // Outer boundary should be unchanged.
        assert_eq!(contours[0].winding(), Winding::CounterClockwise);
        assert_eq!(contours[0].len(), outer.len());

        // Circular hole should be replaced (different vertex count).
        assert_eq!(contours[1].winding(), Winding::Clockwise);
        assert_ne!(
            contours[1].len(),
            original_circle_len,
            "Circular hole should have been replaced with different vertex count"
        );

        // Square hole should be unchanged.
        assert_eq!(contours[2].winding(), Winding::Clockwise);
        assert_eq!(contours[2].len(), original_square_len);
    }

    #[test]
    fn is_circular_hole_respects_min_diameter() {
        // Create a very small circular hole (0.5mm diameter).
        let n = 16;
        let radius = 0.25;
        let points: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let angle = -2.0 * PI * (i as f64) / (n as f64);
                (50.0 + radius * angle.cos(), 50.0 + radius * angle.sin())
            })
            .collect();

        let poly = Polygon::from_mm(&points).validate().unwrap();
        assert_eq!(poly.winding(), Winding::Clockwise);

        // With min_diameter=1.0, this 0.5mm hole should be skipped.
        let result = is_circular_hole(&poly, 1.0);
        assert!(result.is_none(), "Hole below min_diameter should be rejected");

        // With min_diameter=0.1, it should be detected.
        let result = is_circular_hole(&poly, 0.1);
        assert!(result.is_some(), "Hole above min_diameter should be detected");
    }
}
