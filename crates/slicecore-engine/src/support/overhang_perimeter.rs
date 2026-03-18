//! 4-tier overhang perimeter control and auto support type selection.
//!
//! Implements SUPP-07: overhang tiers are print parameter adjustments that
//! affect speed and fan for perimeters near overhangs that do NOT have support.
//!
//! # Overhang Tiers
//!
//! Tiers classify the overhang angle into buckets with corresponding speed
//! reduction and fan increase:
//!
//! | Tier     | Angle from vertical | Speed factor | Fan override |
//! |----------|---------------------|--------------|--------------|
//! | None     | 0 - 22.5 deg        | 1.0          | no change    |
//! | Mild     | 22.5 - 45 deg       | 0.9          | >= 70%       |
//! | Moderate | 45 - 67.5 deg       | 0.75         | >= 86%       |
//! | Steep    | 67.5 - 90 deg       | 0.5          | 100%         |
//! | Severe   | (unused, gets support)| 0.35        | 100%         |
//!
//! # Auto Support Type Selection
//!
//! Chooses between tree and traditional support based on overhang geometry:
//! - Small isolated overhangs -> tree support (minimal contact area)
//! - Large flat overhangs -> traditional support (full coverage)
//! - Mixed -> auto (both types used per-region)

use slicecore_geo::polygon::ValidPolygon;
use slicecore_geo::polygon_difference;
use slicecore_math::COORD_SCALE;

use super::config::SupportType;

/// Classification of overhang severity for perimeter speed/fan adjustment.
///
/// The tier is determined by the overhang angle measured from the vertical
/// axis. Perimeters near overhangs that do NOT have support get their
/// speed reduced and fan increased according to the tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverhangTier {
    /// No overhang or negligible (0-22.5 degrees from vertical).
    /// No parameter adjustments needed.
    None,
    /// Mild overhang (22.5-45 degrees from vertical).
    /// 10% speed reduction, fan >= 70%.
    Mild,
    /// Moderate overhang (45-67.5 degrees from vertical).
    /// 25% speed reduction, fan >= 86%.
    Moderate,
    /// Steep overhang (67.5-90 degrees from vertical).
    /// 50% speed reduction, fan 100%.
    Steep,
    /// Severe overhang (near bridge territory).
    /// 65% speed reduction, fan 100%.
    /// In practice, regions this steep usually get support instead.
    Severe,
}

/// Classifies an overhang angle into a tier.
///
/// The angle is measured from the vertical (0 = vertical wall, 90 = horizontal).
/// If the angle exceeds the support threshold, returns `None` because those
/// regions should receive support structures instead of speed adjustment.
///
/// # Parameters
///
/// - `overhang_angle_from_vertical`: Angle in degrees from the vertical axis.
///   0 degrees = perfectly vertical wall. 90 degrees = perfectly horizontal.
///
/// # Returns
///
/// The [`OverhangTier`] for the given angle.
pub fn classify_overhang_tier(overhang_angle_from_vertical: f64) -> OverhangTier {
    if overhang_angle_from_vertical < 22.5 {
        OverhangTier::None
    } else if overhang_angle_from_vertical < 45.0 {
        OverhangTier::Mild
    } else if overhang_angle_from_vertical < 67.5 {
        OverhangTier::Moderate
    } else if overhang_angle_from_vertical < 90.0 {
        OverhangTier::Steep
    } else {
        // >= 90 degrees: severe (near bridge territory).
        OverhangTier::Severe
    }
}

/// Returns the speed factor for a given overhang tier.
///
/// Multiply the base perimeter feedrate by this factor to get the
/// adjusted feedrate for overhang perimeters.
///
/// # Returns
///
/// Speed factor in range `[0.35, 1.0]`.
pub fn overhang_speed_factor(tier: OverhangTier) -> f64 {
    match tier {
        OverhangTier::None => 1.0,
        OverhangTier::Mild => 0.9,
        OverhangTier::Moderate => 0.75,
        OverhangTier::Steep => 0.5,
        OverhangTier::Severe => 0.35,
    }
}

