//! Metric accumulation structs and per-layer/per-feature aggregation for G-code analysis.
//!
//! Provides [`SpeedStats`], [`FeatureMetrics`], [`LayerMetrics`], [`HeaderMetadata`],
//! and [`GcodeAnalysis`] -- the core types used to accumulate and report G-code
//! analysis results.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::slicer_detect::SlicerType;

/// Incremental speed statistics accumulator.
///
/// Tracks min/max and computes a weighted running mean using total distance
/// as the weight, so that longer moves contribute more to the average speed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedStats {
    /// Minimum speed observed in mm/s.
    pub min_mm_s: f64,
    /// Maximum speed observed in mm/s.
    pub max_mm_s: f64,
    /// Distance-weighted mean speed in mm/s.
    pub mean_mm_s: f64,
    /// Total distance accumulated (used for weighted mean computation).
    pub total_distance: f64,
    /// Number of speed samples recorded.
    pub sample_count: u64,
}

impl Default for SpeedStats {
    fn default() -> Self {
        Self {
            min_mm_s: f64::INFINITY,
            max_mm_s: 0.0,
            mean_mm_s: 0.0,
            total_distance: 0.0,
            sample_count: 0,
        }
    }
}

impl SpeedStats {
    /// Update stats with a new speed observation weighted by move distance.
    ///
    /// Uses incremental weighted mean: `new_mean = old_mean + (distance / new_total) * (speed - old_mean)`
    pub fn update(&mut self, speed_mm_s: f64, distance: f64) {
        if distance <= 0.0 || speed_mm_s <= 0.0 {
            return;
        }
        if speed_mm_s < self.min_mm_s {
            self.min_mm_s = speed_mm_s;
        }
        if speed_mm_s > self.max_mm_s {
            self.max_mm_s = speed_mm_s;
        }
        self.total_distance += distance;
        // Incremental weighted mean update.
        self.mean_mm_s += (distance / self.total_distance) * (speed_mm_s - self.mean_mm_s);
        self.sample_count += 1;
    }

    /// Merge another `SpeedStats` into this one.
    ///
    /// Combines min/max and recalculates weighted mean from the two
    /// accumulators' total distances.
    pub fn merge(&mut self, other: &SpeedStats) {
        if other.sample_count == 0 {
            return;
        }
        if self.sample_count == 0 {
            *self = other.clone();
            return;
        }
        if other.min_mm_s < self.min_mm_s {
            self.min_mm_s = other.min_mm_s;
        }
        if other.max_mm_s > self.max_mm_s {
            self.max_mm_s = other.max_mm_s;
        }
        let combined_distance = self.total_distance + other.total_distance;
        if combined_distance > 0.0 {
            self.mean_mm_s = (self.mean_mm_s * self.total_distance
                + other.mean_mm_s * other.total_distance)
                / combined_distance;
        }
        self.total_distance = combined_distance;
        self.sample_count += other.sample_count;
    }
}

/// Per-feature metric accumulator.
///
/// Tracks move count, distances, extrusion, and speed statistics for a
/// single feature type (e.g., "Outer wall", "Sparse infill").
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureMetrics {
    /// Number of moves for this feature.
    pub move_count: u64,
    /// Total travel (non-extruding) distance in mm.
    pub travel_distance_mm: f64,
    /// Total extruding move distance in mm.
    pub extrusion_distance_mm: f64,
    /// Total filament extruded in mm (E-axis).
    pub extrusion_e_mm: f64,
    /// Estimated time for this feature in seconds.
    pub time_estimate_s: f64,
    /// Speed statistics for extruding moves.
    pub speed_stats: SpeedStats,
}

impl FeatureMetrics {
    /// Merge another `FeatureMetrics` into this one.
    pub fn merge(&mut self, other: &FeatureMetrics) {
        self.move_count += other.move_count;
        self.travel_distance_mm += other.travel_distance_mm;
        self.extrusion_distance_mm += other.extrusion_distance_mm;
        self.extrusion_e_mm += other.extrusion_e_mm;
        self.time_estimate_s += other.time_estimate_s;
        self.speed_stats.merge(&other.speed_stats);
    }
}

/// Per-layer metric accumulator.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayerMetrics {
    /// Z height of this layer in mm.
    pub z_height: f64,
    /// Layer thickness in mm.
    pub layer_height: f64,
    /// Total number of moves in this layer.
    pub move_count: u64,
    /// Total travel (non-extruding) distance in mm.
    pub travel_distance_mm: f64,
    /// Total extruding move distance in mm.
    pub extrusion_distance_mm: f64,
    /// Number of retractions in this layer.
    pub retraction_count: u32,
    /// Estimated time for this layer in seconds.
    pub layer_time_estimate_s: f64,
    /// Per-feature metrics within this layer.
    pub features: HashMap<String, FeatureMetrics>,
}

