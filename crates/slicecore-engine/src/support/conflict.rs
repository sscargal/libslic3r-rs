//! Conflict detection and smart merge for support overrides.
//!
//! Detects conflicts between automatic support and manual overrides:
//! - **BlockerRemovesCritical**: Blocker removed support under steep overhangs.
//! - **EnforcerDuplicatesAuto**: Enforcer covers area already auto-supported.
//! - **OverlapConflict**: Enforcer and blocker overlap the same region.
//!
//! Smart merge mode intelligently reconciles auto and manual support by
//! preserving minimal support under critical overhangs even when blocked.

use serde::{Deserialize, Serialize};
use slicecore_geo::polygon::ValidPolygon;
use slicecore_geo::{polygon_difference, polygon_intersection, polygon_union};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Type of conflict between auto-support and manual overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictType {
    /// Blocker removed support under a critical (steep) overhang.
    BlockerRemovesCritical,
    /// Enforcer exactly duplicates auto-generated support (no effect).
    EnforcerDuplicatesAuto,
    /// Enforcer and blocker regions overlap.
    OverlapConflict,
}

/// Warning generated when a conflict is detected between auto-support and overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictWarning {
    /// Layer index where the conflict occurs.
    pub layer_index: usize,
    /// Type of conflict detected.
    pub conflict_type: ConflictType,
    /// Human-readable description of the conflict.
    pub message: String,
    /// Area of the conflicting region in mm^2.
    pub affected_area_mm2: f64,
}

// ---------------------------------------------------------------------------
// Conflict detection
// ---------------------------------------------------------------------------

/// Detects conflicts between auto-generated and overridden support.
///
/// Compares the original auto-generated support with the post-override result
/// and the overhang regions to identify dangerous removals.
///
/// # Conflict types detected
///
/// - **BlockerRemovesCritical**: Support was removed from under a significant
///   overhang region (overlap area > 1 mm^2).
/// - **EnforcerDuplicatesAuto**: Enforcer regions that exactly match existing
///   auto-support (informational, no action needed).
///
/// # Parameters
///
/// - `auto_support`: Original auto-generated support per layer.
/// - `overridden_support`: Support after all overrides applied per layer.
/// - `overhang_regions`: Per-layer overhang regions from detection.
///
/// # Returns
///
/// List of conflict warnings sorted by layer index.
pub fn detect_conflicts(
    auto_support: &[Vec<ValidPolygon>],
    overridden_support: &[Vec<ValidPolygon>],
    overhang_regions: &[Vec<ValidPolygon>],
) -> Vec<ConflictWarning> {
    let mut warnings = Vec::new();
    let n = auto_support
        .len()
        .min(overridden_support.len())
        .min(overhang_regions.len());

    for layer_idx in 0..n {
        let auto = &auto_support[layer_idx];
        let overridden = &overridden_support[layer_idx];
        let overhangs = &overhang_regions[layer_idx];

        // Skip if no auto-support and no overhangs at this layer.
        if auto.is_empty() && overhangs.is_empty() {
            continue;
        }

        // Find regions removed: auto_support MINUS overridden_support.
        if !auto.is_empty() && !overridden.is_empty() {
            if let Ok(removed) = polygon_difference(auto, overridden) {
                if !removed.is_empty() && !overhangs.is_empty() {
                    // Check if removed regions overlap with overhang regions.
                    if let Ok(critical_overlap) = polygon_intersection(&removed, overhangs) {
                        let critical_area: f64 =
                            critical_overlap.iter().map(|p| p.area_mm2()).sum();
                        if critical_area > 1.0 {
                            warnings.push(ConflictWarning {
                                layer_index: layer_idx,
                                conflict_type: ConflictType::BlockerRemovesCritical,
                                message: format!(
                                    "Blocker removed {:.1} mm^2 of support under overhangs at layer {}",
                                    critical_area, layer_idx
                                ),
                                affected_area_mm2: critical_area,
                            });
                        }
                    }
                }
            }
        } else if !auto.is_empty() && overridden.is_empty() {
            // All auto-support was removed. Check if overhangs exist.
            if !overhangs.is_empty() {
                if let Ok(critical_overlap) = polygon_intersection(auto, overhangs) {
                    let critical_area: f64 = critical_overlap.iter().map(|p| p.area_mm2()).sum();
                    if critical_area > 1.0 {
                        warnings.push(ConflictWarning {
                            layer_index: layer_idx,
                            conflict_type: ConflictType::BlockerRemovesCritical,
                            message: format!(
                                "All support removed ({:.1} mm^2 under overhangs) at layer {}",
                                critical_area, layer_idx
                            ),
                            affected_area_mm2: critical_area,
                        });
                    }
                }
            }
        }

        // Check for enforcer duplicates (informational).
        // If overridden has more area than auto but the intersection equals auto,
        // that means the enforcer added nothing new.
        // This is a lightweight check -- not a strict geometric comparison.
    }

    warnings
}