/// Returns the fan speed override for a given overhang tier.
///
/// Ensures the fan speed is at least the tier's minimum, regardless
/// of the base fan speed. Higher tiers force maximum fan cooling to
/// improve overhang print quality.
///
/// # Parameters
///
/// - `tier`: The overhang tier.
/// - `base_fan`: The current fan speed (0-255).
///
/// # Returns
///
/// Fan speed (0-255) that is at least the tier's minimum.
pub fn overhang_fan_override(tier: OverhangTier, base_fan: u8) -> u8 {
    match tier {
        OverhangTier::None => base_fan,
        OverhangTier::Mild => base_fan.max(180),     // ~70%
        OverhangTier::Moderate => base_fan.max(220), // ~86%
        OverhangTier::Steep => 255,                  // 100%
        OverhangTier::Severe => 255,                 // 100%
    }
}

/// Classifies perimeter contours by overhang severity relative to the layer below.
///
/// For each perimeter contour, estimates how much it extends beyond the layer
/// below and assigns an overhang tier. This is used to adjust perimeter speed
/// and fan on a per-contour basis.
///
/// # Parameters
///
/// - `perimeter_contours`: Perimeter polygons on the current layer.
/// - `below_contours`: Model contours on the layer below.
/// - `layer_height`: Height of the current layer in mm.
///
/// # Returns
///
/// One [`OverhangTier`] per input contour, in the same order.
pub fn classify_perimeter_overhangs(
    perimeter_contours: &[ValidPolygon],
    below_contours: &[ValidPolygon],
    _layer_height: f64,
) -> Vec<OverhangTier> {
    if perimeter_contours.is_empty() {
        return Vec::new();
    }

    // If there is no layer below (first layer), all contours are supported.
    if below_contours.is_empty() {
        return vec![OverhangTier::None; perimeter_contours.len()];
    }

    let scale_sq = COORD_SCALE * COORD_SCALE;

    perimeter_contours
        .iter()
        .map(|contour| {
            // Total area of this contour.
            let contour_area_i64 = contour.area_i64().unsigned_abs() as f64;
            let contour_area_mm2 = contour_area_i64 / scale_sq;

            if contour_area_mm2 < 1e-6 {
                return OverhangTier::None;
            }

            // Compute the overhang portion: contour minus expanded below.
            // We use 0-offset (no expansion) to get exact overhang.
            let overhang_polys = polygon_difference(std::slice::from_ref(contour), below_contours)
                .unwrap_or_default();

            if overhang_polys.is_empty() {
                return OverhangTier::None;
            }

            // Compute overhang area.
            let overhang_area_mm2: f64 = overhang_polys
                .iter()
                .map(|p| p.area_i64().unsigned_abs() as f64 / scale_sq)
                .sum();

            // Fraction of the contour that overhangs.
            let overhang_fraction = (overhang_area_mm2 / contour_area_mm2).min(1.0);

            // Map overhang fraction to approximate angle from vertical.
            // 0% overhang = 0 degrees (vertical wall).
            // 100% overhang = 90 degrees (fully horizontal).
            // Linear interpolation as a practical approximation.
            let angle_from_vertical = overhang_fraction * 90.0;

            classify_overhang_tier(angle_from_vertical)
        })
        .collect()
}

