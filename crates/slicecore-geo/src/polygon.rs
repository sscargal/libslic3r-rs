//! Polygon types with two-tier validation.
//!
//! [`Polygon`] is an unvalidated polygon suitable for construction and I/O.
//! [`ValidPolygon`] is a validated polygon with guaranteed geometric properties:
//! at least 3 non-collinear points, non-zero area, and known winding direction.
//!
//! The validation boundary enforces invariants: downstream algorithms accept
//! only `ValidPolygon`, preventing degenerate geometry from propagating.

use serde::{Deserialize, Serialize};
use slicecore_math::{IPoint2, COORD_SCALE};

use crate::area::{cross_product_i128, signed_area_2x};
use crate::error::GeoError;

/// Winding direction of a polygon.
///
/// In the slicing engine convention:
/// - [`CounterClockwise`](Winding::CounterClockwise): outer boundary
/// - [`Clockwise`](Winding::Clockwise): hole
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Winding {
    /// Counter-clockwise winding (positive signed area). Used for outer boundaries.
    CounterClockwise,
    /// Clockwise winding (negative signed area). Used for holes.
    Clockwise,
}

/// An unvalidated polygon -- a sequence of 2D integer points forming a closed path.
///
/// The `points` field is public for easy construction. The polygon may contain
/// degenerate geometry (duplicate points, collinear edges, zero area).
/// Call [`validate`](Polygon::validate) to produce a [`ValidPolygon`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Polygon {
    /// The vertices of the polygon in order. The polygon is implicitly closed
    /// (the last point connects back to the first).
    pub points: Vec<IPoint2>,
}

impl Polygon {
    /// Creates a new polygon from a vector of integer points.
    pub fn new(points: Vec<IPoint2>) -> Self {
        Self { points }
    }

    /// Creates a polygon from millimeter coordinate pairs.
    ///
    /// Each `(x, y)` pair is converted to integer coordinates via
    /// [`IPoint2::from_mm`].
    pub fn from_mm(points: &[(f64, f64)]) -> Self {
        Self {
            points: points
                .iter()
                .map(|&(x, y)| IPoint2::from_mm(x, y))
                .collect(),
        }
    }

    /// Returns the number of vertices.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Returns true if the polygon has no vertices.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Returns an iterator over the vertices.
    pub fn iter(&self) -> std::slice::Iter<'_, IPoint2> {
        self.points.iter()
    }

    /// Reverses the vertex order in place (flips winding direction).
    pub fn reverse(&mut self) {
        self.points.reverse();
    }

    /// Validates the polygon and produces a [`ValidPolygon`].
    ///
    /// Validation steps:
    /// 1. Check at least 3 points exist
    /// 2. Remove consecutive duplicate points
    /// 3. Remove collinear points (cross product == 0)
    /// 4. Check at least 3 effective vertices remain
    /// 5. Compute signed area, reject zero area
    /// 6. Determine winding direction from signed area sign
    pub fn validate(self) -> Result<ValidPolygon, GeoError> {
        let n = self.points.len();
        if n < 3 {
            return Err(GeoError::TooFewPoints(n));
        }

        // Step 2: Remove consecutive duplicate points
        let mut cleaned: Vec<IPoint2> = Vec::with_capacity(n);
        for &p in &self.points {
            if cleaned.last().map_or(true, |last| *last != p) {
                cleaned.push(p);
            }
        }
        // Also check wrap-around: if last == first, remove last
        if cleaned.len() > 1 && cleaned.first() == cleaned.last() {
            cleaned.pop();
        }

        if cleaned.len() < 3 {
            return Err(GeoError::TooFewPoints(cleaned.len()));
        }

        // Step 3: Remove collinear points
        let mut non_collinear: Vec<IPoint2> = Vec::with_capacity(cleaned.len());
        let cn = cleaned.len();
        for i in 0..cn {
            let prev = cleaned[(i + cn - 1) % cn];
            let curr = cleaned[i];
            let next = cleaned[(i + 1) % cn];
            let cross = cross_product_i128(&prev, &curr, &next);
            if cross != 0 {
                non_collinear.push(curr);
            }
        }

        // Step 4: Check remaining count
        if non_collinear.len() < 3 {
            return Err(GeoError::AllCollinear);
        }

        // Step 5: Compute signed area
        let area_2x = signed_area_2x(&non_collinear);
        if area_2x == 0 {
            return Err(GeoError::ZeroArea);
        }

        // Step 6: Determine winding
        let winding = if area_2x > 0 {
            Winding::CounterClockwise
        } else {
            Winding::Clockwise
        };

        let area = (area_2x / 2) as i64;

        Ok(ValidPolygon {
            points: non_collinear,
            area,
            winding,
        })
    }
}

