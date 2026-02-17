//! Conversion utilities between internal slicecore types and FFI-safe plugin types.
//!
//! These functions bridge the gap between the engine's internal types
//! ([`ValidPolygon`], [`IPoint2`]) and the FFI-safe types used by plugins
//! ([`InfillRequest`], [`InfillResult`],
//! [`FfiInfillLine`](slicecore_plugin_api::FfiInfillLine)).

use abi_stable::std_types::RVec;

use slicecore_geo::polygon::ValidPolygon;
use slicecore_math::IPoint2;
use slicecore_plugin_api::{InfillRequest, InfillResult};

/// An infill line converted from FFI-safe format to internal coordinates.
///
/// This is a simple (start, end) pair that the engine integration layer
/// can use to construct its own infill line type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConvertedInfillLine {
    /// Start point of the infill line.
    pub start: IPoint2,
    /// End point of the infill line.
    pub end: IPoint2,
}

/// Converts internal [`ValidPolygon`] regions to an FFI-safe [`InfillRequest`].
///
/// Polygon boundaries are flattened into coordinate pairs `[x0, y0, x1, y1, ...]`
/// with a separate lengths array indicating vertex counts per polygon.
///
/// # Parameters
/// - `regions`: The boundary polygons defining the infill area.
/// - `density`: Fill density (0.0 = empty, 1.0 = solid).
/// - `layer_index`: Zero-based layer index.
/// - `layer_z`: Layer Z height in millimeters.
/// - `line_width`: Extrusion line width in millimeters.
pub fn regions_to_request(
    regions: &[ValidPolygon],
    density: f64,
    layer_index: usize,
    layer_z: f64,
    line_width: f64,
) -> InfillRequest {
    let mut boundary_points = RVec::new();
    let mut boundary_lengths = RVec::new();

    for poly in regions {
        let pts = poly.points();
        boundary_lengths.push(pts.len() as u32);
        for pt in pts {
            boundary_points.push(pt.x);
            boundary_points.push(pt.y);
        }
    }

    InfillRequest {
        boundary_points,
        boundary_lengths,
        density,
        layer_index: layer_index as u64,
        layer_z,
        line_width,
    }
}

/// Converts an FFI-safe [`InfillResult`] back to internal [`ConvertedInfillLine`] pairs.
///
/// The engine integration layer (plan 04) will convert these to its own
/// `InfillLine` type.
pub fn ffi_result_to_lines(result: &InfillResult) -> Vec<ConvertedInfillLine> {
    result
        .lines
        .iter()
        .map(|line| ConvertedInfillLine {
            start: IPoint2::new(line.start_x, line.start_y),
            end: IPoint2::new(line.end_x, line.end_y),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_geo::polygon::Polygon;
    use slicecore_plugin_api::FfiInfillLine;

    fn make_square_polygon() -> ValidPolygon {
        Polygon::from_mm(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)])
            .validate()
            .unwrap()
    }

    fn make_triangle_polygon() -> ValidPolygon {
        Polygon::from_mm(&[(0.0, 0.0), (20.0, 0.0), (10.0, 15.0)])
            .validate()
            .unwrap()
    }

    #[test]
    fn regions_to_request_single_polygon() {
        let square = make_square_polygon();
        let request = regions_to_request(&[square.clone()], 0.2, 5, 1.0, 0.4);

        assert_eq!(request.boundary_lengths.len(), 1);
        assert_eq!(request.boundary_lengths[0], square.points().len() as u32);
        assert_eq!(request.boundary_points.len(), square.points().len() * 2);
        assert_eq!(request.density, 0.2);
        assert_eq!(request.layer_index, 5);
        assert_eq!(request.layer_z, 1.0);
        assert_eq!(request.line_width, 0.4);
    }

    #[test]
    fn regions_to_request_multiple_polygons() {
        let square = make_square_polygon();
        let triangle = make_triangle_polygon();
        let request = regions_to_request(&[square.clone(), triangle.clone()], 0.5, 0, 0.2, 0.4);

        assert_eq!(request.boundary_lengths.len(), 2);
        assert_eq!(request.boundary_lengths[0], square.points().len() as u32);
        assert_eq!(request.boundary_lengths[1], triangle.points().len() as u32);

        let expected_points = (square.points().len() + triangle.points().len()) * 2;
        assert_eq!(request.boundary_points.len(), expected_points);
    }

    #[test]
    fn regions_to_request_empty_regions() {
        let request = regions_to_request(&[], 0.2, 0, 0.2, 0.4);

        assert!(request.boundary_points.is_empty());
        assert!(request.boundary_lengths.is_empty());
    }

    #[test]
    fn regions_to_request_preserves_coordinates() {
        let square = make_square_polygon();
        let pts: Vec<IPoint2> = square.points().to_vec();
        let request = regions_to_request(&[square], 0.2, 0, 0.2, 0.4);

        // Verify first point's coordinates
        assert_eq!(request.boundary_points[0], pts[0].x);
        assert_eq!(request.boundary_points[1], pts[0].y);
        // Verify second point's coordinates
        assert_eq!(request.boundary_points[2], pts[1].x);
        assert_eq!(request.boundary_points[3], pts[1].y);
    }

    #[test]
    fn ffi_result_to_lines_empty() {
        let result = InfillResult { lines: RVec::new() };
        let lines = ffi_result_to_lines(&result);
        assert!(lines.is_empty());
    }

    #[test]
    fn ffi_result_to_lines_converts_correctly() {
        let result = InfillResult {
            lines: RVec::from(vec![
                FfiInfillLine {
                    start_x: 1_000_000,
                    start_y: 2_000_000,
                    end_x: 3_000_000,
                    end_y: 4_000_000,
                },
                FfiInfillLine {
                    start_x: 5_000_000,
                    start_y: 6_000_000,
                    end_x: 7_000_000,
                    end_y: 8_000_000,
                },
            ]),
        };

        let lines = ffi_result_to_lines(&result);
        assert_eq!(lines.len(), 2);

        assert_eq!(lines[0].start, IPoint2::new(1_000_000, 2_000_000));
        assert_eq!(lines[0].end, IPoint2::new(3_000_000, 4_000_000));
        assert_eq!(lines[1].start, IPoint2::new(5_000_000, 6_000_000));
        assert_eq!(lines[1].end, IPoint2::new(7_000_000, 8_000_000));
    }

    #[test]
    fn round_trip_boundary_encoding() {
        // Verify that encoding a polygon's points into boundary_points and
        // reading them back produces the same coordinates.
        let square = make_square_polygon();
        let pts: Vec<IPoint2> = square.points().to_vec();
        let request = regions_to_request(&[square], 0.2, 0, 0.2, 0.4);

        let n = request.boundary_lengths[0] as usize;
        let mut decoded_points = Vec::new();
        for i in 0..n {
            decoded_points.push(IPoint2::new(
                request.boundary_points[i * 2],
                request.boundary_points[i * 2 + 1],
            ));
        }

        assert_eq!(decoded_points.len(), pts.len());
        for (decoded, original) in decoded_points.iter().zip(pts.iter()) {
            assert_eq!(decoded, original);
        }
    }
}
