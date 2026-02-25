//! G-code analysis module for parsing and extracting metrics from G-code files.
//!
//! This module provides a line-by-line G-code parser that tracks machine state
//! (position, feedrate, extrusion mode) and accumulates per-layer and per-feature
//! metrics. It supports auto-detection of the source slicer (BambuStudio,
//! OrcaSlicer, PrusaSlicer, Slicecore) and adapts comment parsing accordingly.
//!
//! # Usage
//!
//! ```ignore
//! use std::io::BufReader;
//! use std::fs::File;
//! use slicecore_engine::gcode_analysis::parse_gcode_file;
//!
//! let file = File::open("model.gcode").unwrap();
//! let reader = BufReader::new(file);
//! let analysis = parse_gcode_file(reader, "model.gcode", 1.75, 1.24);
//! println!("Slicer: {:?}", analysis.slicer);
//! println!("Layers: {}", analysis.layers.len());
//! println!("Total time: {:.1}s", analysis.total_time_estimate_s);
//! ```

pub mod comparison;
pub mod metrics;
pub mod parser;
pub mod slicer_detect;

// Re-export primary types for convenient access.
pub use comparison::{compare_gcode_analyses, ComparisonDelta, ComparisonResult, FeatureDelta};
pub use metrics::{
    filament_mm_to_volume_mm3, filament_mm_to_weight_g, FeatureMetrics, GcodeAnalysis,
    HeaderMetadata, LayerMetrics, SpeedStats,
};
pub use parser::parse_gcode_file;
pub use slicer_detect::{detect_slicer, FeatureFormat, SlicerType};
