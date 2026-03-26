//! Per-feature print statistics computation.
//!
//! Provides [`PrintStatistics`] -- a comprehensive breakdown of time, filament,
//! distance, and segment counts per feature type. Statistics are computed from
//! [`LayerToolpath`] segments and [`GcodeCommand`] streams, with per-feature
//! times scaled to match the trapezoid motion model total.
//!
//! # Usage
//!
//! ```ignore
//! let stats = compute_statistics(
//!     &layer_toolpaths,
//!     &gcode_commands,
//!     &time_estimate,
//!     &filament_usage,
//!     &config,
//! );
//! ```
//!
//! The resulting [`PrintStatistics`] includes:
//! - [`StatisticsSummary`]: totals including model/support subtotals
//! - [`FeatureStatistics`]: per-feature breakdown for all feature types
//! - [`GcodeMetrics`]: retraction, unretraction, z-hop, wipe counts

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use slicecore_gcode_io::GcodeCommand;

use crate::config::PrintConfig;
use crate::estimation::PrintTimeEstimate;
use crate::filament::FilamentUsage;
use crate::toolpath::{FeatureType, LayerToolpath};

/// Travel optimization statistics for the entire slice job.
///
/// Tracks baseline (original ordering) and optimized travel distances so the
/// caller can see how much the TSP optimizer reduced travel moves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TravelOptStats {
    /// Total travel distance before optimization in mm.
    pub baseline_travel_distance: f64,
    /// Total travel distance after optimization in mm.
    pub optimized_travel_distance: f64,
    /// Percentage reduction: `(baseline - optimized) / baseline * 100`.
    pub travel_reduction_percent: f64,
}

/// Configurable time display precision for statistics output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimePrecision {
    /// Display time in whole seconds.
    Seconds,
    /// Display time with one decimal place (deciseconds).
    Deciseconds,
    /// Display time with three decimal places (milliseconds).
    Milliseconds,
}

/// Configurable sort order for per-feature statistics display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StatsSortOrder {
    /// Default logical print sequence order.
    Default,
    /// Sort by time descending (highest first).
    TimeDesc,
    /// Sort by filament usage descending (highest first).
    FilamentDesc,
    /// Sort alphabetically by feature display name.
    Alphabetical,
}

/// Summary totals for the entire print job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsSummary {
    /// Total estimated print time in seconds (from trapezoid model).
    pub total_time_seconds: f64,
    /// Printing time only (excludes travel, retract, prepare overhead).
    pub print_time_seconds: f64,
    /// Total filament consumed in mm.
    pub total_filament_mm: f64,
    /// Total filament consumed in meters.
    pub total_filament_m: f64,
    /// Total filament consumed in grams.
    pub total_filament_g: f64,
    /// Total filament cost in configured currency units.
    pub total_filament_cost: f64,
    /// Total travel distance in mm.
    pub total_travel_distance_mm: f64,
    /// Total number of toolpath segments.
    pub total_segments: u64,
    /// Number of layers in the print.
    pub layer_count: usize,
    /// Time spent on model features only (excludes support).
    pub model_time_seconds: f64,
    /// Time spent on support features only.
    pub support_time_seconds: f64,
    /// Filament used by model features in mm.
    pub model_filament_mm: f64,
    /// Filament used by support features in mm.
    pub support_filament_mm: f64,
    /// Filament weight for model features in grams.
    pub model_filament_g: f64,
    /// Filament weight for support features in grams.
    pub support_filament_g: f64,
}

/// Per-feature statistics breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureStatistics {
    /// Human-readable display name (e.g., "Outer wall").
    pub feature: String,
    /// Machine-readable key (e.g., "outer_wall").
    pub feature_type: String,
    /// Time spent on this feature in seconds.
    pub time_seconds: f64,
    /// Percentage of total time (including travel, retract, everything).
    pub time_pct_total: f64,
    /// Percentage of print time only (excludes travel/retract features).
    pub time_pct_print: f64,
    /// Filament consumed by this feature in mm.
    pub filament_mm: f64,
    /// Filament consumed by this feature in meters.
    pub filament_m: f64,
    /// Filament consumed by this feature in grams.
    pub filament_g: f64,
    /// Percentage of total filament usage.
    pub filament_pct_total: f64,
    /// Percentage of print filament usage (excludes travel which has 0 filament).
    pub filament_pct_print: f64,
    /// Number of toolpath segments for this feature.
    pub segment_count: u64,
    /// Total distance of this feature in mm.
    pub distance_mm: f64,
    /// Whether this feature is a support feature (for subtotaling).
    pub is_support: bool,
    /// Whether this feature should be displayed (always true for now).
    pub display: bool,
}

/// G-code stream metrics extracted from command analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcodeMetrics {
    /// Number of retractions performed.
    pub retraction_count: u32,
    /// Total retraction distance in mm.
    pub total_retraction_distance_mm: f64,
    /// Number of unretractions performed.
    pub unretraction_count: u32,
    /// Number of wipe moves (0 until wipe-on-retraction is implemented).
    pub wipe_count: u32,
    /// Total wipe distance in mm (0 until wipe-on-retraction is implemented).
    pub total_wipe_distance_mm: f64,
    /// Number of Z-hop moves.
    pub z_hop_count: u32,
    /// Total Z-hop distance in mm.
    pub total_z_hop_distance_mm: f64,
    /// Total number of move commands (LinearMove + ArcMove*).
    pub total_move_count: u64,
}

/// Complete print statistics with summary, per-feature breakdown, and G-code metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintStatistics {
    /// Summary totals for the entire print.
    pub summary: StatisticsSummary,
    /// Per-feature breakdown in display order.
    pub features: Vec<FeatureStatistics>,
    /// G-code stream metrics.
    pub gcode_metrics: GcodeMetrics,
}

