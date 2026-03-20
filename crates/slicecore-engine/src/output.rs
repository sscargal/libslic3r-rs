//! Structured output serialization for slicing results.
//!
//! Provides [`SliceMetadata`] as a structured representation of slicing
//! results, and functions to serialize/deserialize it as JSON or MessagePack.
//!
//! JSON output is human-readable and suitable for CLI tools and web APIs.
//! MessagePack is a compact binary format for machine-to-machine transfer.
//!
//! # Example
//!
//! ```no_run
//! use slicecore_engine::output::{to_json, to_msgpack, from_msgpack};
//! use slicecore_engine::{SliceResult, PrintConfig};
//!
//! fn example(result: &SliceResult, config: &PrintConfig) {
//!     let json = to_json(result, config).unwrap();
//!     println!("{}", json);
//!
//!     let bytes = to_msgpack(result, config).unwrap();
//!     let decoded = from_msgpack(&bytes).unwrap();
//!     assert_eq!(decoded.layer_count, result.layer_count);
//! }
//! ```

use serde::{Deserialize, Serialize};

use crate::config::PrintConfig;
use crate::engine::SliceResult;
use crate::estimation::PrintTimeEstimate;
use crate::filament::FilamentUsage;

/// Summary of key configuration fields used during slicing.
///
/// This captures the essential parameters that affect print quality
/// and is included in structured output for reproducibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSummary {
    /// Layer height in mm.
    pub layer_height: f64,
    /// First layer height in mm.
    pub first_layer_height: f64,
    /// Nozzle diameter in mm.
    pub nozzle_diameter: f64,
    /// Number of perimeter walls.
    pub wall_count: u32,
    /// Infill density as a fraction (0.0 to 1.0).
    pub infill_density: f64,
    /// Infill pattern name.
    pub infill_pattern: String,
    /// Filament diameter in mm.
    pub filament_diameter: f64,
    /// Perimeter print speed in mm/s.
    pub perimeter_speed: f64,
    /// Infill print speed in mm/s.
    pub infill_speed: f64,
    /// Travel speed in mm/s.
    pub travel_speed: f64,
}

/// Structured metadata from a slicing operation.
///
/// Contains all key metrics and configuration summary for external
/// tools to consume. Serializable to JSON and MessagePack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceMetadata {
    /// Number of layers produced.
    pub layer_count: usize,
    /// Detailed print time estimate.
    pub time_estimate: PrintTimeEstimate,
    /// Filament consumption breakdown.
    pub filament_usage: FilamentUsage,
    /// Summary of configuration parameters.
    pub config_summary: ConfigSummary,
}

/// Extracts a [`ConfigSummary`] from a [`PrintConfig`].
fn config_summary(config: &PrintConfig) -> ConfigSummary {
    ConfigSummary {
        layer_height: config.layer_height,
        first_layer_height: config.first_layer_height,
        nozzle_diameter: config.machine.nozzle_diameter(),
        wall_count: config.wall_count,
        infill_density: config.infill_density,
        infill_pattern: format!("{:?}", config.infill_pattern),
        filament_diameter: config.filament.diameter,
        perimeter_speed: config.speeds.perimeter,
        infill_speed: config.speeds.infill,
        travel_speed: config.speeds.travel,
    }
}

/// Builds a [`SliceMetadata`] from a slice result and config.
fn build_metadata(result: &SliceResult, config: &PrintConfig) -> SliceMetadata {
    SliceMetadata {
        layer_count: result.layer_count,
        time_estimate: result.time_estimate.clone(),
        filament_usage: result.filament_usage.clone(),
        config_summary: config_summary(config),
    }
}

/// Serializes slicing results as pretty-printed JSON.
///
/// Returns a human-readable JSON string containing layer count,
/// time estimate, filament usage, and configuration summary.
///
/// # Errors
///
/// Returns `serde_json::Error` if serialization fails (should not
/// happen for well-formed data).
pub fn to_json(result: &SliceResult, config: &PrintConfig) -> Result<String, serde_json::Error> {
    let metadata = build_metadata(result, config);
    serde_json::to_string_pretty(&metadata)
}

