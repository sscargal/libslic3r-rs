//! Slicing pipeline orchestrator for the slicecore 3D slicing engine.
//!
//! This crate ties together all pipeline stages: mesh loading, slicing,
//! perimeter generation, infill, toolpath planning, and G-code emission.
//!
//! Pipeline modules:
//! - [`config`]: Print configuration with TOML deserialization
//! - [`engine`]: Pipeline orchestrator (Engine struct)
//! - [`perimeter`]: Perimeter shell generation via polygon offsetting
//! - [`infill`]: Rectilinear infill pattern generation
//! - [`surface`]: Top/bottom solid layer classification
//! - [`extrusion`]: E-axis value computation (Slic3r cross-section model)
//! - [`toolpath`]: Toolpath segment types and layer toolpath assembly
//! - [`planner`]: Skirt/brim generation, retraction, temperature, fan control
//! - [`gcode_gen`]: Toolpath-to-GcodeCommand conversion
//!
//! # Configuration
//!
//! [`PrintConfig`] contains all parameters controlling the pipeline.
//! It supports TOML deserialization with `#[serde(default)]`, so any
//! unspecified fields use sensible FDM defaults.

pub mod arachne;
pub mod builtin_profiles;
pub mod calibration;
pub mod config;
pub mod config_validate;
pub mod custom_gcode;
pub mod engine;
pub mod error;
pub mod estimation;
pub mod event;
pub mod extrusion;
pub mod filament;
pub mod flow_control;
pub mod gap_fill;
pub mod gcode_analysis;
pub mod gcode_gen;
pub mod infill;
pub mod ironing;
pub mod modifier;
pub mod multimaterial;
pub mod output;
mod parallel;
pub mod perimeter;
pub mod planner;
pub mod polyhole;
pub mod postprocess_builtin;
pub mod preview;
pub mod profile_compose;
pub mod profile_convert;
pub mod profile_resolve;
pub mod profile_import;
pub mod profile_import_ini;
pub mod profile_library;
pub mod scarf;
pub mod seam;
pub mod sequential;
pub mod statistics;
pub mod support;
pub mod surface;
pub mod toolpath;

// Re-export primary types at crate root.
pub use arachne::{generate_arachne_perimeters, ArachnePerimeter, ArachneResult};
pub use builtin_profiles::{get_builtin_profile, list_builtin_profiles, BuiltinProfile};
pub use config_validate::{
    resolve_template_variables, validate_config, ValidationIssue, ValidationSeverity,
};
pub use calibration::{generate_pa_calibration, generate_pa_calibration_gcode};
pub use config::{
    MultiMaterialConfig, PaCalibrationConfig, PrintConfig, ScarfJointConfig, ScarfJointType,
    SequentialConfig, SettingOverrides, ToolConfig, WallOrder,
};
pub use custom_gcode::{substitute_placeholders, CustomGcodeHooks};
pub use engine::{CancellationToken, Engine, SliceResult};
pub use error::EngineError;
pub use estimation::{estimate_print_time, trapezoid_time, PrintTimeEstimate};
pub use event::{CallbackSubscriber, EventBus, EventSubscriber, SliceEvent};
pub use extrusion::{compute_e_value, extrusion_cross_section, move_length};
pub use filament::{estimate_filament_usage, FilamentUsage};
pub use flow_control::PerFeatureFlow;
pub use gap_fill::{detect_and_fill_gaps, GapFillPath};
pub use gcode_analysis::{
    compare_gcode_analyses, detect_slicer, filament_mm_to_volume_mm3, filament_mm_to_weight_g,
    parse_gcode_file, ComparisonDelta, ComparisonResult, FeatureDelta, FeatureFormat,
    FeatureMetrics, GcodeAnalysis, HeaderMetadata, LayerMetrics, SlicerType, SpeedStats,
};
pub use gcode_gen::{generate_full_gcode, generate_layer_gcode};
pub use infill::{
    alternate_infill_angle, generate_infill, generate_rectilinear_infill, InfillLine,
    InfillPattern, LayerInfill,
};
pub use ironing::{generate_ironing_passes, IroningConfig};
pub use modifier::{slice_modifier, split_by_modifiers, ModifierMesh, ModifierRegion};
pub use multimaterial::{
    assign_tools_per_region, generate_purge_tower_layer, generate_tool_change, PurgeTowerLayer,
    ToolChangeSequence,
};
pub use output::{to_json, to_msgpack, SliceMetadata};
pub use perimeter::{generate_perimeters, ContourPerimeters, PerimeterShell};
pub use planner::{
    generate_brim, generate_skirt, plan_fan, plan_retraction, plan_temperatures, RetractionMove,
};
pub use polyhole::{
    convert_polyholes, convert_to_polyhole, is_circular_hole, polyhole_radius, polyhole_sides,
};
pub use postprocess_builtin::create_builtin_postprocessors;
pub use preview::{generate_preview, LayerPreview, SlicePreview};
pub use profile_convert::{convert_to_toml, merge_import_results, ConvertResult};
pub use profile_import::{detect_config_format, ConfigFormat, ImportResult, ProfileMetadata};
pub use profile_library::{
    batch_convert_profiles, batch_convert_prusaslicer_profiles, load_index, write_index,
    write_merged_index, BatchConvertResult, ProfileIndex, ProfileIndexEntry,
};
pub use scarf::apply_scarf_joint;
pub use seam::{select_seam_point, SeamPosition};
pub use sequential::{detect_collision, order_objects, plan_sequential_print, ObjectBounds};
pub use statistics::{
    compute_statistics, FeatureStatistics, GcodeMetrics, PrintStatistics, StatisticsSummary,
    StatsSortOrder, TimePrecision,
};
pub use support::config::SupportConfig;
pub use support::{SupportRegion, SupportResult};
pub use surface::{classify_surfaces, SurfaceClassification};
pub use toolpath::{assemble_layer_toolpath, FeatureType, LayerToolpath, ToolpathSegment};

// Re-export plugin types when the plugins feature is enabled.
#[cfg(feature = "plugins")]
pub use slicecore_plugin::{PluginInfo, PluginKind, PluginRegistry};

// Re-export AI integration types when the ai feature is enabled.
#[cfg(feature = "ai")]
pub use slicecore_ai::{
    extract_geometry_features, AiConfig, AiError as AiIntegrationError, AiProvider,
    GeometryFeatures, ProfileSuggestion, ProviderType,
};
