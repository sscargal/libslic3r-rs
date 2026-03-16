//! Display formatting for G-code analysis and comparison output.
//!
//! Provides ASCII table, CSV, JSON, and markdown output formats for [`GcodeAnalysis`]
//! and [`ComparisonResult`]. The ASCII table uses `comfy-table` for auto-sizing
//! columns, with ANSI color support for comparison deltas.
//!
//! Also provides multi-config comparison display for side-by-side filament
//! cost/time comparison across different material profiles.

use std::io::IsTerminal;

use comfy_table::{ContentArrangement, Table};
use serde::Serialize;

use slicecore_engine::cost_model::{CostEstimate, VolumeEstimate};
use slicecore_engine::{ComparisonResult, GcodeAnalysis, TimePrecision};

use crate::stats_display::{format_length, format_time};

// ---------------------------------------------------------------------------
// Output format enum
// ---------------------------------------------------------------------------

/// Output format for CLI display commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// ASCII table using comfy-table.
    Table,
    /// Pretty-printed JSON.
    Json,
    /// Comma-separated values.
    Csv,
    /// Markdown table.
    Markdown,
}

/// Determines the output format from boolean flags.
///
/// Defaults to `Table` when no flags are set. Only one should be set at a time;
/// priority is JSON > CSV > Markdown > Table.
#[must_use]
pub fn determine_output_format(json: bool, csv: bool, markdown: bool) -> OutputFormat {
    if json {
        OutputFormat::Json
    } else if csv {
        OutputFormat::Csv
    } else if markdown {
        OutputFormat::Markdown
    } else {
        OutputFormat::Table
    }
}

// ---------------------------------------------------------------------------
// Multi-config comparison types and display
// ---------------------------------------------------------------------------

/// A row in the multi-config comparison table.
#[derive(Debug, Clone, Serialize)]
pub struct ComparisonRow {
    /// Config/filament name.
    pub name: String,
    /// Estimated print time in seconds.
    pub time_seconds: f64,
    /// Filament weight in grams.
    pub filament_weight_g: f64,
    /// Filament material cost (if computable).
    pub filament_cost: Option<f64>,
    /// Total estimated cost (if computable).
    pub total_cost: Option<f64>,
}

/// Display a multi-config comparison table.
///
/// The first row is treated as the baseline. Subsequent rows show deltas
/// from that baseline for each numeric column.
pub fn display_config_comparison(
    rows: &[ComparisonRow],
    format: OutputFormat,
    no_color: bool,
) {
    match format {
        OutputFormat::Table => display_config_comparison_table(rows, no_color),
        OutputFormat::Json => display_config_comparison_json(rows),
        OutputFormat::Csv => display_config_comparison_csv(rows),
        OutputFormat::Markdown => display_config_comparison_markdown(rows),
    }
}

/// Display multi-config comparison as ASCII table with deltas.
fn display_config_comparison_table(rows: &[ComparisonRow], no_color: bool) {
    let use_color = !no_color && std::io::stdout().is_terminal();

    println!("{}", bold("=== Multi-Config Comparison ===", use_color));
    println!();

    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec![
        "Config",
        "Time",
        "Delta Time",
        "Weight (g)",
        "Delta Weight",
        "Filament Cost",
        "Delta Cost",
        "Total Cost",
        "Delta Total",
    ]);

    let baseline_time = rows.first().map_or(0.0, |r| r.time_seconds);
    let baseline_weight = rows.first().map_or(0.0, |r| r.filament_weight_g);
    let baseline_fcost = rows.first().and_then(|r| r.filament_cost);
    let baseline_tcost = rows.first().and_then(|r| r.total_cost);
    let prec = TimePrecision::Seconds;

    for (i, row) in rows.iter().enumerate() {
        let time_str = format_time(row.time_seconds, &prec);
        let weight_str = format!("{:.1}", row.filament_weight_g);
        let fcost_str = row
            .filament_cost
            .map_or("N/A".to_string(), |c| format!("${c:.2}"));
        let tcost_str = row
            .total_cost
            .map_or("N/A".to_string(), |c| format!("${c:.2}"));

        if i == 0 {
            table.add_row(vec![
                row.name.clone(),
                time_str,
                dim("(baseline)", use_color),
                weight_str,
                dim("(baseline)", use_color),
                fcost_str,
                dim("(baseline)", use_color),
                tcost_str,
                dim("(baseline)", use_color),
            ]);
        } else {
            let dt = row.time_seconds - baseline_time;
            let dt_pct = if baseline_time.abs() > 1e-12 {
                (dt / baseline_time) * 100.0
            } else {
                0.0
            };
            let dw = row.filament_weight_g - baseline_weight;
            let dw_pct = if baseline_weight.abs() > 1e-12 {
                (dw / baseline_weight) * 100.0
            } else {
                0.0
            };
            let dfc = match (row.filament_cost, baseline_fcost) {
                (Some(c), Some(b)) => {
                    let d = c - b;
                    let sign = if d > 0.0 { "+" } else { "" };
                    format!("{sign}${d:.2}")
                }
                _ => "-".to_string(),
            };
            let dtc = match (row.total_cost, baseline_tcost) {
                (Some(c), Some(b)) => {
                    let d = c - b;
                    let sign = if d > 0.0 { "+" } else { "" };
                    format!("{sign}${d:.2}")
                }
                _ => "-".to_string(),
            };

            table.add_row(vec![
                row.name.clone(),
                time_str,
                color_delta(dt, dt_pct, "s", use_color),
                weight_str,
                color_delta(dw, dw_pct, "g", use_color),
                fcost_str,
                dfc,
                tcost_str,
                dtc,
            ]);
        }
    }

    println!("{table}");
}

