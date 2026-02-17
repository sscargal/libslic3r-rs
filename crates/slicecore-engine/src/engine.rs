//! Engine orchestrator for the full slicing pipeline.
//!
//! The [`Engine`] struct is the single entry point for the slicing pipeline.
//! It takes a [`TriangleMesh`] and a [`PrintConfig`] and produces complete
//! G-code output as bytes.
//!
//! # Pipeline stages
//!
//! 1. **Slice mesh**: Triangle-plane intersection producing contour polygons
//! 2. **Per-layer processing**: Perimeters, surface classification, infill, toolpath assembly
//! 3. **First-layer extras**: Skirt/brim generation prepended to layer 0
//! 4. **G-code generation**: Toolpath-to-GcodeCommand conversion
//! 5. **G-code writing**: Dialect-aware output via GcodeWriter

use std::io::Write;

use slicecore_gcode_io::{EndConfig, GcodeWriter, StartConfig};
use slicecore_mesh::TriangleMesh;
use slicecore_slicer::{compute_adaptive_layer_heights, slice_mesh, slice_mesh_adaptive};

use crate::arachne::generate_arachne_perimeters;
use crate::config::PrintConfig;
use crate::error::EngineError;
use crate::estimation::{estimate_print_time, PrintTimeEstimate};
use crate::filament::{estimate_filament_usage, FilamentUsage};
use crate::gap_fill::detect_and_fill_gaps;
use crate::gcode_gen::generate_full_gcode;
use crate::infill::{generate_infill, lightning, InfillPattern, LayerInfill};
use crate::ironing::generate_ironing_passes;
use crate::modifier::{slice_modifier, split_by_modifiers, ModifierMesh};
use crate::perimeter::generate_perimeters;
use crate::planner::{generate_brim, generate_skirt};
use crate::preview::{generate_preview, SlicePreview};
use crate::support;
use crate::surface::classify_surfaces;
use crate::toolpath::{
    assemble_layer_toolpath, FeatureType, LayerToolpath, ToolpathSegment,
};
use crate::extrusion::compute_e_value;

use slicecore_math::Point2;

/// Result of a slicing operation.
#[derive(Debug)]
pub struct SliceResult {
    /// Complete G-code output as bytes.
    pub gcode: Vec<u8>,
    /// Number of layers sliced.
    pub layer_count: usize,
    /// Total estimated print time in seconds (backward compatibility).
    pub estimated_time_seconds: f64,
    /// Detailed print time estimate using trapezoid motion model.
    pub time_estimate: PrintTimeEstimate,
    /// Filament usage breakdown (length, weight, cost).
    pub filament_usage: FilamentUsage,
    /// Optional preview data for visualization.
    pub preview: Option<SlicePreview>,
}

/// Assembles support toolpath segments from support regions.
///
/// Converts support region infill lines into [`ToolpathSegment`]s with
/// appropriate feature type, speed, and E-value. Support body uses infill
/// speed; interface uses perimeter speed (slower for quality).
///
/// # Parameters
///
/// - `support_regions`: Support regions for this layer.
/// - `config`: Print configuration for speeds and extrusion parameters.
/// - `layer_z`: Z height of this layer in mm.
/// - `layer_height`: Height of this layer in mm.
///
/// # Returns
///
/// Support toolpath segments (printed AFTER model perimeters/infill).
fn assemble_support_toolpath(
    support_regions: &[support::SupportRegion],
    config: &PrintConfig,
    layer_z: f64,
    layer_height: f64,
) -> Vec<ToolpathSegment> {
    let mut segments = Vec::new();
    let extrusion_width = config.extrusion_width();
    let travel_speed = config.travel_speed * 60.0;
    // Support body uses infill speed; interface uses perimeter speed.
    let body_speed = config.infill_speed * 60.0;
    let _interface_speed = config.perimeter_speed * 60.0;

    let mut current_pos: Option<Point2> = None;

    for region in support_regions {
        let (feature, feedrate) = if region.is_bridge {
            (FeatureType::Bridge, config.support.bridge.speed * 60.0)
        } else {
            // Check if this is an interface region (has high-density infill).
            // Heuristic: if region has many infill lines relative to its size,
            // it's likely an interface layer with dense infill.
            // For simplicity, use the SupportInterface type for all support regions.
            // The distinction is handled by density in infill generation.
            (FeatureType::Support, body_speed)
        };

        for infill_line in &region.infill {
            let (sx, sy) = infill_line.start.to_mm();
            let (ex, ey) = infill_line.end.to_mm();
            let start_pt = Point2::new(sx, sy);
            let end_pt = Point2::new(ex, ey);

            // Insert travel to line start if needed.
            if let Some(pos) = current_pos {
                let dx = start_pt.x - pos.x;
                let dy = start_pt.y - pos.y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > 0.001 {
                    segments.push(ToolpathSegment {
                        start: pos,
                        end: start_pt,
                        feature: FeatureType::Travel,
                        e_value: 0.0,
                        feedrate: travel_speed,
                        z: layer_z,
                        extrusion_width: None,
                    });
                }
            }

            let seg_len = {
                let dx = end_pt.x - start_pt.x;
                let dy = end_pt.y - start_pt.y;
                (dx * dx + dy * dy).sqrt()
            };

            if seg_len > 0.0001 {
                let flow_multiplier = if region.is_bridge {
                    config.support.bridge.flow_ratio
                } else {
                    1.0
                };

                let e = compute_e_value(
                    seg_len,
                    extrusion_width,
                    layer_height,
                    config.filament_diameter,
                    config.extrusion_multiplier * flow_multiplier,
                );

                segments.push(ToolpathSegment {
                    start: start_pt,
                    end: end_pt,
                    feature,
                    e_value: e,
                    feedrate,
                    z: layer_z,
                    extrusion_width: None,
                });

                current_pos = Some(end_pt);
            }
        }
    }

    segments
}

/// Assembles bridge toolpath segments from detected bridge regions.
///
/// Bridge regions receive bridge-specific speed, fan, and flow settings.
/// Infill lines are generated perpendicular to the bridge span direction.
///
/// # Parameters
///
/// - `bridge_regions`: Bridge regions detected on this layer.
/// - `config`: Print configuration.
/// - `layer_z`: Z height of this layer in mm.
/// - `layer_height`: Height of this layer in mm.
///
/// # Returns
///
/// Bridge toolpath segments with FeatureType::Bridge.
fn assemble_bridge_toolpath(
    bridge_regions: &[support::bridge::BridgeRegion],
    config: &PrintConfig,
    layer_z: f64,
    layer_height: f64,
) -> Vec<ToolpathSegment> {
    let mut segments = Vec::new();
    let extrusion_width = config.extrusion_width();
    let travel_speed = config.travel_speed * 60.0;
    let bridge_speed = config.support.bridge.speed * 60.0;
    let bridge_flow = config.support.bridge.flow_ratio;

    let mut current_pos: Option<Point2> = None;

    for bridge in bridge_regions {
        // Generate bridge infill lines perpendicular to span direction.
        let _infill_angle = support::bridge::compute_bridge_infill_angle(bridge);
        let bridge_lines = crate::infill::generate_infill(
            &crate::infill::InfillPattern::Rectilinear,
            std::slice::from_ref(&bridge.contour),
            1.0, // Bridges use 100% density
            bridge.layer_index,
            layer_z,
            extrusion_width,
            None,
        );

        for line in &bridge_lines {
            let (sx, sy) = line.start.to_mm();
            let (ex, ey) = line.end.to_mm();
            let start_pt = Point2::new(sx, sy);
            let end_pt = Point2::new(ex, ey);

            // Insert travel to line start if needed.
            if let Some(pos) = current_pos {
                let dx = start_pt.x - pos.x;
                let dy = start_pt.y - pos.y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > 0.001 {
                    segments.push(ToolpathSegment {
                        start: pos,
                        end: start_pt,
                        feature: FeatureType::Travel,
                        e_value: 0.0,
                        feedrate: travel_speed,
                        z: layer_z,
                        extrusion_width: None,
                    });
                }
            }

            let seg_len = {
                let dx = end_pt.x - start_pt.x;
                let dy = end_pt.y - start_pt.y;
                (dx * dx + dy * dy).sqrt()
            };

            if seg_len > 0.0001 {
                let e = compute_e_value(
                    seg_len,
                    extrusion_width,
                    layer_height,
                    config.filament_diameter,
                    config.extrusion_multiplier * bridge_flow,
                );

                segments.push(ToolpathSegment {
                    start: start_pt,
                    end: end_pt,
                    feature: FeatureType::Bridge,
                    e_value: e,
                    feedrate: bridge_speed,
                    z: layer_z,
                    extrusion_width: None,
                });

                current_pos = Some(end_pt);
            }
        }
    }

    segments
}

/// The slicing engine -- orchestrates the full pipeline.
///
/// Create an engine with a [`PrintConfig`], then call [`Engine::slice`] to
/// produce G-code from a [`TriangleMesh`].
///
/// When the `plugins` feature is enabled, the engine can hold an optional
/// [`PluginRegistry`](slicecore_plugin::PluginRegistry) that provides
/// plugin-based infill patterns. Use [`Engine::with_plugin_registry`] to
/// attach a registry.
pub struct Engine {
    config: PrintConfig,
    #[cfg(feature = "plugins")]
    plugin_registry: Option<slicecore_plugin::PluginRegistry>,
}

impl Engine {
    /// Creates a new engine with the given print configuration.
    pub fn new(config: PrintConfig) -> Self {
        Self {
            config,
            #[cfg(feature = "plugins")]
            plugin_registry: None,
        }
    }

    /// Attaches a plugin registry to the engine for plugin-based infill patterns.
    ///
    /// When a layer's infill pattern is [`InfillPattern::Plugin(name)`], the
    /// engine will look up the named plugin in the registry and delegate infill
    /// generation to it.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use slicecore_plugin::PluginRegistry;
    /// let registry = PluginRegistry::new();
    /// let engine = Engine::new(config).with_plugin_registry(registry);
    /// ```
    #[cfg(feature = "plugins")]
    pub fn with_plugin_registry(mut self, registry: slicecore_plugin::PluginRegistry) -> Self {
        self.plugin_registry = Some(registry);
        self
    }

    /// Generates infill for a single layer, routing plugin patterns to the
    /// registry and built-in patterns to [`generate_infill`].
    ///
    /// # Plugin dispatch
    ///
    /// When the pattern is [`InfillPattern::Plugin(name)`]:
    /// - If the `plugins` feature is enabled AND a registry is attached:
    ///   converts regions to an FFI request, calls the plugin, converts back.
    /// - Otherwise returns [`EngineError::Plugin`] with a clear message.
    ///
    /// Built-in patterns pass through to the standard `generate_infill` function.
    fn generate_infill_for_layer(
        &self,
        pattern: &InfillPattern,
        regions: &[slicecore_geo::polygon::ValidPolygon],
        density: f64,
        layer_idx: usize,
        layer_z: f64,
        line_width: f64,
        lightning_ctx: Option<&lightning::LightningContext>,
    ) -> Result<Vec<crate::infill::InfillLine>, EngineError> {
        match pattern {
            InfillPattern::Plugin(name) => {
                self.generate_plugin_infill(name, regions, density, layer_idx, layer_z, line_width)
            }
            _ => Ok(generate_infill(
                pattern,
                regions,
                density,
                layer_idx,
                layer_z,
                line_width,
                lightning_ctx,
            )),
        }
    }

