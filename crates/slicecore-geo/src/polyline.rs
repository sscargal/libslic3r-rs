//! Polyline type -- an open path of connected line segments.
//!
//! Unlike [`Polygon`](crate::Polygon), a polyline is not closed: the last
//! point does not connect back to the first. Polylines are used for travel
//! moves, seam lines, and other non-closed paths.

use serde::{Deserialize, Serialize};
use slicecore_math::{IPoint2, COORD_SCALE};

/// An open path of connected line segments in integer coordinate space.
///
/// The polyline is not closed -- the last point does not connect back to
/// the first. For closed paths, use [`Polygon`](crate::Polygon).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Polyline {
    /// The vertices of the polyline in order.
    pub points: Vec<IPoint2>,
}

impl Polyline {
    /// Creates a new empty polyline.
    pub fn new() -> Self {
        Self { points: Vec::new() }
    }

    /// Creates a polyline from a vector of points.
    pub fn from_points(points: Vec<IPoint2>) -> Self {
        Self { points }
    }

    /// Creates a polyline from millimeter coordinate pairs.
    pub fn from_mm(points: &[(f64, f64)]) -> Self {
        Self {
            points: points.iter().map(|&(x, y)| IPoint2::from_mm(x, y)).collect(),
        }
    }

    /// Returns the number of vertices.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Returns true if the polyline has no vertices.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Returns an iterator over the vertices.
    pub fn iter(&self) -> std::slice::Iter<'_, IPoint2> {
        self.points.iter()
    }

    /// Computes the total length in internal coordinate units.
    ///
    /// This is the sum of all segment lengths (Euclidean distance between
    /// consecutive points).
    pub fn length_i64(&self) -> i64 {
        if self.points.len() < 2 {
            return 0;
        }
        let mut total: f64 = 0.0;
        for i in 0..self.points.len() - 1 {
            let dx = (self.points[i + 1].x - self.points[i].x) as f64;
            let dy = (self.points[i + 1].y - self.points[i].y) as f64;
            total += (dx * dx + dy * dy).sqrt();
        }
        total.round() as i64
    }

    /// Computes the total length in millimeters.
    pub fn length_mm(&self) -> f64 {
        self.length_i64() as f64 / COORD_SCALE
    }

    /// Appends a point to the polyline.
    pub fn push(&mut self, point: IPoint2) {
        self.points.push(point);
    }
}

impl Default for Polyline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polyline_new_is_empty() {
        let pl = Polyline::new();
        assert!(pl.is_empty());
        assert_eq!(pl.len(), 0);
    }

    #[test]
    fn polyline_from_points() {
        let pl = Polyline::from_points(vec![
            IPoint2::new(0, 0),
            IPoint2::new(1_000_000, 0),
        ]);
        assert_eq!(pl.len(), 2);
    }

    #[test]
    fn polyline_from_mm() {
        let pl = Polyline::from_mm(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
        assert_eq!(pl.len(), 3);
    }

    #[test]
    fn polyline_length_straight_line() {
        // Horizontal line from (0,0) to (10mm, 0)
        let pl = Polyline::from_mm(&[(0.0, 0.0), (10.0, 0.0)]);
        let len = pl.length_mm();
        assert!(
            (len - 10.0).abs() < 1e-3,
            "Expected 10mm, got {}",
            len
        );
    }

    #[test]
    fn polyline_length_two_segments() {
        // L-shaped path: (0,0) -> (10,0) -> (10,10)
        let pl = Polyline::from_mm(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
        let len = pl.length_mm();
        assert!(
            (len - 20.0).abs() < 1e-3,
            "Expected 20mm, got {}",
            len
        );
    }

    #[test]
    fn polyline_length_empty() {
        let pl = Polyline::new();
        assert_eq!(pl.length_i64(), 0);
    }

    #[test]
    fn polyline_length_single_point() {
        let pl = Polyline::from_mm(&[(5.0, 5.0)]);
        assert_eq!(pl.length_i64(), 0);
    }

    #[test]
    fn polyline_push() {
        let mut pl = Polyline::new();
        pl.push(IPoint2::from_mm(0.0, 0.0));
        pl.push(IPoint2::from_mm(5.0, 0.0));
        assert_eq!(pl.len(), 2);
    }

    #[test]
    fn polyline_iter() {
        let pl = Polyline::from_mm(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]);
        let pts: Vec<_> = pl.iter().collect();
        assert_eq!(pts.len(), 3);
    }

    #[test]
    fn polyline_default() {
        let pl = Polyline::default();
        assert!(pl.is_empty());
    }
}
