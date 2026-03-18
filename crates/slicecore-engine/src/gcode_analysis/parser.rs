//! Line-by-line G-code parser state machine with move tracking.
//!
//! Provides [`parse_gcode_file`] which reads a G-code file via `BufRead`,
//! tracks machine state (position, feedrate, extrusion mode), and accumulates
//! per-layer and per-feature metrics into a [`GcodeAnalysis`] result.

use std::collections::HashMap;
use std::io::BufRead;

use super::metrics::{
    filament_mm_to_volume_mm3, filament_mm_to_weight_g, FeatureMetrics, GcodeAnalysis,
    HeaderMetadata, LayerMetrics,
};
use super::slicer_detect::{detect_feature_format, detect_slicer, FeatureFormat, SlicerType};

/// Internal parser state tracking machine position and mode.
pub struct GcodeParserState {
    // Machine position.
    x: f64,
    y: f64,
    z: f64,
    e: f64,
    feedrate_mm_min: f64,

    // Extrusion mode.
    absolute_extrusion: bool,   // M82=true, M83=false, default true
    absolute_positioning: bool, // G90=true, G91=false, default true

    // Layer tracking.
    current_layer_z: f64,
    current_layer_index: i32, // -1 before first layer
    layer_height: f64,

    // Feature tracking.
    current_feature: Option<String>,
    feature_format: FeatureFormat,

    // Slicer.
    detected_slicer: SlicerType,

    // Counters.
    unknown_command_count: u32,
    line_count: u64,

    // Retraction tracking.
    retraction_count: u32,
    retraction_distance_mm: f64,

    // Z-hop tracking.
    zhop_count: u32,
    zhop_distance_mm: f64,
    in_retraction: bool, // True after retraction, cleared on extrusion
    zhop_z: Option<f64>, // Z before z-hop move, if we are in a z-hop
}

impl GcodeParserState {
    fn new() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            e: 0.0,
            feedrate_mm_min: 0.0,
            absolute_extrusion: true,
            absolute_positioning: true,
            current_layer_z: 0.0,
            current_layer_index: -1,
            layer_height: 0.0,
            current_feature: None,
            feature_format: FeatureFormat::Both,
            detected_slicer: SlicerType::Unknown,
            unknown_command_count: 0,
            line_count: 0,
            retraction_count: 0,
            retraction_distance_mm: 0.0,
            zhop_count: 0,
            zhop_distance_mm: 0.0,
            in_retraction: false,
            zhop_z: None,
        }
    }
}

/// Parse a G-code file and produce a complete analysis.
///
/// Reads the file line-by-line via `BufRead`, maintaining machine state
/// and accumulating per-layer and per-feature metrics.
///
/// # Parameters
///
/// - `reader`: Buffered reader over the G-code file.
/// - `filename`: Name for reporting purposes.
/// - `filament_diameter`: Filament diameter in mm (default 1.75).
/// - `filament_density`: Filament density in g/cm^3 (default 1.24 for PLA).
pub fn parse_gcode_file<R: BufRead>(
    reader: R,
    filename: &str,
    filament_diameter: f64,
    filament_density: f64,
) -> GcodeAnalysis {
    let mut state = GcodeParserState::new();
    let mut header = HeaderMetadata::default();
    let mut layers: Vec<LayerMetrics> = Vec::new();
    let mut current_layer = LayerMetrics::default();

    // Collect first lines for slicer detection.
    let mut first_lines: Vec<String> = Vec::new();
    let mut all_lines: Vec<String> = Vec::new();

    // First pass: collect all lines (streaming would be better for huge files,
    // but we need to scan headers first for slicer detection).
    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };
        if all_lines.len() < 100 {
            first_lines.push(line.clone());
        }
        all_lines.push(line);
    }

    // Detect slicer from first lines.
    let first_refs: Vec<&str> = first_lines.iter().map(|s| s.as_str()).collect();
    state.detected_slicer = detect_slicer(&first_refs);
    state.feature_format = detect_feature_format(state.detected_slicer);

    // Parse header comments for metadata.
    let header_limit = all_lines.len().min(200);
    for line in &all_lines[..header_limit] {
        let trimmed = line.trim();
        if trimmed.starts_with(';') {
            parse_header_comment(trimmed, &mut header);
        }
    }

    // Also scan the end of the file for PrusaSlicer metadata (it puts some
    // stats at the end of the file).
    if all_lines.len() > 200 {
        let tail_start = all_lines.len().saturating_sub(100);
        for line in &all_lines[tail_start..] {
            let trimmed = line.trim();
            if trimmed.starts_with(';') {
                parse_header_comment(trimmed, &mut header);
            }
        }
    }

    // Main parsing pass.
    for line in &all_lines {
        state.line_count += 1;
        parse_line(line, &mut state, &mut current_layer, &mut layers);
    }

    // Finalize last layer.
    if current_layer.move_count > 0 || state.current_layer_index >= 0 {
        layers.push(current_layer);
    }

    // Aggregate per-layer features into total features.
    let mut total_features: HashMap<String, FeatureMetrics> = HashMap::new();
    let mut total_time = 0.0_f64;
    let mut total_travel = 0.0_f64;
    let mut total_extrusion = 0.0_f64;
    let mut total_filament = 0.0_f64;
    let mut total_moves = 0_u64;

    for layer in &layers {
        total_moves += layer.move_count;
        total_travel += layer.travel_distance_mm;
        total_extrusion += layer.extrusion_distance_mm;
        total_time += layer.layer_time_estimate_s;

        for (name, metrics) in &layer.features {
            total_filament += metrics.extrusion_e_mm;
            total_features
                .entry(name.clone())
                .or_default()
                .merge(metrics);
        }
    }

    let total_filament_volume = filament_mm_to_volume_mm3(total_filament, filament_diameter);
    let total_filament_weight =
        filament_mm_to_weight_g(total_filament, filament_diameter, filament_density);

    GcodeAnalysis {
        filename: filename.to_string(),
        header,
        slicer: state.detected_slicer,
        layers,
        features: total_features,
        total_time_estimate_s: total_time,
        total_filament_mm: total_filament,
        total_filament_volume_mm3: total_filament_volume,
        total_filament_weight_g: total_filament_weight,
        total_travel_mm: total_travel,
        total_extrusion_mm: total_extrusion,
        total_moves,
        retraction_count: state.retraction_count,
        retraction_distance_mm: state.retraction_distance_mm,
        zhop_count: state.zhop_count,
        zhop_distance_mm: state.zhop_distance_mm,
        unknown_command_count: state.unknown_command_count,
        line_count: state.line_count,
    }
}