/// Returns (human_name, machine_key) for a given feature type.
pub fn feature_display_name(feature: FeatureType) -> (&'static str, &'static str) {
    match feature {
        FeatureType::OuterPerimeter => ("Outer wall", "outer_wall"),
        FeatureType::InnerPerimeter => ("Inner wall", "inner_wall"),
        FeatureType::SolidInfill => ("Internal solid infill", "internal_solid_infill"),
        FeatureType::TopSolidInfill => ("Top solid infill", "top_solid_infill"),
        FeatureType::SparseInfill => ("Sparse infill", "sparse_infill"),
        FeatureType::Skirt => ("Skirt", "skirt"),
        FeatureType::Brim => ("Brim", "brim"),
        FeatureType::GapFill => ("Gap infill", "gap_infill"),
        FeatureType::VariableWidthPerimeter => ("Variable width wall", "variable_width_wall"),
        FeatureType::Support => ("Support", "support"),
        FeatureType::SupportInterface => ("Support interface", "support_interface"),
        FeatureType::Bridge => ("Bridge", "bridge"),
        FeatureType::Ironing => ("Ironing", "ironing"),
        FeatureType::PurgeTower => ("Purge tower", "purge_tower"),
        FeatureType::Travel => ("Travel", "travel"),
    }
}

/// Returns features in the default logical print sequence order.
pub fn default_feature_order() -> Vec<FeatureType> {
    vec![
        FeatureType::OuterPerimeter,
        FeatureType::InnerPerimeter,
        FeatureType::VariableWidthPerimeter,
        FeatureType::SparseInfill,
        FeatureType::SolidInfill,
        FeatureType::TopSolidInfill,
        FeatureType::Bridge,
        FeatureType::GapFill,
        FeatureType::Ironing,
        FeatureType::Support,
        FeatureType::SupportInterface,
        FeatureType::PurgeTower,
        FeatureType::Skirt,
        FeatureType::Brim,
        FeatureType::Travel,
    ]
}

/// Returns true if the feature is a support-related feature (for subtotaling).
pub fn is_support_feature(feature: FeatureType) -> bool {
    matches!(
        feature,
        FeatureType::Support | FeatureType::SupportInterface | FeatureType::PurgeTower
    )
}

/// Internal accumulator for per-feature statistics during computation.
#[derive(Debug, Default)]
struct FeatureAccumulator {
    segment_count: u64,
    distance_mm: f64,
    filament_mm: f64,
    time_seconds: f64,
}

/// Computes per-feature statistics from toolpath segments.
///
/// Iterates all segments across all layers, accumulating segment count,
/// distance, filament usage, and naive time (distance / feedrate) per
/// feature type.
fn compute_toolpath_statistics(
    layer_toolpaths: &[LayerToolpath],
) -> HashMap<FeatureType, FeatureAccumulator> {
    let mut accumulators: HashMap<FeatureType, FeatureAccumulator> = HashMap::new();

    for layer in layer_toolpaths {
        for seg in &layer.segments {
            let acc = accumulators.entry(seg.feature).or_default();
            acc.segment_count += 1;
            acc.distance_mm += seg.length();
            acc.filament_mm += seg.e_value;

            // Naive per-segment time for relative proportions.
            let feedrate_mm_per_sec = seg.feedrate / 60.0;
            if feedrate_mm_per_sec > 0.0 {
                acc.time_seconds += seg.length() / feedrate_mm_per_sec;
            }
        }
    }

    accumulators
}

/// Extracts G-code metrics from a command stream.
///
/// Tracks retractions, unretractions, Z-hops, wipes, and total move count
/// by iterating through the command stream and maintaining state.
pub fn extract_gcode_metrics(commands: &[GcodeCommand]) -> GcodeMetrics {
    let mut retraction_count: u32 = 0;
    let mut total_retraction_distance_mm: f64 = 0.0;
    let mut unretraction_count: u32 = 0;
    let mut z_hop_count: u32 = 0;
    let mut total_z_hop_distance_mm: f64 = 0.0;
    let mut total_move_count: u64 = 0;

    let mut cur_z: f64 = 0.0;
    let mut just_retracted = false;

    for cmd in commands {
        match cmd {
            GcodeCommand::Retract { distance, .. } => {
                retraction_count += 1;
                total_retraction_distance_mm += distance;
                just_retracted = true;
            }
            GcodeCommand::Unretract { .. } => {
                unretraction_count += 1;
                just_retracted = false;
            }
            GcodeCommand::RapidMove { x, y, z, .. } => {
                // Detect Z-hop: Z-only rapid move immediately after retraction
                // where Z increases.
                if just_retracted && x.is_none() && y.is_none() {
                    if let Some(new_z) = z {
                        if *new_z > cur_z {
                            z_hop_count += 1;
                            total_z_hop_distance_mm += new_z - cur_z;
                        }
                    }
                }
                // Track Z from all Z-bearing moves.
                if let Some(new_z) = z {
                    cur_z = *new_z;
                }
                // RapidMove is not counted as a move in total_move_count
                // (it's travel, not extrusion). Reset just_retracted on
                // non-z-hop commands.
                if x.is_some() || y.is_some() {
                    just_retracted = false;
                }
            }
            GcodeCommand::LinearMove { z, .. } => {
                total_move_count += 1;
                if let Some(new_z) = z {
                    cur_z = *new_z;
                }
                just_retracted = false;
            }
            GcodeCommand::ArcMoveCW { .. } | GcodeCommand::ArcMoveCCW { .. } => {
                total_move_count += 1;
                just_retracted = false;
            }
            _ => {
                // Other commands don't affect our tracking, but do reset
                // just_retracted for non-movement commands.
                just_retracted = false;
            }
        }
    }

    GcodeMetrics {
        retraction_count,
        total_retraction_distance_mm,
        unretraction_count,
        wipe_count: 0,
        total_wipe_distance_mm: 0.0,
        z_hop_count,
        total_z_hop_distance_mm,
        total_move_count,
    }
}

