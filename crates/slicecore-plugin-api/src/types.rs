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
/// Coordinates use the engine's integer scale: `COORD_SCALE = 1_000_000`,
/// meaning 1 internal unit = 1 nanometer and 1 mm = 1,000,000 units.
///
/// The `i64` coordinate type provides a range of approximately +/- 9.2 x 10^12 mm,
/// which is far beyond any physical print volume.
///
/// # Example
///
/// A line from (10mm, 20mm) to (30mm, 20mm):
///
/// ```
/// use slicecore_plugin_api::FfiInfillLine;
///
/// let line = FfiInfillLine {
///     start_x: 10_000_000, // 10mm
///     start_y: 20_000_000, // 20mm
///     end_x:   30_000_000, // 30mm
///     end_y:   20_000_000, // 20mm
/// };
/// ```
#[repr(C)]
#[derive(StableAbi, Clone, Debug, PartialEq, Eq)]
pub struct FfiInfillLine {
    /// X coordinate of the line start point (in internal units, `COORD_SCALE = 1_000_000`).
    pub start_x: i64,
    /// Y coordinate of the line start point (in internal units, `COORD_SCALE = 1_000_000`).
    pub start_y: i64,
    /// X coordinate of the line end point (in internal units, `COORD_SCALE = 1_000_000`).
    pub end_x: i64,
    /// Y coordinate of the line end point (in internal units, `COORD_SCALE = 1_000_000`).
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
/// `boundary_points` contains flattened `[x0, y0, x1, y1, ...]` coordinate pairs
/// in integer coordinate space (`COORD_SCALE = 1_000_000`).
///
/// `boundary_lengths` contains the number of **points** (not coordinate values)
/// per polygon. For example, a request with two polygons of 4 and 3 vertices:
/// - `boundary_points` has 14 elements (7 points x 2 coords each)
/// - `boundary_lengths` is `[4, 3]`
///
/// Winding convention: CCW = outer boundary (positive area), CW = hole (negative area).
///
/// # Units
///
/// - `boundary_points`: integer units (`COORD_SCALE = 1_000_000` per mm)
/// - `density`: dimensionless ratio `[0.0, 1.0]`
/// - `layer_z`, `line_width`: millimeters (floating point)
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct InfillRequest {
    /// Flattened polygon boundary points as `[x0, y0, x1, y1, ...]` pairs.
    ///
    /// Coordinates are in internal integer units where `COORD_SCALE = 1_000_000`
    /// (1 mm = 1,000,000 units).
    pub boundary_points: RVec<i64>,
    /// Number of points per polygon boundary.
    ///
    /// Each entry specifies how many `(x, y)` pairs in `boundary_points` belong
    /// to that polygon. The sum of all lengths times 2 must equal the length of
    /// `boundary_points`.
    pub boundary_lengths: RVec<u32>,
    /// Fill density from 0.0 (empty) to 1.0 (solid).
    ///
    /// Controls the spacing between infill lines. A density of 0.2 means
    /// approximately 20% of the infill region will be filled with material.
    pub density: f64,
    /// Zero-based layer index within the sliced object.
    ///
    /// Can be used by patterns that vary their angle or offset per layer
    /// (e.g., alternating 0/90 degree rectilinear).
    pub layer_index: u64,
    /// Layer Z height in millimeters from the build plate.
    pub layer_z: f64,
    /// Extrusion line width in millimeters.
    ///
    /// Used to calculate infill line spacing: `spacing = line_width / density`.
    pub line_width: f64,
}

/// The FFI-safe result of infill generation.
///
/// Contains the generated infill line segments that the host will convert
/// back to internal types for toolpath assembly. The host handles E-value
/// computation, travel moves, and ordering -- plugins only need to provide
/// the geometric line segments.
///
/// An empty `lines` vector is valid and means no infill was generated
/// for the given region (e.g., when the boundary is too small).
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct InfillResult {
    /// Generated infill line segments in integer coordinate space.
    ///
    /// Each line represents an extrusion path segment. Lines do not need
    /// to be ordered or connected -- the host applies its own ordering
    /// heuristics (nearest-neighbor, etc.) during toolpath assembly.
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