/// Parse a single G-code line, updating state and metrics.
fn parse_line(
    line: &str,
    state: &mut GcodeParserState,
    current_layer: &mut LayerMetrics,
    layers: &mut Vec<LayerMetrics>,
) {
    let trimmed = line.trim();

    // Empty line.
    if trimmed.is_empty() {
        return;
    }

    // Full-line comment -- check for annotations.
    if trimmed.starts_with(';') {
        parse_comment(trimmed, state, current_layer, layers);
        return;
    }

    // Strip inline comment.
    let code = trimmed.split(';').next().unwrap_or("").trim();
    if code.is_empty() {
        return;
    }

    let parts: Vec<&str> = code.split_whitespace().collect();
    if parts.is_empty() {
        return;
    }

    match parts[0] {
        "G0" => parse_move(&parts[1..], true, state, current_layer),
        "G1" => parse_move(&parts[1..], false, state, current_layer),
        "G2" | "G3" => parse_arc_move(&parts[1..], state, current_layer),
        "G28" => {
            // Home -- reset position to 0.
            state.x = 0.0;
            state.y = 0.0;
            state.z = 0.0;
        }
        "G90" => state.absolute_positioning = true,
        "G91" => state.absolute_positioning = false,
        "G92" => parse_position_reset(&parts[1..], state),
        "M82" => state.absolute_extrusion = true,
        "M83" => state.absolute_extrusion = false,
        // Common M-codes we recognize but don't need to track.
        "M104" | "M109" | "M140" | "M190" | "M106" | "M107" | "M84" | "M204" | "M205" | "M220"
        | "M221" | "M400" | "M900" | "M862" | "M862.3" | "M201" | "M203" | "M206" | "M207"
        | "M208" | "M302" | "M73" | "T0" | "T1" | "T2" | "T3" => {}
        _ => {
            // Check for M-codes and T-codes we don't explicitly list.
            if parts[0].starts_with('M') || parts[0].starts_with('T') || parts[0].starts_with('G') {
                // Known command classes, just skip.
            } else {
                state.unknown_command_count += 1;
            }
        }
    }
}

