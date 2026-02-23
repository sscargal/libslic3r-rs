//! Statistics display formatting for CLI output.
//!
//! Provides ASCII table, CSV, and JSON output formats for [`PrintStatistics`].
//! The ASCII table uses `comfy-table` for auto-sizing columns, with a summary
//! header and per-feature breakdown including model/support subtotals.

use comfy_table::{ContentArrangement, Table};

use slicecore_engine::{
    FeatureStatistics, GcodeMetrics, PrintStatistics, StatsSortOrder, TimePrecision,
};

/// Formats a duration in seconds to a human-readable string.
///
/// Examples:
/// - `45.0` -> `"45s"` (Seconds precision)
/// - `2298.0` -> `"38m18s"` (Seconds precision)
/// - `2298.3` -> `"38m18.3s"` (Deciseconds precision)
/// - `4530.0` -> `"1h15m30s"` (Seconds precision)
pub fn format_time(seconds: f64, precision: &TimePrecision) -> String {
    let total_secs = seconds.max(0.0);
    let hours = (total_secs / 3600.0).floor() as u64;
    let minutes = ((total_secs - hours as f64 * 3600.0) / 60.0).floor() as u64;
    let remaining = total_secs - hours as f64 * 3600.0 - minutes as f64 * 60.0;

    let sec_part = match precision {
        TimePrecision::Seconds => format!("{}s", remaining.round() as u64),
        TimePrecision::Deciseconds => format!("{:.1}s", remaining),
        TimePrecision::Milliseconds => format!("{:.3}s", remaining),
    };

    if hours > 0 {
        format!("{}h{}m{}", hours, minutes, sec_part)
    } else if minutes > 0 {
        format!("{}m{}", minutes, sec_part)
    } else {
        sec_part
    }
}

/// Formats a length in mm to a human-readable string.
///
/// If >= 1000mm, displays as meters (e.g., "3.87m").
/// Otherwise displays as mm (e.g., "450.2mm").
pub fn format_length(mm: f64) -> String {
    if mm >= 1000.0 {
        format!("{:.2}m", mm / 1000.0)
    } else {
        format!("{:.1}mm", mm)
    }
}

/// Formats filament usage as combined length and weight.
///
/// Example: "3.87m / 11.73g"
pub fn format_filament(mm: f64, g: f64) -> String {
    format!("{} / {:.2}g", format_length(mm), g)
}