/// Display multi-config comparison as JSON.
fn display_config_comparison_json(rows: &[ComparisonRow]) {
    println!(
        "{}",
        serde_json::to_string_pretty(rows)
            .unwrap_or_else(|e| format!("{{\"error\": \"Failed to serialize: {e}\"}}"))
    );
}

/// Display multi-config comparison as CSV.
fn display_config_comparison_csv(rows: &[ComparisonRow]) {
    println!("config,time_s,weight_g,filament_cost,total_cost");
    for row in rows {
        let fc = row
            .filament_cost
            .map_or(String::new(), |c| format!("{c:.2}"));
        let tc = row.total_cost.map_or(String::new(), |c| format!("{c:.2}"));
        println!(
            "{},{:.1},{:.1},{},{}",
            row.name, row.time_seconds, row.filament_weight_g, fc, tc
        );
    }
}

/// Display multi-config comparison as markdown table.
fn display_config_comparison_markdown(rows: &[ComparisonRow]) {
    println!("## Multi-Config Comparison");
    println!();
    println!("| Config | Time | Weight (g) | Filament Cost | Total Cost |");
    println!("|--------|------|------------|---------------|------------|");
    let prec = TimePrecision::Seconds;
    for row in rows {
        let fc = row
            .filament_cost
            .map_or("N/A".to_string(), |c| format!("${c:.2}"));
        let tc = row.total_cost.map_or("N/A".to_string(), |c| format!("${c:.2}"));
        println!(
            "| {} | {} | {:.1} | {} | {} |",
            row.name,
            format_time(row.time_seconds, &prec),
            row.filament_weight_g,
            fc,
            tc
        );
    }
}

// ---------------------------------------------------------------------------
// ANSI color helpers
// ---------------------------------------------------------------------------

/// Bold text wrapper.
fn bold(s: &str, use_color: bool) -> String {
    if use_color {
        format!("\x1b[1m{}\x1b[0m", s)
    } else {
        s.to_string()
    }
}

/// Green text (improvement / less).
fn green(s: &str, use_color: bool) -> String {
    if use_color {
        format!("\x1b[32m{}\x1b[0m", s)
    } else {
        s.to_string()
    }
}

/// Red text (regression / more).
fn red(s: &str, use_color: bool) -> String {
    if use_color {
        format!("\x1b[31m{}\x1b[0m", s)
    } else {
        s.to_string()
    }
}

/// Dim text (for zero values).
fn dim(s: &str, use_color: bool) -> String {
    if use_color {
        format!("\x1b[90m{}\x1b[0m", s)
    } else {
        s.to_string()
    }
}

/// Color-code a delta value. Green for negative (less), red for positive (more).
/// Only applies color when absolute percentage exceeds threshold.
fn color_delta(value: f64, pct: f64, unit: &str, use_color: bool) -> String {
    let sign = if value > 0.0 { "+" } else { "" };
    let text = format!("{}{:.1}{} ({}{:.1}%)", sign, value, unit, sign, pct);
    if pct.abs() > 5.0 {
        if value < 0.0 {
            green(&text, use_color)
        } else {
            red(&text, use_color)
        }
    } else {
        text
    }
}

/// Color-code a delta value for integer types.
fn color_delta_int(value: i64, use_color: bool) -> String {
    let sign = if value > 0 { "+" } else { "" };
    let text = format!("{}{}", sign, value);
    if value.abs() > 0 {
        if value < 0 {
            green(&text, use_color)
        } else if value > 0 {
            red(&text, use_color)
        } else {
            text
        }
    } else {
        text
    }
}