/// Parse a full-line comment for feature/layer annotations.
fn parse_comment(
    line: &str,
    state: &mut GcodeParserState,
    current_layer: &mut LayerMetrics,
    layers: &mut Vec<LayerMetrics>,
) {
    let trimmed = line.trim();

    // Feature annotations.
    match state.feature_format {
        FeatureFormat::BambuFeature => {
            if let Some(feature) = trimmed.strip_prefix("; FEATURE: ") {
                state.current_feature = Some(feature.trim().to_string());
                return;
            }
        }
        FeatureFormat::PrusaType => {
            if let Some(feature) = trimmed.strip_prefix(";TYPE:") {
                state.current_feature = Some(feature.trim().to_string());
                return;
            }
            // Also support spaced variant for Slicecore.
            if let Some(feature) = trimmed.strip_prefix("; TYPE:") {
                state.current_feature = Some(feature.trim().to_string());
                return;
            }
        }
        FeatureFormat::Both => {
            if let Some(feature) = trimmed.strip_prefix("; FEATURE: ") {
                state.current_feature = Some(feature.trim().to_string());
                return;
            }
            if let Some(feature) = trimmed.strip_prefix(";TYPE:") {
                state.current_feature = Some(feature.trim().to_string());
                return;
            }
            if let Some(feature) = trimmed.strip_prefix("; TYPE:") {
                state.current_feature = Some(feature.trim().to_string());
                return;
            }
        }
    }

    // Layer change annotations.
    if trimmed == ";LAYER_CHANGE" || trimmed == "; CHANGE_LAYER" {
        finalize_layer(state, current_layer, layers);
        return;
    }

    // Z height annotations.
    if let Some(z_str) = trimmed.strip_prefix(";Z:") {
        if let Ok(z) = z_str.trim().parse::<f64>() {
            handle_z_change(z, state, current_layer, layers);
        }
        return;
    }
    if let Some(z_str) = trimmed.strip_prefix("; Z_HEIGHT: ") {
        if let Ok(z) = z_str.trim().parse::<f64>() {
            handle_z_change(z, state, current_layer, layers);
        }
        return;
    }

    // Layer height annotations.
    if let Some(h_str) = trimmed.strip_prefix(";HEIGHT:") {
        if let Ok(h) = h_str.trim().parse::<f64>() {
            state.layer_height = h;
            current_layer.layer_height = h;
        }
        return;
    }
    if let Some(h_str) = trimmed.strip_prefix("; LAYER_HEIGHT: ") {
        if let Ok(h) = h_str.trim().parse::<f64>() {
            state.layer_height = h;
            current_layer.layer_height = h;
        }
    }
}

/// Handle a Z change event (from annotation or move).
fn handle_z_change(
    new_z: f64,
    state: &mut GcodeParserState,
    current_layer: &mut LayerMetrics,
    layers: &mut Vec<LayerMetrics>,
) {
    if (new_z - state.current_layer_z).abs() > 1e-6 && new_z > state.current_layer_z {
        finalize_layer(state, current_layer, layers);
        state.current_layer_z = new_z;
        current_layer.z_height = new_z;
        if state.layer_height > 0.0 {
            current_layer.layer_height = state.layer_height;
        } else {
            current_layer.layer_height = new_z - layers.last().map_or(0.0, |l| l.z_height);
        }
    }
}

/// Finalize the current layer and start a new one.
fn finalize_layer(
    state: &mut GcodeParserState,
    current_layer: &mut LayerMetrics,
    layers: &mut Vec<LayerMetrics>,
) {
    if state.current_layer_index >= 0 || current_layer.move_count > 0 {
        layers.push(std::mem::take(current_layer));
    }
    state.current_layer_index += 1;
}