/// Parsed slicer header metadata.
///
/// Extracted from comment blocks at the top of the G-code file.
/// All fields are optional since not all slicers provide all metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeaderMetadata {
    /// Slicer name (e.g., "BambuStudio", "PrusaSlicer").
    pub slicer_name: Option<String>,
    /// Slicer version string.
    pub slicer_version: Option<String>,
    /// Slicer-reported estimated print time in seconds.
    pub estimated_time_s: Option<f64>,
    /// Total filament length in mm.
    pub filament_length_mm: Option<f64>,
    /// Total filament volume in cm^3.
    pub filament_volume_cm3: Option<f64>,
    /// Total filament weight in grams.
    pub filament_weight_g: Option<f64>,
    /// Filament density in g/cm^3.
    pub filament_density: Option<f64>,
    /// Filament diameter in mm.
    pub filament_diameter: Option<f64>,
    /// Total number of layers (from slicer header).
    pub layer_count: Option<u32>,
    /// Maximum Z height reported by slicer.
    pub max_z_height: Option<f64>,
}

/// Top-level G-code analysis result.
///
/// Contains all extracted metrics from parsing a G-code file: header metadata,
/// slicer identification, per-layer metrics, aggregated per-feature metrics,
/// and overall totals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcodeAnalysis {
    /// Source filename.
    pub filename: String,
    /// Parsed header metadata.
    pub header: HeaderMetadata,
    /// Detected slicer type.
    pub slicer: SlicerType,
    /// Per-layer metrics in layer order.
    pub layers: Vec<LayerMetrics>,
    /// Aggregated per-feature metrics (feature name -> metrics).
    pub features: HashMap<String, FeatureMetrics>,
    /// Total estimated print time in seconds (from parser).
    pub total_time_estimate_s: f64,
    /// Total filament length in mm.
    pub total_filament_mm: f64,
    /// Total filament volume in mm^3.
    pub total_filament_volume_mm3: f64,
    /// Total filament weight in grams.
    pub total_filament_weight_g: f64,
    /// Total travel distance in mm.
    pub total_travel_mm: f64,
    /// Total extrusion move distance in mm.
    pub total_extrusion_mm: f64,
    /// Total number of moves.
    pub total_moves: u64,
    /// Number of retractions.
    pub retraction_count: u32,
    /// Total retraction distance in mm.
    pub retraction_distance_mm: f64,
    /// Number of Z-hops.
    pub zhop_count: u32,
    /// Total Z-hop distance in mm.
    pub zhop_distance_mm: f64,
    /// Number of unknown/unrecognized commands.
    pub unknown_command_count: u32,
    /// Total number of lines in the file.
    pub line_count: u64,
}

/// Computes filament weight in grams from length, diameter, and density.
///
/// Uses the cross-section formula: `weight = PI * (d/2)^2 * length * density / 1000`
/// where density is in g/cm^3 and the /1000 converts mm^3 to cm^3.
pub fn filament_mm_to_weight_g(length_mm: f64, diameter_mm: f64, density_g_per_cm3: f64) -> f64 {
    let radius = diameter_mm / 2.0;
    let cross_section_mm2 = std::f64::consts::PI * radius * radius;
    let volume_mm3 = length_mm * cross_section_mm2;
    let density_g_per_mm3 = density_g_per_cm3 / 1000.0;
    volume_mm3 * density_g_per_mm3
}