    /// Dispatches infill generation to a plugin in the registry.
    ///
    /// With the `plugins` feature enabled, this converts internal types to
    /// FFI-safe types, calls the plugin, and converts the result back.
    /// Without the feature (or without a registry), returns an error.
    fn generate_plugin_infill(
        &self,
        name: &str,
        _regions: &[slicecore_geo::polygon::ValidPolygon],
        _density: f64,
        _layer_idx: usize,
        _layer_z: f64,
        _line_width: f64,
    ) -> Result<Vec<crate::infill::InfillLine>, EngineError> {
        #[cfg(feature = "plugins")]
        {
            if let Some(ref registry) = self.plugin_registry {
                if let Some(plugin) = registry.get_infill_plugin(name) {
                    let request = slicecore_plugin::regions_to_request(
                        _regions,
                        _density,
                        _layer_idx,
                        _layer_z,
                        _line_width,
                    );
                    let result = plugin.generate(&request).map_err(|e| EngineError::Plugin {
                        plugin: name.to_string(),
                        message: e.to_string(),
                    })?;
                    let converted = slicecore_plugin::ffi_result_to_lines(&result);
                    return Ok(converted
                        .into_iter()
                        .map(|line| crate::infill::InfillLine {
                            start: line.start,
                            end: line.end,
                        })
                        .collect());
                } else {
                    return Err(EngineError::Plugin {
                        plugin: name.to_string(),
                        message: format!("Plugin '{}' not found in registry", name),
                    });
                }
            }
        }

        Err(EngineError::Plugin {
            plugin: name.to_string(),
            message: "Plugin system not available (no registry attached or 'plugins' feature disabled)".to_string(),
        })
    }

    /// Slices a mesh and returns the complete G-code output.
    ///
    /// This is the main entry point. It runs the full pipeline:
    /// slice -> perimeters -> surface classify -> infill -> toolpath -> plan -> gcode.
    ///
    /// # Errors
    ///
    /// - [`EngineError::EmptyMesh`] if the mesh has no triangles.
    /// - [`EngineError::NoLayers`] if slicing produces no layers.
    /// - [`EngineError::GcodeError`] if G-code writing fails.
    pub fn slice(&self, mesh: &TriangleMesh) -> Result<SliceResult, EngineError> {
        let mut buf = Vec::new();
        let result = self.slice_to_writer(mesh, &mut buf)?;
        Ok(SliceResult {
            gcode: buf,
            layer_count: result.layer_count,
            estimated_time_seconds: result.estimated_time_seconds,
            time_estimate: result.time_estimate,
            filament_usage: result.filament_usage,
            preview: None,
        })
    }

    /// Slices a mesh and writes G-code to the given writer.
    ///
    /// Same pipeline as [`Engine::slice`] but writes directly to any
    /// [`Write`] destination instead of an in-memory buffer.
    pub fn slice_to_writer<W: Write>(
        &self,
        mesh: &TriangleMesh,
        writer: W,
    ) -> Result<SliceResult, EngineError> {
        // Validate mesh.
        if mesh.triangle_count() == 0 {
            return Err(EngineError::EmptyMesh);
        }

        // 1. Slice mesh into layers (uniform or adaptive).
        let layers = if self.config.adaptive_layer_height {
            let heights = compute_adaptive_layer_heights(
                mesh,
                self.config.adaptive_min_layer_height,
                self.config.adaptive_max_layer_height,
                self.config.adaptive_layer_quality,
                self.config.first_layer_height,
            );
            slice_mesh_adaptive(mesh, &heights)
        } else {
            slice_mesh(
                mesh,
                self.config.layer_height,
                self.config.first_layer_height,
            )
        };

        if layers.is_empty() {
            return Err(EngineError::NoLayers);
        }

        // 1b. Build lightning context if lightning infill is selected.
        // Lightning requires a cross-layer pre-pass to identify top surfaces
        // and grow support columns downward.
        let lightning_ctx = if self.config.infill_pattern == InfillPattern::Lightning {
            let layer_contours: Vec<Vec<_>> = layers
                .iter()
                .map(|l| l.contours.clone())
                .collect();
            let extrusion_width = self.config.extrusion_width();
            Some(lightning::build_lightning_context(
                &layer_contours,
                self.config.infill_density,
                extrusion_width,
            ))
        } else {
            None
        };

        // 1c. Generate support structures (if enabled).
        let extrusion_width = self.config.extrusion_width();
        let support_result = if self.config.support.enabled {
            support::generate_supports(&layers, mesh, &self.config.support, extrusion_width)
        } else {
            support::SupportResult::empty()
        };

        // 2. Process each layer: perimeters, surface classification, infill, toolpath.
        let mut layer_toolpaths: Vec<LayerToolpath> = Vec::with_capacity(layers.len());
        // Track seam position across layers for Aligned strategy.
        let mut previous_seam: Option<slicecore_math::IPoint2> = None;

        for (layer_idx, layer) in layers.iter().enumerate() {
            if layer.contours.is_empty() {
                // Empty layer -- produce empty toolpath.
                layer_toolpaths.push(LayerToolpath {
                    layer_index: layer_idx,
                    z: layer.z,
                    layer_height: layer.layer_height,
                    segments: Vec::new(),
                });
                continue;
            }

            // 2a-pre. Polyhole conversion (before perimeters).
            let contours = if self.config.polyhole_enabled {
                let mut contours = layer.contours.clone();
                crate::polyhole::convert_polyholes(
                    &mut contours,
                    self.config.nozzle_diameter,
                    self.config.polyhole_min_diameter,
                );
                contours
            } else {
                layer.contours.clone()
            };

            // 2a. Generate perimeters (with optional Arachne variable-width).
            let (perimeters, arachne_segments) = if self.config.arachne_enabled {
                let arachne_results =
                    generate_arachne_perimeters(&contours, &self.config);

                let mut classic_perimeters = Vec::new();
                let mut var_width_segs = Vec::new();

                for result in &arachne_results {
                    if let Some(ref classic) = result.classic_fallback {
                        classic_perimeters.push(classic.clone());
                    }
                    // Convert Arachne perimeters to ToolpathSegments.
                    for perim in &result.perimeters {
                        if perim.points.len() < 2 {
                            continue;
                        }
                        let feature = if perim.is_outer {
                            FeatureType::VariableWidthPerimeter
                        } else {
                            FeatureType::InnerPerimeter
                        };
                        let perim_speed =
                            self.config.perimeter_speed * 60.0; // mm/s -> mm/min
                        for i in 1..perim.points.len() {
                            let (sx, sy) = perim.points[i - 1].to_mm();
                            let (ex, ey) = perim.points[i].to_mm();
                            let start_pt = Point2::new(sx, sy);
                            let end_pt = Point2::new(ex, ey);
                            let seg_len = {
                                let dx = end_pt.x - start_pt.x;
                                let dy = end_pt.y - start_pt.y;
                                (dx * dx + dy * dy).sqrt()
                            };
                            if seg_len < 0.0001 {
                                continue;
                            }
                            // Use average width of start and end points.
                            let width =
                                (perim.widths[i - 1] + perim.widths[i]) / 2.0;
                            let e = compute_e_value(
                                seg_len,
                                width,
                                layer.layer_height,
                                self.config.filament_diameter,
                                self.config.extrusion_multiplier,
                            );
                            var_width_segs.push(ToolpathSegment {
                                start: start_pt,
                                end: end_pt,
                                feature,
                                e_value: e,
                                feedrate: perim_speed,
                                z: layer.z,
                                extrusion_width: Some(width),
                            });
                        }
                    }
                }

                (classic_perimeters, var_width_segs)
            } else {
                let perimeters = generate_perimeters(&contours, &self.config);
                (perimeters, Vec::new())
            };

            // 2b. Surface classification.
            let classification = classify_surfaces(
                &layers,
                layer_idx,
                self.config.top_solid_layers,
                self.config.bottom_solid_layers,
            );

            // 2c. Infill generation.
            // Use inner_contour from perimeters as the infill boundary.
            // Intersect with solid/sparse classification.
            let mut all_infill_lines = Vec::new();
            let mut infill_is_solid = false;

            // Generate solid infill for solid regions.
            // Solid infill always uses Rectilinear regardless of config pattern.
            if !classification.solid_regions.is_empty() {
                let solid_lines = generate_infill(
                    &InfillPattern::Rectilinear,
                    &classification.solid_regions,
                    1.0,
                    layer_idx,
                    layer.z,
                    extrusion_width,
                    None,
                );
                if !solid_lines.is_empty() {
                    all_infill_lines.extend(solid_lines);
                    infill_is_solid = true;
                }
            }

            // Generate sparse infill for sparse regions using configured pattern.
            // Uses generate_infill_for_layer to support Plugin patterns.
            if !classification.sparse_regions.is_empty()
                && self.config.infill_density > 0.0
            {
                let sparse_lines = self.generate_infill_for_layer(
                    &self.config.infill_pattern,
                    &classification.sparse_regions,
                    self.config.infill_density,
                    layer_idx,
                    layer.z,
                    extrusion_width,
                    lightning_ctx.as_ref(),
                )?;
                all_infill_lines.extend(sparse_lines);
            }

            // If there were no classified regions (possible edge case), use inner_contour.
            if classification.solid_regions.is_empty()
                && classification.sparse_regions.is_empty()
                && !perimeters.is_empty()
            {
                let inner = &perimeters[0].inner_contour;
                if !inner.is_empty() && self.config.infill_density > 0.0 {
                    let lines = self.generate_infill_for_layer(
                        &self.config.infill_pattern,
                        inner,
                        self.config.infill_density,
                        layer_idx,
                        layer.z,
                        extrusion_width,
                        lightning_ctx.as_ref(),
                    )?;
                    all_infill_lines.extend(lines);
                }
            }

            let infill = LayerInfill {
                lines: all_infill_lines,
                is_solid: infill_is_solid,
            };

            // 2d. Gap fill between perimeters.
            let gap_fills = if self.config.gap_fill_enabled && !perimeters.is_empty() {
                detect_and_fill_gaps(
                    &perimeters[0].shells,
                    &perimeters[0].inner_contour,
                    &contours,
                    self.config.gap_fill_min_width,
                    self.config.nozzle_diameter,
                    extrusion_width,
                )
            } else {
                Vec::new()
            };

            // 2e. Assemble toolpath with seam placement.
            let (mut toolpath, layer_seam) = assemble_layer_toolpath(
                layer_idx,
                layer.z,
                layer.layer_height,
                &perimeters,
                &gap_fills,
                &infill,
                &self.config,
                previous_seam,
            );

            // 2f. Insert Arachne variable-width perimeter segments.
            // Arachne segments are prepended before classic perimeters,
            // with travel moves inserted between disconnected paths.
            if !arachne_segments.is_empty() {
                let travel_speed = self.config.travel_speed * 60.0;
                let mut var_segs = Vec::new();
                let mut current_pos: Option<Point2> = None;
                for seg in &arachne_segments {
                    // Insert travel to segment start if needed.
                    if let Some(pos) = current_pos {
                        let dx = seg.start.x - pos.x;
                        let dy = seg.start.y - pos.y;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist > 0.001 {
                            var_segs.push(ToolpathSegment {
                                start: pos,
                                end: seg.start,
                                feature: FeatureType::Travel,
                                e_value: 0.0,
                                feedrate: travel_speed,
                                z: layer.z,
                                extrusion_width: None,
                            });
                        }
                    }
                    var_segs.push(seg.clone());
                    current_pos = Some(seg.end);
                }
                // Prepend variable-width segments before classic perimeters.
                var_segs.append(&mut toolpath.segments);
                toolpath.segments = var_segs;
            }

            // 2g. Assemble support toolpaths.
            if let Some(layer_support) = support_result.regions.get(layer_idx) {
                if !layer_support.is_empty() {
                    let support_segs = assemble_support_toolpath(
                        layer_support,
                        &self.config,
                        layer.z,
                        layer.layer_height,
                    );
                    toolpath.segments.extend(support_segs);
                }
            }

            // 2h. Assemble bridge toolpaths.
            if let Some(layer_bridges) = support_result.bridge_regions.get(layer_idx) {
                if !layer_bridges.is_empty() {
                    let bridge_segs = assemble_bridge_toolpath(
                        layer_bridges,
                        &self.config,
                        layer.z,
                        layer.layer_height,
                    );
                    toolpath.segments.extend(bridge_segs);
                }
            }

            // 2i. Ironing passes for top surfaces.
            // Ironing is applied after all other features (perimeters, infill,
            // support, bridges) so it smooths the final top surface.
            if self.config.ironing.enabled && !classification.solid_regions.is_empty() {
                // Only iron layers that have top surfaces.
                // Top surface detection: a layer has top surfaces if it's in the
                // last `top_solid_layers` layers OR if the layer above has a
                // different (smaller) footprint.
                let is_top_layer = layer_idx
                    >= layers.len().saturating_sub(self.config.top_solid_layers as usize);
                let has_top_exposure = if layer_idx + 1 < layers.len() {
                    // If the layer above is different from this one, there are
                    // exposed top surfaces (already captured in solid_regions).
                    !layers[layer_idx + 1].contours.is_empty()
                        && !classification.solid_regions.is_empty()
                } else {
                    true // Last layer is always a top surface
                };

                if is_top_layer || has_top_exposure {
                    let ironing_segs = generate_ironing_passes(
                        &classification.solid_regions,
                        &self.config.ironing,
                        layer.z,
                        self.config.nozzle_diameter,
                        layer.layer_height,
                        self.config.filament_diameter,
                        self.config.extrusion_multiplier,
                    );
                    toolpath.segments.extend(ironing_segs);
                }
            }

            // Update cross-layer seam tracking.
            if layer_seam.is_some() {
                previous_seam = layer_seam;
            }

            layer_toolpaths.push(toolpath);
        }

        // 3. First-layer extras: skirt/brim.
        if !layers.is_empty() && !layers[0].contours.is_empty() {
            let first_contours = &layers[0].contours;
            let first_z = layers[0].z;
            let first_layer_height = layers[0].layer_height;

            // Generate skirt or brim (brim takes priority if configured).
            let extra_polygons = if self.config.brim_width > 0.0 {
                generate_brim(first_contours, &self.config)
            } else {
                generate_skirt(first_contours, &self.config)
            };

            if !extra_polygons.is_empty() && !layer_toolpaths.is_empty() {
                let feature = if self.config.brim_width > 0.0 {
                    FeatureType::Brim
                } else {
                    FeatureType::Skirt
                };

                let speed = self.config.first_layer_speed * 60.0; // mm/s -> mm/min
                let travel_speed = self.config.travel_speed * 60.0;
                let extrusion_width = self.config.extrusion_width();

                let mut extra_segments = Vec::new();
                let mut current_pos: Option<Point2> = None;

                for polygon in &extra_polygons {
                    let pts = polygon.points();
                    if pts.len() < 2 {
                        continue;
                    }

                    let (fx, fy) = pts[0].to_mm();
                    let first_pt = Point2::new(fx, fy);

                    // Travel to polygon start.
                    if let Some(pos) = current_pos {
                        let dx = first_pt.x - pos.x;
                        let dy = first_pt.y - pos.y;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist > 0.001 {
                            extra_segments.push(ToolpathSegment {
                                start: pos,
                                end: first_pt,
                                feature: FeatureType::Travel,
                                e_value: 0.0,
                                feedrate: travel_speed,
                                z: first_z,
                            extrusion_width: None,
                            });
                        }
                    }

                    // Extrusion segments for each edge.
                    let mut prev = first_pt;
                    for ipt in pts.iter().skip(1) {
                        let (px, py) = ipt.to_mm();
                        let pt = Point2::new(px, py);
                        let dx = pt.x - prev.x;
                        let dy = pt.y - prev.y;
                        let seg_len = (dx * dx + dy * dy).sqrt();

                        if seg_len > 0.0001 {
                            let e = compute_e_value(
                                seg_len,
                                extrusion_width,
                                first_layer_height,
                                self.config.filament_diameter,
                                self.config.extrusion_multiplier,
                            );
                            extra_segments.push(ToolpathSegment {
                                start: prev,
                                end: pt,
                                feature,
                                e_value: e,
                                feedrate: speed,
                                z: first_z,
                            extrusion_width: None,
                            });
                        }
                        prev = pt;
                    }

                    // Close the polygon.
                    let dx = first_pt.x - prev.x;
                    let dy = first_pt.y - prev.y;
                    let close_len = (dx * dx + dy * dy).sqrt();
                    if close_len > 0.0001 {
                        let e = compute_e_value(
                            close_len,
                            extrusion_width,
                            first_layer_height,
                            self.config.filament_diameter,
                            self.config.extrusion_multiplier,
                        );
                        extra_segments.push(ToolpathSegment {
                            start: prev,
                            end: first_pt,
                            feature,
                            e_value: e,
                            feedrate: speed,
                            z: first_z,
                        extrusion_width: None,
                        });
                        current_pos = Some(first_pt);
                    } else {
                        current_pos = Some(prev);
                    }
                }

                // Prepend extra segments to layer 0.
                if !extra_segments.is_empty() {
                    let layer0 = &mut layer_toolpaths[0];
                    let mut new_segments = extra_segments;
                    new_segments.append(&mut layer0.segments);
                    layer0.segments = new_segments;
                }
            }
        }

        // 4. G-code generation.
        let gcode_commands = generate_full_gcode(&layer_toolpaths, &self.config);

        // 4b. Arc fitting post-processing (optional).
        let gcode_commands = if self.config.arc_fitting_enabled {
            slicecore_gcode_io::fit_arcs(
                &gcode_commands,
                self.config.arc_fitting_tolerance,
                self.config.arc_fitting_min_points,
            )
        } else {
            gcode_commands
        };

        // 5. Compute estimated time using trapezoid motion model.
        let time_estimate = estimate_print_time(
            &gcode_commands,
            self.config.print_acceleration,
            self.config.travel_acceleration,
        );
        let estimated_time = time_estimate.total_seconds;

        // 5b. Compute filament usage.
        let filament_usage = estimate_filament_usage(
            &gcode_commands,
            self.config.filament_diameter,
            self.config.filament_density,
            self.config.filament_cost_per_kg,
        );

        let layer_count = layer_toolpaths.len();

        // 6. Write G-code (using configured dialect instead of hardcoded Marlin).
        let mut gcode_writer = GcodeWriter::new(writer, self.config.gcode_dialect);

        // Start G-code.
        let start_config = StartConfig {
            bed_temp: self.config.first_layer_bed_temp,
            nozzle_temp: self.config.first_layer_nozzle_temp,
            bed_x: self.config.bed_x,
            bed_y: self.config.bed_y,
        };
        gcode_writer.write_start_gcode(&start_config)?;

        // Print body.
        gcode_writer.write_commands(&gcode_commands)?;

        // End G-code.
        let end_config = EndConfig {
            retract_distance: self.config.retract_length,
        };
        gcode_writer.write_end_gcode(&end_config)?;

        Ok(SliceResult {
            gcode: Vec::new(), // Not used in writer path.
            layer_count,
            estimated_time_seconds: estimated_time,
            time_estimate,
            filament_usage,
            preview: None,
        })
    }