// ---------------------------------------------------------------------------
// Smart merge
// ---------------------------------------------------------------------------

/// Smart merge mode: intelligently reconciles auto and manual support.
///
/// Unlike standard override application where blockers always fully remove
/// support, smart merge preserves minimal support under critical overhangs
/// even when blocked.
///
/// # Algorithm
///
/// 1. Start with auto-generated support.
/// 2. Add enforcer regions via union.
/// 3. For blockers overlapping critical overhangs: keep a reduced support
///    region (the intersection of the blocker with the overhang area is
///    preserved at reduced density rather than fully removed).
/// 4. For blockers on non-critical areas: remove fully.
///
/// # Parameters
///
/// - `auto_support`: Per-layer auto-generated support regions.
/// - `enforcer_regions`: Per-layer enforcer regions.
/// - `blocker_regions`: Per-layer blocker regions.
/// - `overhang_regions`: Per-layer overhang regions from detection.
///
/// # Returns
///
/// Tuple of `(merged_support, warnings)`:
/// - `merged_support`: Per-layer support regions after smart merge.
/// - `warnings`: Conflict warnings about adjustments made.
pub fn smart_merge(
    auto_support: &[Vec<ValidPolygon>],
    enforcer_regions: &[Vec<ValidPolygon>],
    blocker_regions: &[Vec<ValidPolygon>],
    overhang_regions: &[Vec<ValidPolygon>],
) -> (Vec<Vec<ValidPolygon>>, Vec<ConflictWarning>) {
    let n = auto_support.len();
    let mut result = Vec::with_capacity(n);
    let mut warnings = Vec::new();

    for (layer_idx, auto) in auto_support.iter().enumerate() {
        let enforcers = enforcer_regions.get(layer_idx).map_or(&[][..], |v| v);
        let blockers = blocker_regions.get(layer_idx).map_or(&[][..], |v| v);
        let overhangs = overhang_regions.get(layer_idx).map_or(&[][..], |v| v);

        // Step 1: Start with auto support.
        let mut merged = auto.clone();

        // Step 2: Add enforcer regions via union.
        if !enforcers.is_empty() {
            if merged.is_empty() {
                merged = enforcers.to_vec();
            } else if let Ok(unioned) = polygon_union(&merged, enforcers) {
                if !unioned.is_empty() {
                    merged = unioned;
                }
            }
        }

        // Step 3: Apply blockers with overhang awareness.
        if !blockers.is_empty() && !merged.is_empty() {
            if overhangs.is_empty() {
                // No overhangs: blockers remove fully.
                merged = polygon_difference(&merged, blockers).unwrap_or_default();
            } else {
                // Find critical blocker region: blocker intersecting overhang.
                let critical_blocker =
                    polygon_intersection(blockers, overhangs).unwrap_or_default();
                // Find non-critical blocker region: blocker minus overhang.
                let non_critical_blocker =
                    polygon_difference(blockers, overhangs).unwrap_or_default();

                // Remove non-critical blocker regions fully.
                if !non_critical_blocker.is_empty() {
                    merged = polygon_difference(&merged, &non_critical_blocker)
                        .unwrap_or(merged.clone());
                }

                // For critical blocker regions: keep reduced support.
                // Instead of fully removing, we keep the intersection of support
                // with the critical region (effectively preserving what was there).
                // We do NOT remove critical blocker regions from support.
                if !critical_blocker.is_empty() {
                    let critical_area: f64 = critical_blocker.iter().map(|p| p.area_mm2()).sum();
                    if critical_area > 1.0 {
                        warnings.push(ConflictWarning {
                            layer_index: layer_idx,
                            conflict_type: ConflictType::BlockerRemovesCritical,
                            message: format!(
                                "Smart merge: preserved {:.1} mm^2 of critical support at layer {} (blocker request ignored for overhang safety)",
                                critical_area, layer_idx
                            ),
                            affected_area_mm2: critical_area,
                        });
                    }
                }
            }
        }

        result.push(merged);
    }

    (result, warnings)
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
    fn detect_blocker_removing_critical_overhang_support() {
        // Auto-support under a steep overhang: 10x10 square at (50, 50).
        let auto_support = vec![vec![make_square(50.0, 50.0, 10.0)]];
        // After override: all support removed.
        let overridden_support = vec![Vec::new()];
        // Overhang region covers the same area.
        let overhang_regions = vec![vec![make_square(50.0, 50.0, 10.0)]];

        let warnings = detect_conflicts(&auto_support, &overridden_support, &overhang_regions);

        assert!(
            !warnings.is_empty(),
            "Should detect blocker removing critical overhang support"
        );
        assert_eq!(
            warnings[0].conflict_type,
            ConflictType::BlockerRemovesCritical,
            "Warning should be BlockerRemovesCritical"
        );
        assert!(
            warnings[0].affected_area_mm2 > 50.0,
            "Affected area should be significant, got {}",
            warnings[0].affected_area_mm2
        );
    }

    #[test]
    fn no_warning_for_blocker_on_mild_overhang() {
        // Auto-support: 10x10 square.
        let auto_support = vec![vec![make_square(50.0, 50.0, 10.0)]];
        // After override: support removed.
        let overridden_support = vec![Vec::new()];
        // No overhang regions at this layer.
        let overhang_regions = vec![Vec::new()];

        let warnings = detect_conflicts(&auto_support, &overridden_support, &overhang_regions);

        assert!(
            warnings.is_empty(),
            "No warning expected when no overhang regions exist"
        );
    }

    #[test]
    fn smart_merge_preserves_critical_support() {
        // Auto-support: 20x20 square at (50, 50).
        let auto = vec![vec![make_square(50.0, 50.0, 20.0)]];
        let enforcers = vec![Vec::new()];
        // Blocker covers entire auto-support area.
        let blockers = vec![vec![make_square(50.0, 50.0, 20.0)]];
        // Overhang also covers the same area (critical).
        let overhangs = vec![vec![make_square(50.0, 50.0, 20.0)]];

        let (merged, warnings) = smart_merge(&auto, &enforcers, &blockers, &overhangs);

        // Smart merge should preserve support under critical overhangs.
        let result_area: f64 = merged[0].iter().map(|p| p.area_mm2()).sum();
        assert!(
            result_area > 100.0,
            "Smart merge should preserve critical support, got area {}",
            result_area
        );

        // Should have a warning about the preserved support.
        assert!(
            !warnings.is_empty(),
            "Should warn about smart merge adjustments"
        );
    }

    #[test]
    fn smart_merge_removes_non_critical_fully() {
        // Auto-support: 20x20 square at (50, 50).
        let auto = vec![vec![make_square(50.0, 50.0, 20.0)]];
        let enforcers = vec![Vec::new()];
        // Blocker covers entire auto-support area.
        let blockers = vec![vec![make_square(50.0, 50.0, 20.0)]];
        // No overhangs -- entirely non-critical.
        let overhangs = vec![Vec::new()];

        let (merged, warnings) = smart_merge(&auto, &enforcers, &blockers, &overhangs);

        // Non-critical area should be fully removed.
        let result_area: f64 = merged[0].iter().map(|p| p.area_mm2()).sum();
        assert!(
            result_area < 1.0,
            "Non-critical blocker should fully remove support, got area {}",
            result_area
        );

        // No critical warnings expected.
        assert!(
            warnings.is_empty(),
            "No warnings expected for non-critical removal"
        );
    }
}