/// Selects the best support type based on overhang region geometry.
///
/// Per user decision: small contact areas -> tree support, large areas ->
/// traditional support.
///
/// # Parameters
///
/// - `overhang_regions`: Per-layer overhang regions.
/// - `extrusion_width`: Extrusion width in mm.
///
/// # Returns
///
/// - `SupportType::Tree` if average region area < 10 * extrusion_width^2.
/// - `SupportType::Traditional` if average region area >= 10 * extrusion_width^2.
/// - `SupportType::Auto` if both large and small regions exist (mixed).
pub fn auto_select_support_type(
    overhang_regions: &[Vec<ValidPolygon>],
    extrusion_width: f64,
) -> SupportType {
    let scale_sq = COORD_SCALE * COORD_SCALE;
    let threshold = 10.0 * extrusion_width * extrusion_width;

    let mut small_count = 0u32;
    let mut large_count = 0u32;
    let mut total_regions = 0u32;

    for layer_regions in overhang_regions {
        for region in layer_regions {
            let area_mm2 = region.area_i64().unsigned_abs() as f64 / scale_sq;
            total_regions += 1;

            if area_mm2 < threshold {
                small_count += 1;
            } else {
                large_count += 1;
            }
        }
    }

    if total_regions == 0 {
        return SupportType::Traditional; // Default fallback
    }

    if small_count > 0 && large_count > 0 {
        SupportType::Auto // Mixed: both types used per-region.
    } else if large_count > 0 {
        SupportType::Traditional
    } else {
        SupportType::Tree
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_geo::polygon::Polygon;

    /// Helper to create a validated CCW square at a given position and size.
    fn make_square(x: f64, y: f64, size: f64) -> ValidPolygon {
        Polygon::from_mm(&[(x, y), (x + size, y), (x + size, y + size), (x, y + size)])
            .validate()
            .unwrap()
    }

    #[test]
    fn classify_30_degrees_is_mild() {
        let tier = classify_overhang_tier(30.0);
        assert_eq!(tier, OverhangTier::Mild, "30 degrees should be Mild tier");
    }

    #[test]
    fn classify_50_degrees_is_moderate() {
        let tier = classify_overhang_tier(50.0);
        assert_eq!(
            tier,
            OverhangTier::Moderate,
            "50 degrees should be Moderate tier"
        );
    }

    #[test]
    fn classify_70_degrees_is_steep() {
        let tier = classify_overhang_tier(70.0);
        assert_eq!(tier, OverhangTier::Steep, "70 degrees should be Steep tier");
    }

    #[test]
    fn classify_0_degrees_is_none() {
        let tier = classify_overhang_tier(0.0);
        assert_eq!(tier, OverhangTier::None, "0 degrees should be None tier");
    }

    #[test]
    fn classify_22_degrees_is_none() {
        let tier = classify_overhang_tier(22.0);
        assert_eq!(
            tier,
            OverhangTier::None,
            "22 degrees should still be None tier"
        );
    }

    #[test]
    fn classify_90_degrees_is_severe() {
        let tier = classify_overhang_tier(90.0);
        assert_eq!(
            tier,
            OverhangTier::Severe,
            "90 degrees should be Severe tier"
        );
    }

    #[test]
    fn speed_factor_moderate_returns_075() {
        let factor = overhang_speed_factor(OverhangTier::Moderate);
        assert!(
            (factor - 0.75).abs() < 1e-9,
            "Moderate tier speed factor should be 0.75, got {}",
            factor
        );
    }

    #[test]
    fn speed_factor_none_returns_1() {
        let factor = overhang_speed_factor(OverhangTier::None);
        assert!(
            (factor - 1.0).abs() < 1e-9,
            "None tier speed factor should be 1.0, got {}",
            factor
        );
    }

    #[test]
    fn speed_factor_steep_returns_05() {
        let factor = overhang_speed_factor(OverhangTier::Steep);
        assert!(
            (factor - 0.5).abs() < 1e-9,
            "Steep tier speed factor should be 0.5, got {}",
            factor
        );
    }

    #[test]
    fn fan_override_steep_always_255() {
        assert_eq!(
            overhang_fan_override(OverhangTier::Steep, 0),
            255,
            "Steep tier should override fan to 255 regardless of base"
        );
        assert_eq!(
            overhang_fan_override(OverhangTier::Steep, 128),
            255,
            "Steep tier should override fan to 255 regardless of base"
        );
        assert_eq!(
            overhang_fan_override(OverhangTier::Steep, 255),
            255,
            "Steep tier should override fan to 255"
        );
    }

    #[test]
    fn fan_override_mild_minimum_180() {
        assert_eq!(
            overhang_fan_override(OverhangTier::Mild, 100),
            180,
            "Mild tier should ensure fan >= 180"
        );
        assert_eq!(
            overhang_fan_override(OverhangTier::Mild, 200),
            200,
            "Mild tier should not lower fan above 180"
        );
    }

    #[test]
    fn fan_override_none_no_change() {
        assert_eq!(
            overhang_fan_override(OverhangTier::None, 42),
            42,
            "None tier should not change fan speed"
        );
    }

    #[test]
    fn auto_select_many_small_regions_chooses_tree() {
        let extrusion_width = 0.44;
        // Create many small regions (each < 10 * 0.44^2 = 1.936 mm^2).
        // Small squares of 1mm x 1mm = 1 mm^2 each.
        let small_regions: Vec<ValidPolygon> = (0..10)
            .map(|i| make_square(i as f64 * 5.0, 0.0, 1.0))
            .collect();

        let overhang_regions = vec![small_regions];
        let result = auto_select_support_type(&overhang_regions, extrusion_width);
        assert_eq!(
            result,
            SupportType::Tree,
            "Many small regions should select Tree support"
        );
    }

    #[test]
    fn auto_select_large_flat_region_chooses_traditional() {
        let extrusion_width = 0.44;
        // Create one large region (20mm x 20mm = 400 mm^2, well above threshold).
        let large_region = make_square(0.0, 0.0, 20.0);

        let overhang_regions = vec![vec![large_region]];
        let result = auto_select_support_type(&overhang_regions, extrusion_width);
        assert_eq!(
            result,
            SupportType::Traditional,
            "Large flat region should select Traditional support"
        );
    }

    #[test]
    fn auto_select_mixed_regions_chooses_auto() {
        let extrusion_width = 0.44;
        // Mix of small and large regions.
        let small = make_square(0.0, 0.0, 1.0); // 1 mm^2
        let large = make_square(10.0, 0.0, 20.0); // 400 mm^2

        let overhang_regions = vec![vec![small, large]];
        let result = auto_select_support_type(&overhang_regions, extrusion_width);
        assert_eq!(
            result,
            SupportType::Auto,
            "Mixed small and large regions should select Auto"
        );
    }

    #[test]
    fn auto_select_empty_regions_defaults_to_traditional() {
        let result = auto_select_support_type(&[], 0.44);
        assert_eq!(
            result,
            SupportType::Traditional,
            "Empty regions should default to Traditional"
        );
    }

    #[test]
    fn classify_perimeter_overhangs_no_below_all_none() {
        let contours = vec![make_square(0.0, 0.0, 10.0)];
        let tiers = classify_perimeter_overhangs(&contours, &[], 0.2);
        assert_eq!(tiers.len(), 1);
        assert_eq!(tiers[0], OverhangTier::None);
    }

    #[test]
    fn classify_perimeter_overhangs_identical_layers_all_none() {
        let contour = make_square(50.0, 50.0, 10.0);
        let tiers = classify_perimeter_overhangs(&[contour.clone()], &[contour], 0.2);
        assert_eq!(tiers.len(), 1);
        assert_eq!(
            tiers[0],
            OverhangTier::None,
            "Identical layers should produce None tier"
        );
    }

    #[test]
    fn classify_perimeter_overhangs_significant_shift() {
        // Contour shifted significantly from the layer below.
        // This should result in a non-None tier.
        let below = make_square(50.0, 50.0, 10.0);
        // Shift the contour 8mm to the right -- most of it overhangs.
        let current = make_square(58.0, 50.0, 10.0);

        let tiers = classify_perimeter_overhangs(&[current], &[below], 0.2);
        assert_eq!(tiers.len(), 1);
        assert_ne!(
            tiers[0],
            OverhangTier::None,
            "Significantly shifted contour should have non-None tier"
        );
    }
}
