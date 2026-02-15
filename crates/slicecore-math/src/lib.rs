//! Foundation math types for the slicecore 3D slicing engine.
//!
//! This crate provides the core geometric primitives used by every other crate
//! in the slicing pipeline:
//!
//! - **Integer coordinates** ([`Coord`], [`IPoint2`]) for deterministic polygon
//!   operations with nanometer precision
//! - **Floating-point points** ([`Point2`], [`Point3`]) for mesh vertices and
//!   continuous-space calculations
//! - **Vectors** ([`Vec2`], [`Vec3`]) for directions, normals, and displacements
//! - **Bounding boxes** ([`BBox2`], [`BBox3`], [`IBBox2`]) for spatial queries
//! - **Matrices** ([`Matrix3x3`], [`Matrix4x4`]) for affine transformations
//! - **Conversion utilities** ([`mm_to_coord`], [`coord_to_mm`]) for bridging
//!   float and integer coordinate spaces
//! - **Epsilon constants** ([`EPSILON`], [`AREA_EPSILON`]) for floating-point
//!   comparison
//!
//! # Coordinate System
//!
//! The engine uses two coordinate spaces:
//!
//! 1. **Float space** (millimeters): Used for mesh vertices, user-facing
//!    dimensions, and geometric calculations where floating-point is appropriate.
//!
//! 2. **Integer space** (nanometers): Used for polygon boolean operations,
//!    path planning, and any algorithm requiring deterministic arithmetic.
//!    1 mm = 1,000,000 internal units ([`COORD_SCALE`]).

pub mod bbox;
pub mod convert;
pub mod coord;
pub mod epsilon;
pub mod matrix;
pub mod point;
pub mod vec;

// Re-export core types at crate root for ergonomic imports.
pub use bbox::{BBox2, BBox3, IBBox2};
pub use convert::{coord_to_mm, ipoints_to_points, mm_to_coord, points_to_ipoints};
pub use coord::{Coord, IPoint2, COORD_SCALE};
pub use epsilon::{approx_eq, approx_zero, AREA_EPSILON, EPSILON};
pub use matrix::{Matrix3x3, Matrix4x4};
pub use point::{Point2, Point3};
pub use vec::{Vec2, Vec3};

#[cfg(test)]
mod tests {
    use super::*;

    // --- Send+Sync compile-time verification ---
    // If any type fails to be Send+Sync, this will be a compile error.
    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn all_types_are_send_sync() {
        assert_send_sync::<Coord>();
        assert_send_sync::<IPoint2>();
        assert_send_sync::<Point2>();
        assert_send_sync::<Point3>();
        assert_send_sync::<Vec2>();
        assert_send_sync::<Vec3>();
        assert_send_sync::<BBox2>();
        assert_send_sync::<BBox3>();
        assert_send_sync::<IBBox2>();
        assert_send_sync::<Matrix3x3>();
        assert_send_sync::<Matrix4x4>();
    }

    // --- Proptest strategies ---
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// mm_to_coord -> coord_to_mm round-trip preserves value within 1 nanometer (1e-6 mm).
            #[test]
            fn coord_round_trip(x in -500.0f64..500.0, y in -500.0f64..500.0) {
                let ip = IPoint2::from_mm(x, y);
                let (rx, ry) = ip.to_mm();
                prop_assert!((rx - x).abs() < 1e-6, "x: {} -> {} (diff {})", x, rx, (rx - x).abs());
                prop_assert!((ry - y).abs() < 1e-6, "y: {} -> {} (diff {})", y, ry, (ry - y).abs());
            }

            /// BBox3::from_points always contains all source points.
            #[test]
            fn bbox3_contains_all_source_points(
                xs in prop::collection::vec(-1000.0f64..1000.0, 1..50),
                ys in prop::collection::vec(-1000.0f64..1000.0, 1..50),
                zs in prop::collection::vec(-1000.0f64..1000.0, 1..50),
            ) {
                let len = xs.len().min(ys.len()).min(zs.len());
                let points: std::vec::Vec<Point3> = (0..len)
                    .map(|i| Point3::new(xs[i], ys[i], zs[i]))
                    .collect();
                if let Some(bbox) = BBox3::from_points(&points) {
                    for p in &points {
                        prop_assert!(bbox.contains_point(p),
                            "BBox3 {:?} does not contain {:?}", bbox, p);
                    }
                }
            }

            /// Vec3::normalize produces length ~1.0 for non-zero vectors.
            #[test]
            fn vec3_normalize_unit_length(
                x in -1000.0f64..1000.0,
                y in -1000.0f64..1000.0,
                z in -1000.0f64..1000.0,
            ) {
                let v = Vec3::new(x, y, z);
                if v.length() > 1e-12 {
                    let n = v.normalize();
                    let len = n.length();
                    prop_assert!((len - 1.0).abs() < 1e-9,
                        "normalized length: {} (from {:?})", len, v);
                }
            }

            /// BBox2::from_points always contains all source points.
            #[test]
            fn bbox2_contains_all_source_points(
                xs in prop::collection::vec(-1000.0f64..1000.0, 1..50),
                ys in prop::collection::vec(-1000.0f64..1000.0, 1..50),
            ) {
                let len = xs.len().min(ys.len());
                let points: std::vec::Vec<Point2> = (0..len)
                    .map(|i| Point2::new(xs[i], ys[i]))
                    .collect();
                if let Some(bbox) = BBox2::from_points(&points) {
                    for p in &points {
                        prop_assert!(bbox.contains_point(p),
                            "BBox2 {:?} does not contain {:?}", bbox, p);
                    }
                }
            }
        }
    }
}