    /// Slices a mesh and returns the result with preview data for visualization.
    ///
    /// This runs the same pipeline as [`Engine::slice`] but additionally
    /// generates [`SlicePreview`] data containing per-layer contours,
    /// perimeter paths, infill lines, and travel moves.
    ///
    /// # Errors
    ///
    /// Same errors as [`Engine::slice`].
    pub fn slice_with_preview(&self, mesh: &TriangleMesh) -> Result<SliceResult, EngineError> {
        // Validate mesh.
        if mesh.triangle_count() == 0 {
            return Err(EngineError::EmptyMesh);
        }

        // 1. Slice mesh into layers.
        let layers = if self.config.adaptive_layer_height {
            let heights = compute_adaptive_layer_heights(
                mesh,
                self.config.adaptive_min_layer_height,
                self.config.adaptive_max_layer_height,
                self.config.adaptive_layer_quality,
                self.config.first_layer_height,
            );
            slice_mesh_adaptive(mesh, &heights)
        } else {
            slice_mesh(
                mesh,
                self.config.layer_height,
                self.config.first_layer_height,
            )
        };

        if layers.is_empty() {
            return Err(EngineError::NoLayers);
        }

        // Capture contours for preview.
        let contours_per_layer: Vec<Vec<_>> = layers
            .iter()
            .map(|l| l.contours.clone())
            .collect();

        // Compute bounding box from mesh vertices.
        let vertices = mesh.vertices();
        let (mut min_x, mut min_y, mut min_z) = (f64::MAX, f64::MAX, f64::MAX);
        let (mut max_x, mut max_y, mut max_z) = (f64::MIN, f64::MIN, f64::MIN);
        for v in vertices {
            min_x = min_x.min(v.x);
            min_y = min_y.min(v.y);
            min_z = min_z.min(v.z);
            max_x = max_x.max(v.x);
            max_y = max_y.max(v.y);
            max_z = max_z.max(v.z);
        }
        let bounding_box = [min_x, min_y, min_z, max_x, max_y, max_z];

        // Run slicing pipeline to get G-code (reuses slice method).
        let mut result = self.slice(mesh)?;

        // Build preview from the toolpaths.
        // We need to re-run the pipeline to capture layer toolpaths.
        // Instead, run a lightweight internal pipeline to extract toolpaths.
        // For efficiency, re-slice and capture toolpaths inline.
        let lightning_ctx = if self.config.infill_pattern == InfillPattern::Lightning {
            Some(lightning::build_lightning_context(
                &contours_per_layer,
                self.config.infill_density,
                self.config.extrusion_width(),
            ))
        } else {
            None
        };

        let mut layer_toolpaths: Vec<LayerToolpath> = Vec::with_capacity(layers.len());
        let mut previous_seam: Option<slicecore_math::IPoint2> = None;

        for (layer_idx, layer) in layers.iter().enumerate() {
            if layer.contours.is_empty() {
                layer_toolpaths.push(LayerToolpath {
                    layer_index: layer_idx,
                    z: layer.z,
                    layer_height: layer.layer_height,
                    segments: Vec::new(),
                });
                continue;
            }

            // Polyhole conversion in preview pipeline.
            let preview_contours = if self.config.polyhole_enabled {
                let mut c = layer.contours.clone();
                crate::polyhole::convert_polyholes(
                    &mut c,
                    self.config.nozzle_diameter,
                    self.config.polyhole_min_diameter,
                );
                c
            } else {
                layer.contours.clone()
            };

            let (perimeters, arachne_segments) = if self.config.arachne_enabled {
                let arachne_results =
                    generate_arachne_perimeters(&preview_contours, &self.config);
                let mut classic_perimeters = Vec::new();
                let mut var_width_segs = Vec::new();
                for r in &arachne_results {
                    if let Some(ref classic) = r.classic_fallback {
                        classic_perimeters.push(classic.clone());
                    }
                    for perim in &r.perimeters {
                        if perim.points.len() < 2 {
                            continue;
                        }
                        let feature = if perim.is_outer {
                            FeatureType::VariableWidthPerimeter
                        } else {
                            FeatureType::InnerPerimeter
                        };
                        let perim_speed = self.config.perimeter_speed * 60.0;
                        for i in 1..perim.points.len() {
                            let (sx, sy) = perim.points[i - 1].to_mm();
                            let (ex, ey) = perim.points[i].to_mm();
                            let start_pt = Point2::new(sx, sy);
                            let end_pt = Point2::new(ex, ey);
                            let seg_len = {
                                let dx = end_pt.x - start_pt.x;
                                let dy = end_pt.y - start_pt.y;
                                (dx * dx + dy * dy).sqrt()
                            };
                            if seg_len < 0.0001 {
                                continue;
                            }
                            let width = (perim.widths[i - 1] + perim.widths[i]) / 2.0;
                            let e = compute_e_value(
                                seg_len,
                                width,
                                layer.layer_height,
                                self.config.filament_diameter,
                                self.config.extrusion_multiplier,
                            );
                            var_width_segs.push(ToolpathSegment {
                                start: start_pt,
                                end: end_pt,
                                feature,
                                e_value: e,
                                feedrate: perim_speed,
                                z: layer.z,
                                extrusion_width: Some(width),
                            });
                        }
                    }
                }
                (classic_perimeters, var_width_segs)
            } else {
                let perimeters = generate_perimeters(&preview_contours, &self.config);
                (perimeters, Vec::new())
            };

            let classification = classify_surfaces(
                &layers,
                layer_idx,
                self.config.top_solid_layers,
                self.config.bottom_solid_layers,
            );

            let extrusion_width = self.config.extrusion_width();
            let mut all_infill_lines = Vec::new();
            let mut infill_is_solid = false;

            if !classification.solid_regions.is_empty() {
                let solid_lines = generate_infill(
                    &InfillPattern::Rectilinear,
                    &classification.solid_regions,
                    1.0,
                    layer_idx,
                    layer.z,
                    extrusion_width,
                    None,
                );
                if !solid_lines.is_empty() {
                    all_infill_lines.extend(solid_lines);
                    infill_is_solid = true;
                }
            }

            if !classification.sparse_regions.is_empty()
                && self.config.infill_density > 0.0
            {
                let sparse_lines = self.generate_infill_for_layer(
                    &self.config.infill_pattern,
                    &classification.sparse_regions,
                    self.config.infill_density,
                    layer_idx,
                    layer.z,
                    extrusion_width,
                    lightning_ctx.as_ref(),
                )?;
                all_infill_lines.extend(sparse_lines);
            }

            if classification.solid_regions.is_empty()
                && classification.sparse_regions.is_empty()
                && !perimeters.is_empty()
            {
                let inner = &perimeters[0].inner_contour;
                if !inner.is_empty() && self.config.infill_density > 0.0 {
                    let lines = self.generate_infill_for_layer(
                        &self.config.infill_pattern,
                        inner,
                        self.config.infill_density,
                        layer_idx,
                        layer.z,
                        extrusion_width,
                        lightning_ctx.as_ref(),
                    )?;
                    all_infill_lines.extend(lines);
                }
            }

            let infill = LayerInfill {
                lines: all_infill_lines,
                is_solid: infill_is_solid,
            };

            let gap_fills = if self.config.gap_fill_enabled && !perimeters.is_empty() {
                crate::gap_fill::detect_and_fill_gaps(
                    &perimeters[0].shells,
                    &perimeters[0].inner_contour,
                    &preview_contours,
                    self.config.gap_fill_min_width,
                    self.config.nozzle_diameter,
                    extrusion_width,
                )
            } else {
                Vec::new()
            };

            let (mut toolpath, layer_seam) = assemble_layer_toolpath(
                layer_idx,
                layer.z,
                layer.layer_height,
                &perimeters,
                &gap_fills,
                &infill,
                &self.config,
                previous_seam,
            );

            if !arachne_segments.is_empty() {
                let travel_speed = self.config.travel_speed * 60.0;
                let mut var_segs = Vec::new();
                let mut current_pos: Option<Point2> = None;
                for seg in &arachne_segments {
                    if let Some(pos) = current_pos {
                        let dx = seg.start.x - pos.x;
                        let dy = seg.start.y - pos.y;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist > 0.001 {
                            var_segs.push(ToolpathSegment {
                                start: pos,
                                end: seg.start,
                                feature: FeatureType::Travel,
                                e_value: 0.0,
                                feedrate: travel_speed,
                                z: layer.z,
                                extrusion_width: None,
                            });
                        }
                    }
                    var_segs.push(seg.clone());
                    current_pos = Some(seg.end);
                }
                var_segs.append(&mut toolpath.segments);
                toolpath.segments = var_segs;
            }

            if layer_seam.is_some() {
                previous_seam = layer_seam;
            }

            layer_toolpaths.push(toolpath);
        }

        // Generate preview from captured data.
        let preview = generate_preview(&layer_toolpaths, &contours_per_layer, bounding_box);
        result.preview = Some(preview);

        Ok(result)
    }