/// Parse a G0/G1 move command.
fn parse_move(
    parts: &[&str],
    is_rapid: bool,
    state: &mut GcodeParserState,
    metrics: &mut LayerMetrics,
) {
    let mut new_x = state.x;
    let mut new_y = state.y;
    let mut new_z = state.z;
    let mut has_e = false;
    let mut e_param = 0.0_f64;
    let mut has_z = false;
    let mut has_xy = false;

    for part in parts {
        if part.is_empty() {
            continue;
        }
        let (letter, value_str) = part.split_at(1);
        let value = match value_str.parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };

        match letter {
            "X" => {
                new_x = if state.absolute_positioning {
                    value
                } else {
                    state.x + value
                };
                has_xy = true;
            }
            "Y" => {
                new_y = if state.absolute_positioning {
                    value
                } else {
                    state.y + value
                };
                has_xy = true;
            }
            "Z" => {
                new_z = if state.absolute_positioning {
                    value
                } else {
                    state.z + value
                };
                has_z = true;
            }
            "E" => {
                e_param = value;
                has_e = true;
            }
            "F" => {
                state.feedrate_mm_min = value;
            }
            _ => {}
        }
    }

    // Compute XY distance.
    let dx = new_x - state.x;
    let dy = new_y - state.y;
    let dz = new_z - state.z;
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();

    // Compute extrusion delta.
    let delta_e = if has_e {
        if state.absolute_extrusion {
            e_param - state.e
        } else {
            e_param
        }
    } else {
        0.0
    };

    // Z-hop detection: Z-only rapid move after retraction.
    if has_z && !has_xy && !has_e && is_rapid && state.in_retraction && new_z > state.z {
        // Z going up after retraction = z-hop start.
        state.zhop_z = Some(state.z);
    }

    // Z-hop end: returning to layer Z.
    if has_z && state.zhop_z.is_some() && new_z <= state.zhop_z.unwrap() + 1e-6 {
        let zhop_dist = state.z - new_z;
        if zhop_dist > 0.0 {
            state.zhop_count += 1;
            // Total Z-hop distance is both up and down.
            state.zhop_distance_mm += (state.z - state.zhop_z.unwrap()) + zhop_dist;
        }
        state.zhop_z = None;
    }

    // Retraction/extrusion tracking.
    if has_e {
        if delta_e < -1e-6 {
            // Retraction.
            state.retraction_count += 1;
            state.retraction_distance_mm += delta_e.abs();
            state.in_retraction = true;
            metrics.retraction_count += 1;
        } else if delta_e > 1e-6 {
            state.in_retraction = false;
            state.zhop_z = None;
        }
    }

    // Layer detection from Z moves (when no annotation-based layer changes).
    if has_z && new_z > state.current_layer_z + 1e-6 && has_xy {
        // Z increased with XY movement -- probable new layer.
        // Only trigger if we don't have annotation-based layer detection.
        if state.current_layer_index < 0 {
            state.current_layer_index = 0;
            state.current_layer_z = new_z;
            metrics.z_height = new_z;
        }
    }

    // Update metrics.
    metrics.move_count += 1;

    if delta_e > 1e-6 {
        // Extruding move.
        metrics.extrusion_distance_mm += distance;

        // Per-feature metrics.
        let feature_name = state
            .current_feature
            .clone()
            .unwrap_or_else(|| "Unknown".to_string());
        let feature_metrics = metrics.features.entry(feature_name).or_default();
        feature_metrics.move_count += 1;
        feature_metrics.extrusion_distance_mm += distance;
        feature_metrics.extrusion_e_mm += delta_e;

        // Speed stats.
        let speed_mm_s = if state.feedrate_mm_min > 0.0 {
            state.feedrate_mm_min / 60.0
        } else {
            0.0
        };
        feature_metrics.speed_stats.update(speed_mm_s, distance);

        // Time estimate.
        if state.feedrate_mm_min > 0.0 {
            let time_s = distance / (state.feedrate_mm_min / 60.0);
            metrics.layer_time_estimate_s += time_s;
            feature_metrics.time_estimate_s += time_s;
        }
    } else if distance > 1e-6 {
        // Travel move (no extrusion or retraction-only).
        metrics.travel_distance_mm += distance;

        // Per-feature travel.
        let feature_name = state
            .current_feature
            .clone()
            .unwrap_or_else(|| "Travel".to_string());
        let feature_metrics = metrics.features.entry(feature_name).or_default();
        feature_metrics.move_count += 1;
        feature_metrics.travel_distance_mm += distance;

        // Time estimate for travel.
        if state.feedrate_mm_min > 0.0 {
            let time_s = distance / (state.feedrate_mm_min / 60.0);
            metrics.layer_time_estimate_s += time_s;
            feature_metrics.time_estimate_s += time_s;
        }
    }

    // Update position.
    state.x = new_x;
    state.y = new_y;
    state.z = new_z;
    if has_e {
        if state.absolute_extrusion {
            state.e = e_param;
        } else {
            state.e += e_param;
        }
    }
}

/// Parse an arc move (G2/G3) using chord distance approximation.
fn parse_arc_move(parts: &[&str], state: &mut GcodeParserState, metrics: &mut LayerMetrics) {
    // For metrics purposes, approximate arc as chord distance between
    // start and end points (acceptable for analysis).
    let mut new_x = state.x;
    let mut new_y = state.y;
    let mut new_z = state.z;
    let mut has_e = false;
    let mut e_param = 0.0_f64;

    for part in parts {
        if part.is_empty() {
            continue;
        }
        let (letter, value_str) = part.split_at(1);
        let value = match value_str.parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };

        match letter {
            "X" => new_x = value,
            "Y" => new_y = value,
            "Z" => new_z = value,
            "E" => {
                e_param = value;
                has_e = true;
            }
            "F" => state.feedrate_mm_min = value,
            _ => {} // I, J, R arc parameters -- not needed for chord distance
        }
    }

    let dx = new_x - state.x;
    let dy = new_y - state.y;
    let dz = new_z - state.z;
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();

    let delta_e = if has_e {
        if state.absolute_extrusion {
            e_param - state.e
        } else {
            e_param
        }
    } else {
        0.0
    };

    metrics.move_count += 1;

    if delta_e > 1e-6 {
        metrics.extrusion_distance_mm += distance;

        let feature_name = state
            .current_feature
            .clone()
            .unwrap_or_else(|| "Unknown".to_string());
        let feature_metrics = metrics.features.entry(feature_name).or_default();
        feature_metrics.move_count += 1;
        feature_metrics.extrusion_distance_mm += distance;
        feature_metrics.extrusion_e_mm += delta_e;

        let speed_mm_s = if state.feedrate_mm_min > 0.0 {
            state.feedrate_mm_min / 60.0
        } else {
            0.0
        };
        feature_metrics.speed_stats.update(speed_mm_s, distance);

        if state.feedrate_mm_min > 0.0 {
            let time_s = distance / (state.feedrate_mm_min / 60.0);
            metrics.layer_time_estimate_s += time_s;
            feature_metrics.time_estimate_s += time_s;
        }
    } else if distance > 1e-6 {
        metrics.travel_distance_mm += distance;

        let feature_name = state
            .current_feature
            .clone()
            .unwrap_or_else(|| "Travel".to_string());
        let feature_metrics = metrics.features.entry(feature_name).or_default();
        feature_metrics.move_count += 1;
        feature_metrics.travel_distance_mm += distance;

        if state.feedrate_mm_min > 0.0 {
            let time_s = distance / (state.feedrate_mm_min / 60.0);
            metrics.layer_time_estimate_s += time_s;
            feature_metrics.time_estimate_s += time_s;
        }
    }

    state.x = new_x;
    state.y = new_y;
    state.z = new_z;
    if has_e {
        if state.absolute_extrusion {
            state.e = e_param;
        } else {
            state.e += e_param;
        }
    }
}

