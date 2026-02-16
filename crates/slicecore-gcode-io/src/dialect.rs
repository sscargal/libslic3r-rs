//! G-code dialect definitions and configuration.
//!
//! Different 3D printer firmware flavors require different start/end sequences
//! and may interpret certain commands differently. The [`GcodeDialect`] enum
//! selects which firmware-specific behavior the writer should use.

use serde::{Deserialize, Serialize};

/// Supported G-code firmware dialects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GcodeDialect {
    /// Marlin firmware (most common for FDM printers).
    Marlin,
    /// Klipper firmware (uses extended commands for some operations).
    Klipper,
    /// RepRapFirmware (Duet boards, RRF3+).
    RepRapFirmware,
    /// Bambu Lab firmware (simplified sequences, built-in calibration).
    Bambu,
}

/// Configuration for generating start G-code sequences.
#[derive(Debug, Clone)]
pub struct StartConfig {
    /// Target bed temperature in degrees Celsius.
    pub bed_temp: f64,
    /// Target nozzle (extruder) temperature in degrees Celsius.
    pub nozzle_temp: f64,
    /// Bed width in millimeters (X axis).
    pub bed_x: f64,
    /// Bed depth in millimeters (Y axis).
    pub bed_y: f64,
}

/// Configuration for generating end G-code sequences.
#[derive(Debug, Clone)]
pub struct EndConfig {
    /// Retraction distance in millimeters for the final retract.
    pub retract_distance: f64,
}