/// Computes filament weight in grams from length in mm.
///
/// Uses the same cross-section model as `filament.rs`:
/// `volume_mm3 = length_mm * PI * (diameter/2)^2`
/// `weight_g = volume_mm3 * (density_g_per_cm3 / 1000)`
pub fn filament_mm_to_grams(length_mm: f64, filament_diameter: f64, filament_density: f64) -> f64 {
    let radius = filament_diameter / 2.0;
    let cross_section_mm2 = std::f64::consts::PI * radius * radius;
    let volume_mm3 = length_mm * cross_section_mm2;
    let density_g_per_mm3 = filament_density / 1000.0;
    volume_mm3 * density_g_per_mm3
}

/// Computes comprehensive print statistics from toolpath and G-code data.
///
/// This is the main entry point for statistics computation. It:
/// 1. Computes per-feature accumulators from toolpath segments
/// 2. Extracts G-code metrics from the command stream
/// 3. Scales per-feature times to match the trapezoid total
/// 4. Computes percentage breakdowns (both pct-of-total and pct-of-print)
/// 5. Adds virtual features for retract/unretract/wipe from G-code metrics
/// 6. Computes model/support subtotals
///
/// # Parameters
///
/// - `layer_toolpaths`: All layer toolpaths from the slice pipeline.
/// - `gcode_commands`: The G-code command stream.
/// - `time_estimate`: Trapezoid motion model time estimate.
/// - `filament_usage`: Filament usage from G-code analysis.
/// - `config`: Print configuration for filament physical properties.
pub fn compute_statistics(
    layer_toolpaths: &[LayerToolpath],
    gcode_commands: &[GcodeCommand],
    time_estimate: &PrintTimeEstimate,
    filament_usage: &FilamentUsage,
    config: &PrintConfig,
) -> PrintStatistics {
    // 1. Per-feature accumulators from toolpath segments.
    let accumulators = compute_toolpath_statistics(layer_toolpaths);

    // 2. G-code metrics.
    let gcode_metrics = extract_gcode_metrics(gcode_commands);

    // 3. Compute naive total time and scaling factor.
    let naive_total_time: f64 = accumulators.values().map(|a| a.time_seconds).sum();
    let scaling_factor = if naive_total_time > 0.0 {
        time_estimate.total_seconds / naive_total_time
    } else {
        1.0
    };

    // 4. Retraction overhead (matching estimation.rs: 0.5s per retraction).
    let retraction_overhead = gcode_metrics.retraction_count as f64 * 0.5;

    // Compute travel time from accumulator (scaled).
    let travel_acc = accumulators.get(&FeatureType::Travel);
    let travel_time_scaled = travel_acc.map_or(0.0, |a| a.time_seconds * scaling_factor);

    // Print time = total - travel - retraction overhead.
    let total_time = time_estimate.total_seconds;
    let print_time = (total_time - travel_time_scaled - retraction_overhead).max(0.0);

    // Total filament from all features (for percentage denominator).
    let total_filament_from_features: f64 = accumulators.values().map(|a| a.filament_mm).sum();

    // 5. Build per-feature statistics in default order.
    let mut features = Vec::new();
    let mut total_segments: u64 = 0;
    let mut total_travel_distance: f64 = 0.0;
    let mut model_time: f64 = 0.0;
    let mut support_time: f64 = 0.0;
    let mut model_filament_mm: f64 = 0.0;
    let mut support_filament_mm: f64 = 0.0;

    for feature_type in default_feature_order() {
        let acc = accumulators.get(&feature_type);
        let (display_name, machine_key) = feature_display_name(feature_type);

        let (seg_count, distance, filament, raw_time) = match acc {
            Some(a) => (
                a.segment_count,
                a.distance_mm,
                a.filament_mm,
                a.time_seconds,
            ),
            None => (0, 0.0, 0.0, 0.0),
        };

        // Scale time to match trapezoid total.
        let scaled_time = raw_time * scaling_factor;

        // Percentage of total time.
        let time_pct_total = if total_time > 0.0 {
            (scaled_time / total_time) * 100.0
        } else {
            0.0
        };

        // Percentage of print time (0 for travel).
        let time_pct_print = if feature_type == FeatureType::Travel {
            0.0
        } else if print_time > 0.0 {
            (scaled_time / print_time) * 100.0
        } else {
            0.0
        };

        let filament_m = filament / 1000.0;
        let filament_g =
            filament_mm_to_grams(filament, config.filament.diameter, config.filament.density);

        let filament_pct_total = if total_filament_from_features > 0.0 {
            (filament / total_filament_from_features) * 100.0
        } else {
            0.0
        };

        // Print filament pct excludes Travel (which has 0 filament anyway).
        let filament_pct_print = filament_pct_total;

        let is_support = is_support_feature(feature_type);

        total_segments += seg_count;
        if feature_type == FeatureType::Travel {
            total_travel_distance += distance;
        }

        // Accumulate model/support subtotals.
        if is_support {
            support_time += scaled_time;
            support_filament_mm += filament;
        } else if feature_type != FeatureType::Travel {
            model_time += scaled_time;
            model_filament_mm += filament;
        }

        features.push(FeatureStatistics {
            feature: display_name.to_string(),
            feature_type: machine_key.to_string(),
            time_seconds: scaled_time,
            time_pct_total,
            time_pct_print,
            filament_mm: filament,
            filament_m,
            filament_g,
            filament_pct_total,
            filament_pct_print,
            segment_count: seg_count,
            distance_mm: distance,
            is_support,
            display: true,
        });
    }

    // 6. Add virtual features from G-code metrics.
    // Retract: time = retraction_count * 0.5s, no filament.
    let retract_time = retraction_overhead;
    let retract_time_pct_total = if total_time > 0.0 {
        (retract_time / total_time) * 100.0
    } else {
        0.0
    };
    features.push(FeatureStatistics {
        feature: "Retract".to_string(),
        feature_type: "retract".to_string(),
        time_seconds: retract_time,
        time_pct_total: retract_time_pct_total,
        time_pct_print: 0.0,
        filament_mm: 0.0,
        filament_m: 0.0,
        filament_g: 0.0,
        filament_pct_total: 0.0,
        filament_pct_print: 0.0,
        segment_count: gcode_metrics.retraction_count as u64,
        distance_mm: gcode_metrics.total_retraction_distance_mm,
        is_support: false,
        display: true,
    });

    // Unretract virtual feature.
    features.push(FeatureStatistics {
        feature: "Unretract".to_string(),
        feature_type: "unretract".to_string(),
        time_seconds: 0.0,
        time_pct_total: 0.0,
        time_pct_print: 0.0,
        filament_mm: 0.0,
        filament_m: 0.0,
        filament_g: 0.0,
        filament_pct_total: 0.0,
        filament_pct_print: 0.0,
        segment_count: gcode_metrics.unretraction_count as u64,
        distance_mm: 0.0,
        is_support: false,
        display: true,
    });

    // Wipe virtual feature (0 until wipe-on-retraction is implemented).
    features.push(FeatureStatistics {
        feature: "Wipe".to_string(),
        feature_type: "wipe".to_string(),
        time_seconds: 0.0,
        time_pct_total: 0.0,
        time_pct_print: 0.0,
        filament_mm: 0.0,
        filament_m: 0.0,
        filament_g: 0.0,
        filament_pct_total: 0.0,
        filament_pct_print: 0.0,
        segment_count: gcode_metrics.wipe_count as u64,
        distance_mm: gcode_metrics.total_wipe_distance_mm,
        is_support: false,
        display: true,
    });

    // 7. Compute summary.
    let model_filament_g = filament_mm_to_grams(
        model_filament_mm,
        config.filament.diameter,
        config.filament.density,
    );
    let support_filament_g = filament_mm_to_grams(
        support_filament_mm,
        config.filament.diameter,
        config.filament.density,
    );

    let summary = StatisticsSummary {
        total_time_seconds: total_time,
        print_time_seconds: print_time,
        total_filament_mm: filament_usage.length_mm,
        total_filament_m: filament_usage.length_m,
        total_filament_g: filament_usage.weight_g,
        total_filament_cost: filament_usage.cost,
        total_travel_distance_mm: total_travel_distance,
        total_segments,
        layer_count: layer_toolpaths.len(),
        model_time_seconds: model_time,
        support_time_seconds: support_time,
        model_filament_mm,
        support_filament_mm,
        model_filament_g,
        support_filament_g,
    };

    PrintStatistics {
        summary,
        features,
        gcode_metrics,
    }
}