/// Serializes slicing results as MessagePack bytes.
///
/// Returns a compact binary representation suitable for machine-to-machine
/// transfer and storage.
///
/// # Errors
///
/// Returns `rmp_serde::encode::Error` if serialization fails.
pub fn to_msgpack(
    result: &SliceResult,
    config: &PrintConfig,
) -> Result<Vec<u8>, rmp_serde::encode::Error> {
    let metadata = build_metadata(result, config);
    rmp_serde::to_vec(&metadata)
}

/// Deserializes [`SliceMetadata`] from MessagePack bytes.
///
/// # Errors
///
/// Returns `rmp_serde::decode::Error` if the data is malformed or
/// does not match the expected schema.
pub fn from_msgpack(data: &[u8]) -> Result<SliceMetadata, rmp_serde::decode::Error> {
    rmp_serde::from_slice(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PrintConfig;
    use crate::engine::SliceResult;
    use crate::estimation::PrintTimeEstimate;
    use crate::filament::FilamentUsage;

    fn sample_result() -> SliceResult {
        SliceResult {
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
        }
    }

    #[test]
    fn to_json_produces_valid_json() {
        let result = sample_result();
        let config = PrintConfig::default();

        let json_str = to_json(&result, &config).unwrap();

        // Parse it back as generic JSON to verify structure.
        let v: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(v["layer_count"], 100);
        assert!(v["time_estimate"]["total_seconds"].as_f64().unwrap() > 0.0);
        assert!(v["filament_usage"]["length_mm"].as_f64().unwrap() > 0.0);
        assert!(v["config_summary"]["layer_height"].as_f64().is_some());
        assert!(v["config_summary"]["infill_pattern"].as_str().is_some());
    }

    #[test]
    fn to_json_is_pretty_printed() {
        let result = sample_result();
        let config = PrintConfig::default();

        let json_str = to_json(&result, &config).unwrap();
        // Pretty-printed JSON contains newlines.
        assert!(json_str.contains('\n'));
        // And indentation.
        assert!(json_str.contains("  "));
    }

    #[test]
    fn msgpack_roundtrip() {
        let result = sample_result();
        let config = PrintConfig::default();

        let bytes = to_msgpack(&result, &config).unwrap();
        assert!(!bytes.is_empty());

        let decoded = from_msgpack(&bytes).unwrap();
        assert_eq!(decoded.layer_count, 100);
        assert!((decoded.time_estimate.total_seconds - 3600.0).abs() < 1e-9);
        assert!((decoded.filament_usage.length_mm - 5000.0).abs() < 1e-9);
        assert!((decoded.config_summary.layer_height - config.layer_height).abs() < 1e-9);
    }

    #[test]
    fn config_summary_captures_key_fields() {
        let config = PrintConfig::default();
        let summary = config_summary(&config);

        assert!((summary.layer_height - config.layer_height).abs() < 1e-9);
        assert!((summary.nozzle_diameter - config.machine.nozzle_diameter()).abs() < 1e-9);
        assert_eq!(summary.wall_count, config.wall_count);
        assert!((summary.infill_density - config.infill_density).abs() < 1e-9);
    }

    #[test]
    fn from_msgpack_rejects_invalid_data() {
        let result = from_msgpack(&[0xFF, 0x00, 0x42]);
        assert!(result.is_err());
    }

    #[test]
    fn metadata_json_deserialize_roundtrip() {
        let result = sample_result();
        let config = PrintConfig::default();

        let json_str = to_json(&result, &config).unwrap();
        let decoded: SliceMetadata = serde_json::from_str(&json_str).unwrap();
        assert_eq!(decoded.layer_count, 100);
    }
}
