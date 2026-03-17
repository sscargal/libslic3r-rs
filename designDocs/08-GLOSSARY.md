# LibSlic3r-RS: Glossary of Terms

**Version:** 1.0.0-draft
**Date:** 2026-02-13

---

This glossary maps 3D printing terminology to their LibSlic3r-RS code equivalents. It serves as a reference for developers, AI assistants, and documentation authors.

---

## A

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **Adaptive Layer Height** | Variable layer heights based on surface curvature — thin where detail matters, thick where geometry is simple | `slicecore-slicer::adaptive_heights()` |
| **Adaptive Pressure Advance** | Dynamic PA adjustment based on flow rate and acceleration (OrcaSlicer innovation) | `slicecore-planner::adaptive_pa` |
| **AMS** | Automatic Material System — Bambu Lab's multi-filament system | Handled in `GcodeDialect::Bambu` |
| **Arachne** | Variable-width perimeter algorithm based on Voronoi/medial axis | `slicecore-perimeters::arachne` module |
| **Arc Fitting** | Converting linear G-code segments into G2/G3 arc commands | `slicecore-pathing::arc_fit()` |

## B

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **Bed Adhesion** | Methods to keep the print stuck to the build plate (skirt, brim, raft) | `config.bed_adhesion` settings group |
| **Boolean Operation** | Union, intersection, difference, XOR on polygons | `slicecore-geo::clip()` |
| **Bridge** | Horizontal span of filament over empty space between two supports | `BridgeDetector` in `slicecore-slicer` |
| **Brim** | Single-layer ring around the base of a print for adhesion | `slicecore-gcode-gen::brim()` |
| **Brim Ears** | Small adhesion pads placed only at sharp corners of the model base (mouse ears) | `slicecore-gcode-gen::brim_ears()` |
| **BVH** | Bounding Volume Hierarchy — spatial index for fast mesh queries | `slicecore-mesh::BVH` |

## C

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **Clipper** | Polygon clipping library (originally C++, by Angus Johnson) | Replaced by `slicecore-geo` |
| **Contour** | The outline of a slice layer — a closed polygon | `Polygon` in `slicecore-geo` |
| **Cooling** | Fan control and speed reduction to prevent overheating | `slicecore-planner::cooling` |
| **Counterbore Hole Bridging** | Technique to bridge over counterbore holes instead of filling them (OrcaSlicer) | `slicecore-slicer::counterbore_bridge` |
| **CW / CCW** | Clockwise / Counter-Clockwise polygon winding direction. CCW = outer boundary, CW = hole | `Polygon::winding()` |

## D

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **Degenerate Triangle** | Triangle with zero area (all vertices collinear) | `MeshError::DegenerateTriangle` |
| **Deterministic Output** | Same input + config always produces identical G-code | Enforced by architecture |

## E

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **EdgeGrid** | Custom 2D spatial grid for fast polygon proximity queries — O(1) average lookup | `slicecore-geo::EdgeGrid` |
| **Elephant's Foot** | First-layer bulging due to bed heat and compression | Compensated in `slicecore-slicer` |
| **Extrusion** | The act of pushing filament through the nozzle; also the resulting deposited material | `ExtrusionSegment` |
| **Extrusion Width** | The width of a single deposited line of filament | `config.extrusion_width` |

## F

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **Fan Speed** | Cooling fan speed (0–255 or 0–100%) | `PlannedMove::fan_speed` |
| **FDM** | Fused Deposition Modeling — the 3D printing process we target | — |
| **Filament** | The thermoplastic material fed into the printer | `FilamentProfile` in config |
| **Flow Rate** | Volume of material extruded per unit time (mm³/s) | `ExtrusionSegment::flow_rate` |
| **Fuzzy Skin** | Random displacement of outer wall for textured finish | `slicecore-perimeters::fuzzy_skin` |