// ---------------------------------------------------------------------------
// Per-object and plate-level statistics
// ---------------------------------------------------------------------------

/// Statistics for a single object in a plate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectStatistics {
    /// Object index in the plate (0-based).
    pub object_index: usize,
    /// Object name.
    pub object_name: String,
    /// Number of copies on the plate.
    pub copies: u32,
    /// Number of layers.
    pub layer_count: usize,
    /// Filament used per single copy in mm.
    pub filament_used_mm: f64,
    /// Filament used per single copy in grams.
    pub filament_used_grams: f64,
    /// Filament cost per single copy (currency units).
    pub filament_cost: Option<f64>,
    /// Estimated time per single copy in seconds.
    pub estimated_time_seconds: f64,
}

/// Aggregated statistics for an entire plate of objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlateStatistics {
    /// Per-object statistics.
    pub objects: Vec<ObjectStatistics>,
    /// Total layers (max across objects, since Z heights are shared).
    pub total_layer_count: usize,
    /// Total filament in mm (accounting for copies).
    pub total_filament_used_mm: f64,
    /// Total filament in grams (accounting for copies).
    pub total_filament_used_grams: f64,
    /// Total filament cost (accounting for copies).
    pub total_filament_cost: Option<f64>,
    /// Total estimated time in seconds (accounting for copies).
    pub total_estimated_time_seconds: f64,
}

impl PlateStatistics {
    /// Aggregates per-object statistics from slice results.
    ///
    /// Copies are accounted for: filament and time are multiplied by copy count.
    /// The total layer count is the max across all objects (since they share the
    /// Z axis and print layer-by-layer).
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_engine::statistics::{ObjectStatistics, PlateStatistics};
    ///
    /// let obj_stats = vec![
    ///     ObjectStatistics {
    ///         object_index: 0,
    ///         object_name: "part_a".to_string(),
    ///         copies: 2,
    ///         layer_count: 100,
    ///         filament_used_mm: 5000.0,
    ///         filament_used_grams: 15.0,
    ///         filament_cost: Some(0.38),
    ///         estimated_time_seconds: 1800.0,
    ///     },
    /// ];
    /// let plate = PlateStatistics::from_object_stats(obj_stats);
    /// assert_eq!(plate.total_layer_count, 100);
    /// assert!((plate.total_filament_used_grams - 30.0).abs() < 0.01);
    /// assert!((plate.total_estimated_time_seconds - 3600.0).abs() < 0.01);
    /// ```
    #[must_use]
    pub fn from_object_stats(objects: Vec<ObjectStatistics>) -> Self {
        let mut total_layer_count = 0_usize;
        let mut total_filament_mm = 0.0_f64;
        let mut total_filament_g = 0.0_f64;
        let mut total_cost = 0.0_f64;
        let mut has_cost = false;
        let mut total_time = 0.0_f64;

        for obj in &objects {
            let copies_f64 = f64::from(obj.copies);
            if obj.layer_count > total_layer_count {
                total_layer_count = obj.layer_count;
            }
            total_filament_mm += obj.filament_used_mm * copies_f64;
            total_filament_g += obj.filament_used_grams * copies_f64;
            total_time += obj.estimated_time_seconds * copies_f64;
            if let Some(cost) = obj.filament_cost {
                has_cost = true;
                total_cost += cost * copies_f64;
            }
        }

        Self {
            objects,
            total_layer_count,
            total_filament_used_mm: total_filament_mm,
            total_filament_used_grams: total_filament_g,
            total_filament_cost: if has_cost { Some(total_cost) } else { None },
            total_estimated_time_seconds: total_time,
        }
    }