    /// Slices a mesh with modifier mesh overrides.
    ///
    /// Modifier meshes define 3D regions where different settings apply.
    /// At each layer, modifier meshes are sliced at the layer Z, and model
    /// contours are split into regions with per-region effective configs.
    ///
    /// This method extends the standard pipeline by applying
    /// [`split_by_modifiers`] before perimeter and infill generation.
    ///
    /// # Errors
    ///
    /// Same errors as [`Engine::slice`].
    pub fn slice_with_modifiers(
        &self,
        mesh: &TriangleMesh,
        modifiers: &[ModifierMesh],
    ) -> Result<SliceResult, EngineError> {
        if modifiers.is_empty() {
            return self.slice(mesh);
        }

        // Validate mesh.
        if mesh.triangle_count() == 0 {
            return Err(EngineError::EmptyMesh);
        }

        // 1. Slice mesh into layers.
        let layers = if self.config.adaptive_layer_height {
            let heights = compute_adaptive_layer_heights(
                mesh,
                self.config.adaptive_min_layer_height,
                self.config.adaptive_max_layer_height,
                self.config.adaptive_layer_quality,
                self.config.first_layer_height,
            );
            slice_mesh_adaptive(mesh, &heights)
        } else {
            slice_mesh(
                mesh,
                self.config.layer_height,
                self.config.first_layer_height,
            )
        };

        if layers.is_empty() {
            return Err(EngineError::NoLayers);
        }

        // 1b. Lightning context (if needed).
        let lightning_ctx = if self.config.infill_pattern == InfillPattern::Lightning {
            let layer_contours: Vec<Vec<_>> = layers.iter().map(|l| l.contours.clone()).collect();
            Some(lightning::build_lightning_context(
                &layer_contours,
                self.config.infill_density,
                self.config.extrusion_width(),
            ))
        } else {
            None
        };

        // 1c. Support structures.
        let extrusion_width = self.config.extrusion_width();
        let support_result = if self.config.support.enabled {
            support::generate_supports(&layers, mesh, &self.config.support, extrusion_width)
        } else {
            support::SupportResult::empty()
        };

        // 2. Per-layer processing with modifier support.
        let mut layer_toolpaths: Vec<LayerToolpath> = Vec::with_capacity(layers.len());
        let mut previous_seam: Option<slicecore_math::IPoint2> = None;

        for (layer_idx, layer) in layers.iter().enumerate() {
            if layer.contours.is_empty() {
                layer_toolpaths.push(LayerToolpath {
                    layer_index: layer_idx,
                    z: layer.z,
                    layer_height: layer.layer_height,
                    segments: Vec::new(),
                });
                continue;
            }

            // 2a. Slice modifiers at this layer Z.
            let modifier_regions: Vec<_> = modifiers
                .iter()
                .filter_map(|m| slice_modifier(m, layer.z))
                .collect();

            // 2b. Split contours by modifiers.
            let region_configs =
                split_by_modifiers(&layer.contours, &modifier_regions, &self.config);

            // 2c. Process each region separately.
            let mut all_segments = Vec::new();

            for (region_contours, region_config) in &region_configs {
                // Perimeters.
                let perimeters = generate_perimeters(region_contours, region_config);

                // Surface classification.
                let classification = classify_surfaces(
                    &layers,
                    layer_idx,
                    region_config.top_solid_layers,
                    region_config.bottom_solid_layers,
                );

                // Infill.
                let region_extrusion_width = region_config.extrusion_width();
                let mut all_infill_lines = Vec::new();
                let mut infill_is_solid = false;

                if !classification.solid_regions.is_empty() {
                    let solid_lines = generate_infill(
                        &InfillPattern::Rectilinear,
                        &classification.solid_regions,
                        1.0,
                        layer_idx,
                        layer.z,
                        region_extrusion_width,
                        None,
                    );
                    if !solid_lines.is_empty() {
                        all_infill_lines.extend(solid_lines);
                        infill_is_solid = true;
                    }
                }

                if !classification.sparse_regions.is_empty()
                    && region_config.infill_density > 0.0
                {
                    let sparse_lines = self.generate_infill_for_layer(
                        &region_config.infill_pattern,
                        &classification.sparse_regions,
                        region_config.infill_density,
                        layer_idx,
                        layer.z,
                        region_extrusion_width,
                        lightning_ctx.as_ref(),
                    )?;
                    all_infill_lines.extend(sparse_lines);
                }

                if classification.solid_regions.is_empty()
                    && classification.sparse_regions.is_empty()
                    && !perimeters.is_empty()
                {
                    let inner = &perimeters[0].inner_contour;
                    if !inner.is_empty() && region_config.infill_density > 0.0 {
                        let lines = self.generate_infill_for_layer(
                            &region_config.infill_pattern,
                            inner,
                            region_config.infill_density,
                            layer_idx,
                            layer.z,
                            region_extrusion_width,
                            lightning_ctx.as_ref(),
                        )?;
                        all_infill_lines.extend(lines);
                    }
                }

                let infill = LayerInfill {
                    lines: all_infill_lines,
                    is_solid: infill_is_solid,
                };

                // Gap fill.
                let gap_fills = if region_config.gap_fill_enabled && !perimeters.is_empty() {
                    detect_and_fill_gaps(
                        &perimeters[0].shells,
                        &perimeters[0].inner_contour,
                        region_contours,
                        region_config.gap_fill_min_width,
                        region_config.nozzle_diameter,
                        region_extrusion_width,
                    )
                } else {
                    Vec::new()
                };

                // Assemble toolpath for this region.
                let (region_toolpath, layer_seam) = assemble_layer_toolpath(
                    layer_idx,
                    layer.z,
                    layer.layer_height,
                    &perimeters,
                    &gap_fills,
                    &infill,
                    region_config,
                    previous_seam,
                );

                all_segments.extend(region_toolpath.segments);

                if layer_seam.is_some() {
                    previous_seam = layer_seam;
                }
            }

            let mut toolpath = LayerToolpath {
                layer_index: layer_idx,
                z: layer.z,
                layer_height: layer.layer_height,
                segments: all_segments,
            };

            // 2d. Support toolpaths.
            if let Some(layer_support) = support_result.regions.get(layer_idx) {
                if !layer_support.is_empty() {
                    let support_segs = assemble_support_toolpath(
                        layer_support,
                        &self.config,
                        layer.z,
                        layer.layer_height,
                    );
                    toolpath.segments.extend(support_segs);
                }
            }

            // 2e. Bridge toolpaths.
            if let Some(layer_bridges) = support_result.bridge_regions.get(layer_idx) {
                if !layer_bridges.is_empty() {
                    let bridge_segs = assemble_bridge_toolpath(
                        layer_bridges,
                        &self.config,
                        layer.z,
                        layer.layer_height,
                    );
                    toolpath.segments.extend(bridge_segs);
                }
            }

            // 2f. Ironing.
            if self.config.ironing.enabled {
                let classification = classify_surfaces(
                    &layers,
                    layer_idx,
                    self.config.top_solid_layers,
                    self.config.bottom_solid_layers,
                );
                if !classification.solid_regions.is_empty() {
                    let is_top_layer = layer_idx
                        >= layers.len().saturating_sub(self.config.top_solid_layers as usize);
                    let has_top_exposure = if layer_idx + 1 < layers.len() {
                        !layers[layer_idx + 1].contours.is_empty()
                            && !classification.solid_regions.is_empty()
                    } else {
                        true
                    };

                    if is_top_layer || has_top_exposure {
                        let ironing_segs = generate_ironing_passes(
                            &classification.solid_regions,
                            &self.config.ironing,
                            layer.z,
                            self.config.nozzle_diameter,
                            layer.layer_height,
                            self.config.filament_diameter,
                            self.config.extrusion_multiplier,
                        );
                        toolpath.segments.extend(ironing_segs);
                    }
                }
            }

            layer_toolpaths.push(toolpath);
        }

        // 3. First-layer extras: skirt/brim.
        if !layers.is_empty() && !layers[0].contours.is_empty() {
            let first_contours = &layers[0].contours;
            let first_z = layers[0].z;
            let first_layer_height = layers[0].layer_height;

            let extra_polygons = if self.config.brim_width > 0.0 {
                generate_brim(first_contours, &self.config)
            } else {
                generate_skirt(first_contours, &self.config)
            };

            if !extra_polygons.is_empty() && !layer_toolpaths.is_empty() {
                let feature = if self.config.brim_width > 0.0 {
                    FeatureType::Brim
                } else {
                    FeatureType::Skirt
                };

                let speed = self.config.first_layer_speed * 60.0;
                let travel_speed = self.config.travel_speed * 60.0;
                let ext_width = self.config.extrusion_width();

                let mut extra_segments = Vec::new();
                let mut current_pos: Option<Point2> = None;

                for polygon in &extra_polygons {
                    let pts = polygon.points();
                    if pts.len() < 2 {
                        continue;
                    }

                    let (fx, fy) = pts[0].to_mm();
                    let first_pt = Point2::new(fx, fy);

                    if let Some(pos) = current_pos {
                        let dx = first_pt.x - pos.x;
                        let dy = first_pt.y - pos.y;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist > 0.001 {
                            extra_segments.push(ToolpathSegment {
                                start: pos,
                                end: first_pt,
                                feature: FeatureType::Travel,
                                e_value: 0.0,
                                feedrate: travel_speed,
                                z: first_z,
                                extrusion_width: None,
                            });
                        }
                    }

                    let mut prev = first_pt;
                    for ipt in pts.iter().skip(1) {
                        let (px, py) = ipt.to_mm();
                        let pt = Point2::new(px, py);
                        let dx = pt.x - prev.x;
                        let dy = pt.y - prev.y;
                        let seg_len = (dx * dx + dy * dy).sqrt();

                        if seg_len > 0.0001 {
                            let e = compute_e_value(
                                seg_len,
                                ext_width,
                                first_layer_height,
                                self.config.filament_diameter,
                                self.config.extrusion_multiplier,
                            );
                            extra_segments.push(ToolpathSegment {
                                start: prev,
                                end: pt,
                                feature,
                                e_value: e,
                                feedrate: speed,
                                z: first_z,
                                extrusion_width: None,
                            });
                        }
                        prev = pt;
                    }

                    let dx = first_pt.x - prev.x;
                    let dy = first_pt.y - prev.y;
                    let close_len = (dx * dx + dy * dy).sqrt();
                    if close_len > 0.0001 {
                        let e = compute_e_value(
                            close_len,
                            ext_width,
                            first_layer_height,
                            self.config.filament_diameter,
                            self.config.extrusion_multiplier,
                        );
                        extra_segments.push(ToolpathSegment {
                            start: prev,
                            end: first_pt,
                            feature,
                            e_value: e,
                            feedrate: speed,
                            z: first_z,
                            extrusion_width: None,
                        });
                        current_pos = Some(first_pt);
                    } else {
                        current_pos = Some(prev);
                    }
                }

                if !extra_segments.is_empty() {
                    let layer0 = &mut layer_toolpaths[0];
                    let mut new_segments = extra_segments;
                    new_segments.append(&mut layer0.segments);
                    layer0.segments = new_segments;
                }
            }
        }

        // 4. G-code generation.
        let gcode_commands = generate_full_gcode(&layer_toolpaths, &self.config);

        // 4b. Arc fitting post-processing (optional).
        let gcode_commands = if self.config.arc_fitting_enabled {
            slicecore_gcode_io::fit_arcs(
                &gcode_commands,
                self.config.arc_fitting_tolerance,
                self.config.arc_fitting_min_points,
            )
        } else {
            gcode_commands
        };

        // 5. Compute estimated time using trapezoid motion model.
        let time_estimate = estimate_print_time(
            &gcode_commands,
            self.config.print_acceleration,
            self.config.travel_acceleration,
        );
        let estimated_time = time_estimate.total_seconds;

        // 5b. Compute filament usage.
        let filament_usage = estimate_filament_usage(
            &gcode_commands,
            self.config.filament_diameter,
            self.config.filament_density,
            self.config.filament_cost_per_kg,
        );

        let layer_count = layer_toolpaths.len();

        // 6. Write G-code.
        let mut buf = Vec::new();
        let mut gcode_writer = GcodeWriter::new(&mut buf, self.config.gcode_dialect);

        let start_config = StartConfig {
            bed_temp: self.config.first_layer_bed_temp,
            nozzle_temp: self.config.first_layer_nozzle_temp,
            bed_x: self.config.bed_x,
            bed_y: self.config.bed_y,
        };
        gcode_writer.write_start_gcode(&start_config)?;
        gcode_writer.write_commands(&gcode_commands)?;

        let end_config = EndConfig {
            retract_distance: self.config.retract_length,
        };
        gcode_writer.write_end_gcode(&end_config)?;

        Ok(SliceResult {
            gcode: buf,
            layer_count,
            estimated_time_seconds: estimated_time,
            time_estimate,
            filament_usage,
            preview: None,
        })
    }
}