// ---------------------------------------------------------------------------
// Analysis display functions
// ---------------------------------------------------------------------------

/// Display a G-code analysis as an ASCII table.
///
/// Shows a summary header with file metadata, a per-feature breakdown table,
/// and optionally a per-layer detail table.
pub fn display_analysis_table(
    analysis: &GcodeAnalysis,
    use_color: bool,
    summary_only: bool,
    filter: &Option<Vec<String>>,
) {
    let prec = TimePrecision::Seconds;

    // Summary header
    println!("{}", bold("=== G-code Analysis ===", use_color));
    println!("File:              {}", bold(&analysis.filename, use_color));
    println!(
        "Slicer:            {:?} {}",
        analysis.slicer,
        analysis.header.slicer_version.as_deref().unwrap_or("")
    );
    println!("Layers:            {}", analysis.layers.len());
    println!(
        "Total time (est):  {}",
        format_time(analysis.total_time_estimate_s, &prec)
    );
    if let Some(header_time) = analysis.header.estimated_time_s {
        let delta = analysis.total_time_estimate_s - header_time;
        let sign = if delta > 0.0 { "+" } else { "" };
        println!(
            "Total time (hdr):  {} (delta: {}{}s)",
            format_time(header_time, &prec),
            sign,
            delta.round() as i64
        );
    }
    println!(
        "Filament:          {} ({:.1} mm\u{00b3}, {:.2}g)",
        format_length(analysis.total_filament_mm),
        analysis.total_filament_volume_mm3,
        analysis.total_filament_weight_g
    );
    println!(
        "Travel distance:   {}",
        format_length(analysis.total_travel_mm)
    );
    println!(
        "Retractions:       {} ({})",
        analysis.retraction_count,
        format_length(analysis.retraction_distance_mm)
    );
    println!(
        "Z-hops:            {} ({})",
        analysis.zhop_count,
        format_length(analysis.zhop_distance_mm)
    );
    println!("Unknown commands:  {}", analysis.unknown_command_count);
    println!();

    // Per-feature summary table
    println!("{}", bold("--- Feature Summary ---", use_color));

    let mut feature_table = Table::new();
    feature_table.set_content_arrangement(ContentArrangement::Dynamic);
    feature_table.set_header(vec![
        "Feature",
        "Time",
        "Time%",
        "Filament",
        "Moves",
        "Speed min",
        "Speed max",
        "Speed avg",
    ]);

    // Sort features by time descending
    let mut features: Vec<(&String, &slicecore_engine::FeatureMetrics)> =
        analysis.features.iter().collect();
    features.sort_by(|a, b| {
        b.1.time_estimate_s
            .partial_cmp(&a.1.time_estimate_s)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let total_time = analysis.total_time_estimate_s;

    for (name, metrics) in &features {
        // Apply filter if provided
        if let Some(ref filter_list) = filter {
            let name_lower = name.to_lowercase();
            if !filter_list
                .iter()
                .any(|f| name_lower.contains(&f.to_lowercase()))
            {
                continue;
            }
        }

        let time_pct = if total_time > 0.0 {
            metrics.time_estimate_s / total_time * 100.0
        } else {
            0.0
        };

        let speed_min = if metrics.speed_stats.sample_count > 0 {
            format!("{:.0}", metrics.speed_stats.min_mm_s)
        } else {
            dim("-", use_color)
        };
        let speed_max = if metrics.speed_stats.sample_count > 0 {
            format!("{:.0}", metrics.speed_stats.max_mm_s)
        } else {
            dim("-", use_color)
        };
        let speed_avg = if metrics.speed_stats.sample_count > 0 {
            format!("{:.0}", metrics.speed_stats.mean_mm_s)
        } else {
            dim("-", use_color)
        };

        let row_data = vec![
            name.to_string(),
            format_time(metrics.time_estimate_s, &prec),
            format!("{:.1}%", time_pct),
            format_length(metrics.extrusion_e_mm),
            format!("{}", metrics.move_count),
            speed_min,
            speed_max,
            speed_avg,
        ];

        // Dim zero-value rows
        if metrics.time_estimate_s < 0.01 && metrics.extrusion_e_mm < 0.01 {
            let dimmed: Vec<String> = row_data.into_iter().map(|s| dim(&s, use_color)).collect();
            feature_table.add_row(dimmed);
        } else {
            feature_table.add_row(row_data);
        }
    }

    println!("{}", feature_table);
    println!();

    // Per-layer detail table (default view, unless --summary)
    if !summary_only {
        println!("{}", bold("--- Per-Layer Detail ---", use_color));

        let mut layer_table = Table::new();
        layer_table.set_content_arrangement(ContentArrangement::Dynamic);
        layer_table.set_header(vec![
            "Layer",
            "Z",
            "Height",
            "Moves",
            "Travel",
            "Extrusion",
            "Retractions",
            "Time",
        ]);

        for (i, layer) in analysis.layers.iter().enumerate() {
            layer_table.add_row(vec![
                format!("{}", i + 1),
                format!("{:.3}", layer.z_height),
                format!("{:.3}", layer.layer_height),
                format!("{}", layer.move_count),
                format_length(layer.travel_distance_mm),
                format_length(layer.extrusion_distance_mm),
                format!("{}", layer.retraction_count),
                format_time(layer.layer_time_estimate_s, &prec),
            ]);
        }

        println!("{}", layer_table);
    }
}

/// Display a G-code analysis as pretty-printed JSON.
pub fn display_analysis_json(analysis: &GcodeAnalysis) {
    println!(
        "{}",
        serde_json::to_string_pretty(analysis)
            .unwrap_or_else(|e| { format!("{{\"error\": \"Failed to serialize: {}\"}}", e) })
    );
}

/// Display a G-code analysis as CSV.
///
/// If `summary_only`, outputs per-feature summary rows.
/// Otherwise outputs per-layer detail rows.
pub fn display_analysis_csv(analysis: &GcodeAnalysis, summary_only: bool) {
    if summary_only {
        // Feature summary CSV
        println!("feature,time_s,time_pct,filament_mm,moves,speed_min,speed_max,speed_mean");

        let total_time = analysis.total_time_estimate_s;
        let mut features: Vec<(&String, &slicecore_engine::FeatureMetrics)> =
            analysis.features.iter().collect();
        features.sort_by(|a, b| {
            b.1.time_estimate_s
                .partial_cmp(&a.1.time_estimate_s)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for (name, metrics) in &features {
            let time_pct = if total_time > 0.0 {
                metrics.time_estimate_s / total_time * 100.0
            } else {
                0.0
            };
            let speed_min = if metrics.speed_stats.sample_count > 0 {
                metrics.speed_stats.min_mm_s
            } else {
                0.0
            };
            let speed_max = if metrics.speed_stats.sample_count > 0 {
                metrics.speed_stats.max_mm_s
            } else {
                0.0
            };
            let speed_mean = if metrics.speed_stats.sample_count > 0 {
                metrics.speed_stats.mean_mm_s
            } else {
                0.0
            };
            println!(
                "{},{:.3},{:.2},{:.3},{},{:.1},{:.1},{:.1}",
                name,
                metrics.time_estimate_s,
                time_pct,
                metrics.extrusion_e_mm,
                metrics.move_count,
                speed_min,
                speed_max,
                speed_mean
            );
        }
    } else {
        // Per-layer CSV
        println!(
            "layer,z_height,layer_height,moves,travel_mm,extrusion_mm,retraction_count,time_s"
        );
        for (i, layer) in analysis.layers.iter().enumerate() {
            println!(
                "{},{:.3},{:.3},{},{:.3},{:.3},{},{:.3}",
                i + 1,
                layer.z_height,
                layer.layer_height,
                layer.move_count,
                layer.travel_distance_mm,
                layer.extrusion_distance_mm,
                layer.retraction_count,
                layer.layer_time_estimate_s
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Comparison display functions
// ---------------------------------------------------------------------------

/// Display a comparison result as an ASCII table.
///
/// Shows a summary comparison table with delta columns for each file,
/// plus a per-feature comparison table.
pub fn display_comparison_table(result: &ComparisonResult, use_color: bool) {
    let prec = TimePrecision::Seconds;

    println!("{}", bold("=== G-code Comparison ===", use_color));
    println!();

    // Build header columns: Metric | baseline | file2 | Delta2 | file3 | Delta3 | ...
    let mut headers: Vec<String> = vec!["Metric".to_string()];
    headers.push(
        result
            .baseline
            .filename
            .rsplit('/')
            .next()
            .unwrap_or(&result.baseline.filename)
            .to_string(),
    );
    for delta in &result.deltas {
        let short_name = delta
            .filename
            .rsplit('/')
            .next()
            .unwrap_or(&delta.filename)
            .to_string();
        headers.push(short_name);
        headers.push("Delta".to_string());
    }

    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(headers);

    // Row helper: builds a row with metric name, baseline value, then for each other: value + delta
    // Total time
    {
        let mut row: Vec<String> = vec![
            "Total time".to_string(),
            format_time(result.baseline.total_time_estimate_s, &prec),
        ];
        for (i, delta) in result.deltas.iter().enumerate() {
            let other_time = result.others[i].total_time_estimate_s;
            row.push(format_time(other_time, &prec));
            row.push(color_delta(
                delta.total_time_delta_s,
                delta.total_time_delta_pct,
                "s",
                use_color,
            ));
        }
        table.add_row(row);
    }

    // Header time
    if result.baseline.header.estimated_time_s.is_some() {
        let mut row: Vec<String> = vec![
            "Header time".to_string(),
            format_time(
                result.baseline.header.estimated_time_s.unwrap_or(0.0),
                &prec,
            ),
        ];
        for (i, delta) in result.deltas.iter().enumerate() {
            let other_time = result.others[i].header.estimated_time_s.unwrap_or(0.0);
            row.push(format_time(other_time, &prec));
            if let (Some(ds), Some(dp)) = (delta.header_time_delta_s, delta.header_time_delta_pct) {
                row.push(color_delta(ds, dp, "s", use_color));
            } else {
                row.push("-".to_string());
            }
        }
        table.add_row(row);
    }

    // Filament
    {
        let mut row: Vec<String> = vec![
            "Filament".to_string(),
            format_length(result.baseline.total_filament_mm),
        ];
        for (i, delta) in result.deltas.iter().enumerate() {
            row.push(format_length(result.others[i].total_filament_mm));
            row.push(color_delta(
                delta.filament_delta_mm,
                delta.filament_delta_pct,
                "mm",
                use_color,
            ));
        }
        table.add_row(row);
    }

    // Weight
    {
        let mut row: Vec<String> = vec![
            "Weight".to_string(),
            format!("{:.2}g", result.baseline.total_filament_weight_g),
        ];
        for (i, delta) in result.deltas.iter().enumerate() {
            row.push(format!("{:.2}g", result.others[i].total_filament_weight_g));
            let base_w = result.baseline.total_filament_weight_g;
            let other_w = result.others[i].total_filament_weight_g;
            let pct = if base_w.abs() > 1e-12 {
                ((other_w - base_w) / base_w) * 100.0
            } else {
                0.0
            };
            row.push(color_delta(
                delta.filament_weight_delta_g,
                pct,
                "g",
                use_color,
            ));
        }
        table.add_row(row);
    }

    // Layers
    {
        let mut row: Vec<String> = vec![
            "Layers".to_string(),
            format!("{}", result.baseline.layers.len()),
        ];
        for (i, delta) in result.deltas.iter().enumerate() {
            row.push(format!("{}", result.others[i].layers.len()));
            row.push(color_delta_int(delta.layer_count_delta, use_color));
        }
        table.add_row(row);
    }

    // Retractions
    {
        let mut row: Vec<String> = vec![
            "Retractions".to_string(),
            format!("{}", result.baseline.retraction_count),
        ];
        for (i, delta) in result.deltas.iter().enumerate() {
            row.push(format!("{}", result.others[i].retraction_count));
            row.push(color_delta_int(
                delta.retraction_count_delta as i64,
                use_color,
            ));
        }
        table.add_row(row);
    }

    // Travel
    {
        let mut row: Vec<String> = vec![
            "Travel".to_string(),
            format_length(result.baseline.total_travel_mm),
        ];
        for (i, delta) in result.deltas.iter().enumerate() {
            row.push(format_length(result.others[i].total_travel_mm));
            let base = result.baseline.total_travel_mm;
            let other = result.others[i].total_travel_mm;
            let pct = if base.abs() > 1e-12 {
                ((other - base) / base) * 100.0
            } else {
                0.0
            };
            row.push(color_delta(
                delta.total_travel_delta_mm,
                pct,
                "mm",
                use_color,
            ));
        }
        table.add_row(row);
    }

    // Extrusion
    {
        let mut row: Vec<String> = vec![
            "Extrusion dist".to_string(),
            format_length(result.baseline.total_extrusion_mm),
        ];
        for (i, delta) in result.deltas.iter().enumerate() {
            row.push(format_length(result.others[i].total_extrusion_mm));
            let base = result.baseline.total_extrusion_mm;
            let other = result.others[i].total_extrusion_mm;
            let pct = if base.abs() > 1e-12 {
                ((other - base) / base) * 100.0
            } else {
                0.0
            };
            row.push(color_delta(
                delta.total_extrusion_delta_mm,
                pct,
                "mm",
                use_color,
            ));
        }
        table.add_row(row);
    }

    // Moves
    {
        let mut row: Vec<String> = vec![
            "Total moves".to_string(),
            format!("{}", result.baseline.total_moves),
        ];
        for (i, delta) in result.deltas.iter().enumerate() {
            row.push(format!("{}", result.others[i].total_moves));
            row.push(color_delta_int(delta.total_moves_delta, use_color));
        }
        table.add_row(row);
    }

    println!("{}", table);
    println!();

    // Per-feature comparison table
    println!("{}", bold("--- Per-Feature Comparison ---", use_color));

    // Collect union of all feature keys
    let mut all_features: Vec<String> = result.baseline.features.keys().cloned().collect();
    for other in &result.others {
        for key in other.features.keys() {
            if !all_features.contains(key) {
                all_features.push(key.clone());
            }
        }
    }
    all_features.sort();

    // Build header
    let mut feat_headers: Vec<String> = vec!["Feature".to_string()];
    let baseline_short = result
        .baseline
        .filename
        .rsplit('/')
        .next()
        .unwrap_or(&result.baseline.filename)
        .to_string();
    feat_headers.push(format!("{} time", baseline_short));
    for delta in &result.deltas {
        let short = delta
            .filename
            .rsplit('/')
            .next()
            .unwrap_or(&delta.filename)
            .to_string();
        feat_headers.push(format!("{} time", short));
        feat_headers.push("Delta".to_string());
    }

    let mut feat_table = Table::new();
    feat_table.set_content_arrangement(ContentArrangement::Dynamic);
    feat_table.set_header(feat_headers);

    for feature_name in &all_features {
        let mut row: Vec<String> = vec![feature_name.clone()];
        let base_time = result
            .baseline
            .features
            .get(feature_name)
            .map(|f| f.time_estimate_s)
            .unwrap_or(0.0);
        row.push(format_time(base_time, &prec));

        for (i, delta) in result.deltas.iter().enumerate() {
            let other_time = result.others[i]
                .features
                .get(feature_name)
                .map(|f| f.time_estimate_s)
                .unwrap_or(0.0);
            row.push(format_time(other_time, &prec));

            if let Some(fd) = delta.feature_deltas.get(feature_name) {
                row.push(color_delta(
                    fd.time_delta_s,
                    fd.time_delta_pct,
                    "s",
                    use_color,
                ));
            } else {
                row.push("-".to_string());
            }
        }

        feat_table.add_row(row);
    }

    println!("{}", feat_table);
}

/// Display a comparison result as pretty-printed JSON.
pub fn display_comparison_json(result: &ComparisonResult) {
    println!(
        "{}",
        serde_json::to_string_pretty(result)
            .unwrap_or_else(|e| { format!("{{\"error\": \"Failed to serialize: {}\"}}", e) })
    );
}

/// Display a comparison result as CSV.
///
/// Format: `metric,baseline,file2,delta2,delta2_pct,file3,delta3,delta3_pct,...`
pub fn display_comparison_csv(result: &ComparisonResult) {
    // Build header
    let mut header = "metric,baseline".to_string();
    for delta in &result.deltas {
        let short = delta.filename.rsplit('/').next().unwrap_or(&delta.filename);
        header.push_str(&format!(",{},delta,delta_pct", short));
    }
    println!("{}", header);

    // Total time
    {
        let mut row = format!("total_time,{:.3}", result.baseline.total_time_estimate_s);
        for (i, delta) in result.deltas.iter().enumerate() {
            row.push_str(&format!(
                ",{:.3},{:.3},{:.2}",
                result.others[i].total_time_estimate_s,
                delta.total_time_delta_s,
                delta.total_time_delta_pct
            ));
        }
        println!("{}", row);
    }

    // Header time
    {
        let baseline_ht = result.baseline.header.estimated_time_s.unwrap_or(0.0);
        let mut row = format!("header_time,{:.3}", baseline_ht);
        for (i, delta) in result.deltas.iter().enumerate() {
            let other_ht = result.others[i].header.estimated_time_s.unwrap_or(0.0);
            let ds = delta.header_time_delta_s.unwrap_or(0.0);
            let dp = delta.header_time_delta_pct.unwrap_or(0.0);
            row.push_str(&format!(",{:.3},{:.3},{:.2}", other_ht, ds, dp));
        }
        println!("{}", row);
    }

    // Filament
    {
        let mut row = format!("filament_mm,{:.3}", result.baseline.total_filament_mm);
        for (i, delta) in result.deltas.iter().enumerate() {
            row.push_str(&format!(
                ",{:.3},{:.3},{:.2}",
                result.others[i].total_filament_mm,
                delta.filament_delta_mm,
                delta.filament_delta_pct
            ));
        }
        println!("{}", row);
    }

    // Weight
    {
        let mut row = format!(
            "filament_weight_g,{:.4}",
            result.baseline.total_filament_weight_g
        );
        for (i, delta) in result.deltas.iter().enumerate() {
            let base_w = result.baseline.total_filament_weight_g;
            let other_w = result.others[i].total_filament_weight_g;
            let pct = if base_w.abs() > 1e-12 {
                ((other_w - base_w) / base_w) * 100.0
            } else {
                0.0
            };
            row.push_str(&format!(
                ",{:.4},{:.4},{:.2}",
                other_w, delta.filament_weight_delta_g, pct
            ));
        }
        println!("{}", row);
    }

    // Layers
    {
        let mut row = format!("layers,{}", result.baseline.layers.len());
        for (i, delta) in result.deltas.iter().enumerate() {
            row.push_str(&format!(
                ",{},{},",
                result.others[i].layers.len(),
                delta.layer_count_delta
            ));
        }
        println!("{}", row);
    }

    // Retractions
    {
        let mut row = format!("retractions,{}", result.baseline.retraction_count);
        for (i, delta) in result.deltas.iter().enumerate() {
            row.push_str(&format!(
                ",{},{},",
                result.others[i].retraction_count, delta.retraction_count_delta
            ));
        }
        println!("{}", row);
    }

    // Travel
    {
        let mut row = format!("travel_mm,{:.3}", result.baseline.total_travel_mm);
        for (i, delta) in result.deltas.iter().enumerate() {
            let base = result.baseline.total_travel_mm;
            let other = result.others[i].total_travel_mm;
            let pct = if base.abs() > 1e-12 {
                ((other - base) / base) * 100.0
            } else {
                0.0
            };
            row.push_str(&format!(
                ",{:.3},{:.3},{:.2}",
                other, delta.total_travel_delta_mm, pct
            ));
        }
        println!("{}", row);
    }

    // Extrusion
    {
        let mut row = format!("extrusion_mm,{:.3}", result.baseline.total_extrusion_mm);
        for (i, delta) in result.deltas.iter().enumerate() {
            let base = result.baseline.total_extrusion_mm;
            let other = result.others[i].total_extrusion_mm;
            let pct = if base.abs() > 1e-12 {
                ((other - base) / base) * 100.0
            } else {
                0.0
            };
            row.push_str(&format!(
                ",{:.3},{:.3},{:.2}",
                other, delta.total_extrusion_delta_mm, pct
            ));
        }
        println!("{}", row);
    }

    // Moves
    {
        let mut row = format!("total_moves,{}", result.baseline.total_moves);
        for (i, delta) in result.deltas.iter().enumerate() {
            row.push_str(&format!(
                ",{},{},",
                result.others[i].total_moves, delta.total_moves_delta
            ));
        }
        println!("{}", row);
    }
}

// ---------------------------------------------------------------------------
// Cost estimation display functions
// ---------------------------------------------------------------------------

/// Format a cost value as currency with 2 decimal places, or "N/A" with hint.
fn format_cost_cell(value: Option<f64>, hint: Option<&str>) -> String {
    match value {
        Some(v) => format!("${:.2}", v),
        None => match hint {
            Some(h) => format!("N/A ({})", h),
            None => "N/A".to_string(),
        },
    }
}

/// Find the hint associated with a cost component keyword (e.g. "filament", "electricity").
fn find_hint<'a>(hints: &'a [String], keyword: &str) -> Option<&'a str> {
    hints.iter().find(|h| h.contains(keyword)).map(|h| h.as_str())
}

/// Display a cost estimate as an ASCII table.
///
/// Shows each cost component with its computed value or "N/A" with a helpful
/// hint explaining what flag to provide. A total row is shown at the bottom.
pub fn display_cost_table(estimate: &CostEstimate, use_color: bool) {
    println!("{}", bold("--- Cost Estimate ---", use_color));

    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec!["Component", "Cost"]);

    table.add_row(vec![
        "Filament".to_string(),
        format_cost_cell(estimate.filament_cost, find_hint(&estimate.missing_hints, "filament-price")),
    ]);
    table.add_row(vec![
        "Electricity".to_string(),
        format_cost_cell(estimate.electricity_cost, find_hint(&estimate.missing_hints, "electricity")),
    ]);
    table.add_row(vec![
        "Depreciation".to_string(),
        format_cost_cell(estimate.depreciation_cost, find_hint(&estimate.missing_hints, "depreciation")),
    ]);
    table.add_row(vec![
        "Labor".to_string(),
        format_cost_cell(estimate.labor_cost, find_hint(&estimate.missing_hints, "labor")),
    ]);
    table.add_row(vec![
        bold("Total", use_color),
        bold(&format_cost_cell(estimate.total_cost, None), use_color),
    ]);

    println!("{table}");

    if !estimate.missing_hints.is_empty() {
        println!();
        println!(
            "{}",
            dim("Hints for more accurate estimates:", use_color)
        );
        for hint in &estimate.missing_hints {
            println!("  - {hint}");
        }
    }
}

/// Display a cost estimate as JSON.
pub fn display_cost_json(estimate: &CostEstimate) {
    println!(
        "{}",
        serde_json::to_string_pretty(estimate)
            .unwrap_or_else(|e| format!("{{\"error\": \"Failed to serialize: {e}\"}}"))
    );
}

/// Display a cost estimate as CSV with `component,amount,hint` columns.
pub fn display_cost_csv(estimate: &CostEstimate) {
    println!("component,amount,hint");
    let rows: Vec<(&str, Option<f64>, &str)> = vec![
        ("filament", estimate.filament_cost, "filament-price"),
        ("electricity", estimate.electricity_cost, "electricity"),
        ("depreciation", estimate.depreciation_cost, "depreciation"),
        ("labor", estimate.labor_cost, "labor"),
        ("total", estimate.total_cost, ""),
    ];
    for (name, value, keyword) in rows {
        let amount = value.map_or(String::new(), |v| format!("{v:.2}"));
        let hint = if value.is_none() {
            find_hint(&estimate.missing_hints, keyword).unwrap_or("")
        } else {
            ""
        };
        println!("{name},{amount},{hint}");
    }
}

/// Display a cost estimate as a markdown table.
pub fn display_cost_markdown(estimate: &CostEstimate) {
    println!("## Cost Estimate");
    println!();
    println!("| Component | Cost |");
    println!("|-----------|------|");
    let rows: Vec<(&str, Option<f64>, &str)> = vec![
        ("Filament", estimate.filament_cost, "filament-price"),
        ("Electricity", estimate.electricity_cost, "electricity"),
        ("Depreciation", estimate.depreciation_cost, "depreciation"),
        ("Labor", estimate.labor_cost, "labor"),
        ("**Total**", estimate.total_cost, ""),
    ];
    for (name, value, keyword) in rows {
        let cell = format_cost_cell(value, find_hint(&estimate.missing_hints, keyword));
        println!("| {name} | {cell} |");
    }
}

/// Display a volume-based rough estimate as an ASCII table.
///
/// Shows rough filament length, weight, and time with a disclaimer
/// about accuracy limitations.
pub fn display_volume_estimate(estimate: &VolumeEstimate, use_color: bool) {
    println!(
        "{}",
        bold("--- Volume-Based Rough Estimate ---", use_color)
    );

    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec!["Metric", "Value"]);

    table.add_row(vec![
        "Filament length".to_string(),
        format_length(estimate.filament_length_mm),
    ]);
    table.add_row(vec![
        "Filament weight".to_string(),
        format!("{:.2}g", estimate.filament_weight_g),
    ]);
    table.add_row(vec![
        "Rough print time".to_string(),
        format_time(estimate.rough_time_seconds, &TimePrecision::Seconds),
    ]);

    println!("{table}");
    println!();
    println!(
        "{}",
        dim(
            &format!("Disclaimer: {}", estimate.disclaimer),
            use_color
        )
    );
}

/// Display a volume estimate as CSV.
pub fn display_volume_estimate_csv(estimate: &VolumeEstimate) {
    println!("metric,value");
    println!("filament_length_mm,{:.2}", estimate.filament_length_mm);
    println!("filament_weight_g,{:.2}", estimate.filament_weight_g);
    println!("rough_time_seconds,{:.2}", estimate.rough_time_seconds);
    println!("disclaimer,{}", estimate.disclaimer);
}

/// Display a volume estimate as a markdown table.
pub fn display_volume_estimate_markdown(estimate: &VolumeEstimate) {
    println!("## Volume-Based Rough Estimate");
    println!();
    println!("| Metric | Value |");
    println!("|--------|-------|");
    println!(
        "| Filament length | {} |",
        format_length(estimate.filament_length_mm)
    );
    println!("| Filament weight | {:.2}g |", estimate.filament_weight_g);
    println!(
        "| Rough print time | {} |",
        format_time(estimate.rough_time_seconds, &TimePrecision::Seconds)
    );
    println!();
    println!("> Disclaimer: {}", estimate.disclaimer);
}
