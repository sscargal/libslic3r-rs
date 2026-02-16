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
use slicecore_slicer::slice_mesh;

use crate::config::PrintConfig;
use crate::error::EngineError;
use crate::gcode_gen::generate_full_gcode;
use crate::infill::{generate_rectilinear_infill, alternate_infill_angle, LayerInfill};
use crate::perimeter::generate_perimeters;
use crate::planner::{generate_brim, generate_skirt};
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

        // 1. Slice mesh into layers.
        let layers = slice_mesh(
            mesh,
            self.config.layer_height,
            self.config.first_layer_height,
        );

        if layers.is_empty() {
            return Err(EngineError::NoLayers);
        }

        // 2. Process each layer: perimeters, surface classification, infill, toolpath.
        let mut layer_toolpaths: Vec<LayerToolpath> = Vec::with_capacity(layers.len());

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

            // 2a. Generate perimeters.
            let perimeters = generate_perimeters(&layer.contours, &self.config);

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
            let angle = alternate_infill_angle(layer_idx);
            let extrusion_width = self.config.extrusion_width();

            let mut all_infill_lines = Vec::new();
            let mut infill_is_solid = false;

            // Generate solid infill for solid regions.
            if !classification.solid_regions.is_empty() {
                // In Phase 3, we use the solid_regions directly as the infill boundary
                // since they represent the areas needing 100% fill.
                let solid_lines = generate_rectilinear_infill(
                    &classification.solid_regions,
                    1.0,
                    angle,
                    extrusion_width,
                );
                if !solid_lines.is_empty() {
                    all_infill_lines.extend(solid_lines);
                    infill_is_solid = true;
                }
            }

            // Generate sparse infill for sparse regions.
            if !classification.sparse_regions.is_empty()
                && self.config.infill_density > 0.0
            {
                let sparse_lines = generate_rectilinear_infill(
                    &classification.sparse_regions,
                    self.config.infill_density,
                    angle,
                    extrusion_width,
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
                    let lines = generate_rectilinear_infill(
                        inner,
                        self.config.infill_density,
                        angle,
                        extrusion_width,
                    );
                    all_infill_lines.extend(lines);
                }
            }

            let infill = LayerInfill {
                lines: all_infill_lines,
                is_solid: infill_is_solid,
            };

            // 2d. Assemble toolpath.
            let toolpath = assemble_layer_toolpath(
                layer_idx,
                layer.z,
                layer.layer_height,
                &perimeters,
                &infill,
                &self.config,
            );

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
        })
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
}
