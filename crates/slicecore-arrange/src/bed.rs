//! Bed shape parsing and utilities.
//!
//! Provides functions to parse bed shape strings (e.g., `"0x0,220x0,220x220,0x220"`)
//! into polygon boundaries, create rectangular beds from dimensions, and test
//! whether points lie within the bed boundary.

use slicecore_math::{mm_to_coord, Coord, IPoint2, COORD_SCALE};

use crate::error::ArrangeError;

/// Parses a bed shape string into a polygon boundary.
///
/// The expected format is comma-separated `XxY` pairs, where X and Y are
/// floating-point millimeter values. For example: `"0x0,220x0,220x220,0x220"`.
///
/// # Errors
///
/// Returns [`ArrangeError::InvalidBedShape`] if the string is empty, contains
/// unparseable pairs, or yields fewer than 3 points.
///
/// # Examples
///
/// ```
/// use slicecore_arrange::bed::parse_bed_shape;
///
/// let bed = parse_bed_shape("0x0,220x0,220x220,0x220").unwrap();
/// assert_eq!(bed.len(), 4);
/// ```
pub fn parse_bed_shape(bed_shape: &str) -> Result<Vec<IPoint2>, ArrangeError> {
    let trimmed = bed_shape.trim();
    if trimmed.is_empty() {
        return Err(ArrangeError::InvalidBedShape(
            "empty bed shape string".into(),
        ));
    }

    let mut points = Vec::new();
    for pair in trimmed.split(',') {
        let pair = pair.trim();
        let parts: Vec<&str> = pair.split('x').collect();
        if parts.len() != 2 {
            return Err(ArrangeError::InvalidBedShape(format!(
                "expected 'XxY' format, got '{pair}'"
            )));
        }
        let x: f64 = parts[0].trim().parse().map_err(|_| {
            ArrangeError::InvalidBedShape(format!("invalid X coordinate in '{pair}'"))
        })?;
        let y: f64 = parts[1].trim().parse().map_err(|_| {
            ArrangeError::InvalidBedShape(format!("invalid Y coordinate in '{pair}'"))
        })?;
        points.push(IPoint2::from_mm(x, y));
    }

    if points.len() < 3 {
        return Err(ArrangeError::InvalidBedShape(format!(
            "need at least 3 points, got {}",
            points.len()
        )));
    }

    Ok(points)
}

/// Creates a rectangular bed polygon from width and height dimensions.
///
/// The bed is placed with its origin at (0, 0) and extends to
/// (`bed_x`, `bed_y`). This is a fallback for when `bed_shape` is empty.
///
/// # Examples
///
/// ```
/// use slicecore_arrange::bed::bed_from_dimensions;
///
/// let bed = bed_from_dimensions(220.0, 220.0);
/// assert_eq!(bed.len(), 4);
/// ```
#[must_use]
pub fn bed_from_dimensions(bed_x: f64, bed_y: f64) -> Vec<IPoint2> {
    vec![
        IPoint2::from_mm(0.0, 0.0),
        IPoint2::from_mm(bed_x, 0.0),
        IPoint2::from_mm(bed_x, bed_y),
        IPoint2::from_mm(0.0, bed_y),
    ]
}

/// Tests whether a point lies inside or on the boundary of the bed polygon.
///
/// Returns `true` for points that are [`Inside`] or [`OnBoundary`].
///
/// [`Inside`]: slicecore_geo::PointLocation::Inside
/// [`OnBoundary`]: slicecore_geo::PointLocation::OnBoundary
#[must_use]
pub fn point_in_bed(point: &IPoint2, bed: &[IPoint2]) -> bool {
    use slicecore_geo::{point_in_polygon, PointLocation};
    matches!(
        point_in_polygon(point, bed),
        PointLocation::Inside | PointLocation::OnBoundary
    )
}

/// Computes the area of a bed polygon in square millimeters.
///
/// Uses the shoelace formula on integer coordinates, converting back
/// to mm^2 via [`COORD_SCALE`].
///
/// # Examples
///
/// ```
/// use slicecore_arrange::bed::{bed_from_dimensions, bed_area};
///
/// let bed = bed_from_dimensions(100.0, 100.0);
/// let area = bed_area(&bed);
/// assert!((area - 10000.0).abs() < 1.0);
/// ```
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "bed polygon area fits comfortably in f64 mantissa"
)]
pub fn bed_area(bed: &[IPoint2]) -> f64 {
    if bed.len() < 3 {
        return 0.0;
    }
    let mut sum: i128 = 0;
    let n = bed.len();
    for i in 0..n {
        let a = &bed[i];
        let b = &bed[(i + 1) % n];
        sum += i128::from(a.x) * i128::from(b.y) - i128::from(b.x) * i128::from(a.y);
    }
    let area_coord2 = sum.unsigned_abs();
    area_coord2 as f64 / (COORD_SCALE * COORD_SCALE * 2.0)
}