/// Sorts feature statistics references by the given sort order.
pub fn sort_features<'a>(
    features: &'a [FeatureStatistics],
    order: &StatsSortOrder,
) -> Vec<&'a FeatureStatistics> {
    let mut sorted: Vec<&FeatureStatistics> = features.iter().collect();
    match order {
        StatsSortOrder::Default => {
            // Already in default order from compute_statistics.
        }
        StatsSortOrder::TimeDesc => {
            sorted.sort_by(|a, b| {
                b.time_seconds
                    .partial_cmp(&a.time_seconds)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        StatsSortOrder::FilamentDesc => {
            sorted.sort_by(|a, b| {
                b.filament_mm
                    .partial_cmp(&a.filament_mm)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        StatsSortOrder::Alphabetical => {
            sorted.sort_by(|a, b| a.feature.cmp(&b.feature));
        }
    }
    sorted
}

/// Formats statistics as an ASCII table with summary header and per-feature breakdown.
///
/// Output structure:
/// 1. Summary section (total time, filament, cost, layers, etc.)
/// 2. Per-feature breakdown table using comfy-table
/// 3. G-code metrics section (retractions, z-hops, etc.)
pub fn format_ascii_table(
    stats: &PrintStatistics,
    precision: &TimePrecision,
    sort_order: &StatsSortOrder,
) -> String {
    let mut output = String::new();

    // Summary section.
    output.push_str("=== Slicing Statistics ===\n");
    output.push_str(&format!(
        "Total time:     {}\n",
        format_time(stats.summary.total_time_seconds, precision)
    ));
    output.push_str(&format!(
        "Print time:     {}\n",
        format_time(stats.summary.print_time_seconds, precision)
    ));
    output.push_str(&format!(
        "Filament:       {}\n",
        format_filament(stats.summary.total_filament_mm, stats.summary.total_filament_g)
    ));
    output.push_str(&format!(
        "Cost:           {:.2}\n",
        stats.summary.total_filament_cost
    ));
    output.push_str(&format!("Layers:         {}\n", stats.summary.layer_count));
    output.push_str(&format!("Segments:       {}\n", stats.summary.total_segments));
    output.push_str(&format!(
        "Travel:         {}\n",
        format_length(stats.summary.total_travel_distance_mm)
    ));
    output.push_str(&format!(
        "Retractions:    {} ({})\n",
        stats.gcode_metrics.retraction_count,
        format_length(stats.gcode_metrics.total_retraction_distance_mm)
    ));
    output.push_str(&format!(
        "Z-hops:         {} ({})\n",
        stats.gcode_metrics.z_hop_count,
        format_length(stats.gcode_metrics.total_z_hop_distance_mm)
    ));
    output.push('\n');

    // Per-feature breakdown table.
    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec![
        "Feature", "Time", "% Total", "% Print", "Filament", "Weight", "% Fil",
    ]);

    // Separate features into model, support, and virtual categories.
    let sorted = sort_features(&stats.features, sort_order);

    // Collect model and support features for subtotals.
    let mut model_features: Vec<&FeatureStatistics> = Vec::new();
    let mut support_features: Vec<&FeatureStatistics> = Vec::new();
    let mut virtual_features: Vec<&FeatureStatistics> = Vec::new();

    for f in &sorted {
        let ftype = f.feature_type.as_str();
        if ftype == "retract" || ftype == "unretract" || ftype == "wipe" {
            virtual_features.push(f);
        } else if f.is_support {
            support_features.push(f);
        } else if ftype == "travel" {
            // Travel goes after support subtotal.
            virtual_features.push(f);
        } else {
            model_features.push(f);
        }
    }

    // Add model features.
    for f in &model_features {
        add_feature_row(&mut table, f, precision);
    }

    // Model subtotal.
    table.add_row(vec![
        "-- Model total --".to_string(),
        format_time(stats.summary.model_time_seconds, precision),
        String::new(),
        String::new(),
        format_length(stats.summary.model_filament_mm),
        format!("{:.2}g", stats.summary.model_filament_g),
        String::new(),
    ]);

    // Support features and subtotal (only if any support has non-zero time).
    let has_support = support_features.iter().any(|f| f.time_seconds > 0.0);
    if has_support {
        for f in &support_features {
            add_feature_row(&mut table, f, precision);
        }

        table.add_row(vec![
            "-- Support total --".to_string(),
            format_time(stats.summary.support_time_seconds, precision),
            String::new(),
            String::new(),
            format_length(stats.summary.support_filament_mm),
            format!("{:.2}g", stats.summary.support_filament_g),
            String::new(),
        ]);
    } else {
        // Still show support features (with zero values) but no subtotal row.
        for f in &support_features {
            add_feature_row(&mut table, f, precision);
        }
    }

    // Virtual/travel features.
    for f in &virtual_features {
        add_feature_row(&mut table, f, precision);
    }

    // Overall total.
    table.add_row(vec![
        "-- Overall total --".to_string(),
        format_time(stats.summary.total_time_seconds, precision),
        "100.0%".to_string(),
        String::new(),
        format_length(stats.summary.total_filament_mm),
        format!("{:.2}g", stats.summary.total_filament_g),
        "100.0%".to_string(),
    ]);

    output.push_str(&table.to_string());
    output.push('\n');

    // G-code metrics section.
    output.push('\n');
    output.push_str(&format!(
        "Retractions:    {} ({})\n",
        stats.gcode_metrics.retraction_count,
        format_length(stats.gcode_metrics.total_retraction_distance_mm)
    ));
    output.push_str(&format!(
        "Unretractions:  {}\n",
        stats.gcode_metrics.unretraction_count
    ));
    output.push_str(&format!(
        "Z-hops:         {} ({})\n",
        stats.gcode_metrics.z_hop_count,
        format_length(stats.gcode_metrics.total_z_hop_distance_mm)
    ));
    output.push_str(&format!(
        "Wipes:          {} ({})\n",
        stats.gcode_metrics.wipe_count,
        format_length(stats.gcode_metrics.total_wipe_distance_mm)
    ));

    output
}

/// Adds a single feature row to the comfy-table.
fn add_feature_row(table: &mut Table, f: &FeatureStatistics, precision: &TimePrecision) {
    table.add_row(vec![
        f.feature.clone(),
        format_time(f.time_seconds, precision),
        format!("{:.1}%", f.time_pct_total),
        format!("{:.1}%", f.time_pct_print),
        format_length(f.filament_mm),
        format!("{:.2}g", f.filament_g),
        format!("{:.1}%", f.filament_pct_total),
    ]);
}

/// Formats statistics as CSV with standardized column names.
///
/// Header: `feature,time_s,time_pct_total,time_pct_print,filament_mm,filament_g,filament_pct_total,filament_pct_print`
/// One data row per feature.
pub fn format_csv(stats: &PrintStatistics, sort_order: &StatsSortOrder) -> String {
    let mut output = String::new();
    output.push_str(
        "feature,time_s,time_pct_total,time_pct_print,filament_mm,filament_g,filament_pct_total,filament_pct_print\n",
    );

    let sorted = sort_features(&stats.features, sort_order);
    for f in &sorted {
        output.push_str(&format!(
            "{},{:.3},{:.2},{:.2},{:.3},{:.4},{:.2},{:.2}\n",
            f.feature_type,
            f.time_seconds,
            f.time_pct_total,
            f.time_pct_print,
            f.filament_mm,
            f.filament_g,
            f.filament_pct_total,
            f.filament_pct_print,
        ));
    }

    output
}

/// Formats statistics as pretty-printed JSON.
///
/// Serializes the full [`PrintStatistics`] structure.
pub fn format_json(stats: &PrintStatistics) -> String {
    serde_json::to_string_pretty(stats).unwrap_or_else(|e| {
        format!("{{\"error\": \"Failed to serialize statistics: {}\"}}", e)
    })
}

/// Parses a time precision string from CLI argument to enum.
pub fn parse_time_precision(s: &str) -> TimePrecision {
    match s {
        "deciseconds" => TimePrecision::Deciseconds,
        "milliseconds" => TimePrecision::Milliseconds,
        _ => TimePrecision::Seconds,
    }
}

/// Parses a sort order string from CLI argument to enum.
pub fn parse_sort_order(s: &str) -> StatsSortOrder {
    match s {
        "time" => StatsSortOrder::TimeDesc,
        "filament" => StatsSortOrder::FilamentDesc,
        "alpha" => StatsSortOrder::Alphabetical,
        _ => StatsSortOrder::Default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_engine::{GcodeMetrics, PrintStatistics, StatisticsSummary};

    fn make_test_stats() -> PrintStatistics {
        PrintStatistics {
            summary: StatisticsSummary {
                total_time_seconds: 2298.0,
                print_time_seconds: 2000.0,
                total_filament_mm: 3870.0,
                total_filament_m: 3.87,
                total_filament_g: 11.73,
                total_filament_cost: 0.29,
                total_travel_distance_mm: 5400.0,
                total_segments: 15000,
                layer_count: 100,
                model_time_seconds: 1800.0,
                support_time_seconds: 200.0,
                model_filament_mm: 3500.0,
                support_filament_mm: 370.0,
                model_filament_g: 10.61,
                support_filament_g: 1.12,
            },
            features: vec![
                FeatureStatistics {
                    feature: "Outer wall".to_string(),
                    feature_type: "outer_wall".to_string(),
                    time_seconds: 800.0,
                    time_pct_total: 34.81,
                    time_pct_print: 40.0,
                    filament_mm: 1500.0,
                    filament_m: 1.5,
                    filament_g: 4.55,
                    filament_pct_total: 38.76,
                    filament_pct_print: 38.76,
                    segment_count: 5000,
                    distance_mm: 8000.0,
                    is_support: false,
                    display: true,
                },
                FeatureStatistics {
                    feature: "Inner wall".to_string(),
                    feature_type: "inner_wall".to_string(),
                    time_seconds: 600.0,
                    time_pct_total: 26.11,
                    time_pct_print: 30.0,
                    filament_mm: 1200.0,
                    filament_m: 1.2,
                    filament_g: 3.64,
                    filament_pct_total: 31.01,
                    filament_pct_print: 31.01,
                    segment_count: 4000,
                    distance_mm: 6000.0,
                    is_support: false,
                    display: true,
                },
                FeatureStatistics {
                    feature: "Sparse infill".to_string(),
                    feature_type: "sparse_infill".to_string(),
                    time_seconds: 400.0,
                    time_pct_total: 17.41,
                    time_pct_print: 20.0,
                    filament_mm: 800.0,
                    filament_m: 0.8,
                    filament_g: 2.42,
                    filament_pct_total: 20.67,
                    filament_pct_print: 20.67,
                    segment_count: 3000,
                    distance_mm: 5000.0,
                    is_support: false,
                    display: true,
                },
                FeatureStatistics {
                    feature: "Support".to_string(),
                    feature_type: "support".to_string(),
                    time_seconds: 200.0,
                    time_pct_total: 8.70,
                    time_pct_print: 10.0,
                    filament_mm: 370.0,
                    filament_m: 0.37,
                    filament_g: 1.12,
                    filament_pct_total: 9.56,
                    filament_pct_print: 9.56,
                    segment_count: 2000,
                    distance_mm: 3000.0,
                    is_support: true,
                    display: true,
                },
                FeatureStatistics {
                    feature: "Travel".to_string(),
                    feature_type: "travel".to_string(),
                    time_seconds: 298.0,
                    time_pct_total: 12.97,
                    time_pct_print: 0.0,
                    filament_mm: 0.0,
                    filament_m: 0.0,
                    filament_g: 0.0,
                    filament_pct_total: 0.0,
                    filament_pct_print: 0.0,
                    segment_count: 1000,
                    distance_mm: 5400.0,
                    is_support: false,
                    display: true,
                },
            ],
            gcode_metrics: GcodeMetrics {
                retraction_count: 450,
                total_retraction_distance_mm: 360.0,
                unretraction_count: 450,
                wipe_count: 0,
                total_wipe_distance_mm: 0.0,
                z_hop_count: 100,
                total_z_hop_distance_mm: 40.0,
                total_move_count: 14000,
            },
        }
    }

    // -----------------------------------------------------------------------
    // format_time tests
    // -----------------------------------------------------------------------

    #[test]
    fn format_time_zero() {
        assert_eq!(format_time(0.0, &TimePrecision::Seconds), "0s");
    }

    #[test]
    fn format_time_seconds_only() {
        assert_eq!(format_time(45.0, &TimePrecision::Seconds), "45s");
    }

    #[test]
    fn format_time_minutes_and_seconds() {
        assert_eq!(format_time(200.0, &TimePrecision::Seconds), "3m20s");
    }

    #[test]
    fn format_time_hours_minutes_seconds() {
        assert_eq!(format_time(4530.0, &TimePrecision::Seconds), "1h15m30s");
    }

    #[test]
    fn format_time_deciseconds() {
        // 38m18.3s
        assert_eq!(format_time(2298.3, &TimePrecision::Deciseconds), "38m18.3s");
    }

    #[test]
    fn format_time_milliseconds() {
        assert_eq!(
            format_time(2298.312, &TimePrecision::Milliseconds),
            "38m18.312s"
        );
    }

    #[test]
    fn format_time_all_precisions_45s() {
        assert_eq!(format_time(45.0, &TimePrecision::Seconds), "45s");
        assert_eq!(format_time(45.0, &TimePrecision::Deciseconds), "45.0s");
        assert_eq!(format_time(45.0, &TimePrecision::Milliseconds), "45.000s");
    }

    // -----------------------------------------------------------------------
    // format_length tests
    // -----------------------------------------------------------------------

    #[test]
    fn format_length_mm() {
        assert_eq!(format_length(450.2), "450.2mm");
    }

    #[test]
    fn format_length_meters() {
        assert_eq!(format_length(3870.0), "3.87m");
    }

    #[test]
    fn format_length_exactly_1000() {
        assert_eq!(format_length(1000.0), "1.00m");
    }

    #[test]
    fn format_length_below_1000() {
        assert_eq!(format_length(999.9), "999.9mm");
    }

    // -----------------------------------------------------------------------
    // format_filament tests
    // -----------------------------------------------------------------------

    #[test]
    fn format_filament_meters_and_grams() {
        assert_eq!(format_filament(3870.0, 11.73), "3.87m / 11.73g");
    }

    #[test]
    fn format_filament_mm_and_grams() {
        assert_eq!(format_filament(450.2, 1.36), "450.2mm / 1.36g");
    }

    // -----------------------------------------------------------------------
    // format_csv tests
    // -----------------------------------------------------------------------

    #[test]
    fn format_csv_correct_header() {
        let stats = make_test_stats();
        let csv = format_csv(&stats, &StatsSortOrder::Default);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(
            lines[0],
            "feature,time_s,time_pct_total,time_pct_print,filament_mm,filament_g,filament_pct_total,filament_pct_print"
        );
    }

    #[test]
    fn format_csv_correct_row_count() {
        let stats = make_test_stats();
        let csv = format_csv(&stats, &StatsSortOrder::Default);
        let lines: Vec<&str> = csv.lines().collect();
        // 1 header + 5 feature rows = 6 lines.
        assert_eq!(lines.len(), 6, "Should have header + 5 feature rows");
    }

    #[test]
    fn format_csv_data_row_content() {
        let stats = make_test_stats();
        let csv = format_csv(&stats, &StatsSortOrder::Default);
        let lines: Vec<&str> = csv.lines().collect();
        // First data row: outer_wall
        assert!(
            lines[1].starts_with("outer_wall,"),
            "First data row should be outer_wall, got: {}",
            lines[1]
        );
    }

    // -----------------------------------------------------------------------
    // format_ascii_table tests
    // -----------------------------------------------------------------------

    #[test]
    fn format_ascii_table_has_summary_section() {
        let stats = make_test_stats();
        let output = format_ascii_table(&stats, &TimePrecision::Seconds, &StatsSortOrder::Default);
        assert!(output.contains("=== Slicing Statistics ==="));
        assert!(output.contains("Total time:"));
        assert!(output.contains("Print time:"));
        assert!(output.contains("Filament:"));
        assert!(output.contains("Cost:"));
        assert!(output.contains("Layers:"));
    }

    #[test]
    fn format_ascii_table_has_feature_table() {
        let stats = make_test_stats();
        let output = format_ascii_table(&stats, &TimePrecision::Seconds, &StatsSortOrder::Default);
        assert!(output.contains("Outer wall"));
        assert!(output.contains("Inner wall"));
        assert!(output.contains("Sparse infill"));
    }

    #[test]
    fn format_ascii_table_has_model_total() {
        let stats = make_test_stats();
        let output = format_ascii_table(&stats, &TimePrecision::Seconds, &StatsSortOrder::Default);
        assert!(
            output.contains("-- Model total --"),
            "Should contain model total row"
        );
    }

    #[test]
    fn format_ascii_table_has_support_total_when_support_present() {
        let stats = make_test_stats();
        let output = format_ascii_table(&stats, &TimePrecision::Seconds, &StatsSortOrder::Default);
        assert!(
            output.contains("-- Support total --"),
            "Should contain support total when support time > 0"
        );
    }

    #[test]
    fn format_ascii_table_has_overall_total() {
        let stats = make_test_stats();
        let output = format_ascii_table(&stats, &TimePrecision::Seconds, &StatsSortOrder::Default);
        assert!(
            output.contains("-- Overall total --"),
            "Should contain overall total row"
        );
    }

    #[test]
    fn format_ascii_table_has_gcode_metrics() {
        let stats = make_test_stats();
        let output = format_ascii_table(&stats, &TimePrecision::Seconds, &StatsSortOrder::Default);
        assert!(output.contains("Unretractions:"));
        assert!(output.contains("Wipes:"));
    }

    #[test]
    fn format_ascii_table_nonempty() {
        let stats = make_test_stats();
        let output = format_ascii_table(&stats, &TimePrecision::Seconds, &StatsSortOrder::Default);
        assert!(
            output.len() > 100,
            "ASCII table should have substantial output, got {} bytes",
            output.len()
        );
    }

    // -----------------------------------------------------------------------
    // format_json test
    // -----------------------------------------------------------------------

    #[test]
    fn format_json_valid() {
        let stats = make_test_stats();
        let json = format_json(&stats);
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Should be valid JSON");
        assert!(parsed.get("summary").is_some());
        assert!(parsed.get("features").is_some());
        assert!(parsed.get("gcode_metrics").is_some());
    }

    // -----------------------------------------------------------------------
    // sort_features tests
    // -----------------------------------------------------------------------

    #[test]
    fn sort_features_time_desc() {
        let stats = make_test_stats();
        let sorted = sort_features(&stats.features, &StatsSortOrder::TimeDesc);
        // Outer wall (800) > Inner wall (600) > Sparse infill (400) > Travel (298) > Support (200)
        assert_eq!(sorted[0].feature, "Outer wall");
        assert_eq!(sorted[1].feature, "Inner wall");
    }

    #[test]
    fn sort_features_alphabetical() {
        let stats = make_test_stats();
        let sorted = sort_features(&stats.features, &StatsSortOrder::Alphabetical);
        // Alphabetical: Inner wall, Outer wall, Sparse infill, Support, Travel
        assert_eq!(sorted[0].feature, "Inner wall");
        assert_eq!(sorted[1].feature, "Outer wall");
    }

    // -----------------------------------------------------------------------
    // parse helpers tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_time_precision_variants() {
        assert!(matches!(
            parse_time_precision("seconds"),
            TimePrecision::Seconds
        ));
        assert!(matches!(
            parse_time_precision("deciseconds"),
            TimePrecision::Deciseconds
        ));
        assert!(matches!(
            parse_time_precision("milliseconds"),
            TimePrecision::Milliseconds
        ));
        assert!(matches!(
            parse_time_precision("unknown"),
            TimePrecision::Seconds
        ));
    }

    #[test]
    fn parse_sort_order_variants() {
        assert!(matches!(
            parse_sort_order("default"),
            StatsSortOrder::Default
        ));
        assert!(matches!(
            parse_sort_order("time"),
            StatsSortOrder::TimeDesc
        ));
        assert!(matches!(
            parse_sort_order("filament"),
            StatsSortOrder::FilamentDesc
        ));
        assert!(matches!(
            parse_sort_order("alpha"),
            StatsSortOrder::Alphabetical
        ));
    }
}
