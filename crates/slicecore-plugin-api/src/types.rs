//! FFI-safe types that cross the plugin boundary.
//!
//! These types mirror the internal infill types but use [`abi_stable`] FFI-safe
//! wrappers (`RVec`, `RString`) instead of standard library types. All types
//! derive [`StableAbi`] for load-time layout verification.
//!
//! Plugins must use these types exclusively when communicating with the host.
//! Standard `Vec<T>` and `String` must **never** cross the FFI boundary.

use abi_stable::std_types::RVec;
use abi_stable::StableAbi;

/// An FFI-safe infill line segment in integer coordinate space.
///
/// Represents a single extrusion path segment with start and end points.
/// Coordinates use the same scale as the host engine (`COORD_SCALE = 1_000_000`,
/// i.e. nanometer precision).
#[repr(C)]
#[derive(StableAbi, Clone, Debug, PartialEq, Eq)]
pub struct FfiInfillLine {
    /// X coordinate of the line start point (in internal units).
    pub start_x: i64,
    /// Y coordinate of the line start point (in internal units).
    pub start_y: i64,
    /// X coordinate of the line end point (in internal units).
    pub end_x: i64,
    /// Y coordinate of the line end point (in internal units).
    pub end_y: i64,
}

/// An FFI-safe infill generation request.
///
/// Contains the boundary polygons and parameters needed to generate infill
/// for a single layer region. Polygon boundaries are represented as flattened
/// coordinate pairs with a separate lengths array indicating vertex counts
/// per polygon.
///
/// # Boundary Encoding
///
/// `boundary_points` contains flattened `[x0, y0, x1, y1, ...]` coordinate pairs.
/// `boundary_lengths` contains the number of **points** (not coordinate values)
/// per polygon. For example, a request with two polygons of 4 and 3 vertices would have:
/// - `boundary_points` with 14 elements (7 points * 2 coords)
/// - `boundary_lengths` of `[4, 3]`
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct InfillRequest {
    /// Flattened polygon boundary points as `[x0, y0, x1, y1, ...]` pairs.
    pub boundary_points: RVec<i64>,
    /// Number of points per polygon boundary.
    pub boundary_lengths: RVec<u32>,
    /// Fill density from 0.0 (empty) to 1.0 (solid).
    pub density: f64,
    /// Zero-based layer index.
    pub layer_index: u64,
    /// Layer Z height in millimeters.
    pub layer_z: f64,
    /// Extrusion line width in millimeters.
    pub line_width: f64,
}

/// The FFI-safe result of infill generation.
///
/// Contains the generated infill line segments that the host will convert
/// back to internal types for toolpath assembly.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct InfillResult {
    /// Generated infill line segments.
    pub lines: RVec<FfiInfillLine>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use abi_stable::std_types::RVec;

    #[test]
    fn ffi_infill_line_roundtrip() {
        let line = FfiInfillLine {
            start_x: 1_000_000,
            start_y: 2_000_000,
            end_x: 3_000_000,
            end_y: 4_000_000,
        };
        let cloned = line.clone();
        assert_eq!(line, cloned);
    }

    #[test]
    fn infill_request_construction() {
        let request = InfillRequest {
            boundary_points: RVec::from(vec![0i64, 0, 100, 0, 100, 100, 0, 100]),
            boundary_lengths: RVec::from(vec![4u32]),
            density: 0.2,
            layer_index: 5,
            layer_z: 1.0,
            line_width: 0.4,
        };
        assert_eq!(request.boundary_points.len(), 8);
        assert_eq!(request.boundary_lengths.len(), 1);
        assert_eq!(request.boundary_lengths[0], 4);
    }

    #[test]
    fn infill_result_construction() {
        let result = InfillResult {
            lines: RVec::from(vec![
                FfiInfillLine {
                    start_x: 0,
                    start_y: 0,
                    end_x: 100,
                    end_y: 0,
                },
                FfiInfillLine {
                    start_x: 0,
                    start_y: 10,
                    end_x: 100,
                    end_y: 10,
                },
            ]),
        };
        assert_eq!(result.lines.len(), 2);
    }
}