    /// Creates a [`PlateStatistics`] from [`crate::engine::ObjectSliceResult`] entries.
    ///
    /// Extracts per-object statistics from each slice result and aggregates.
    #[must_use]
    pub fn from_results(results: &[crate::engine::ObjectSliceResult]) -> Self {
        let obj_stats: Vec<ObjectStatistics> = results
            .iter()
            .map(|obj| ObjectStatistics {
                object_index: obj.index,
                object_name: obj.name.clone(),
                copies: obj.copies,
                layer_count: obj.result.layer_count,
                filament_used_mm: obj.result.filament_usage.length_mm,
                filament_used_grams: obj.result.filament_usage.weight_g,
                filament_cost: Some(obj.result.filament_usage.cost),
                estimated_time_seconds: obj.result.estimated_time_seconds,
            })
            .collect();

        Self::from_object_stats(obj_stats)
    }
}

/// Formats a time in seconds as `Xh Ym Zs` or `Ym Zs` for display.
pub fn format_time_display(seconds: f64) -> String {
    let total_secs = seconds as u64;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if hours > 0 {
        format!("{hours}:{mins:02}:{secs:02}")
    } else {
        format!("{mins}:{secs:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolpath::ToolpathSegment;
    use slicecore_math::Point2;

    #[test]
    fn feature_display_name_covers_all_variants() {
        let all_features = default_feature_order();
        assert_eq!(all_features.len(), 15, "Should have 15 feature types");

        for feature in &all_features {
            let (name, key) = feature_display_name(*feature);
            assert!(
                !name.is_empty(),
                "Display name should not be empty for {:?}",
                feature
            );
            assert!(
                !key.is_empty(),
                "Machine key should not be empty for {:?}",
                feature
            );
        }
    }

    #[test]
    fn feature_display_name_specific_values() {
        assert_eq!(
            feature_display_name(FeatureType::OuterPerimeter),
            ("Outer wall", "outer_wall")
        );
        assert_eq!(
            feature_display_name(FeatureType::InnerPerimeter),
            ("Inner wall", "inner_wall")
        );
        assert_eq!(
            feature_display_name(FeatureType::SparseInfill),
            ("Sparse infill", "sparse_infill")
        );
        assert_eq!(
            feature_display_name(FeatureType::Travel),
            ("Travel", "travel")
        );
    }

    #[test]
    fn is_support_feature_classification() {
        assert!(is_support_feature(FeatureType::Support));
        assert!(is_support_feature(FeatureType::SupportInterface));
        assert!(is_support_feature(FeatureType::PurgeTower));

        assert!(!is_support_feature(FeatureType::OuterPerimeter));
        assert!(!is_support_feature(FeatureType::InnerPerimeter));
        assert!(!is_support_feature(FeatureType::SparseInfill));
        assert!(!is_support_feature(FeatureType::Travel));
        assert!(!is_support_feature(FeatureType::Bridge));
    }

    fn make_segment(
        start: (f64, f64),
        end: (f64, f64),
        feature: FeatureType,
        e: f64,
        feedrate: f64,
    ) -> ToolpathSegment {
        ToolpathSegment {
            start: Point2::new(start.0, start.1),
            end: Point2::new(end.0, end.1),
            feature,
            e_value: e,
            feedrate,
            z: 0.2,
            extrusion_width: None,
        }
    }

    #[test]
    fn compute_toolpath_statistics_synthetic_segments() {
        let layer = LayerToolpath {
            layer_index: 0,
            z: 0.2,
            layer_height: 0.2,
            segments: vec![
                make_segment(
                    (0.0, 0.0),
                    (10.0, 0.0),
                    FeatureType::OuterPerimeter,
                    0.5,
                    2700.0,
                ),
                make_segment(
                    (10.0, 0.0),
                    (10.0, 10.0),
                    FeatureType::OuterPerimeter,
                    0.5,
                    2700.0,
                ),
                make_segment(
                    (10.0, 10.0),
                    (5.0, 5.0),
                    FeatureType::SparseInfill,
                    0.3,
                    4800.0,
                ),
                make_segment((0.0, 0.0), (10.0, 10.0), FeatureType::Travel, 0.0, 9000.0),
            ],
        };

        let accumulators = compute_toolpath_statistics(&[layer]);

        // Outer perimeter: 2 segments.
        let outer = accumulators.get(&FeatureType::OuterPerimeter).unwrap();
        assert_eq!(outer.segment_count, 2);
        assert!(
            (outer.distance_mm - 20.0).abs() < 0.01,
            "Two 10mm segments = 20mm"
        );
        assert!((outer.filament_mm - 1.0).abs() < 0.01, "0.5 + 0.5 = 1.0mm");

        // Sparse infill: 1 segment.
        let sparse = accumulators.get(&FeatureType::SparseInfill).unwrap();
        assert_eq!(sparse.segment_count, 1);

        // Travel: 1 segment, 0 filament.
        let travel = accumulators.get(&FeatureType::Travel).unwrap();
        assert_eq!(travel.segment_count, 1);
        assert!((travel.filament_mm - 0.0).abs() < 0.01);
    }

    #[test]
    fn extract_gcode_metrics_retract_unretract_zhop() {
        let commands = vec![
            GcodeCommand::LinearMove {
                x: Some(10.0),
                y: Some(0.0),
                z: Some(0.2),
                e: Some(0.5),
                f: Some(3000.0),
            },
            GcodeCommand::Retract {
                distance: 0.8,
                feedrate: 2700.0,
            },
            // Z-hop: Z-only rapid move after retraction
            GcodeCommand::RapidMove {
                x: None,
                y: None,
                z: Some(0.6),
                f: Some(9000.0),
            },
            // Travel
            GcodeCommand::RapidMove {
                x: Some(50.0),
                y: Some(50.0),
                z: None,
                f: Some(9000.0),
            },
            // Z-drop back
            GcodeCommand::RapidMove {
                x: None,
                y: None,
                z: Some(0.2),
                f: Some(9000.0),
            },
            GcodeCommand::Unretract {
                distance: 0.8,
                feedrate: 2700.0,
            },
            GcodeCommand::LinearMove {
                x: Some(60.0),
                y: Some(50.0),
                z: None,
                e: Some(0.5),
                f: Some(3000.0),
            },
        ];

        let metrics = extract_gcode_metrics(&commands);

        assert_eq!(metrics.retraction_count, 1);
        assert!((metrics.total_retraction_distance_mm - 0.8).abs() < 1e-6);
        assert_eq!(metrics.unretraction_count, 1);
        assert_eq!(metrics.z_hop_count, 1);
        assert!(
            (metrics.total_z_hop_distance_mm - 0.4).abs() < 1e-6,
            "0.6 - 0.2 = 0.4"
        );
        assert_eq!(metrics.wipe_count, 0);
        assert!((metrics.total_wipe_distance_mm - 0.0).abs() < 1e-6);
        assert_eq!(metrics.total_move_count, 2, "2 LinearMove commands");
    }

    #[test]
    fn extract_gcode_metrics_multiple_retractions() {
        let commands = vec![
            GcodeCommand::Retract {
                distance: 0.8,
                feedrate: 2700.0,
            },
            GcodeCommand::Unretract {
                distance: 0.8,
                feedrate: 2700.0,
            },
            GcodeCommand::LinearMove {
                x: Some(10.0),
                y: None,
                z: None,
                e: Some(0.5),
                f: Some(3000.0),
            },
            GcodeCommand::Retract {
                distance: 1.0,
                feedrate: 2700.0,
            },
            GcodeCommand::Unretract {
                distance: 1.0,
                feedrate: 2700.0,
            },
        ];

        let metrics = extract_gcode_metrics(&commands);
        assert_eq!(metrics.retraction_count, 2);
        assert!((metrics.total_retraction_distance_mm - 1.8).abs() < 1e-6);
        assert_eq!(metrics.unretraction_count, 2);
    }

    #[test]
    fn compute_statistics_percentages_sum_approximately_100() {
        // Create synthetic toolpath with multiple features.
        let layer = LayerToolpath {
            layer_index: 0,
            z: 0.2,
            layer_height: 0.2,
            segments: vec![
                make_segment(
                    (0.0, 0.0),
                    (20.0, 0.0),
                    FeatureType::OuterPerimeter,
                    1.0,
                    2700.0,
                ),
                make_segment(
                    (20.0, 0.0),
                    (20.0, 20.0),
                    FeatureType::InnerPerimeter,
                    1.0,
                    2700.0,
                ),
                make_segment(
                    (0.0, 0.0),
                    (15.0, 15.0),
                    FeatureType::SparseInfill,
                    0.8,
                    4800.0,
                ),
                make_segment((0.0, 0.0), (20.0, 20.0), FeatureType::Travel, 0.0, 9000.0),
            ],
        };

        let gcode_commands = vec![
            GcodeCommand::LinearMove {
                x: Some(20.0),
                y: Some(0.0),
                z: Some(0.2),
                e: Some(1.0),
                f: Some(2700.0),
            },
            GcodeCommand::LinearMove {
                x: Some(20.0),
                y: Some(20.0),
                z: None,
                e: Some(1.0),
                f: None,
            },
            GcodeCommand::LinearMove {
                x: Some(15.0),
                y: Some(15.0),
                z: None,
                e: Some(0.8),
                f: Some(4800.0),
            },
            GcodeCommand::RapidMove {
                x: Some(0.0),
                y: Some(0.0),
                z: None,
                f: Some(9000.0),
            },
        ];

        let time_estimate = PrintTimeEstimate {
            total_seconds: 10.0,
            move_time_seconds: 8.0,
            travel_time_seconds: 2.0,
            retraction_count: 0,
        };

        let filament_usage = FilamentUsage {
            length_mm: 2.8,
            length_m: 0.0028,
            weight_g: 0.01,
            cost: 0.001,
        };

        let config = PrintConfig::default();

        let stats = compute_statistics(
            &[layer],
            &gcode_commands,
            &time_estimate,
            &filament_usage,
            &config,
        );

        // Time percentages of total should sum to approximately 100%
        // (excluding virtual features).
        let time_pct_sum: f64 = stats
            .features
            .iter()
            .filter(|f| {
                f.feature_type != "retract"
                    && f.feature_type != "unretract"
                    && f.feature_type != "wipe"
            })
            .map(|f| f.time_pct_total)
            .sum();

        assert!(
            (time_pct_sum - 100.0).abs() < 1.0,
            "Time pct_total should sum to ~100%, got {:.2}%",
            time_pct_sum
        );

        // Filament percentages should sum to ~100% for features with filament.
        let filament_pct_sum: f64 = stats
            .features
            .iter()
            .filter(|f| {
                f.feature_type != "retract"
                    && f.feature_type != "unretract"
                    && f.feature_type != "wipe"
            })
            .map(|f| f.filament_pct_total)
            .sum();

        assert!(
            (filament_pct_sum - 100.0).abs() < 1.0,
            "Filament pct_total should sum to ~100%, got {:.2}%",
            filament_pct_sum
        );
    }

    #[test]
    fn compute_statistics_zero_features_appear() {
        // Create toolpath with only outer perimeter -- all other features
        // should still appear with zero values.
        let layer = LayerToolpath {
            layer_index: 0,
            z: 0.2,
            layer_height: 0.2,
            segments: vec![make_segment(
                (0.0, 0.0),
                (10.0, 0.0),
                FeatureType::OuterPerimeter,
                0.5,
                2700.0,
            )],
        };

        let gcode_commands = vec![GcodeCommand::LinearMove {
            x: Some(10.0),
            y: Some(0.0),
            z: Some(0.2),
            e: Some(0.5),
            f: Some(2700.0),
        }];

        let time_estimate = PrintTimeEstimate {
            total_seconds: 5.0,
            move_time_seconds: 5.0,
            travel_time_seconds: 0.0,
            retraction_count: 0,
        };

        let filament_usage = FilamentUsage {
            length_mm: 0.5,
            length_m: 0.0005,
            weight_g: 0.001,
            cost: 0.0001,
        };

        let config = PrintConfig::default();

        let stats = compute_statistics(
            &[layer],
            &gcode_commands,
            &time_estimate,
            &filament_usage,
            &config,
        );

        // Should have 15 real features + 3 virtual (retract, unretract, wipe) = 18.
        assert_eq!(
            stats.features.len(),
            18,
            "Should have 18 features (15 real + 3 virtual), got {}",
            stats.features.len()
        );

        // OuterPerimeter should have data.
        let outer = stats
            .features
            .iter()
            .find(|f| f.feature_type == "outer_wall")
            .unwrap();
        assert!(
            outer.time_seconds > 0.0,
            "Outer wall should have positive time"
        );
        assert!(
            outer.filament_mm > 0.0,
            "Outer wall should have positive filament"
        );

        // InnerPerimeter should exist but with zero values.
        let inner = stats
            .features
            .iter()
            .find(|f| f.feature_type == "inner_wall")
            .unwrap();
        assert!(
            (inner.time_seconds - 0.0).abs() < 1e-9,
            "Inner wall should have 0 time"
        );
        assert!(
            (inner.filament_mm - 0.0).abs() < 1e-9,
            "Inner wall should have 0 filament"
        );
        assert_eq!(inner.segment_count, 0, "Inner wall should have 0 segments");

        // All features should have display=true.
        for feature in &stats.features {
            assert!(feature.display, "All features should have display=true");
        }
    }

    #[test]
    fn compute_statistics_support_subtotals() {
        let layer = LayerToolpath {
            layer_index: 0,
            z: 0.2,
            layer_height: 0.2,
            segments: vec![
                make_segment(
                    (0.0, 0.0),
                    (10.0, 0.0),
                    FeatureType::OuterPerimeter,
                    0.5,
                    2700.0,
                ),
                make_segment((10.0, 0.0), (20.0, 0.0), FeatureType::Support, 0.4, 4800.0),
                make_segment(
                    (20.0, 0.0),
                    (25.0, 0.0),
                    FeatureType::SupportInterface,
                    0.2,
                    2700.0,
                ),
            ],
        };

        let gcode_commands = vec![
            GcodeCommand::LinearMove {
                x: Some(10.0),
                y: Some(0.0),
                z: Some(0.2),
                e: Some(0.5),
                f: Some(2700.0),
            },
            GcodeCommand::LinearMove {
                x: Some(20.0),
                y: Some(0.0),
                z: None,
                e: Some(0.4),
                f: Some(4800.0),
            },
            GcodeCommand::LinearMove {
                x: Some(25.0),
                y: Some(0.0),
                z: None,
                e: Some(0.2),
                f: Some(2700.0),
            },
        ];

        let time_estimate = PrintTimeEstimate {
            total_seconds: 10.0,
            move_time_seconds: 10.0,
            travel_time_seconds: 0.0,
            retraction_count: 0,
        };

        let filament_usage = FilamentUsage {
            length_mm: 1.1,
            length_m: 0.0011,
            weight_g: 0.003,
            cost: 0.0001,
        };

        let config = PrintConfig::default();

        let stats = compute_statistics(
            &[layer],
            &gcode_commands,
            &time_estimate,
            &filament_usage,
            &config,
        );

        // Support subtotals should be positive.
        assert!(
            stats.summary.support_time_seconds > 0.0,
            "Support time should be positive"
        );
        assert!(
            stats.summary.support_filament_mm > 0.0,
            "Support filament should be positive"
        );

        // Model subtotals should be positive (outer perimeter).
        assert!(
            stats.summary.model_time_seconds > 0.0,
            "Model time should be positive"
        );
        assert!(
            stats.summary.model_filament_mm > 0.0,
            "Model filament should be positive"
        );

        // Support features flagged correctly.
        let support = stats
            .features
            .iter()
            .find(|f| f.feature_type == "support")
            .unwrap();
        assert!(
            support.is_support,
            "Support should be flagged as is_support"
        );

        let support_iface = stats
            .features
            .iter()
            .find(|f| f.feature_type == "support_interface")
            .unwrap();
        assert!(
            support_iface.is_support,
            "Support interface should be flagged as is_support"
        );

        let outer = stats
            .features
            .iter()
            .find(|f| f.feature_type == "outer_wall")
            .unwrap();
        assert!(
            !outer.is_support,
            "Outer wall should not be flagged as is_support"
        );
    }

    #[test]
    fn compute_statistics_summary_layer_count() {
        let layers = vec![
            LayerToolpath {
                layer_index: 0,
                z: 0.2,
                layer_height: 0.2,
                segments: vec![make_segment(
                    (0.0, 0.0),
                    (10.0, 0.0),
                    FeatureType::OuterPerimeter,
                    0.5,
                    2700.0,
                )],
            },
            LayerToolpath {
                layer_index: 1,
                z: 0.4,
                layer_height: 0.2,
                segments: vec![make_segment(
                    (0.0, 0.0),
                    (10.0, 0.0),
                    FeatureType::OuterPerimeter,
                    0.5,
                    2700.0,
                )],
            },
        ];

        let gcode_commands = vec![
            GcodeCommand::LinearMove {
                x: Some(10.0),
                y: None,
                z: Some(0.2),
                e: Some(0.5),
                f: Some(2700.0),
            },
            GcodeCommand::LinearMove {
                x: Some(10.0),
                y: None,
                z: Some(0.4),
                e: Some(0.5),
                f: Some(2700.0),
            },
        ];

        let time_estimate = PrintTimeEstimate {
            total_seconds: 5.0,
            move_time_seconds: 5.0,
            travel_time_seconds: 0.0,
            retraction_count: 0,
        };

        let filament_usage = FilamentUsage {
            length_mm: 1.0,
            length_m: 0.001,
            weight_g: 0.003,
            cost: 0.0001,
        };

        let config = PrintConfig::default();

        let stats = compute_statistics(
            &layers,
            &gcode_commands,
            &time_estimate,
            &filament_usage,
            &config,
        );
        assert_eq!(stats.summary.layer_count, 2, "Should report 2 layers");
        assert_eq!(
            stats.summary.total_segments, 2,
            "Should have 2 total segments"
        );
    }

    #[test]
    fn plate_statistics_from_object_stats_aggregates_correctly() {
        let obj_stats = vec![
            ObjectStatistics {
                object_index: 0,
                object_name: "part_a".to_string(),
                copies: 2,
                layer_count: 200,
                filament_used_mm: 5000.0,
                filament_used_grams: 15.0,
                filament_cost: Some(0.38),
                estimated_time_seconds: 1800.0,
            },
            ObjectStatistics {
                object_index: 1,
                object_name: "part_b".to_string(),
                copies: 1,
                layer_count: 100,
                filament_used_mm: 2000.0,
                filament_used_grams: 6.0,
                filament_cost: Some(0.15),
                estimated_time_seconds: 900.0,
            },
        ];

        let plate = PlateStatistics::from_object_stats(obj_stats);

        assert_eq!(
            plate.total_layer_count, 200,
            "Max layer count from all objects"
        );
        assert!(
            (plate.total_filament_used_grams - 36.0).abs() < 0.01,
            "15*2 + 6*1 = 36g, got {}",
            plate.total_filament_used_grams
        );
        assert!(
            (plate.total_filament_used_mm - 12000.0).abs() < 0.01,
            "5000*2 + 2000*1 = 12000mm"
        );
        assert!(
            (plate.total_estimated_time_seconds - 4500.0).abs() < 0.01,
            "1800*2 + 900*1 = 4500s"
        );
        let cost = plate.total_filament_cost.unwrap();
        assert!(
            (cost - 0.91).abs() < 0.01,
            "0.38*2 + 0.15*1 = 0.91, got {}",
            cost
        );
    }

    #[test]
    fn plate_statistics_copies_multiplied_in_totals() {
        let obj_stats = vec![ObjectStatistics {
            object_index: 0,
            object_name: "bracket".to_string(),
            copies: 3,
            layer_count: 50,
            filament_used_mm: 1000.0,
            filament_used_grams: 3.0,
            filament_cost: Some(0.10),
            estimated_time_seconds: 600.0,
        }];

        let plate = PlateStatistics::from_object_stats(obj_stats);

        assert_eq!(plate.total_layer_count, 50);
        assert!(
            (plate.total_filament_used_mm - 3000.0).abs() < 0.01,
            "1000 * 3 = 3000"
        );
        assert!(
            (plate.total_filament_used_grams - 9.0).abs() < 0.01,
            "3 * 3 = 9"
        );
        assert!(
            (plate.total_estimated_time_seconds - 1800.0).abs() < 0.01,
            "600 * 3 = 1800"
        );
    }

    #[test]
    fn plate_statistics_from_results_works() {
        use crate::engine::{ObjectSliceResult, SliceResult};
        use crate::estimation::PrintTimeEstimate;
        use crate::filament::FilamentUsage;

        let results = vec![ObjectSliceResult {
            name: "obj1".to_string(),
            index: 0,
            result: SliceResult {
                gcode: Vec::new(),
                layer_count: 100,
                estimated_time_seconds: 3600.0,
                time_estimate: PrintTimeEstimate {
                    total_seconds: 3600.0,
                    move_time_seconds: 2800.0,
                    travel_time_seconds: 600.0,
                    retraction_count: 50,
                },
                filament_usage: FilamentUsage {
                    length_mm: 5000.0,
                    length_m: 5.0,
                    weight_g: 15.0,
                    cost: 0.38,
                },
                preview: None,
                statistics: None,
                travel_opt_stats: None,
            },
            copies: 1,
        }];

        let plate = PlateStatistics::from_results(&results);
        assert_eq!(plate.objects.len(), 1);
        assert_eq!(plate.objects[0].object_name, "obj1");
        assert_eq!(plate.total_layer_count, 100);
    }

    #[test]
    fn format_time_display_works() {
        assert_eq!(format_time_display(3661.0), "1:01:01");
        assert_eq!(format_time_display(125.0), "2:05");
        assert_eq!(format_time_display(59.0), "0:59");
    }

    #[test]
    fn filament_mm_to_grams_matches_filament_rs() {
        // Verify our helper matches the filament.rs computation.
        let length_mm = 1000.0;
        let diameter = 1.75;
        let density = 1.24;

        let weight = filament_mm_to_grams(length_mm, diameter, density);

        // Same calculation as filament.rs:
        let radius = diameter / 2.0;
        let cross_section = std::f64::consts::PI * radius * radius;
        let volume = length_mm * cross_section;
        let expected = volume * (density / 1000.0);

        assert!(
            (weight - expected).abs() < 1e-9,
            "filament_mm_to_grams should match filament.rs: {} vs {}",
            weight,
            expected
        );
    }
}