/// A validated polygon with guaranteed geometric properties.
///
/// Invariants:
/// - At least 3 non-collinear points
/// - Non-zero area
/// - Known winding direction (CCW or CW)
/// - No consecutive duplicate points
/// - No collinear points
///
/// The `points` field is private to prevent modification that could
/// violate invariants. Use [`into_polygon`](ValidPolygon::into_polygon) to
/// convert back to an unvalidated [`Polygon`] for modification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidPolygon {
    /// Vertices in order (closed polygon, last connects to first).
    points: Vec<IPoint2>,
    /// Cached signed area in internal coordinate units squared.
    /// Positive for CCW, negative for CW.
    area: i64,
    /// Cached winding direction.
    winding: Winding,
}

impl ValidPolygon {
    /// Returns the polygon vertices.
    pub fn points(&self) -> &[IPoint2] {
        &self.points
    }

    /// Returns the signed area in internal coordinate units squared.
    ///
    /// Positive for CCW, negative for CW.
    pub fn area_i64(&self) -> i64 {
        self.area
    }

    /// Returns the area in square millimeters (always positive).
    pub fn area_mm2(&self) -> f64 {
        (self.area as f64 / (COORD_SCALE * COORD_SCALE)).abs()
    }

    /// Returns the winding direction.
    pub fn winding(&self) -> Winding {
        self.winding
    }

