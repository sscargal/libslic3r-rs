//! Sequential (object-by-object) printing with collision detection.
//!
//! Sequential printing slices and prints each object completely before
//! moving to the next. This requires:
//! - **Collision detection**: Ensures the extruder clearance envelope
//!   (head width and gantry height) does not collide with previously
//!   printed objects.
//! - **Object ordering**: Sorts objects shortest-first to minimize
//!   collision risk.
//! - **Safe Z transitions**: Raises to a safe height between objects.

use crate::config::PrintConfig;
use crate::error::EngineError;

/// Bounding box of a single object in the print.
///
/// Stores the XY extents and maximum Z height, used for collision
/// detection and ordering in sequential printing mode.
#[derive(Debug, Clone)]
pub struct ObjectBounds {
    /// Minimum X coordinate in mm.
    pub min_x: f64,
    /// Maximum X coordinate in mm.
    pub max_x: f64,
    /// Minimum Y coordinate in mm.
    pub min_y: f64,
    /// Maximum Y coordinate in mm.
    pub max_y: f64,
    /// Maximum Z height in mm (top of object).
    pub max_z: f64,
    /// Index of this object in the input list.
    pub object_index: usize,
}

impl ObjectBounds {
    /// Width of the object in the X direction.
    pub fn width_x(&self) -> f64 {
        self.max_x - self.min_x
    }

    /// Width of the object in the Y direction.
    pub fn width_y(&self) -> f64 {
        self.max_y - self.min_y
    }
}

/// Detects whether two objects would collide in sequential printing.
///
/// Collision checking considers the extruder clearance envelope:
/// - If both objects are shorter than `clearance_height`, only the XY
///   clearance matters (the gantry can pass over them).
/// - If either object is taller than `clearance_height`, the full XY
///   clearance radius is checked (the gantry/carriage could hit).
///
/// XY distance is measured as the gap between bounding boxes (not
/// center-to-center), which is the minimum distance between any two
/// points on the bounding box edges.
///
/// # Returns
///
/// `true` if a collision would occur, `false` if safe.
pub fn detect_collision(
    obj_a: &ObjectBounds,
    obj_b: &ObjectBounds,
    clearance_radius: f64,
    clearance_height: f64,
) -> bool {
    // Compute gap between bounding boxes in X and Y.
    let gap_x = if obj_a.max_x < obj_b.min_x {
        obj_b.min_x - obj_a.max_x
    } else if obj_b.max_x < obj_a.min_x {
        obj_a.min_x - obj_b.max_x
    } else {
        0.0 // Overlapping in X
    };

    let gap_y = if obj_a.max_y < obj_b.min_y {
        obj_b.min_y - obj_a.max_y
    } else if obj_b.max_y < obj_a.min_y {
        obj_a.min_y - obj_b.max_y
    } else {
        0.0 // Overlapping in Y
    };

    // The minimum XY distance between the bounding boxes.
    let xy_gap = if gap_x > 0.0 && gap_y > 0.0 {
        // Diagonal gap (corner-to-corner).
        (gap_x * gap_x + gap_y * gap_y).sqrt()
    } else if gap_x > 0.0 {
        gap_x
    } else if gap_y > 0.0 {
        gap_y
    } else {
        0.0 // Bounding boxes overlap
    };

    // If both objects are short enough, the gantry passes over them.
    // Only XY overlap matters (the nozzle itself is small).
    let both_short = obj_a.max_z <= clearance_height && obj_b.max_z <= clearance_height;

    if both_short {
        // With both objects under the clearance height, the extruder
        // only needs enough XY gap for the nozzle assembly.
        // We still check clearance_radius for safety.
        xy_gap < clearance_radius
    } else {
        // One or both objects exceed the clearance height.
        // The carriage/gantry could collide -- need full clearance.
        xy_gap < clearance_radius
    }
}