## G

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **G-code** | Machine instructions for the printer (G0, G1, G2, G3, M commands) | `slicecore-gcode-io`, `slicecore-gcode-gen` |
| **G0** | Rapid travel move (no extrusion) | `MoveType::Travel` |
| **G1** | Linear extrusion or controlled travel | `MoveType::Extrusion` |
| **G2/G3** | Clockwise/counter-clockwise arc moves | `MoveType::Arc` |
| **Gap Fill** | Thin extrusions to fill gaps between perimeters | `slicecore-perimeters::gap_fill()` |
| **Gyroid** | TPMS-based infill pattern with excellent strength-to-weight | `slicecore-infill::patterns::gyroid` |

## I

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **i-overlay** | Rust polygon boolean operations crate — candidate replacement for C++ Clipper library | External crate |
| **Infill** | Internal fill pattern that provides structure and supports top surfaces | `slicecore-infill` crate |
| **Infill Density** | Percentage of internal volume filled (0–100%) | `config.infill.density` |
| **InnerOuterInner** | Wall ordering that prints inner walls, then outer, then remaining inner for best accuracy + overhang support | `config.perimeters.wall_sequence` |
| **Ironing** | Final smoothing pass over top surfaces with low flow | `slicecore-perimeters::ironing()` |

## J-K

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **Jerk** | Instantaneous speed change allowed at direction changes (mm/s) | `config.printer.limits.max_jerk` |
| **Junction Deviation** | Klipper's alternative to jerk for cornering speed | Supported in `slicecore-planner` |
| **Klipper** | Open-source firmware running on a host computer + MCU | `GcodeDialect::Klipper` |

## L

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **Layer** | One horizontal slice of the 3D model | `SliceLayer` |
| **Layer Height** | Vertical thickness of each layer (mm) | `config.quality.layer_height` |
| **Lightning Infill** | Tree-like infill that only supports top surfaces | `slicecore-infill::patterns::lightning` |

## M

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **Make Overhang Printable** | Auto-modify model geometry to reduce extreme overhangs below a threshold angle (OrcaSlicer) | `slicecore-mesh::make_overhang_printable()` |
| **Manifold** | A mesh where every edge is shared by exactly two triangles | `TriangleMesh::is_manifold()` |
| **Marlin** | Most common open-source 3D printer firmware | `GcodeDialect::Marlin` |
| **Medial Axis** | The skeleton of a polygon — used for Arachne perimeters | `slicecore-geo::medial_axis()` |
| **Modifier Mesh** | A volume that overrides settings for the region it intersects | `ModifierMesh` in engine |
| **Monotonic** | Fill pattern where lines are printed in one consistent direction | `slicecore-infill::patterns::monotonic` |

## N-O

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **Non-Manifold** | Mesh defect where an edge has more or fewer than 2 adjacent faces | `MeshError::NonManifold` |
| **Nozzle Diameter** | Physical diameter of the printer nozzle (typically 0.4mm) | `config.printer.nozzle_diameter` |
| **Overhang** | Part of the model that extends beyond the layer below without support | `DetectedFeatures::overhangs` |

## P

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **Perimeter** | The walls/outline of each layer (also called "walls" or "shells") | `slicecore-perimeters` crate |
| **Polyhole** | Polygon approximation of circular holes that prints at correct diameter due to FDM corner rounding (OrcaSlicer) | `slicecore-perimeters::hole_to_polyhole()` |
| **Pressure Advance** | Firmware feature that compensates for pressure buildup in the nozzle | `config.retraction.pressure_advance` |
| **PrintObjectStep** | The 9-step slicing pipeline enum from C++: posSlice, posPerimeters, posPrepareInfill, posInfill, posIroning, posSupportSpotsSearch, posSupportMaterial, posEstimateCurledExtrusions, posCalculateOverhangingPerimeters | Pipeline stages in `slicecore-engine` |
| **Profile** | A complete set of settings for a print job | `PrintConfig` |
| **Purge Tower** | Structure printed during tool changes to flush the old filament | `slicecore-gcode-gen::wipe_tower` |

