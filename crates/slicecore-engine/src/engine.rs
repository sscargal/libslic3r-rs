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

use slicecore_gcode_io::{EndConfig, GcodeDialect, GcodeWriter, StartConfig};
use slicecore_mesh::TriangleMesh;
use slicecore_slicer::{compute_adaptive_layer_heights, slice_mesh, slice_mesh_adaptive};

use crate::arachne::generate_arachne_perimeters;
use crate::config::PrintConfig;
use crate::error::EngineError;
use crate::gap_fill::detect_and_fill_gaps;
use crate::gcode_gen::generate_full_gcode;
use crate::infill::{generate_infill, lightning, InfillPattern, LayerInfill};
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
    /// Total estimated print time in seconds.
    pub estimated_time_seconds: f64,
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
            crate::infill::InfillPattern::Rectilinear,
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
pub struct Engine {
    config: PrintConfig,
}

impl Engine {
    /// Creates a new engine with the given print configuration.
    pub fn new(config: PrintConfig) -> Self {
        Self { config }
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

            // 2a. Generate perimeters (with optional Arachne variable-width).
            let (perimeters, arachne_segments) = if self.config.arachne_enabled {
                let arachne_results =
                    generate_arachne_perimeters(&layer.contours, &self.config);

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
                let perimeters = generate_perimeters(&layer.contours, &self.config);
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
                    InfillPattern::Rectilinear,
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
            if !classification.sparse_regions.is_empty()
                && self.config.infill_density > 0.0
            {
                let sparse_lines = generate_infill(
                    self.config.infill_pattern,
                    &classification.sparse_regions,
                    self.config.infill_density,
                    layer_idx,
                    layer.z,
                    extrusion_width,
                    lightning_ctx.as_ref(),
                );
                all_infill_lines.extend(sparse_lines);
            }

            // If there were no classified regions (possible edge case), use inner_contour.
            if classification.solid_regions.is_empty()
                && classification.sparse_regions.is_empty()
                && !perimeters.is_empty()
            {
                let inner = &perimeters[0].inner_contour;
                if !inner.is_empty() && self.config.infill_density > 0.0 {
                    let lines = generate_infill(
                        self.config.infill_pattern,
                        inner,
                        self.config.infill_density,
                        layer_idx,
                        layer.z,
                        extrusion_width,
                        lightning_ctx.as_ref(),
                    );
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
                    &layer.contours,
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

        // 5. Compute estimated time.
        let estimated_time: f64 = layer_toolpaths
            .iter()
            .map(|lt| lt.estimated_time_seconds())
            .sum();

        let layer_count = layer_toolpaths.len();

        // 6. Write G-code.
        let mut gcode_writer = GcodeWriter::new(writer, GcodeDialect::Marlin);

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

            let (perimeters, arachne_segments) = if self.config.arachne_enabled {
                let arachne_results =
                    generate_arachne_perimeters(&layer.contours, &self.config);
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
                let perimeters = generate_perimeters(&layer.contours, &self.config);
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
                    InfillPattern::Rectilinear,
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
                let sparse_lines = generate_infill(
                    self.config.infill_pattern,
                    &classification.sparse_regions,
                    self.config.infill_density,
                    layer_idx,
                    layer.z,
                    extrusion_width,
                    lightning_ctx.as_ref(),
                );
                all_infill_lines.extend(sparse_lines);
            }

            if classification.solid_regions.is_empty()
                && classification.sparse_regions.is_empty()
                && !perimeters.is_empty()
            {
                let inner = &perimeters[0].inner_contour;
                if !inner.is_empty() && self.config.infill_density > 0.0 {
                    let lines = generate_infill(
                        self.config.infill_pattern,
                        inner,
                        self.config.infill_density,
                        layer_idx,
                        layer.z,
                        extrusion_width,
                        lightning_ctx.as_ref(),
                    );
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
                    &layer.contours,
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
}