    /// Returns the number of vertices.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Returns true if the polygon has no vertices (should never happen for ValidPolygon).
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Returns an iterator over the vertices.
    pub fn iter(&self) -> std::slice::Iter<'_, IPoint2> {
        self.points.iter()
    }

    /// Returns a new ValidPolygon with reversed winding direction.
    pub fn reversed(&self) -> ValidPolygon {
        let mut pts = self.points.clone();
        pts.reverse();
        ValidPolygon {
            points: pts,
            area: -self.area,
            winding: match self.winding {
                Winding::CounterClockwise => Winding::Clockwise,
                Winding::Clockwise => Winding::CounterClockwise,
            },
        }
    }

    /// Converts this ValidPolygon back to an unvalidated [`Polygon`].
    pub fn into_polygon(self) -> Polygon {
        Polygon {
            points: self.points,
        }
    }

    /// Returns a CCW version of this polygon.
    ///
    /// If already CCW, returns a clone. If CW, returns a reversed copy.
    pub fn ensure_ccw(&self) -> ValidPolygon {
        match self.winding {
            Winding::CounterClockwise => self.clone(),
            Winding::Clockwise => self.reversed(),
        }
    }

    /// Returns a CW version of this polygon.
    ///
    /// If already CW, returns a clone. If CCW, returns a reversed copy.
    pub fn ensure_cw(&self) -> ValidPolygon {
        match self.winding {
            Winding::Clockwise => self.clone(),
            Winding::CounterClockwise => self.reversed(),
        }
    }

    /// Constructs a `ValidPolygon` from raw parts without validation.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - `points` has at least 3 non-collinear points
    /// - `area` matches the actual signed area of the polygon
    /// - `winding` matches the sign of `area`
    ///
    /// This is used internally by boolean operations and offset functions
    /// where the library guarantees the output is valid.
    pub(crate) fn from_raw_parts(points: Vec<IPoint2>, area: i64, winding: Winding) -> Self {
        Self {
            points,
            area,
            winding,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ccw_square_10mm() -> Polygon {
        Polygon::from_mm(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)])
    }

    fn cw_square_10mm() -> Polygon {
        Polygon::from_mm(&[(0.0, 10.0), (10.0, 10.0), (10.0, 0.0), (0.0, 0.0)])
    }

    #[test]
    fn polygon_new_and_len() {
        let p = Polygon::new(vec![
            IPoint2::new(0, 0),
            IPoint2::new(1, 0),
            IPoint2::new(0, 1),
        ]);
        assert_eq!(p.len(), 3);
        assert!(!p.is_empty());
    }

    #[test]
    fn polygon_from_mm() {
        let p = Polygon::from_mm(&[(1.0, 2.0), (3.0, 4.0)]);
        assert_eq!(p.len(), 2);
        assert_eq!(p.points[0], IPoint2::from_mm(1.0, 2.0));
    }

    #[test]
    fn polygon_reverse() {
        let mut p = Polygon::from_mm(&[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]);
        p.reverse();
        assert_eq!(p.points[0], IPoint2::from_mm(0.0, 1.0));
        assert_eq!(p.points[2], IPoint2::from_mm(0.0, 0.0));
    }

    #[test]
    fn validate_ccw_square() {
        let vp = ccw_square_10mm().validate().unwrap();
        assert_eq!(vp.winding(), Winding::CounterClockwise);
        assert!(vp.area_i64() > 0);
        let area_mm2 = vp.area_mm2();
        assert!(
            (area_mm2 - 100.0).abs() < 1e-3,
            "Expected ~100 mm^2, got {}",
            area_mm2
        );
    }

    #[test]
    fn validate_cw_square() {
        let vp = cw_square_10mm().validate().unwrap();
        assert_eq!(vp.winding(), Winding::Clockwise);
        assert!(vp.area_i64() < 0);
    }

    #[test]
    fn validate_too_few_points() {
        let p = Polygon::new(vec![IPoint2::new(0, 0), IPoint2::new(1, 0)]);
        match p.validate() {
            Err(GeoError::TooFewPoints(2)) => {} // expected
            other => panic!("Expected TooFewPoints(2), got {:?}", other),
        }
    }

    #[test]
    fn validate_zero_points() {
        let p = Polygon::new(vec![]);
        match p.validate() {
            Err(GeoError::TooFewPoints(0)) => {}
            other => panic!("Expected TooFewPoints(0), got {:?}", other),
        }
    }

    #[test]
    fn validate_collinear_points() {
        let p = Polygon::from_mm(&[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)]);
        match p.validate() {
            Err(GeoError::AllCollinear) => {}
            other => panic!("Expected AllCollinear, got {:?}", other),
        }
    }

    #[test]
    fn validate_removes_duplicate_consecutive() {
        // Square with duplicate vertex -- should still validate
        let p = Polygon::from_mm(&[
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 0.0), // duplicate
            (10.0, 10.0),
            (0.0, 10.0),
        ]);
        let vp = p.validate().unwrap();
        assert_eq!(vp.len(), 4);
    }

    #[test]
    fn validate_removes_collinear() {
        // Square with extra collinear point on one edge
        let p = Polygon::from_mm(&[
            (0.0, 0.0),
            (5.0, 0.0), // collinear with (0,0)-(10,0)
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
        ]);
        let vp = p.validate().unwrap();
        assert_eq!(vp.len(), 4, "Should have 4 non-collinear vertices");
    }

    #[test]
    fn validate_all_duplicate_points() {
        let p = Polygon::new(vec![IPoint2::new(5, 5); 10]);
        assert!(p.validate().is_err());
    }

    #[test]
    fn valid_polygon_reversed() {
        let vp = ccw_square_10mm().validate().unwrap();
        assert_eq!(vp.winding(), Winding::CounterClockwise);

        let reversed = vp.reversed();
        assert_eq!(reversed.winding(), Winding::Clockwise);
        assert_eq!(reversed.area_i64(), -vp.area_i64());
    }

    #[test]
    fn valid_polygon_ensure_ccw() {
        let vp = cw_square_10mm().validate().unwrap();
        assert_eq!(vp.winding(), Winding::Clockwise);

        let ccw = vp.ensure_ccw();
        assert_eq!(ccw.winding(), Winding::CounterClockwise);
    }

    #[test]
    fn valid_polygon_ensure_cw() {
        let vp = ccw_square_10mm().validate().unwrap();
        assert_eq!(vp.winding(), Winding::CounterClockwise);

        let cw = vp.ensure_cw();
        assert_eq!(cw.winding(), Winding::Clockwise);
    }

    #[test]
    fn valid_polygon_into_polygon_and_back() {
        let vp = ccw_square_10mm().validate().unwrap();
        let original_area = vp.area_mm2();
        let p = vp.into_polygon();
        let vp2 = p.validate().unwrap();
        assert!((vp2.area_mm2() - original_area).abs() < 1e-6);
    }

    #[test]
    fn valid_polygon_iter() {
        let vp = ccw_square_10mm().validate().unwrap();
        let pts: Vec<_> = vp.iter().collect();
        assert_eq!(pts.len(), 4);
    }

    #[test]
    fn validate_removes_wrap_around_duplicate() {
        // First and last points are the same
        let p = Polygon::from_mm(&[
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
            (0.0, 0.0), // duplicate of first
        ]);
        let vp = p.validate().unwrap();
        assert_eq!(vp.len(), 4);
    }

    #[test]
    fn winding_enum_serde() {
        let ccw = Winding::CounterClockwise;
        let json = serde_json::to_string(&ccw).unwrap();
        let deserialized: Winding = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Winding::CounterClockwise);
    }
}
