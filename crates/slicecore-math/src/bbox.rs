//! Axis-aligned bounding box types for 2D and 3D spaces.
//!
//! Bounding boxes are used for spatial queries, collision detection,
//! viewport culling, and quick containment tests throughout the slicing
//! pipeline.

use serde::{Deserialize, Serialize};

use crate::coord::IPoint2;
use crate::point::{Point2, Point3};

/// A 2D axis-aligned bounding box in floating-point space.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BBox2 {
    pub min: Point2,
    pub max: Point2,
}

impl PartialEq for BBox2 {
    fn eq(&self, other: &Self) -> bool {
        self.min == other.min && self.max == other.max
    }
}

impl BBox2 {
    /// Creates a bounding box from explicit min and max corners.
    #[inline]
    pub fn new(min: Point2, max: Point2) -> Self {
        Self { min, max }
    }

    /// Creates a bounding box enclosing all given points.
    ///
    /// Returns `None` if the slice is empty.
    pub fn from_points(points: &[Point2]) -> Option<Self> {
        if points.is_empty() {
            return None;
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for p in points {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }

        Some(Self {
            min: Point2::new(min_x, min_y),
            max: Point2::new(max_x, max_y),
        })
    }

    /// Returns the smallest bounding box enclosing both `self` and `other`.
    #[inline]
    pub fn union(&self, other: &BBox2) -> BBox2 {
        BBox2 {
            min: Point2::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: Point2::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    /// Returns the intersection of two bounding boxes, or `None` if they
    /// do not overlap.
    pub fn intersection(&self, other: &BBox2) -> Option<BBox2> {
        let min_x = self.min.x.max(other.min.x);
        let min_y = self.min.y.max(other.min.y);
        let max_x = self.max.x.min(other.max.x);
        let max_y = self.max.y.min(other.max.y);

        if min_x <= max_x && min_y <= max_y {
            Some(BBox2 {
                min: Point2::new(min_x, min_y),
                max: Point2::new(max_x, max_y),
            })
        } else {
            None
        }
    }

    /// Returns `true` if the point is inside or on the boundary of this box.
    #[inline]
    pub fn contains_point(&self, p: &Point2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    /// Returns the center point of this bounding box.
    #[inline]
    pub fn center(&self) -> Point2 {
        Point2::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
        )
    }

    /// Returns the width (extent along X axis).
    #[inline]
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    /// Returns the height (extent along Y axis).
    #[inline]
    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }

    /// Returns the area of this bounding box.
    #[inline]
    pub fn area(&self) -> f64 {
        self.width() * self.height()
    }

    /// Returns a new bounding box expanded by `margin` in all directions.
    #[inline]
    pub fn expand(&self, margin: f64) -> BBox2 {
        BBox2 {
            min: Point2::new(self.min.x - margin, self.min.y - margin),
            max: Point2::new(self.max.x + margin, self.max.y + margin),
        }
    }
}

/// A 3D axis-aligned bounding box in floating-point space.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BBox3 {
    pub min: Point3,
    pub max: Point3,
}

impl PartialEq for BBox3 {
    fn eq(&self, other: &Self) -> bool {
        self.min == other.min && self.max == other.max
    }
}

impl BBox3 {
    /// Creates a bounding box from explicit min and max corners.
    #[inline]
    pub fn new(min: Point3, max: Point3) -> Self {
        Self { min, max }
    }

    /// Creates a bounding box enclosing all given points.
    ///
    /// Returns `None` if the slice is empty.
    pub fn from_points(points: &[Point3]) -> Option<Self> {
        if points.is_empty() {
            return None;
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut min_z = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut max_z = f64::NEG_INFINITY;

        for p in points {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            min_z = min_z.min(p.z);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
            max_z = max_z.max(p.z);
        }

        Some(Self {
            min: Point3::new(min_x, min_y, min_z),
            max: Point3::new(max_x, max_y, max_z),
        })
    }

    /// Returns the smallest bounding box enclosing both `self` and `other`.
    #[inline]
    pub fn union(&self, other: &BBox3) -> BBox3 {
        BBox3 {
            min: Point3::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
                self.min.z.min(other.min.z),
            ),
            max: Point3::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
                self.max.z.max(other.max.z),
            ),
        }
    }