/// Parse G92 position reset command.
fn parse_position_reset(parts: &[&str], state: &mut GcodeParserState) {
    for part in parts {
        if part.is_empty() {
            continue;
        }
        let (letter, value_str) = part.split_at(1);
        let value = match value_str.parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };

        match letter {
            "X" => state.x = value,
            "Y" => state.y = value,
            "Z" => state.z = value,
            "E" => state.e = value,
            _ => {}
        }
    }
}

/// Parse header comments for metadata extraction.
fn parse_header_comment(line: &str, header: &mut HeaderMetadata) {
    let trimmed = line.trim();

    // BambuStudio slicer name/version.
    // Example: "; BambuStudio 02.05.00.66"
    {
        let lower = trimmed.to_lowercase();
        if header.slicer_name.is_none() {
            if lower.contains("bambustudio") {
                header.slicer_name = Some("BambuStudio".to_string());
                // Extract version.
                if let Some(rest) = lower.strip_prefix("; bambustudio ") {
                    header.slicer_version = Some(rest.trim().to_string());
                }
            } else if lower.contains("prusaslicer") {
                header.slicer_name = Some("PrusaSlicer".to_string());
                if let Some(pos) = lower.find("prusaslicer ") {
                    let after = &trimmed[pos + "prusaslicer ".len()..];
                    let version = after.split_whitespace().next().unwrap_or("");
                    if !version.is_empty() {
                        header.slicer_version = Some(version.to_string());
                    }
                }
            } else if lower.contains("orcaslicer") {
                header.slicer_name = Some("OrcaSlicer".to_string());
                if let Some(pos) = lower.find("orcaslicer ") {
                    let after = &trimmed[pos + "orcaslicer ".len()..];
                    let version = after.split_whitespace().next().unwrap_or("");
                    if !version.is_empty() {
                        header.slicer_version = Some(version.to_string());
                    }
                }
            } else if lower.contains("generated by slicecore") {
                header.slicer_name = Some("Slicecore".to_string());
                if let Some(pos) = lower.find("slicecore ") {
                    let after = &trimmed[pos + "slicecore ".len()..];
                    let version = after.split_whitespace().next().unwrap_or("");
                    if !version.is_empty() {
                        header.slicer_version = Some(version.to_string());
                    }
                }
            }
        }
    }

    // BambuStudio format metadata.
    if let Some(rest) = trimmed.strip_prefix("; model printing time:") {
        if header.estimated_time_s.is_none() {
            if let Some(time) = parse_time_string(rest.trim()) {
                header.estimated_time_s = Some(time);
            }
        }
    }
    // BambuStudio total estimated time (overrides model printing time).
    if let Some(rest) = trimmed.strip_prefix("; total estimated time:") {
        if let Some(time) = parse_time_string(rest.trim()) {
            header.estimated_time_s = Some(time);
        }
    }
    // Handle combined line: "; model printing time: 9m 48s; total estimated time: 18m 1s"
    if trimmed.contains("total estimated time:") && !trimmed.starts_with("; total estimated time:")
    {
        if let Some(pos) = trimmed.find("total estimated time:") {
            let rest = &trimmed[pos + "total estimated time:".len()..];
            if let Some(time) = parse_time_string(rest.trim()) {
                header.estimated_time_s = Some(time);
            }
        }
    }

    if let Some(rest) = trimmed.strip_prefix("; total filament length [mm] :") {
        if let Ok(val) = rest.trim().parse::<f64>() {
            header.filament_length_mm = Some(val);
        }
    }
    if let Some(rest) = trimmed.strip_prefix("; total filament volume [cm^3] :") {
        if let Ok(val) = rest.trim().parse::<f64>() {
            header.filament_volume_cm3 = Some(val);
        }
    }
    if let Some(rest) = trimmed.strip_prefix("; total filament weight [g] :") {
        if let Ok(val) = rest.trim().parse::<f64>() {
            header.filament_weight_g = Some(val);
        }
    }
    if let Some(rest) = trimmed.strip_prefix("; filament_density:") {
        // May be comma-separated for multi-extruder; take first.
        let first = rest.trim().split(',').next().unwrap_or("").trim();
        if let Ok(val) = first.parse::<f64>() {
            header.filament_density = Some(val);
        }
    }
    if let Some(rest) = trimmed.strip_prefix("; filament_diameter:") {
        let first = rest.trim().split(',').next().unwrap_or("").trim();
        if let Ok(val) = first.parse::<f64>() {
            header.filament_diameter = Some(val);
        }
    }
    if let Some(rest) = trimmed.strip_prefix("; total layer number:") {
        if let Ok(val) = rest.trim().parse::<u32>() {
            header.layer_count = Some(val);
        }
    }
    if let Some(rest) = trimmed.strip_prefix("; max_z_height:") {
        if let Ok(val) = rest.trim().parse::<f64>() {
            header.max_z_height = Some(val);
        }
    }

    // PrusaSlicer format metadata.
    if let Some(rest) = trimmed.strip_prefix("; estimated printing time (normal mode) =") {
        if header.estimated_time_s.is_none() {
            if let Some(time) = parse_time_string(rest.trim()) {
                header.estimated_time_s = Some(time);
            }
        }
    }
    if let Some(rest) = trimmed.strip_prefix("; filament used [mm] =") {
        if let Ok(val) = rest.trim().parse::<f64>() {
            header.filament_length_mm = Some(val);
        }
    }
    if let Some(rest) = trimmed.strip_prefix("; filament used [cm3] =") {
        if let Ok(val) = rest.trim().parse::<f64>() {
            header.filament_volume_cm3 = Some(val);
        }
    }
    if let Some(rest) = trimmed.strip_prefix("; filament used [g] =") {
        if let Ok(val) = rest.trim().parse::<f64>() {
            header.filament_weight_g = Some(val);
        }
    }
}