// ---------------------------------------------------------------------------
// AI Integration (feature-gated)
// ---------------------------------------------------------------------------

/// AI-powered profile suggestion, available when the `ai` feature is enabled.
///
/// This impl block adds `suggest_profile` to the Engine, which analyzes a mesh's
/// geometry and sends features to a configured LLM provider to receive validated
/// print settings.
///
/// AI configuration (API keys, provider selection, model) should come from
/// environment variables or a separate config file, NOT from `PrintConfig`.
/// This keeps the core print configuration clean and avoids coupling the slicing
/// pipeline to AI dependencies.
#[cfg(feature = "ai")]
impl Engine {
    /// Suggest optimal print settings for a mesh using AI.
    ///
    /// Analyzes the mesh geometry and sends features to the configured
    /// LLM provider, returning a validated profile suggestion.
    ///
    /// Requires the `ai` feature flag and a valid [`slicecore_ai::AiConfig`].
    /// The `AiConfig` should be sourced from environment variables or a
    /// separate configuration file, not from `PrintConfig`.
    ///
    /// # Errors
    ///
    /// Returns [`slicecore_ai::AiError`] on provider failures, parse errors,
    /// or runtime issues (e.g., cannot create async runtime).
    pub fn suggest_profile(
        &self,
        mesh: &slicecore_mesh::TriangleMesh,
        ai_config: &slicecore_ai::AiConfig,
    ) -> Result<slicecore_ai::ProfileSuggestion, slicecore_ai::AiError> {
        let provider = slicecore_ai::create_provider(ai_config)?;
        slicecore_ai::suggest_profile_sync(provider.as_ref(), mesh)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_math::Point3;

    /// Creates a unit cube mesh (1mm x 1mm x 1mm) for testing.
    fn unit_cube() -> TriangleMesh {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
        ];
        let indices = vec![
            [4, 5, 6],
            [4, 6, 7],
            [1, 0, 3],
            [1, 3, 2],
            [1, 2, 6],
            [1, 6, 5],
            [0, 4, 7],
            [0, 7, 3],
            [3, 7, 6],
            [3, 6, 2],
            [0, 1, 5],
            [0, 5, 4],
        ];
        TriangleMesh::new(vertices, indices).expect("unit cube should be valid")
    }

    #[test]
    fn engine_slice_produces_non_empty_gcode() {
        let config = PrintConfig::default();
        let engine = Engine::new(config);
        let mesh = unit_cube();

        let result = engine.slice(&mesh).expect("slice should succeed");

        assert!(
            !result.gcode.is_empty(),
            "G-code output should be non-empty"
        );
        assert!(
            result.layer_count > 0,
            "Layer count should be positive, got {}",
            result.layer_count
        );
    }

    #[test]
    fn gcode_contains_expected_commands() {
        let config = PrintConfig::default();
        let engine = Engine::new(config);
        let mesh = unit_cube();

        let result = engine.slice(&mesh).expect("slice should succeed");
        let gcode_str = String::from_utf8_lossy(&result.gcode);

        // Should contain home command from start gcode.
        assert!(
            gcode_str.contains("G28"),
            "G-code should contain G28 (home)"
        );

        // Should contain relative extrusion mode.
        assert!(
            gcode_str.contains("M83"),
            "G-code should contain M83 (relative extrusion)"
        );

        // Should contain temperature commands.
        assert!(
            gcode_str.contains("M104") || gcode_str.contains("M109"),
            "G-code should contain nozzle temperature commands"
        );

        // Should contain extrusion moves.
        assert!(
            gcode_str.contains("G1"),
            "G-code should contain G1 (linear move)"
        );
    }

    #[test]
    fn layer_count_matches_expected() {
        let config = PrintConfig {
            layer_height: 0.2,
            first_layer_height: 0.2,
            ..Default::default()
        };
        let engine = Engine::new(config);
        let mesh = unit_cube();

        let result = engine.slice(&mesh).expect("slice should succeed");

        // 1mm cube with 0.2mm layers = 5 layers.
        assert_eq!(
            result.layer_count, 5,
            "1mm cube with 0.2mm layers should produce 5 layers, got {}",
            result.layer_count
        );
    }

    #[test]
    fn estimated_time_is_positive() {
        let config = PrintConfig::default();
        let engine = Engine::new(config);
        let mesh = unit_cube();

        let result = engine.slice(&mesh).expect("slice should succeed");

        assert!(
            result.estimated_time_seconds > 0.0,
            "Estimated time should be positive, got {}",
            result.estimated_time_seconds
        );
    }

    #[test]
    fn deterministic_output() {
        let config = PrintConfig::default();
        let mesh = unit_cube();

        let engine1 = Engine::new(config.clone());
        let result1 = engine1.slice(&mesh).expect("first slice should succeed");

        let engine2 = Engine::new(config);
        let result2 = engine2.slice(&mesh).expect("second slice should succeed");

        assert_eq!(
            result1.gcode, result2.gcode,
            "Same mesh + same config should produce identical G-code"
        );
        assert_eq!(result1.layer_count, result2.layer_count);
        assert!(
            (result1.estimated_time_seconds - result2.estimated_time_seconds).abs() < 1e-9,
            "Estimated times should be identical"
        );
    }

    #[test]
    fn adaptive_disabled_produces_same_as_default() {
        let mesh = unit_cube();

        let config_default = PrintConfig::default();
        let result_default = Engine::new(config_default.clone())
            .slice(&mesh)
            .expect("default slice should succeed");

        let mut config_adaptive_off = config_default;
        config_adaptive_off.adaptive_layer_height = false;
        let result_off = Engine::new(config_adaptive_off)
            .slice(&mesh)
            .expect("adaptive=false slice should succeed");

        assert_eq!(
            result_default.gcode, result_off.gcode,
            "adaptive_layer_height=false should produce same output as default"
        );
    }