    /// Returns the intersection of two bounding boxes, or `None` if they
    /// do not overlap.
    pub fn intersection(&self, other: &BBox3) -> Option<BBox3> {
        let min_x = self.min.x.max(other.min.x);
        let min_y = self.min.y.max(other.min.y);
        let min_z = self.min.z.max(other.min.z);
        let max_x = self.max.x.min(other.max.x);
        let max_y = self.max.y.min(other.max.y);
        let max_z = self.max.z.min(other.max.z);

        if min_x <= max_x && min_y <= max_y && min_z <= max_z {
            Some(BBox3 {
                min: Point3::new(min_x, min_y, min_z),
                max: Point3::new(max_x, max_y, max_z),
            })
        } else {
            None
        }
    }

    /// Returns `true` if the point is inside or on the boundary of this box.
    #[inline]
    pub fn contains_point(&self, p: &Point3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }

    /// Returns the center point of this bounding box.
    #[inline]
    pub fn center(&self) -> Point3 {
        Point3::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }

    /// Returns the width (extent along X axis).
    #[inline]
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    /// Returns the height (extent along Y axis).
    #[inline]
    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }

    /// Returns the depth (extent along Z axis).
    #[inline]
    pub fn depth(&self) -> f64 {
        self.max.z - self.min.z
    }

    /// Returns the volume of this bounding box.
    #[inline]
    pub fn volume(&self) -> f64 {
        self.width() * self.height() * self.depth()
    }

    /// Returns a new bounding box expanded by `margin` in all directions.
    #[inline]
    pub fn expand(&self, margin: f64) -> BBox3 {
        BBox3 {
            min: Point3::new(
                self.min.x - margin,
                self.min.y - margin,
                self.min.z - margin,
            ),
            max: Point3::new(
                self.max.x + margin,
                self.max.y + margin,
                self.max.z + margin,
            ),
        }
    }
}

/// A 2D axis-aligned bounding box in integer coordinate space.
///
/// Used for spatial queries on polygon data that operates in integer
/// coordinates for deterministic results.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IBBox2 {
    pub min: IPoint2,
    pub max: IPoint2,
}

impl IBBox2 {
    /// Creates a bounding box from explicit min and max corners.
    #[inline]
    pub fn new(min: IPoint2, max: IPoint2) -> Self {
        Self { min, max }
    }

    /// Creates a bounding box enclosing all given integer points.
    ///
    /// Returns `None` if the slice is empty.
    pub fn from_points(points: &[IPoint2]) -> Option<Self> {
        if points.is_empty() {
            return None;
        }
        let mut min_x = i64::MAX;
        let mut min_y = i64::MAX;
        let mut max_x = i64::MIN;
        let mut max_y = i64::MIN;

        for p in points {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }

        Some(Self {
            min: IPoint2::new(min_x, min_y),
            max: IPoint2::new(max_x, max_y),
        })
    }

    /// Returns the smallest bounding box enclosing both `self` and `other`.
    #[inline]
    pub fn union(&self, other: &IBBox2) -> IBBox2 {
        IBBox2 {
            min: IPoint2::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: IPoint2::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    /// Returns the intersection of two bounding boxes, or `None` if they
    /// do not overlap.
    pub fn intersection(&self, other: &IBBox2) -> Option<IBBox2> {
        let min_x = self.min.x.max(other.min.x);
        let min_y = self.min.y.max(other.min.y);
        let max_x = self.max.x.min(other.max.x);
        let max_y = self.max.y.min(other.max.y);

        if min_x <= max_x && min_y <= max_y {
            Some(IBBox2 {
                min: IPoint2::new(min_x, min_y),
                max: IPoint2::new(max_x, max_y),
            })
        } else {
            None
        }
    }

    /// Returns `true` if the point is inside or on the boundary of this box.
    #[inline]
    pub fn contains_point(&self, p: &IPoint2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
}