/// Computes filament volume in mm^3 from length and diameter.
///
/// Uses the cylinder formula: `volume = PI * (d/2)^2 * length`.
pub fn filament_mm_to_volume_mm3(length_mm: f64, diameter_mm: f64) -> f64 {
    let radius = diameter_mm / 2.0;
    let cross_section_mm2 = std::f64::consts::PI * radius * radius;
    length_mm * cross_section_mm2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_stats_update_single_sample() {
        let mut stats = SpeedStats::default();
        stats.update(50.0, 10.0);
        assert_eq!(stats.sample_count, 1);
        assert!((stats.min_mm_s - 50.0).abs() < 1e-9);
        assert!((stats.max_mm_s - 50.0).abs() < 1e-9);
        assert!((stats.mean_mm_s - 50.0).abs() < 1e-9);
        assert!((stats.total_distance - 10.0).abs() < 1e-9);
    }

    #[test]
    fn speed_stats_update_weighted_mean() {
        let mut stats = SpeedStats::default();
        // 10mm at 50mm/s, 30mm at 100mm/s
        // Weighted mean = (10*50 + 30*100) / (10+30) = 3500/40 = 87.5
        stats.update(50.0, 10.0);
        stats.update(100.0, 30.0);
        assert_eq!(stats.sample_count, 2);
        assert!((stats.min_mm_s - 50.0).abs() < 1e-9);
        assert!((stats.max_mm_s - 100.0).abs() < 1e-9);
        assert!((stats.mean_mm_s - 87.5).abs() < 1e-6);
    }

    #[test]
    fn speed_stats_update_ignores_zero() {
        let mut stats = SpeedStats::default();
        stats.update(0.0, 10.0);
        stats.update(50.0, 0.0);
        assert_eq!(stats.sample_count, 0);
    }

    #[test]
    fn speed_stats_merge_two_accumulators() {
        let mut a = SpeedStats::default();
        a.update(30.0, 20.0);
        a.update(60.0, 20.0);

        let mut b = SpeedStats::default();
        b.update(20.0, 10.0);
        b.update(80.0, 10.0);

        a.merge(&b);
        assert_eq!(a.sample_count, 4);
        assert!((a.min_mm_s - 20.0).abs() < 1e-9);
        assert!((a.max_mm_s - 80.0).abs() < 1e-9);
        assert!((a.total_distance - 60.0).abs() < 1e-9);
        // Weighted mean: (45.0*40 + 50.0*20) / 60 = 2800/60 = 46.667
        let expected = (45.0 * 40.0 + 50.0 * 20.0) / 60.0;
        assert!(
            (a.mean_mm_s - expected).abs() < 1e-6,
            "mean={} expected={}",
            a.mean_mm_s,
            expected
        );
    }

    #[test]
    fn speed_stats_merge_into_empty() {
        let mut a = SpeedStats::default();
        let mut b = SpeedStats::default();
        b.update(50.0, 10.0);
        a.merge(&b);
        assert_eq!(a.sample_count, 1);
        assert!((a.mean_mm_s - 50.0).abs() < 1e-9);
    }

    #[test]
    fn speed_stats_merge_empty_into_nonempty() {
        let mut a = SpeedStats::default();
        a.update(50.0, 10.0);
        let b = SpeedStats::default();
        a.merge(&b);
        assert_eq!(a.sample_count, 1);
        assert!((a.mean_mm_s - 50.0).abs() < 1e-9);
    }

    #[test]
    fn feature_metrics_merge() {
        let mut a = FeatureMetrics {
            move_count: 10,
            travel_distance_mm: 5.0,
            extrusion_distance_mm: 20.0,
            extrusion_e_mm: 1.5,
            time_estimate_s: 3.0,
            speed_stats: SpeedStats::default(),
        };
        let b = FeatureMetrics {
            move_count: 5,
            travel_distance_mm: 2.0,
            extrusion_distance_mm: 8.0,
            extrusion_e_mm: 0.6,
            time_estimate_s: 1.5,
            speed_stats: SpeedStats::default(),
        };
        a.merge(&b);
        assert_eq!(a.move_count, 15);
        assert!((a.travel_distance_mm - 7.0).abs() < 1e-9);
        assert!((a.extrusion_distance_mm - 28.0).abs() < 1e-9);
        assert!((a.extrusion_e_mm - 2.1).abs() < 1e-9);
        assert!((a.time_estimate_s - 4.5).abs() < 1e-9);
    }

    #[test]
    fn filament_weight_computation() {
        // 1000mm of 1.75mm PLA at 1.24 g/cm3
        let weight = filament_mm_to_weight_g(1000.0, 1.75, 1.24);
        let radius = 1.75 / 2.0;
        let cross_section = std::f64::consts::PI * radius * radius;
        let volume = 1000.0 * cross_section;
        let expected = volume * (1.24 / 1000.0);
        assert!(
            (weight - expected).abs() < 1e-9,
            "weight={} expected={}",
            weight,
            expected
        );
        // Sanity check: ~3g for 1m of PLA
        assert!(weight > 2.0 && weight < 4.0, "weight={}", weight);
    }

    #[test]
    fn filament_volume_computation() {
        let volume = filament_mm_to_volume_mm3(1000.0, 1.75);
        let radius = 1.75 / 2.0;
        let expected = std::f64::consts::PI * radius * radius * 1000.0;
        assert!(
            (volume - expected).abs() < 1e-9,
            "volume={} expected={}",
            volume,
            expected
        );
    }
}