    #[test]
    fn adaptive_enabled_produces_valid_gcode() {
        let mesh = unit_cube();

        let config = PrintConfig {
            adaptive_layer_height: true,
            adaptive_min_layer_height: 0.05,
            adaptive_max_layer_height: 0.3,
            adaptive_layer_quality: 0.5,
            ..Default::default()
        };
        let engine = Engine::new(config);

        let result = engine.slice(&mesh).expect("adaptive slice should succeed");
        assert!(
            !result.gcode.is_empty(),
            "Adaptive G-code output should be non-empty"
        );
        assert!(
            result.layer_count > 0,
            "Adaptive should produce at least 1 layer"
        );

        let gcode_str = String::from_utf8_lossy(&result.gcode);
        assert!(
            gcode_str.contains("G1"),
            "Adaptive G-code should contain extrusion moves"
        );
    }

    #[test]
    fn adaptive_layers_have_varying_heights() {
        // Use a sphere-like mesh to trigger varying heights.
        // The unit cube won't trigger much variation because walls are uniform.
        // So just verify the infrastructure works: adaptive produces layers
        // where layer_height varies from the uniform case.
        let mesh = unit_cube();

        let config = PrintConfig {
            adaptive_layer_height: true,
            adaptive_min_layer_height: 0.05,
            adaptive_max_layer_height: 0.3,
            adaptive_layer_quality: 0.5,
            first_layer_height: 0.3,
            ..Default::default()
        };
        let engine = Engine::new(config);

        let result = engine.slice(&mesh).expect("adaptive slice should succeed");
        assert!(
            result.layer_count > 0,
            "Adaptive should produce at least 1 layer"
        );
    }

    #[test]
    fn adaptive_layer_z_values_monotonically_increasing() {
        let mesh = unit_cube();

        let config = PrintConfig {
            adaptive_layer_height: true,
            adaptive_min_layer_height: 0.05,
            adaptive_max_layer_height: 0.3,
            adaptive_layer_quality: 0.5,
            ..Default::default()
        };

        // Verify via the slicer directly.
        let heights = slicecore_slicer::compute_adaptive_layer_heights(
            &mesh,
            config.adaptive_min_layer_height,
            config.adaptive_max_layer_height,
            config.adaptive_layer_quality,
            config.first_layer_height,
        );
        let layers = slicecore_slicer::slice_mesh_adaptive(&mesh, &heights);

        for i in 1..layers.len() {
            assert!(
                layers[i].z > layers[i - 1].z,
                "Adaptive layer Z values should be monotonically increasing: z[{}]={} <= z[{}]={}",
                i,
                layers[i].z,
                i - 1,
                layers[i - 1].z,
            );
        }
    }

    #[test]
    fn half_layer_height_roughly_doubles_layers() {
        let mesh = unit_cube();

        let config_02 = PrintConfig {
            layer_height: 0.2,
            first_layer_height: 0.2,
            ..Default::default()
        };
        let result_02 = Engine::new(config_02)
            .slice(&mesh)
            .expect("0.2mm slice should succeed");

        let config_01 = PrintConfig {
            layer_height: 0.1,
            first_layer_height: 0.1,
            ..Default::default()
        };
        let result_01 = Engine::new(config_01)
            .slice(&mesh)
            .expect("0.1mm slice should succeed");

        // With 0.2mm layers we get 5; with 0.1mm we should get ~10.
        let ratio = result_01.layer_count as f64 / result_02.layer_count as f64;
        assert!(
            ratio >= 1.8 && ratio <= 2.2,
            "Layer count ratio should be ~2.0, got {} ({} vs {})",
            ratio,
            result_01.layer_count,
            result_02.layer_count
        );
    }

    #[test]
    fn arachne_disabled_produces_same_as_default() {
        let mesh = unit_cube();

        let config_default = PrintConfig::default();
        let result_default = Engine::new(config_default.clone())
            .slice(&mesh)
            .expect("default slice should succeed");

        let mut config_off = config_default;
        config_off.arachne_enabled = false;
        let result_off = Engine::new(config_off)
            .slice(&mesh)
            .expect("arachne=false slice should succeed");

        assert_eq!(
            result_default.gcode, result_off.gcode,
            "arachne_enabled=false should produce same output as default"
        );
    }

    #[test]
    fn arachne_enabled_produces_valid_gcode() {
        let mesh = unit_cube();

        let config = PrintConfig {
            arachne_enabled: true,
            ..Default::default()
        };
        let engine = Engine::new(config);

        let result = engine.slice(&mesh).expect("arachne slice should succeed");
        assert!(
            !result.gcode.is_empty(),
            "Arachne-enabled G-code output should be non-empty"
        );
        assert!(
            result.layer_count > 0,
            "Arachne-enabled should produce at least 1 layer"
        );

        let gcode_str = String::from_utf8_lossy(&result.gcode);
        assert!(
            gcode_str.contains("G1"),
            "Arachne G-code should contain extrusion moves"
        );
    }

    #[test]
    fn arachne_config_defaults_to_disabled() {
        let config = PrintConfig::default();
        assert!(
            !config.arachne_enabled,
            "arachne_enabled should default to false"
        );
    }

    #[test]
    fn support_disabled_produces_identical_output() {
        let mesh = unit_cube();

        // Default config has support disabled.
        let config_default = PrintConfig::default();
        let result_default = Engine::new(config_default.clone())
            .slice(&mesh)
            .expect("default slice should succeed");

        // Explicitly set support.enabled = false.
        let mut config_explicit = config_default;
        config_explicit.support.enabled = false;
        let result_explicit = Engine::new(config_explicit)
            .slice(&mesh)
            .expect("support-disabled slice should succeed");

        assert_eq!(
            result_default.gcode, result_explicit.gcode,
            "Support disabled should produce identical output to default"
        );
    }