/// Orders objects for sequential printing.
///
/// Sorts objects by maximum Z height (shortest first) to minimize
/// collision risk. After sorting, validates that consecutive pairs
/// do not collide.
///
/// # Returns
///
/// `Ok(indices)` with ordered object indices on success,
/// `Err(message)` if an unavoidable collision is detected.
pub fn order_objects(
    objects: &[ObjectBounds],
    clearance_radius: f64,
    clearance_height: f64,
) -> Result<Vec<usize>, String> {
    if objects.is_empty() {
        return Ok(Vec::new());
    }
    if objects.len() == 1 {
        return Ok(vec![objects[0].object_index]);
    }

    // Sort by max_z (shortest first).
    let mut sorted_indices: Vec<usize> = (0..objects.len()).collect();
    sorted_indices.sort_by(|&a, &b| {
        objects[a]
            .max_z
            .partial_cmp(&objects[b].max_z)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Check all pairs for collisions (not just consecutive, since
    // a tall early object could collide with any later object).
    for i in 0..sorted_indices.len() {
        for j in (i + 1)..sorted_indices.len() {
            let obj_i = &objects[sorted_indices[i]];
            let obj_j = &objects[sorted_indices[j]];
            if detect_collision(obj_i, obj_j, clearance_radius, clearance_height) {
                return Err(format!(
                    "Collision detected between object {} (max_z={:.1}mm) and object {} (max_z={:.1}mm): \
                     XY gap is less than clearance radius ({:.1}mm)",
                    obj_i.object_index,
                    obj_i.max_z,
                    obj_j.object_index,
                    obj_j.max_z,
                    clearance_radius,
                ));
            }
        }
    }

    Ok(sorted_indices
        .iter()
        .map(|&i| objects[i].object_index)
        .collect())
}

/// Plans a sequential print of multiple objects.
///
/// Each object is sliced independently and produces its own G-code.
/// Between objects, a safe Z travel above the clearance height is
/// inserted to avoid collisions.
///
/// # Parameters
///
/// - `object_bounds`: Bounding boxes for each object.
/// - `config`: Print configuration.
///
/// # Returns
///
/// Ordered list of `(object_index, safe_z)` pairs where `safe_z` is
/// the Z height to travel to before starting each object.
///
/// # Errors
///
/// Returns [`EngineError::ConfigError`] if collision detection fails.
pub fn plan_sequential_print(
    object_bounds: &[ObjectBounds],
    config: &PrintConfig,
) -> Result<Vec<(usize, f64)>, EngineError> {
    let clearance_radius = config.sequential.extruder_clearance_radius;
    let clearance_height = config.sequential.extruder_clearance_height;

    let ordered = order_objects(object_bounds, clearance_radius, clearance_height)
        .map_err(EngineError::ConfigError)?;

    let safe_z = clearance_height + 5.0; // 5mm margin above clearance

    Ok(ordered.iter().map(|&idx| (idx, safe_z)).collect())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PrintConfig, SequentialConfig};

    fn make_bounds(
        min_x: f64,
        max_x: f64,
        min_y: f64,
        max_y: f64,
        max_z: f64,
        index: usize,
    ) -> ObjectBounds {
        ObjectBounds {
            min_x,
            max_x,
            min_y,
            max_y,
            max_z,
            object_index: index,
        }
    }

    #[test]
    fn two_objects_far_apart_no_collision() {
        // Object A at (0,0), Object B at (100,100) -- well separated.
        let obj_a = make_bounds(0.0, 20.0, 0.0, 20.0, 30.0, 0);
        let obj_b = make_bounds(100.0, 120.0, 100.0, 120.0, 30.0, 1);

        let collision = detect_collision(&obj_a, &obj_b, 35.0, 40.0);
        assert!(
            !collision,
            "Objects 80mm apart should not collide with 35mm clearance"
        );
    }

    #[test]
    fn two_objects_close_with_tall_one_collision() {
        // Object A at (0,0) height 50mm, Object B at (20,0) -- close together.
        // With clearance_height=40mm, Object A is taller, so full clearance applies.
        let obj_a = make_bounds(0.0, 20.0, 0.0, 20.0, 50.0, 0);
        let obj_b = make_bounds(30.0, 50.0, 0.0, 20.0, 10.0, 1);

        // Gap between bboxes: 30.0 - 20.0 = 10mm in X, 0mm in Y
        // clearance_radius = 35mm, gap = 10mm < 35mm -> collision
        let collision = detect_collision(&obj_a, &obj_b, 35.0, 40.0);
        assert!(
            collision,
            "Objects 10mm apart with one taller than clearance should collide"
        );
    }

    #[test]
    fn two_short_objects_close_collision_check() {
        // Both objects shorter than clearance_height but overlapping in XY.
        let obj_a = make_bounds(0.0, 20.0, 0.0, 20.0, 10.0, 0);
        let obj_b = make_bounds(15.0, 35.0, 0.0, 20.0, 10.0, 1);

        // Bounding boxes overlap in X (15-20), gap = 0
        let collision = detect_collision(&obj_a, &obj_b, 35.0, 40.0);
        assert!(
            collision,
            "Overlapping bounding boxes should detect collision"
        );
    }

    #[test]
    fn order_objects_sorts_shortest_first() {
        let objects = vec![
            make_bounds(0.0, 20.0, 0.0, 20.0, 50.0, 0),
            make_bounds(100.0, 120.0, 100.0, 120.0, 10.0, 1),
            make_bounds(200.0, 220.0, 200.0, 220.0, 30.0, 2),
        ];

        let ordered = order_objects(&objects, 35.0, 40.0).unwrap();

        // Shortest first: obj1 (10mm) -> obj2 (30mm) -> obj0 (50mm)
        assert_eq!(ordered, vec![1, 2, 0]);
    }

    #[test]
    fn order_objects_returns_error_on_collision() {
        // Two objects too close together.
        let objects = vec![
            make_bounds(0.0, 20.0, 0.0, 20.0, 50.0, 0),
            make_bounds(30.0, 50.0, 0.0, 20.0, 50.0, 1),
        ];

        let result = order_objects(&objects, 35.0, 40.0);
        assert!(
            result.is_err(),
            "Should return error for colliding objects"
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("Collision"),
            "Error should mention collision: {}",
            err
        );
    }

    #[test]
    fn sequential_config_default_disabled() {
        let config = SequentialConfig::default();
        assert!(!config.enabled, "Sequential should default to disabled");
        assert!(
            (config.extruder_clearance_radius - 35.0).abs() < 1e-9,
            "Default clearance radius should be 35mm"
        );
        assert!(
            (config.extruder_clearance_height - 40.0).abs() < 1e-9,
            "Default clearance height should be 40mm"
        );
    }

    #[test]
    fn sequential_toml_parsing() {
        let toml = r#"
[sequential]
enabled = true
extruder_clearance_radius = 40.0
extruder_clearance_height = 50.0
"#;
        let config = PrintConfig::from_toml(toml).unwrap();
        assert!(config.sequential.enabled);
        assert!((config.sequential.extruder_clearance_radius - 40.0).abs() < 1e-9);
        assert!((config.sequential.extruder_clearance_height - 50.0).abs() < 1e-9);
    }

    #[test]
    fn order_empty_objects() {
        let result = order_objects(&[], 35.0, 40.0);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn order_single_object() {
        let objects = vec![make_bounds(0.0, 20.0, 0.0, 20.0, 30.0, 42)];
        let result = order_objects(&objects, 35.0, 40.0).unwrap();
        assert_eq!(result, vec![42]);
    }

    #[test]
    fn plan_sequential_print_valid_order() {
        let objects = vec![
            make_bounds(0.0, 20.0, 0.0, 20.0, 50.0, 0),
            make_bounds(200.0, 220.0, 200.0, 220.0, 10.0, 1),
        ];

        let config = PrintConfig {
            sequential: SequentialConfig {
                enabled: true,
                extruder_clearance_radius: 35.0,
                extruder_clearance_height: 40.0,
            },
            ..Default::default()
        };

        let plan = plan_sequential_print(&objects, &config).unwrap();
        assert_eq!(plan.len(), 2);
        // Shortest first: object 1 (10mm) before object 0 (50mm).
        assert_eq!(plan[0].0, 1);
        assert_eq!(plan[1].0, 0);
        // Safe Z should be above clearance height.
        assert!(plan[0].1 > 40.0);
    }

    #[test]
    fn plan_sequential_print_collision_error() {
        let objects = vec![
            make_bounds(0.0, 20.0, 0.0, 20.0, 50.0, 0),
            make_bounds(25.0, 45.0, 0.0, 20.0, 50.0, 1),
        ];

        let config = PrintConfig {
            sequential: SequentialConfig {
                enabled: true,
                extruder_clearance_radius: 35.0,
                extruder_clearance_height: 40.0,
            },
            ..Default::default()
        };

        let result = plan_sequential_print(&objects, &config);
        assert!(result.is_err(), "Should fail with collision");
    }

    #[test]
    fn detect_collision_diagonal_gap() {
        // Objects separated diagonally.
        let obj_a = make_bounds(0.0, 10.0, 0.0, 10.0, 20.0, 0);
        let obj_b = make_bounds(40.0, 50.0, 40.0, 50.0, 20.0, 1);

        // Diagonal gap = sqrt(30^2 + 30^2) = ~42.4mm > 35mm
        let collision = detect_collision(&obj_a, &obj_b, 35.0, 40.0);
        assert!(!collision, "Diagonal gap of ~42mm should not collide with 35mm radius");
    }

    #[test]
    fn object_bounds_dimensions() {
        let obj = make_bounds(10.0, 30.0, 5.0, 25.0, 15.0, 0);
        assert!((obj.width_x() - 20.0).abs() < 1e-9);
        assert!((obj.width_y() - 20.0).abs() < 1e-9);
    }
}