## R

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **Raft** | Multi-layer platform printed under the model for adhesion | `slicecore-gcode-gen::raft()` |
| **Retraction** | Pulling filament back to prevent oozing during travel | `Retraction` struct |
| **RepRapFirmware (RRF)** | Firmware by Duet3D | `GcodeDialect::RepRapFirmware` |

## S

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **Safety Offset** | Tiny polygon expansion before boolean operations to prevent numerical artifacts at shared edges (Clipper pattern) | `slicecore-geo::safety_offset()` |
| **Scarf Joint Seam** | Gradient flow/speed transition at seam point, spreading the seam over a slope instead of a sharp join (OrcaSlicer innovation, 12 parameters) | `slicecore-pathing::scarf_seam` |
| **Seam** | The point where each perimeter loop starts and ends — visible as a line on the surface | `SeamPlacer` in `slicecore-pathing` |
| **Sequential Printing** | Printing objects one-at-a-time (complete one before starting next) | `slicecore-engine::sequential` |
| **Skirt** | Lines printed around the model on the first layer (primes the nozzle) | `slicecore-gcode-gen::skirt()` |
| **Slicer** | Software that converts a 3D model into G-code instructions | LibSlic3r-RS is the core engine |
| **Spiral Vase** | Mode where the Z-axis rises continuously (single-wall, no seam) | `slicecore-gcode-gen::spiral_vase` |
| **STL** | Stereolithography file format (triangulated surface mesh) | `slicecore-fileio::stl` |
| **Support** | Temporary structures printed to support overhangs, removed after printing | `slicecore-supports` crate |

## T

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **3MF** | 3D Manufacturing Format — modern replacement for STL with richer metadata | `lib3mf-core` crate |
| **TBB** | Intel Threading Building Blocks — the parallelism library used in C++ LibSlic3r (47+ parallel_for sites). Maps to `rayon` in Rust | Replaced by `rayon` |
| **Thin Wall** | Wall thinner than 2× nozzle diameter, requiring special handling | `DetectedFeatures::thin_walls` |
| **Toolpath** | The planned path the nozzle follows, including extrusions and travels | `LayerToolpath` |
| **Travel Move** | Non-extruding nozzle movement between features | `TravelMove` |
| **Tree Support** | Organic branching support structures that minimize surface contact | `slicecore-supports::tree` |
| **TPMS** | Triply Periodic Minimal Surface — mathematically defined 3D patterns (Gyroid, Diamond, Fischer-Koch) used for infill | `slicecore-infill::patterns::tpms` |

## V-W

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **Vase Mode** | See Spiral Vase | — |
| **Voronoi Diagram** | Partitioning of a plane into regions based on distance to seed points | `slicecore-geo::voronoi()` |
| **Wall** | Synonym for perimeter — the vertical shells of a print | `config.perimeters.wall_count` |
| **Wipe** | Nozzle movement along the previous extrusion before retraction to reduce blobs | `config.retraction.wipe` |
| **Wipe Tower** | See Purge Tower | — |

## Z

| Term | Definition | Code Equivalent |
|------|-----------|-----------------|
| **Z-hop** | Lifting the nozzle during travel moves to avoid hitting printed parts | `config.retraction.z_hop` |
| **Z-seam** | The vertical line where perimeter seams align across layers | `config.perimeters.seam_position` |

---

## Units Reference

| Unit | Meaning | Context |
|------|---------|---------|
| mm | Millimeters | All dimensional measurements |
| mm/s | Millimeters per second | Print speed, travel speed |
| mm/s² | Millimeters per second squared | Acceleration |
| mm³/s | Cubic millimeters per second | Volumetric flow rate |
| °C | Degrees Celsius | Nozzle and bed temperature |
| % | Percentage | Infill density, fan speed, flow rate multiplier |
| g | Grams | Filament weight |
| m | Meters | Filament length |
| g/cm³ | Grams per cubic centimeter | Filament density |

---