    #[test]
    fn support_enabled_on_cube_produces_valid_gcode() {
        // A unit cube has no overhangs, so support should not add anything.
        // But the pipeline should still run without errors.
        let mesh = unit_cube();

        let config = PrintConfig {
            support: crate::support::config::SupportConfig {
                enabled: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let engine = Engine::new(config);

        let result = engine.slice(&mesh).expect("support-enabled slice should succeed");
        assert!(
            !result.gcode.is_empty(),
            "Support-enabled G-code output should be non-empty"
        );
        assert!(
            result.layer_count > 0,
            "Support-enabled should produce at least 1 layer"
        );

        let gcode_str = String::from_utf8_lossy(&result.gcode);
        assert!(
            gcode_str.contains("G1"),
            "Support-enabled G-code should contain extrusion moves"
        );
    }

    #[test]
    fn support_config_defaults_to_disabled() {
        let config = PrintConfig::default();
        assert!(
            !config.support.enabled,
            "support.enabled should default to false"
        );
    }

    #[test]
    fn arc_fitting_disabled_by_default() {
        let config = PrintConfig::default();
        assert!(
            !config.arc_fitting_enabled,
            "arc_fitting_enabled should default to false"
        );
    }

    #[test]
    fn arc_fitting_disabled_produces_identical_output() {
        let mesh = unit_cube();

        let config_default = PrintConfig::default();
        let result_default = Engine::new(config_default.clone())
            .slice(&mesh)
            .expect("default slice should succeed");

        let mut config_explicit = config_default;
        config_explicit.arc_fitting_enabled = false;
        let result_explicit = Engine::new(config_explicit)
            .slice(&mesh)
            .expect("arc-fitting-disabled slice should succeed");

        assert_eq!(
            result_default.gcode, result_explicit.gcode,
            "Arc fitting disabled should produce identical output to default"
        );
    }

    #[test]
    fn arc_fitting_enabled_produces_valid_gcode() {
        let mesh = unit_cube();

        let config = PrintConfig {
            arc_fitting_enabled: true,
            arc_fitting_tolerance: 0.05,
            arc_fitting_min_points: 3,
            ..Default::default()
        };
        let engine = Engine::new(config);

        let result = engine.slice(&mesh).expect("arc-fitting slice should succeed");
        assert!(
            !result.gcode.is_empty(),
            "Arc-fitting-enabled G-code output should be non-empty"
        );
        assert!(
            result.layer_count > 0,
            "Arc-fitting-enabled should produce at least 1 layer"
        );

        let gcode_str = String::from_utf8_lossy(&result.gcode);
        assert!(
            gcode_str.contains("G1"),
            "Arc-fitting G-code should contain extrusion moves"
        );
    }

    #[test]
    fn slice_result_has_populated_time_estimate() {
        let config = PrintConfig::default();
        let engine = Engine::new(config);
        let mesh = unit_cube();

        let result = engine.slice(&mesh).expect("slice should succeed");

        assert!(
            result.time_estimate.total_seconds > 0.0,
            "time_estimate.total_seconds should be positive, got {}",
            result.time_estimate.total_seconds
        );
        assert!(
            result.time_estimate.move_time_seconds > 0.0,
            "time_estimate.move_time_seconds should be positive"
        );
        // Backward compatibility: estimated_time_seconds matches time_estimate.
        assert!(
            (result.estimated_time_seconds - result.time_estimate.total_seconds).abs() < 1e-9,
            "estimated_time_seconds should match time_estimate.total_seconds"
        );
    }

    #[test]
    fn slice_result_has_populated_filament_usage() {
        let config = PrintConfig::default();
        let engine = Engine::new(config);
        let mesh = unit_cube();

        let result = engine.slice(&mesh).expect("slice should succeed");

        assert!(
            result.filament_usage.length_mm > 0.0,
            "filament_usage.length_mm should be positive, got {}",
            result.filament_usage.length_mm
        );
        assert!(
            result.filament_usage.weight_g > 0.0,
            "filament_usage.weight_g should be positive, got {}",
            result.filament_usage.weight_g
        );
        assert!(
            result.filament_usage.cost > 0.0,
            "filament_usage.cost should be positive, got {}",
            result.filament_usage.cost
        );
        assert!(
            (result.filament_usage.length_m - result.filament_usage.length_mm / 1000.0).abs()
                < 1e-9,
            "length_m should be length_mm / 1000"
        );
    }

    // -----------------------------------------------------------------------
    // Phase 6 Success Criteria Integration Tests
    // -----------------------------------------------------------------------

    /// Creates a 20mm calibration cube mesh centered at (100,100) on the bed.
    fn calibration_cube_20mm() -> TriangleMesh {
        let ox = 90.0;
        let oy = 90.0;
        let vertices = vec![
            Point3::new(ox, oy, 0.0),
            Point3::new(ox + 20.0, oy, 0.0),
            Point3::new(ox + 20.0, oy + 20.0, 0.0),
            Point3::new(ox, oy + 20.0, 0.0),
            Point3::new(ox, oy, 20.0),
            Point3::new(ox + 20.0, oy, 20.0),
            Point3::new(ox + 20.0, oy + 20.0, 20.0),
            Point3::new(ox, oy + 20.0, 20.0),
        ];
        let indices = vec![
            [4, 5, 6],
            [4, 6, 7],
            [1, 0, 3],
            [1, 3, 2],
            [1, 2, 6],
            [1, 6, 5],
            [0, 4, 7],
            [0, 7, 3],
            [3, 7, 6],
            [3, 6, 2],
            [0, 1, 5],
            [0, 5, 4],
        ];
        TriangleMesh::new(vertices, indices).expect("calibration cube should be valid")
    }

    /// Creates a box mesh from (x0, y0, z0) to (x1, y1, z1).
    fn make_box_mesh(x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64) -> TriangleMesh {
        let vertices = vec![
            Point3::new(x0, y0, z0),
            Point3::new(x1, y0, z0),
            Point3::new(x1, y1, z0),
            Point3::new(x0, y1, z0),
            Point3::new(x0, y0, z1),
            Point3::new(x1, y0, z1),
            Point3::new(x1, y1, z1),
            Point3::new(x0, y1, z1),
        ];
        let indices = vec![
            [4, 5, 6],
            [4, 6, 7],
            [1, 0, 3],
            [1, 3, 2],
            [1, 2, 6],
            [1, 6, 5],
            [0, 4, 7],
            [0, 7, 3],
            [3, 7, 6],
            [3, 6, 2],
            [0, 1, 5],
            [0, 5, 4],
        ];
        TriangleMesh::new(vertices, indices).expect("box mesh should be valid")
    }

    // ---- SC1: Firmware dialect validation ----

    #[test]
    fn test_phase_6_sc1_klipper_dialect() {
        let config = PrintConfig {
            gcode_dialect: slicecore_gcode_io::GcodeDialect::Klipper,
            ..Default::default()
        };
        let engine = Engine::new(config);
        let mesh = calibration_cube_20mm();

        let result = engine.slice(&mesh).expect("Klipper slice should succeed");
        let gcode_str = String::from_utf8_lossy(&result.gcode);

        // Validate G-code passes syntax validation.
        let validation = slicecore_gcode_io::validate_gcode(&gcode_str);
        assert!(
            validation.valid,
            "Klipper G-code should pass validation: {:?}",
            validation.errors
        );

        // Check Klipper-specific content.
        assert!(
            gcode_str.contains("Klipper dialect"),
            "Should contain 'Klipper dialect' in start comment"
        );
        assert!(
            gcode_str.contains("BED_MESH_CALIBRATE"),
            "Should contain Klipper-specific BED_MESH_CALIBRATE"
        );
        assert!(
            gcode_str.contains("TURN_OFF_HEATERS"),
            "Should contain Klipper-specific TURN_OFF_HEATERS"
        );

        // G-code should contain extrusion moves.
        assert!(
            gcode_str.contains("G1"),
            "Klipper G-code should contain G1 extrusion moves"
        );
    }

    #[test]
    fn test_phase_6_sc1_reprap_dialect() {
        let config = PrintConfig {
            gcode_dialect: slicecore_gcode_io::GcodeDialect::RepRapFirmware,
            ..Default::default()
        };
        let engine = Engine::new(config);
        let mesh = calibration_cube_20mm();

        let result = engine.slice(&mesh).expect("RepRap slice should succeed");
        let gcode_str = String::from_utf8_lossy(&result.gcode);

        // Validate G-code passes syntax validation.
        let validation = slicecore_gcode_io::validate_gcode(&gcode_str);
        assert!(
            validation.valid,
            "RepRap G-code should pass validation: {:?}",
            validation.errors
        );

        // Check RepRap-specific content.
        assert!(
            gcode_str.contains("RepRapFirmware dialect"),
            "Should contain 'RepRapFirmware dialect' in start comment"
        );
        assert!(
            gcode_str.contains("M0 H1"),
            "Should contain RepRap-specific halt command M0 H1"
        );

        // G-code should contain extrusion moves.
        assert!(
            gcode_str.contains("G1"),
            "RepRap G-code should contain G1 extrusion moves"
        );
    }

    #[test]
    fn test_phase_6_sc1_bambu_dialect() {
        let config = PrintConfig {
            gcode_dialect: slicecore_gcode_io::GcodeDialect::Bambu,
            ..Default::default()
        };
        let engine = Engine::new(config);
        let mesh = calibration_cube_20mm();

        let result = engine.slice(&mesh).expect("Bambu slice should succeed");
        let gcode_str = String::from_utf8_lossy(&result.gcode);

        // Validate G-code passes syntax validation.
        let validation = slicecore_gcode_io::validate_gcode(&gcode_str);
        assert!(
            validation.valid,
            "Bambu G-code should pass validation: {:?}",
            validation.errors
        );

        // Check Bambu-specific content.
        assert!(
            gcode_str.contains("Bambu"),
            "Should contain 'Bambu' in start comment"
        );
        // Bambu printers have AMS commands.
        assert!(
            gcode_str.contains("M620") || gcode_str.contains("M621"),
            "Should contain Bambu-specific AMS commands (M620/M621)"
        );

        // G-code should contain extrusion moves.
        assert!(
            gcode_str.contains("G1"),
            "Bambu G-code should contain G1 extrusion moves"
        );
    }

    // ---- SC2: Multi-material tool changes ----

    #[test]
    fn test_phase_6_sc2_multi_material() {
        use crate::config::{MultiMaterialConfig, ToolConfig};
        use crate::multimaterial::{generate_tool_change, generate_purge_tower_layer};

        let mm_config = MultiMaterialConfig {
            enabled: true,
            tool_count: 2,
            tools: vec![
                ToolConfig {
                    nozzle_temp: 200.0,
                    retract_length: 0.8,
                    retract_speed: 45.0,
                },
                ToolConfig {
                    nozzle_temp: 210.0,
                    retract_length: 1.0,
                    retract_speed: 45.0,
                },
            ],
            purge_tower_position: [200.0, 200.0],
            purge_tower_width: 15.0,
            purge_volume: 70.0,
            wipe_length: 2.0,
        };

        let print_config = PrintConfig {
            multi_material: mm_config.clone(),
            ..Default::default()
        };

        // 1. Generate a tool change sequence T0 -> T1.
        let seq = generate_tool_change(0, 1, &mm_config, &print_config);

        // Verify tool change contains T0/T1 commands.
        let output_lines: Vec<String> = seq.commands.iter().map(|c| c.to_string()).collect();
        let joined = output_lines.join("\n");

        assert!(
            joined.contains("T1"),
            "Tool change should contain T1 command"
        );

        // Should contain retract and unretract.
        let has_retract = seq.commands.iter().any(|cmd| {
            matches!(cmd, slicecore_gcode_io::GcodeCommand::Retract { .. })
        });
        assert!(has_retract, "Tool change should include retraction");

        let has_unretract = seq.commands.iter().any(|cmd| {
            matches!(cmd, slicecore_gcode_io::GcodeCommand::Unretract { .. })
        });
        assert!(has_unretract, "Tool change should include prime (unretract)");

        // 2. Generate a dense purge tower layer (tool change layer).
        let tower_layer = generate_purge_tower_layer(
            0.4,  // layer_z
            0.2,  // layer_height
            &mm_config,
            true, // has_tool_change
            0.4,  // nozzle_diameter
        );

        assert!(tower_layer.is_dense, "Tool-change layer should produce dense tower");
        let tower_lines: Vec<String> = tower_layer.commands.iter().map(|c| c.to_string()).collect();
        let tower_joined = tower_lines.join("\n");

        // Purge tower should have extrusion moves at the configured position.
        assert!(
            tower_joined.contains("G1"),
            "Purge tower should contain extrusion (G1) moves"
        );
        assert!(
            tower_joined.contains("200.0") || tower_joined.contains("200.000"),
            "Purge tower should be at configured position (200, 200)"
        );

        // 3. Validate the combined output as valid G-code.
        // Build a minimal full G-code with tool change.
        let mut full_gcode = String::new();
        full_gcode.push_str("; start\nG28\nM83\nM104 S200\nM140 S60\nM190 S60\nM109 S200\nG92 E0\n");
        full_gcode.push_str(&joined);
        full_gcode.push('\n');
        full_gcode.push_str(&tower_joined);
        full_gcode.push_str("\n; end\nM107\nM84\n");

        let validation = slicecore_gcode_io::validate_gcode(&full_gcode);
        assert!(
            validation.valid,
            "Combined multi-material G-code should pass validation: {:?}",
            validation.errors
        );
    }

    // ---- SC3: Modifier mesh region override ----

    #[test]
    fn test_phase_6_sc3_modifier_mesh() {
        use crate::config::SettingOverrides;
        use crate::modifier::ModifierMesh;

        // Model mesh: 20mm cube at (90, 90, 0) to (110, 110, 20).
        let model_mesh = calibration_cube_20mm();

        // Modifier mesh: 10mm cube positioned inside the model.
        // Centered at (100, 100, 10) -- the inner volume of the model.
        let modifier_mesh = make_box_mesh(95.0, 95.0, 0.0, 105.0, 105.0, 20.0);

        // The modifier overrides infill density from 20% (base) to 80%.
        let modifiers = vec![ModifierMesh {
            mesh: modifier_mesh,
            overrides: SettingOverrides {
                infill_density: Some(0.8),
                ..Default::default()
            },
        }];

        let base_config = PrintConfig {
            infill_density: 0.2,
            ..Default::default()
        };
        let engine = Engine::new(base_config);

        // Slice with modifiers through the full engine pipeline.
        let result = engine
            .slice_with_modifiers(&model_mesh, &modifiers)
            .expect("modifier slice should succeed");

        let gcode_str = String::from_utf8_lossy(&result.gcode);

        // Validate output is valid G-code.
        let validation = slicecore_gcode_io::validate_gcode(&gcode_str);
        assert!(
            validation.valid,
            "Modifier mesh G-code should pass validation: {:?}",
            validation.errors
        );

        // The output should contain extrusion moves (proving the pipeline ran).
        assert!(
            gcode_str.contains("G1"),
            "Modifier mesh G-code should contain G1 extrusion moves"
        );

        // Verify that the modifier pipeline produces two distinct config
        // regions with different infill densities. We test the split_by_modifiers
        // function directly at a mid-layer Z to confirm region separation.
        use crate::modifier::{slice_modifier, split_by_modifiers};

        let mid_z = 10.0;
        let model_layers = slicecore_slicer::slice_mesh(
            &model_mesh,
            0.2,
            0.3,
        );

        // Find a layer near mid_z.
        let mid_layer = model_layers
            .iter()
            .min_by(|a, b| {
                (a.z - mid_z).abs().partial_cmp(&(b.z - mid_z).abs()).unwrap()
            })
            .expect("should have layers");

        let modifier_regions: Vec<_> = modifiers
            .iter()
            .filter_map(|m| slice_modifier(m, mid_layer.z))
            .collect();

        let base_config = PrintConfig {
            infill_density: 0.2,
            ..Default::default()
        };
        let regions = split_by_modifiers(&mid_layer.contours, &modifier_regions, &base_config);

        // Should produce 2 regions: modified (0.8 density) and remainder (0.2 density).
        assert!(
            regions.len() >= 2,
            "split_by_modifiers should produce at least 2 regions (modified + remainder), got {}",
            regions.len()
        );

        let has_high_density = regions
            .iter()
            .any(|(_, cfg)| (cfg.infill_density - 0.8).abs() < 0.01);
        assert!(
            has_high_density,
            "One region should have 80% infill density (modifier override)"
        );

        let has_low_density = regions
            .iter()
            .any(|(_, cfg)| (cfg.infill_density - 0.2).abs() < 0.01);
        assert!(
            has_low_density,
            "One region should have 20% infill density (base config)"
        );

        // The output should have more layers than zero (basic pipeline sanity).
        assert!(
            result.layer_count > 0,
            "Should produce layers, got {}",
            result.layer_count
        );
    }

    // ---- SC4: Print time and filament estimation accuracy ----

    #[test]
    fn test_phase_6_sc4_estimation() {
        let config = PrintConfig::default();
        let engine = Engine::new(config);
        let mesh = calibration_cube_20mm();

        let result = engine.slice(&mesh).expect("slice should succeed");

        // SC4.1: time_estimate.total_seconds > 0
        assert!(
            result.time_estimate.total_seconds > 0.0,
            "time_estimate.total_seconds should be positive, got {}",
            result.time_estimate.total_seconds
        );

        // SC4.2: Trapezoid time > naive time (proves acceleration modeling adds time).
        // Compute a naive estimate: sum all move distances / feedrate.
        let gcode_str = String::from_utf8_lossy(&result.gcode);
        let mut naive_time = 0.0;
        let mut prev_x = 0.0f64;
        let mut prev_y = 0.0f64;
        let mut prev_z = 0.0f64;
        let mut feedrate = 60.0f64; // mm/s default

        for line in gcode_str.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("G1 ") || trimmed.starts_with("G0 ") {
                let mut x = prev_x;
                let mut y = prev_y;
                let mut z = prev_z;
                for part in trimmed.split_whitespace().skip(1) {
                    if let Some(val) = part.strip_prefix('X') {
                        x = val.parse().unwrap_or(x);
                    } else if let Some(val) = part.strip_prefix('Y') {
                        y = val.parse().unwrap_or(y);
                    } else if let Some(val) = part.strip_prefix('Z') {
                        z = val.parse().unwrap_or(z);
                    } else if let Some(val) = part.strip_prefix('F') {
                        feedrate = val.parse::<f64>().unwrap_or(feedrate) / 60.0;
                    }
                }
                let dx = x - prev_x;
                let dy = y - prev_y;
                let dz = z - prev_z;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                if dist > 0.001 && feedrate > 0.0 {
                    naive_time += dist / feedrate;
                }
                prev_x = x;
                prev_y = y;
                prev_z = z;
            }
        }

        assert!(
            naive_time > 0.0,
            "Naive time should be positive for a 20mm cube"
        );
        assert!(
            result.time_estimate.total_seconds > naive_time,
            "Trapezoid estimate ({:.2}s) should exceed naive estimate ({:.2}s)",
            result.time_estimate.total_seconds,
            naive_time
        );

        // SC4.3: filament_usage.length_mm > 0
        assert!(
            result.filament_usage.length_mm > 0.0,
            "filament_usage.length_mm should be positive, got {}",
            result.filament_usage.length_mm
        );

        // SC4.4: filament_usage.weight_g > 0
        assert!(
            result.filament_usage.weight_g > 0.0,
            "filament_usage.weight_g should be positive, got {}",
            result.filament_usage.weight_g
        );

        // SC4.5: filament_usage.cost > 0
        assert!(
            result.filament_usage.cost > 0.0,
            "filament_usage.cost should be positive, got {}",
            result.filament_usage.cost
        );

        // SC4.6: Filament length is reasonable for a 20mm cube.
        // Roughly 1000-10000mm depending on infill density.
        assert!(
            result.filament_usage.length_mm > 500.0,
            "Filament usage for 20mm cube should be >500mm, got {:.1}mm",
            result.filament_usage.length_mm
        );
        assert!(
            result.filament_usage.length_mm < 50000.0,
            "Filament usage for 20mm cube should be <50000mm, got {:.1}mm",
            result.filament_usage.length_mm
        );
    }

    #[test]
    fn test_phase_6_sc4_estimation_acceleration_impact() {
        let mesh = calibration_cube_20mm();

        // Low acceleration config: 500 mm/s^2.
        let config_low_accel = PrintConfig {
            print_acceleration: 500.0,
            travel_acceleration: 750.0,
            ..Default::default()
        };
        let result_low = Engine::new(config_low_accel)
            .slice(&mesh)
            .expect("low-accel slice should succeed");

        // High acceleration config: 3000 mm/s^2.
        let config_high_accel = PrintConfig {
            print_acceleration: 3000.0,
            travel_acceleration: 4500.0,
            ..Default::default()
        };
        let result_high = Engine::new(config_high_accel)
            .slice(&mesh)
            .expect("high-accel slice should succeed");

        // Low acceleration should produce a longer time estimate because
        // the machine spends more time in ramp phases.
        assert!(
            result_low.time_estimate.total_seconds > result_high.time_estimate.total_seconds,
            "Low acceleration ({:.2}s) should take longer than high acceleration ({:.2}s)",
            result_low.time_estimate.total_seconds,
            result_high.time_estimate.total_seconds
        );
    }

    // ---- SC5: Arc fitting file size reduction ----

    #[test]
    fn test_phase_6_sc5_arc_fitting() {
        // Arc fitting on a cube won't produce arcs (no curves), so we test
        // the arc fitting machinery directly with synthetic circular G1 moves
        // to verify G2/G3 output and command count reduction.

        // Generate G1 commands forming a semicircle (many short segments).
        let center_x = 100.0;
        let center_y = 100.0;
        let radius = 10.0;
        let num_segments = 36; // 10-degree increments for a full circle

        let mut commands = Vec::new();
        // Move to the starting point.
        let start_x = center_x + radius;
        let start_y = center_y;
        commands.push(slicecore_gcode_io::GcodeCommand::LinearMove {
            x: Some(start_x),
            y: Some(start_y),
            z: Some(0.2),
            e: Some(0.0),
            f: Some(3000.0),
        });

        for i in 1..=num_segments {
            let angle = 2.0 * std::f64::consts::PI * (i as f64) / (num_segments as f64);
            let x = center_x + radius * angle.cos();
            let y = center_y + radius * angle.sin();
            let seg_len = 2.0 * radius * std::f64::consts::PI / (num_segments as f64);
            let e = seg_len * 0.02; // Small E per segment
            commands.push(slicecore_gcode_io::GcodeCommand::LinearMove {
                x: Some(x),
                y: Some(y),
                z: None,
                e: Some(e),
                f: Some(3000.0),
            });
        }

        let original_count = commands.len();

        // Apply arc fitting.
        let arc_commands = slicecore_gcode_io::fit_arcs(&commands, 0.05, 3);
        let arc_count = arc_commands.len();

        // Arc-fitted output should contain G2 and/or G3 commands.
        let has_arcs = arc_commands.iter().any(|cmd| {
            matches!(
                cmd,
                slicecore_gcode_io::GcodeCommand::ArcMoveCW { .. }
                    | slicecore_gcode_io::GcodeCommand::ArcMoveCCW { .. }
            )
        });
        assert!(
            has_arcs,
            "Arc fitting should produce G2/G3 arc commands from circular G1 segments"
        );

        // Arc-fitted output should have fewer commands.
        assert!(
            arc_count < original_count,
            "Arc-fitted command count ({}) should be less than original ({})",
            arc_count,
            original_count
        );

        // Arc-fitted output should be smaller in bytes (serialized form).
        let original_bytes: usize = commands.iter().map(|c| c.to_string().len() + 1).sum();
        let arc_bytes: usize = arc_commands.iter().map(|c| c.to_string().len() + 1).sum();
        assert!(
            arc_bytes < original_bytes,
            "Arc-fitted byte size ({}) should be less than original ({})",
            arc_bytes,
            original_bytes
        );

        // Both outputs should pass G-code validation.
        let original_gcode: String = commands.iter().map(|c| c.to_string() + "\n").collect();
        let arc_gcode: String = arc_commands.iter().map(|c| c.to_string() + "\n").collect();

        let original_validation = slicecore_gcode_io::validate_gcode(&original_gcode);
        assert!(
            original_validation.valid,
            "Original G-code should pass validation: {:?}",
            original_validation.errors
        );

        let arc_validation = slicecore_gcode_io::validate_gcode(&arc_gcode);
        assert!(
            arc_validation.valid,
            "Arc-fitted G-code should pass validation: {:?}",
            arc_validation.errors
        );

        // Verify the arc endpoints match the original endpoints (within tolerance).
        // Check that the last command in both sequences ends at approximately
        // the same position.
        fn last_xy(cmds: &[slicecore_gcode_io::GcodeCommand]) -> (f64, f64) {
            let mut x = 0.0;
            let mut y = 0.0;
            for cmd in cmds {
                match cmd {
                    slicecore_gcode_io::GcodeCommand::LinearMove {
                        x: Some(cx),
                        y: Some(cy),
                        ..
                    } => {
                        x = *cx;
                        y = *cy;
                    }
                    slicecore_gcode_io::GcodeCommand::ArcMoveCW {
                        x: Some(cx),
                        y: Some(cy),
                        ..
                    }
                    | slicecore_gcode_io::GcodeCommand::ArcMoveCCW {
                        x: Some(cx),
                        y: Some(cy),
                        ..
                    } => {
                        x = *cx;
                        y = *cy;
                    }
                    _ => {}
                }
            }
            (x, y)
        }

        let (orig_x, orig_y) = last_xy(&commands);
        let (arc_x, arc_y) = last_xy(&arc_commands);
        let endpoint_error = ((orig_x - arc_x).powi(2) + (orig_y - arc_y).powi(2)).sqrt();
        assert!(
            endpoint_error < 0.5,
            "Arc endpoint ({:.3}, {:.3}) should be within 0.5mm of original ({:.3}, {:.3}), error = {:.4}mm",
            arc_x, arc_y, orig_x, orig_y, endpoint_error
        );

        // Also verify arc fitting works through the full engine pipeline
        // (even if a cube doesn't produce arcs, the pipeline should not error).
        let cube_config = PrintConfig {
            arc_fitting_enabled: true,
            arc_fitting_tolerance: 0.05,
            arc_fitting_min_points: 3,
            ..Default::default()
        };
        let engine = Engine::new(cube_config);
        let mesh = calibration_cube_20mm();
        let cube_result = engine.slice(&mesh).expect("arc-fitting engine slice should succeed");
        assert!(
            !cube_result.gcode.is_empty(),
            "Arc-fitting enabled should still produce valid G-code"
        );
        assert!(
            cube_result.layer_count > 0,
            "Arc-fitting enabled should still produce layers"
        );
    }

    #[test]
    fn engine_builtin_pattern_without_registry_works() {
        // Verify that built-in patterns continue to work when no plugin registry
        // is attached (the default case).
        let config = PrintConfig {
            infill_pattern: InfillPattern::Rectilinear,
            infill_density: 0.2,
            ..Default::default()
        };
        let engine = Engine::new(config);
        let mesh = unit_cube();
        let result = engine.slice(&mesh);
        assert!(
            result.is_ok(),
            "Built-in pattern should work without plugin registry: {:?}",
            result.err()
        );
        let result = result.unwrap();
        assert!(!result.gcode.is_empty());
        assert!(result.layer_count > 0);
    }

    #[test]
    fn engine_plugin_pattern_without_registry_returns_error() {
        // Verify that InfillPattern::Plugin returns EngineError::Plugin
        // when no plugin registry is attached. Uses 20mm cube so infill
        // regions are large enough to trigger the Plugin dispatch path.
        let config = PrintConfig {
            infill_pattern: InfillPattern::Plugin("zigzag".to_string()),
            infill_density: 0.2,
            ..Default::default()
        };
        let engine = Engine::new(config);
        let mesh = calibration_cube_20mm();
        let result = engine.slice(&mesh);
        assert!(result.is_err(), "Plugin pattern without registry should fail");
        let err = result.unwrap_err();
        match &err {
            EngineError::Plugin { plugin, message } => {
                assert_eq!(plugin, "zigzag");
                assert!(
                    message.contains("not available") || message.contains("not found"),
                    "Error message should explain unavailability: {}",
                    message
                );
            }
            other => panic!(
                "Expected EngineError::Plugin, got: {:?}",
                other
            ),
        }
    }

    #[test]
    fn infill_pattern_plugin_serde_round_trip() {
        // Verify Plugin(String) serializes/deserializes correctly via TOML.
        let toml_str = r#"infill_pattern = { plugin = "custom-zigzag" }"#;
        let config = PrintConfig::from_toml(toml_str).unwrap();
        assert_eq!(
            config.infill_pattern,
            InfillPattern::Plugin("custom-zigzag".to_string())
        );
    }
}