/// Computes the effective bed boundary after applying an inward margin.
///
/// Returns the bed polygon shrunk by `margin_mm` on all sides. This is
/// useful for reserving edge space. Returns the original bed if the
/// margin would collapse the polygon.
#[must_use]
pub fn bed_with_margin(bed: &[IPoint2], margin_mm: f64) -> Vec<IPoint2> {
    if margin_mm <= 0.0 || bed.len() < 3 {
        return bed.to_vec();
    }

    let delta: Coord = -mm_to_coord(margin_mm);
    // Convert bed to ValidPolygon for offset
    let polygon = slicecore_geo::Polygon::new(bed.to_vec());
    let Ok(valid) = polygon.validate() else {
        return bed.to_vec();
    };

    match slicecore_geo::offset_polygon(&valid, delta, slicecore_geo::JoinType::Miter) {
        Ok(result) if !result.is_empty() => result[0].points().to_vec(),
        _ => bed.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rectangular_bed() {
        let bed = parse_bed_shape("0x0,220x0,220x220,0x220").unwrap();
        assert_eq!(bed.len(), 4);
        assert_eq!(bed[0], IPoint2::from_mm(0.0, 0.0));
        assert_eq!(bed[1], IPoint2::from_mm(220.0, 0.0));
        assert_eq!(bed[2], IPoint2::from_mm(220.0, 220.0));
        assert_eq!(bed[3], IPoint2::from_mm(0.0, 220.0));
    }

    #[test]
    fn parse_triangular_bed() {
        let bed = parse_bed_shape("0x0,200x0,100x173").unwrap();
        assert_eq!(bed.len(), 3);
    }

    #[test]
    fn parse_empty_string_fails() {
        assert!(parse_bed_shape("").is_err());
        assert!(parse_bed_shape("  ").is_err());
    }

    #[test]
    fn parse_too_few_points_fails() {
        assert!(parse_bed_shape("0x0,100x0").is_err());
    }

    #[test]
    fn parse_invalid_format_fails() {
        assert!(parse_bed_shape("0x0,abc,100x100").is_err());
        assert!(parse_bed_shape("0x0,100,100x100").is_err());
    }

    #[test]
    fn bed_from_dimensions_rectangular() {
        let bed = bed_from_dimensions(220.0, 220.0);
        assert_eq!(bed.len(), 4);
        let area = bed_area(&bed);
        assert!(
            (area - 48400.0).abs() < 1.0,
            "Expected 220*220=48400, got {area}"
        );
    }

    #[test]
    fn point_inside_bed() {
        let bed = bed_from_dimensions(220.0, 220.0);
        assert!(point_in_bed(&IPoint2::from_mm(110.0, 110.0), &bed));
    }

    #[test]
    fn point_outside_bed() {
        let bed = bed_from_dimensions(220.0, 220.0);
        assert!(!point_in_bed(&IPoint2::from_mm(300.0, 110.0), &bed));
    }

    #[test]
    fn point_on_bed_boundary() {
        let bed = bed_from_dimensions(220.0, 220.0);
        assert!(point_in_bed(&IPoint2::from_mm(0.0, 0.0), &bed));
        assert!(point_in_bed(&IPoint2::from_mm(110.0, 0.0), &bed));
    }

    #[test]
    fn bed_area_triangle() {
        let bed = parse_bed_shape("0x0,200x0,100x100").unwrap();
        let area = bed_area(&bed);
        // Triangle with base 200 and height 100 -> area = 10000
        assert!((area - 10000.0).abs() < 1.0, "Expected 10000, got {area}");
    }

    #[test]
    fn bed_area_empty() {
        assert_eq!(bed_area(&[]), 0.0);
        assert_eq!(bed_area(&[IPoint2::from_mm(0.0, 0.0)]), 0.0);
    }

    #[test]
    fn bed_with_margin_shrinks() {
        let bed = bed_from_dimensions(220.0, 220.0);
        let margined = bed_with_margin(&bed, 5.0);
        let original_area = bed_area(&bed);
        let margined_area = bed_area(&margined);
        assert!(
            margined_area < original_area,
            "Margined area ({margined_area}) should be < original ({original_area})"
        );
        // 220x220 - margin 5mm each side = 210x210 = 44100
        assert!(
            (margined_area - 44100.0).abs() < 10.0,
            "Expected ~44100, got {margined_area}"
        );
    }
}
