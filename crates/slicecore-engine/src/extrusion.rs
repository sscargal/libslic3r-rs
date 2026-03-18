//! Extrusion math for E-axis value computation.
//!
//! In FDM 3D printing, the E-axis controls filament feed. This module computes
//! the correct E values for each move based on the Slic3r cross-section model:
//! a rectangle with semicircular ends.
//!
//! # Formulas
//!
//! - **Cross-section area**: `(width - height) * height + PI * (height/2)^2`
//! - **Extrusion volume**: `cross_section * move_length`
//! - **Filament area**: `PI * (filament_diameter/2)^2`
//! - **E value**: `volume / filament_area * extrusion_multiplier`
//!
//! All values are in millimeters. E values are for relative extrusion (M83 mode).

use std::f64::consts::PI;

/// Computes the cross-sectional area of an extrusion bead in mm^2.
///
/// Uses the Slic3r model: a rectangle with semicircular ends. The bead is
/// `width` mm wide and `height` mm tall (matching the layer height).
///
/// Formula: `(width - height) * height + PI * (height/2)^2`
///
/// # Parameters
/// - `width`: Extrusion width in mm.
/// - `height`: Layer height in mm.
///
/// # Returns
/// Cross-sectional area in mm^2.
pub fn extrusion_cross_section(width: f64, height: f64) -> f64 {
    let rect = (width - height) * height;
    let semicircles = PI * (height / 2.0) * (height / 2.0);
    rect + semicircles
}

/// Computes the E-axis value for a linear move (relative extrusion, M83).
///
/// This converts a geometric move into the correct filament feed amount based
/// on the extrusion cross-section, filament diameter, and extrusion multiplier.
///
/// # Parameters
/// - `move_length_mm`: Length of the move in mm.
/// - `extrusion_width`: Width of the extrusion bead in mm.
/// - `layer_height`: Layer height in mm.
/// - `filament_diameter`: Filament diameter in mm (typically 1.75 or 2.85).
/// - `extrusion_multiplier`: Flow rate multiplier (1.0 = 100%).
///
/// # Returns
/// E-axis value in mm of filament to feed.
pub fn compute_e_value(
    move_length_mm: f64,
    extrusion_width: f64,
    layer_height: f64,
    filament_diameter: f64,
    extrusion_multiplier: f64,
) -> f64 {
    if move_length_mm <= 0.0 || filament_diameter <= 0.0 {
        return 0.0;
    }

    let cross_section = extrusion_cross_section(extrusion_width, layer_height);
    let volume = cross_section * move_length_mm; // mm^3
    let filament_area = PI * (filament_diameter / 2.0) * (filament_diameter / 2.0);
    let e = volume / filament_area;
    e * extrusion_multiplier
}

/// Computes the Euclidean distance between two 2D points in mm.
///
/// # Parameters
/// - `from`: Starting point as (x, y) in mm.
/// - `to`: Ending point as (x, y) in mm.
///
/// # Returns
/// Distance in mm.
pub fn move_length(from: (f64, f64), to: (f64, f64)) -> f64 {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    (dx * dx + dy * dy).sqrt()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_section_0_4_width_0_2_height() {
        // width=0.4, height=0.2
        // rect = (0.4 - 0.2) * 0.2 = 0.04
        // semicircles = PI * 0.1^2 = PI * 0.01 ~= 0.031416
        // total ~= 0.071416 mm^2
        let area = extrusion_cross_section(0.4, 0.2);
        assert!(
            (area - 0.071416).abs() < 0.001,
            "Expected ~0.071 mm^2, got {}",
            area
        );
    }

    #[test]
    fn e_value_10mm_move() {
        // 10mm move, 0.44mm width, 0.2mm height, 1.75mm filament, multiplier 1.0
        // cross_section = (0.44 - 0.2) * 0.2 + PI * 0.1^2
        //               = 0.048 + 0.031416 = 0.079416 mm^2
        // volume = 0.079416 * 10 = 0.79416 mm^3
        // filament_area = PI * (1.75/2)^2 = PI * 0.765625 = 2.40528 mm^2
        // e = 0.79416 / 2.40528 = 0.33017
        let e = compute_e_value(10.0, 0.44, 0.2, 1.75, 1.0);
        assert!((e - 0.33017).abs() < 0.002, "Expected ~0.330, got {}", e);
        // Sanity: reasonable E value for a 10mm move (not too large, not too small).
        assert!(
            e > 0.1 && e < 1.0,
            "E value should be reasonable, got {}",
            e
        );
    }

    #[test]
    fn e_value_scales_linearly_with_move_length() {
        let e1 = compute_e_value(10.0, 0.4, 0.2, 1.75, 1.0);
        let e2 = compute_e_value(20.0, 0.4, 0.2, 1.75, 1.0);
        assert!(
            (e2 - 2.0 * e1).abs() < 1e-9,
            "E should scale linearly: e1={}, e2={}, 2*e1={}",
            e1,
            e2,
            2.0 * e1
        );
    }

    #[test]
    fn e_value_scales_with_extrusion_multiplier() {
        let e1 = compute_e_value(10.0, 0.4, 0.2, 1.75, 1.0);
        let e2 = compute_e_value(10.0, 0.4, 0.2, 1.75, 1.5);
        assert!(
            (e2 - 1.5 * e1).abs() < 1e-9,
            "E should scale with multiplier: e1={}, e2={}, 1.5*e1={}",
            e1,
            e2,
            1.5 * e1
        );
    }

    #[test]
    fn e_value_zero_move_length() {
        let e = compute_e_value(0.0, 0.4, 0.2, 1.75, 1.0);
        assert!(
            e.abs() < 1e-15,
            "Zero move length should produce zero E, got {}",
            e
        );
    }

    #[test]
    fn move_length_3_4_5_triangle() {
        let d = move_length((0.0, 0.0), (3.0, 4.0));
        assert!((d - 5.0).abs() < 1e-9, "Expected 5.0, got {}", d);
    }

    #[test]
    fn move_length_zero() {
        let d = move_length((5.0, 5.0), (5.0, 5.0));
        assert!(d.abs() < 1e-15, "Same point should be 0.0, got {}", d);
    }

    #[test]
    fn move_length_negative_coords() {
        let d = move_length((-3.0, -4.0), (0.0, 0.0));
        assert!((d - 5.0).abs() < 1e-9, "Expected 5.0, got {}", d);
    }

    #[test]
    fn e_value_negative_move_length_is_zero() {
        let e = compute_e_value(-5.0, 0.4, 0.2, 1.75, 1.0);
        assert!(
            e.abs() < 1e-15,
            "Negative move length should produce zero E, got {}",
            e
        );
    }
}