/// Parse a time string like "1h 15m 30s" or "9m 48s" or "30s" into seconds.
fn parse_time_string(s: &str) -> Option<f64> {
    let mut total_seconds = 0.0_f64;
    let mut current_num = String::new();
    let mut found_any = false;

    for ch in s.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            current_num.push(ch);
        } else if ch == 'h' || ch == 'H' {
            if let Ok(val) = current_num.parse::<f64>() {
                total_seconds += val * 3600.0;
                found_any = true;
            }
            current_num.clear();
        } else if ch == 'm' || ch == 'M' {
            if let Ok(val) = current_num.parse::<f64>() {
                total_seconds += val * 60.0;
                found_any = true;
            }
            current_num.clear();
        } else if ch == 's' || ch == 'S' {
            if let Ok(val) = current_num.parse::<f64>() {
                total_seconds += val;
                found_any = true;
            }
            current_num.clear();
        } else if ch == ' ' || ch == ';' {
            // Skip whitespace and separators, but don't clear number yet.
        }
    }

    if found_any {
        Some(total_seconds)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_absolute_extrusion_m82() {
        let gcode = "M82\nG1 X10 Y0 E1.0 F3000\nG1 X20 Y0 E2.5 F3000\n";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "test.gcode", 1.75, 1.24);
        // Total filament: E went from 0 to 1.0 (delta=1.0) then 1.0 to 2.5 (delta=1.5) = 2.5
        assert!(
            (result.total_filament_mm - 2.5).abs() < 0.1,
            "total_filament_mm={} expected ~2.5",
            result.total_filament_mm,
        );
    }

    #[test]
    fn test_relative_extrusion_m83() {
        let gcode = "M83\nG1 X10 Y0 E1.0 F3000\nG1 X20 Y0 E1.5 F3000\n";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "test.gcode", 1.75, 1.24);
        // In relative mode: E values are deltas directly = 1.0 + 1.5 = 2.5
        assert!(
            (result.total_filament_mm - 2.5).abs() < 0.1,
            "total_filament_mm={} expected ~2.5",
            result.total_filament_mm,
        );
    }

    #[test]
    fn test_g92_e0_reset() {
        let gcode = "M82\nG1 X10 Y0 E5.0 F3000\nG92 E0\nG1 X20 Y0 E3.0 F3000\n";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "test.gcode", 1.75, 1.24);
        // First move: E 0->5 = 5.0mm, reset to 0, second move: E 0->3 = 3.0mm
        // Total = 8.0mm
        assert!(
            (result.total_filament_mm - 8.0).abs() < 0.1,
            "total_filament_mm={} expected ~8.0",
            result.total_filament_mm,
        );
    }

    #[test]
    fn test_g0_travel_distance() {
        let gcode = "G0 X10 Y0 F9000\nG0 X10 Y10 F9000\n";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "test.gcode", 1.75, 1.24);
        // Travel: 10mm + 10mm = 20mm
        assert!(
            (result.total_travel_mm - 20.0).abs() < 0.1,
            "total_travel_mm={} expected ~20.0",
            result.total_travel_mm,
        );
        // No extrusion.
        assert!(
            result.total_filament_mm.abs() < 0.01,
            "total_filament_mm={} expected ~0",
            result.total_filament_mm,
        );
    }

    #[test]
    fn test_g1_extrusion_move() {
        let gcode = "M83\nG1 X10 Y0 E0.5 F3000\n";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "test.gcode", 1.75, 1.24);
        // Extrusion distance: sqrt(10^2 + 0^2) = 10mm
        assert!(
            (result.total_extrusion_mm - 10.0).abs() < 0.1,
            "total_extrusion_mm={} expected ~10.0",
            result.total_extrusion_mm,
        );
        assert!(
            (result.total_filament_mm - 0.5).abs() < 0.01,
            "total_filament_mm={} expected ~0.5",
            result.total_filament_mm,
        );
    }

    #[test]
    fn test_z_layer_change_detection() {
        let gcode = "\
;LAYER_CHANGE
;Z:0.2
;HEIGHT:0.2
G1 X10 Y0 E0.5 F3000
;LAYER_CHANGE
;Z:0.4
;HEIGHT:0.2
G1 X20 Y0 E0.5 F3000
";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "test.gcode", 1.75, 1.24);
        // Should have 2 layers.
        assert!(
            result.layers.len() >= 2,
            "layers.len()={} expected >=2",
            result.layers.len()
        );
    }

    #[test]
    fn test_feedrate_tracking() {
        let gcode = "M83\nG1 X10 Y0 E0.5 F3000\nG1 X20 Y0 E0.5 F6000\n";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "test.gcode", 1.75, 1.24);
        // Check time estimate is based on feedrate.
        // First move: 10mm at 3000mm/min (50mm/s) = 0.2s
        // Second move: 10mm at 6000mm/min (100mm/s) = 0.1s
        // Total: 0.3s
        assert!(
            (result.total_time_estimate_s - 0.3).abs() < 0.05,
            "total_time_estimate_s={} expected ~0.3",
            result.total_time_estimate_s,
        );
    }

    #[test]
    fn test_inline_comment_stripping() {
        let gcode = "M83\nG1 X10 Y0 E0.5 F3000 ; move to start\nG1 X20 Y0 E0.5 F3000\n";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "test.gcode", 1.75, 1.24);
        // Should parse correctly despite inline comment.
        assert_eq!(result.total_moves, 2, "Should have 2 moves");
        assert!(
            (result.total_filament_mm - 1.0).abs() < 0.1,
            "total_filament_mm={} expected ~1.0",
            result.total_filament_mm,
        );
    }

    #[test]
    fn test_bambustudio_header_parsing() {
        let gcode = "\
; HEADER_BLOCK_START
; BambuStudio 02.05.00.66
; model printing time: 9m 48s; total estimated time: 18m 1s
; total layer number: 100
; total filament length [mm] : 1393.21
; total filament volume [cm^3] : 3351.07
; total filament weight [g] : 4.22
; filament_density: 1.26,1.24,1.25,1.24
; filament_diameter: 1.75,1.75,1.75,1.75
; max_z_height: 20.00
; HEADER_BLOCK_END
G28
";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "test.gcode", 1.75, 1.24);
        assert_eq!(result.slicer, SlicerType::BambuStudio);
        assert_eq!(result.header.slicer_name.as_deref(), Some("BambuStudio"));
        assert_eq!(result.header.layer_count, Some(100));
        assert!((result.header.filament_length_mm.unwrap() - 1393.21).abs() < 0.01,);
        assert!((result.header.filament_weight_g.unwrap() - 4.22).abs() < 0.01,);
        assert!((result.header.filament_density.unwrap() - 1.26).abs() < 0.01,);
        assert!((result.header.max_z_height.unwrap() - 20.0).abs() < 0.01,);
        // Total estimated time: 18m 1s = 1081s
        assert!(
            (result.header.estimated_time_s.unwrap() - 1081.0).abs() < 1.0,
            "estimated_time_s={} expected ~1081",
            result.header.estimated_time_s.unwrap(),
        );
    }

    #[test]
    fn test_prusaslicer_header_parsing() {
        let gcode = "\
; generated by PrusaSlicer 2.8.0+linux-x86_64
; estimated printing time (normal mode) = 1h 15m 30s
; filament used [mm] = 3870.0
; filament used [cm3] = 9.31
; filament used [g] = 11.73
G28
";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "test.gcode", 1.75, 1.24);
        assert_eq!(result.slicer, SlicerType::PrusaSlicer);
        assert_eq!(result.header.slicer_name.as_deref(), Some("PrusaSlicer"),);
        // Time: 1h 15m 30s = 3600 + 900 + 30 = 4530s
        assert!(
            (result.header.estimated_time_s.unwrap() - 4530.0).abs() < 1.0,
            "estimated_time_s={} expected ~4530",
            result.header.estimated_time_s.unwrap(),
        );
        assert!((result.header.filament_length_mm.unwrap() - 3870.0).abs() < 0.01,);
        assert!((result.header.filament_volume_cm3.unwrap() - 9.31).abs() < 0.01,);
        assert!((result.header.filament_weight_g.unwrap() - 11.73).abs() < 0.01,);
    }

    #[test]
    fn test_feature_annotation_bambu() {
        let gcode = "\
; BambuStudio 02.05.00.66
; CHANGE_LAYER
; Z_HEIGHT: 0.2
; FEATURE: Outer wall
M83
G1 X10 Y0 E0.5 F3000
; FEATURE: Sparse infill
G1 X20 Y0 E0.3 F6000
";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "test.gcode", 1.75, 1.24);
        assert!(result.features.contains_key("Outer wall"));
        assert!(result.features.contains_key("Sparse infill"));
    }

    #[test]
    fn test_feature_annotation_prusaslicer() {
        let gcode = "\
; generated by PrusaSlicer 2.8.0
;LAYER_CHANGE
;Z:0.2
;TYPE:External perimeter
M83
G1 X10 Y0 E0.5 F3000
;TYPE:Solid infill
G1 X20 Y0 E0.3 F6000
";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "test.gcode", 1.75, 1.24);
        assert!(result.features.contains_key("External perimeter"));
        assert!(result.features.contains_key("Solid infill"));
    }

    #[test]
    fn test_retraction_tracking() {
        let gcode = "\
M83
G1 X10 Y0 E0.5 F3000
G1 E-0.8 F2700
G0 X50 Y50 F9000
G1 E0.8 F2700
G1 X60 Y50 E0.5 F3000
";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "test.gcode", 1.75, 1.24);
        assert_eq!(result.retraction_count, 1);
        assert!(
            (result.retraction_distance_mm - 0.8).abs() < 0.01,
            "retraction_distance_mm={} expected ~0.8",
            result.retraction_distance_mm,
        );
    }

    #[test]
    fn test_unknown_command_count() {
        let gcode = "CUSTOM_CMD\nANOTHER\nG1 X10 Y0 F3000\n";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "test.gcode", 1.75, 1.24);
        assert_eq!(
            result.unknown_command_count, 2,
            "Should have 2 unknown commands"
        );
    }

    #[test]
    fn test_line_count() {
        let gcode = "G28\nG90\nM82\nG1 X10 Y0 F3000\n";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "test.gcode", 1.75, 1.24);
        assert_eq!(result.line_count, 4);
    }

    #[test]
    fn test_time_string_parsing() {
        assert!((parse_time_string("1h 15m 30s").unwrap() - 4530.0).abs() < 0.01);
        assert!((parse_time_string("9m 48s").unwrap() - 588.0).abs() < 0.01);
        assert!((parse_time_string("30s").unwrap() - 30.0).abs() < 0.01);
        assert!((parse_time_string("2h").unwrap() - 7200.0).abs() < 0.01);
        assert!(parse_time_string("no time here").is_none());
    }

    #[test]
    fn test_empty_file() {
        let gcode = "";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "empty.gcode", 1.75, 1.24);
        assert_eq!(result.total_moves, 0);
        assert_eq!(result.line_count, 0);
        assert_eq!(result.slicer, SlicerType::Unknown);
    }

    #[test]
    fn test_filament_volume_and_weight() {
        // Parse a file with known extrusion.
        let gcode = "M83\nG1 X100 Y0 E10.0 F3000\n";
        let reader = Cursor::new(gcode);
        let result = parse_gcode_file(reader, "test.gcode", 1.75, 1.24);
        // Volume and weight should be computed from 10mm filament.
        assert!(result.total_filament_volume_mm3 > 0.0);
        assert!(result.total_filament_weight_g > 0.0);

        // Cross-check with standalone functions.
        let expected_volume = filament_mm_to_volume_mm3(10.0, 1.75);
        let expected_weight = filament_mm_to_weight_g(10.0, 1.75, 1.24);
        assert!((result.total_filament_volume_mm3 - expected_volume).abs() < 0.01,);
        assert!((result.total_filament_weight_g - expected_weight).abs() < 0.001,);
    }
}
