//! G-code I/O for the slicecore 3D slicing engine.
//!
//! This crate provides:
//!
//! - **Structured command types** ([`GcodeCommand`]) representing G-code commands
//!   as typed enum variants rather than raw strings
//! - **Dialect-aware writer** ([`GcodeWriter`]) that generates firmware-specific
//!   start/end sequences for Marlin, Klipper, RepRapFirmware, and Bambu
//! - **G-code dialect configuration** ([`GcodeDialect`], [`StartConfig`], [`EndConfig`])
//! - **Error types** ([`GcodeError`]) for validation and I/O failures
//!
//! # Example
//!
//! ```rust
//! use slicecore_gcode_io::{GcodeCommand, GcodeDialect, GcodeWriter, StartConfig, EndConfig};
//!
//! let mut buf = Vec::new();
//! let mut writer = GcodeWriter::new(&mut buf, GcodeDialect::Marlin);
//!
//! writer.write_start_gcode(&StartConfig {
//!     bed_temp: 60.0,
//!     nozzle_temp: 200.0,
//!     bed_x: 220.0,
//!     bed_y: 220.0,
//! }).unwrap();
//!
//! writer.write_command(&GcodeCommand::LinearMove {
//!     x: Some(100.0), y: Some(100.0), z: Some(0.3),
//!     e: Some(0.5), f: Some(1800.0),
//! }).unwrap();
//!
//! writer.write_end_gcode(&EndConfig { retract_distance: 5.0 }).unwrap();
//! ```

pub mod arc;
pub mod bambu;
pub mod commands;
pub mod dialect;
pub mod error;
pub mod klipper;
pub mod marlin;
pub mod reprap;
pub mod validate;
pub mod writer;

// Re-export primary types at crate root for ergonomic imports.
pub use commands::GcodeCommand;
pub use dialect::{
    format_acceleration, format_jerk, format_pressure_advance, EndConfig, GcodeDialect,
    StartConfig,
};
pub use error::GcodeError;
pub use validate::{validate_gcode, ValidationResult};
pub use arc::fit_arcs;
pub use writer::GcodeWriter;
