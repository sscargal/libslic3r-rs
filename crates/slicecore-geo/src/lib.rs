//! Polygon types, boolean operations, and geometry utilities for the slicecore
//! 3D slicing engine.
//!
//! This crate provides the core geometric primitives for polygon operations
//! used throughout the slicing pipeline:
//!
//! - **Polygon types** ([`Polygon`], [`ValidPolygon`], [`Winding`]) with two-tier
//!   validation ensuring geometric invariants
//! - **Polyline** ([`Polyline`]) for open paths (travel moves, seam lines)
//! - **Area computation** ([`signed_area_i64`], [`signed_area_f64`],
//!   [`winding_direction`]) using the shoelace formula
//! - **Point-in-polygon** ([`point_in_polygon`], [`PointLocation`]) via winding
//!   number test
//! - **Simplification** ([`simplify()`]) via Ramer-Douglas-Peucker
//! - **Convex hull** ([`convex_hull()`]) via Graham scan
//! - **Boolean operations** ([`polygon_union`], [`polygon_intersection`],
//!   [`polygon_difference`], [`polygon_xor`]) via clipper2-rust
//! - **Polygon offsetting** ([`offset_polygon`], [`offset_polygons`]) for
//!   inward/outward polygon inflation/deflation
//! - **Error types** ([`GeoError`]) for validation and operation failures
//!
//! # Two-Tier Polygon System
//!
//! [`Polygon`] is an unvalidated polygon with a public `points` field for
//! easy construction and I/O. Call [`Polygon::validate`] to produce a
//! [`ValidPolygon`] with guaranteed properties (non-degenerate, known winding).
//! Downstream algorithms accept only `ValidPolygon`.

pub mod area;
pub mod boolean;
pub mod convex_hull;
pub mod error;
pub mod offset;
pub mod point_in_poly;
pub mod polygon;
pub mod polyline;
pub mod simplify;

// Re-export key types at crate root.
pub use area::{signed_area_2x, signed_area_f64, signed_area_i64, winding_direction};
pub use boolean::{polygon_difference, polygon_intersection, polygon_union, polygon_xor};
pub use convex_hull::convex_hull;
pub use error::GeoError;
pub use offset::{offset_polygon, offset_polygons, JoinType};
pub use point_in_poly::{point_in_polygon, PointLocation};
pub use polygon::{Polygon, ValidPolygon, Winding};
pub use polyline::Polyline;
pub use simplify::simplify;

#[cfg(test)]
mod tests {
    use super::*;

    // --- Send+Sync compile-time verification ---
    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn key_types_are_send_sync() {
        assert_send_sync::<Polygon>();
        assert_send_sync::<ValidPolygon>();
        assert_send_sync::<Winding>();
        assert_send_sync::<Polyline>();
        assert_send_sync::<PointLocation>();
    }
}
