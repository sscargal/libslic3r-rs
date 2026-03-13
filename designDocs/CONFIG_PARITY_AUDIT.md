# Config Parity Audit: libslic3r-rs vs OrcaSlicer/PrusaSlicer

**Date:** 2026-03-13
**Scope:** PrintConfig and all sub-structs vs OrcaSlicer 2.x / BambuStudio / PrusaSlicer 2.8+

---

## Section 1: Executive Summary

| Metric | Count |
|--------|-------|
| Total libslic3r-rs typed fields (across all sub-structs) | ~258 |
| Fields with upstream JSON mapping (profile_import.rs) | ~150 unique upstream keys mapped |
| Estimated OrcaSlicer full config fields (process+filament+machine) | ~400+ |
| Current typed coverage vs OrcaSlicer | ~60-65% |
| Fields going to passthrough (unmapped catch-all) | All unrecognized keys |
| Missing P0 (critical for print quality) | ~15 fields |
| Missing P1 (important for profile fidelity) | ~30 fields |
| Missing P2 (nice-to-have / niche) | ~20 fields |

**Summary:** libslic3r-rs has strong coverage of core slicing parameters (layer heights,
speeds, temperatures, retraction, acceleration, line widths, support, ironing, scarf joint).
The primary gaps are in dimensional compensation (XY hole/contour), chamber temperature,
advanced fan curves, brim/skirt refinements, fuzzy skin, and several filament-specific
overrides that OrcaSlicer profiles commonly set.

---

## Section 2: Current Field Inventory

### 2.1 PrintConfig (top-level)

| # | Field | Type | Upstream Mapped | OrcaSlicer Key |
|---|-------|------|:---:|----------------|
| 1 | layer_height | f64 | Yes | layer_height |
| 2 | first_layer_height | f64 | Yes | initial_layer_print_height |
| 3 | wall_count | u32 | Yes | wall_loops |
| 4 | wall_order | WallOrder | -- | -- |
| 5 | seam_position | SeamPosition | Yes | seam_position |
| 6 | infill_pattern | InfillPattern | Yes | sparse_infill_pattern |
| 7 | infill_density | f64 | Yes | sparse_infill_density |
| 8 | top_solid_layers | u32 | Yes | top_shell_layers |
| 9 | bottom_solid_layers | u32 | Yes | bottom_shell_layers |
| 10 | skirt_loops | u32 | Yes | skirt_loops |
| 11 | skirt_distance | f64 | Yes | skirt_distance |
| 12 | brim_width | f64 | Yes | brim_width |
| 13 | extrusion_multiplier | f64 | Yes | filament_flow_ratio |
| 14 | adaptive_layer_height | bool | Yes | adaptive_layer_height |
| 15 | adaptive_min_layer_height | f64 | -- | -- |
| 16 | adaptive_max_layer_height | f64 | -- | -- |
| 17 | adaptive_layer_quality | f64 | -- | -- |
| 18 | gap_fill_enabled | bool | -- | -- |
| 19 | gap_fill_min_width | f64 | -- | -- |
| 20 | polyhole_enabled | bool | -- | -- |
| 21 | polyhole_min_diameter | f64 | -- | -- |
| 22 | arachne_enabled | bool | Yes | wall_generator |
| 23 | gcode_dialect | GcodeDialect | Yes | gcode_flavor |
| 24 | arc_fitting_enabled | bool | Yes | enable_arc_fitting |
| 25 | arc_fitting_tolerance | f64 | -- | -- |
| 26 | arc_fitting_min_points | usize | -- | -- |
| 27 | pressure_advance | f64 | -- | -- |
| 28 | acceleration_enabled | bool | -- | -- |
| 29 | plugin_dir | Option<String> | -- | -- |
| 30 | bridge_flow | f64 | Yes | bridge_flow / bridge_flow_ratio |
| 31 | elefant_foot_compensation | f64 | Yes | elefant_foot_compensation |
| 32 | infill_direction | f64 | Yes | infill_direction |
| 33 | infill_wall_overlap | f64 | Yes | infill_wall_overlap / infill_overlap |
| 34 | spiral_mode | bool | Yes | spiral_mode / spiral_vase |
| 35 | only_one_wall_top | bool | Yes | only_one_wall_top |
| 36 | resolution | f64 | Yes | resolution |
| 37 | raft_layers | u32 | Yes | raft_layers |
| 38 | detect_thin_wall | bool | Yes | detect_thin_wall / thin_walls |
| 39 | parallel_slicing | bool | -- | -- |
| 40 | thread_count | Option<usize> | -- | -- |
| 41 | thumbnail_resolution | [u32; 2] | -- | -- |
| 42 | passthrough | BTreeMap<String,String> | -- | (catch-all) |

**Sub-struct fields follow.**

### 2.2 LineWidthConfig (7 fields)

| # | Field | Type | Upstream Mapped | OrcaSlicer Key |
|---|-------|------|:---:|----------------|
| 1 | outer_wall | f64 | Yes | outer_wall_line_width |
| 2 | inner_wall | f64 | Yes | inner_wall_line_width |
| 3 | infill | f64 | Yes | sparse_infill_line_width |
| 4 | top_surface | f64 | Yes | top_surface_line_width |
| 5 | initial_layer | f64 | Yes | initial_layer_line_width |
| 6 | internal_solid_infill | f64 | Yes | internal_solid_infill_line_width |
| 7 | support | f64 | Yes | support_line_width |

### 2.3 SpeedConfig (19 fields)

| # | Field | Type | Upstream Mapped | OrcaSlicer Key |
|---|-------|------|:---:|----------------|
| 1 | perimeter | f64 | Yes | outer_wall_speed |
| 2 | infill | f64 | Yes | sparse_infill_speed |
| 3 | travel | f64 | Yes | travel_speed |
| 4 | first_layer | f64 | Yes | initial_layer_speed |
| 5 | bridge | f64 | Yes | bridge_speed |
| 6 | inner_wall | f64 | Yes | inner_wall_speed |
| 7 | gap_fill | f64 | Yes | gap_infill_speed |
| 8 | top_surface | f64 | Yes | top_surface_speed |
| 9 | internal_solid_infill | f64 | Yes | internal_solid_infill_speed |
| 10 | initial_layer_infill | f64 | Yes | initial_layer_infill_speed |
| 11 | support | f64 | Yes | support_speed |
| 12 | support_interface | f64 | Yes | support_interface_speed |
| 13 | small_perimeter | f64 | Yes | small_perimeter_speed |
| 14 | solid_infill | f64 | Yes | solid_infill_speed |
| 15 | overhang_1_4 | f64 | Yes | overhang_1_4_speed |
| 16 | overhang_2_4 | f64 | Yes | overhang_2_4_speed |
| 17 | overhang_3_4 | f64 | Yes | overhang_3_4_speed |
| 18 | overhang_4_4 | f64 | Yes | overhang_4_4_speed |
| 19 | travel_z | f64 | Yes | travel_speed_z |

### 2.4 CoolingConfig (11 fields)

| # | Field | Type | Upstream Mapped | OrcaSlicer Key |
|---|-------|------|:---:|----------------|
| 1 | fan_speed | u8 | -- | -- |
| 2 | fan_below_layer_time | f64 | Yes | fan_cooling_layer_time |
| 3 | disable_fan_first_layers | u32 | Yes | close_fan_the_first_x_layers |
| 4 | fan_max_speed | f64 | Yes | fan_max_speed |
| 5 | fan_min_speed | f64 | Yes | fan_min_speed |
| 6 | slow_down_layer_time | f64 | Yes | slow_down_layer_time |
| 7 | slow_down_min_speed | f64 | Yes | slow_down_min_speed / min_print_speed |
| 8 | overhang_fan_speed | f64 | Yes | overhang_fan_speed |
| 9 | overhang_fan_threshold | f64 | Yes | overhang_fan_threshold |
| 10 | full_fan_speed_layer | u32 | Yes | full_fan_speed_layer |
| 11 | slow_down_for_layer_cooling | bool | Yes | slow_down_for_layer_cooling |

### 2.5 RetractionConfig (9 fields)

| # | Field | Type | Upstream Mapped | OrcaSlicer Key |
|---|-------|------|:---:|----------------|
| 1 | length | f64 | Yes | retraction_length |
| 2 | speed | f64 | Yes | retraction_speed |
| 3 | z_hop | f64 | Yes | z_hop |
| 4 | min_travel | f64 | Yes | retraction_minimum_travel |
| 5 | deretraction_speed | f64 | Yes | deretraction_speed |
| 6 | retract_before_wipe | f64 | Yes | retract_before_wipe |
| 7 | retract_when_changing_layer | bool | Yes | retract_when_changing_layer |
| 8 | wipe | bool | Yes | wipe |
| 9 | wipe_distance | f64 | Yes | wipe_distance |

### 2.6 MachineConfig (32 fields)

| # | Field | Type | Upstream Mapped | OrcaSlicer Key |
|---|-------|------|:---:|----------------|
| 1 | bed_x | f64 | -- | (derived from bed_shape) |
| 2 | bed_y | f64 | -- | (derived from bed_shape) |
| 3 | printable_height | f64 | Yes | printable_height / max_print_height |
| 4 | max_acceleration_x | f64 | Yes | machine_max_acceleration_x |
| 5 | max_acceleration_y | f64 | Yes | machine_max_acceleration_y |
| 6 | max_acceleration_z | f64 | Yes | machine_max_acceleration_z |
| 7 | max_acceleration_e | f64 | Yes | machine_max_acceleration_e |
| 8 | max_acceleration_extruding | f64 | Yes | machine_max_acceleration_extruding |
| 9 | max_acceleration_retracting | f64 | Yes | machine_max_acceleration_retracting |
| 10 | max_acceleration_travel | f64 | Yes | machine_max_acceleration_travel |
| 11 | max_speed_x | f64 | Yes | machine_max_speed_x |
| 12 | max_speed_y | f64 | Yes | machine_max_speed_y |
| 13 | max_speed_z | f64 | Yes | machine_max_speed_z |
| 14 | max_speed_e | f64 | Yes | machine_max_speed_e |
| 15 | nozzle_diameters | Vec<f64> | Yes | nozzle_diameter |
| 16 | jerk_values_x | Vec<f64> | Yes | machine_max_jerk_x |
| 17 | jerk_values_y | Vec<f64> | Yes | machine_max_jerk_y |
| 18 | jerk_values_z | Vec<f64> | Yes | machine_max_jerk_z |
| 19 | jerk_values_e | Vec<f64> | Yes | machine_max_jerk_e |
| 20 | start_gcode | String | Yes | machine_start_gcode |
| 21 | end_gcode | String | Yes | machine_end_gcode |
| 22 | layer_change_gcode | String | Yes | layer_change_gcode |
| 23 | nozzle_type | String | Yes | nozzle_type |
| 24 | printer_model | String | Yes | printer_model |
| 25 | bed_shape | String | Yes | bed_shape / printable_area |
| 26 | min_layer_height | f64 | Yes | min_layer_height |
| 27 | max_layer_height | f64 | Yes | max_layer_height |
| 28 | extruder_count | u32 | -- | -- |

### 2.7 AccelerationConfig (9 fields)

| # | Field | Type | Upstream Mapped | OrcaSlicer Key |
|---|-------|------|:---:|----------------|
| 1 | print | f64 | Yes | default_acceleration |
| 2 | travel | f64 | Yes | travel_acceleration |
| 3 | outer_wall | f64 | Yes | outer_wall_acceleration |
| 4 | inner_wall | f64 | Yes | inner_wall_acceleration |
| 5 | initial_layer | f64 | Yes | initial_layer_acceleration |
| 6 | initial_layer_travel | f64 | Yes | initial_layer_travel_acceleration |
| 7 | top_surface | f64 | Yes | top_surface_acceleration |
| 8 | sparse_infill | f64 | Yes | sparse_infill_acceleration |
| 9 | bridge | f64 | Yes | bridge_acceleration |

### 2.8 FilamentPropsConfig (16 fields)

| # | Field | Type | Upstream Mapped | OrcaSlicer Key |
|---|-------|------|:---:|----------------|
| 1 | diameter | f64 | Yes | filament_diameter |
| 2 | density | f64 | Yes | filament_density |
| 3 | cost_per_kg | f64 | Yes | filament_cost |
| 4 | filament_type | String | Yes | filament_type |
| 5 | filament_vendor | String | Yes | filament_vendor |
| 6 | max_volumetric_speed | f64 | Yes | filament_max_volumetric_speed |
| 7 | nozzle_temperature_range_low | f64 | Yes | nozzle_temperature_range_low |
| 8 | nozzle_temperature_range_high | f64 | Yes | nozzle_temperature_range_high |
| 9 | nozzle_temperatures | Vec<f64> | Yes | nozzle_temperature |
| 10 | bed_temperatures | Vec<f64> | Yes | hot_plate_temp / bed_temperature |
| 11 | first_layer_nozzle_temperatures | Vec<f64> | Yes | nozzle_temperature_initial_layer |
| 12 | first_layer_bed_temperatures | Vec<f64> | Yes | bed_temperature_initial_layer |
| 13 | filament_retraction_length | Option<f64> | Yes | filament_retraction_length |
| 14 | filament_retraction_speed | Option<f64> | Yes | filament_retraction_speed |
| 15 | filament_start_gcode | String | Yes | filament_start_gcode |
| 16 | filament_end_gcode | String | Yes | filament_end_gcode |

### 2.9 SupportConfig (16 fields) + BridgeConfig (5) + TreeSupportConfig (7)

| # | Field | Type | Upstream Mapped | OrcaSlicer Key |
|---|-------|------|:---:|----------------|
| 1 | enabled | bool | -- | enable_support |
| 2 | support_type | SupportType | -- | support_type |
| 3 | overhang_angle | f64 | -- | support_threshold_angle |
| 4 | min_support_area | f64 | -- | -- |
| 5 | support_density | f64 | -- | support_base_pattern_spacing (derived) |
| 6 | support_pattern | SupportPattern | -- | support_base_pattern |
| 7 | interface_layers | u32 | -- | support_interface_top_layers |
| 8 | interface_density | f64 | -- | support_interface_spacing (derived) |
| 9 | interface_pattern | InterfacePattern | -- | support_interface_pattern |
| 10 | z_gap | f64 | -- | support_top_z_distance |
| 11 | xy_gap | f64 | -- | support_object_xy_distance |
| 12 | build_plate_only | bool | -- | support_on_build_plate_only |
| 13 | bridge_detection | bool | -- | -- |
| 14 | bridge.speed | f64 | -- | -- |
| 15 | bridge.fan_speed | u8 | -- | -- |
| 16 | bridge.flow_ratio | f64 | -- | -- |
| 17 | bridge.acceleration | f64 | -- | -- |
| 18 | bridge.line_width_ratio | f64 | -- | -- |
| 19 | tree.branch_style | TreeBranchStyle | -- | -- |
| 20 | tree.taper_method | TaperMethod | -- | -- |
| 21 | tree.branch_angle | f64 | -- | tree_support_branch_angle |
| 22 | tree.min_branch_angle | f64 | -- | -- |
| 23 | tree.max_trunk_diameter | f64 | -- | tree_support_branch_diameter |
| 24 | tree.merge_distance_factor | f64 | -- | -- |
| 25 | tree.tip_diameter | f64 | -- | tree_support_tip_diameter |
| 26 | quality_preset | Option<QualityPreset> | -- | -- |
| 27 | conflict_resolution | ConflictResolution | -- | -- |

**Note:** Support fields are NOT yet mapped from upstream JSON in profile_import.rs.
They use our internal defaults. This is a known gap -- support profile import was deferred
since support config has different parameter models between slicers.

### 2.10 IroningConfig (5 fields)

| # | Field | Type | Upstream Mapped | OrcaSlicer Key |
|---|-------|------|:---:|----------------|
| 1 | enabled | bool | Yes | ironing_type |
| 2 | flow_rate | f64 | Yes | ironing_flow |
| 3 | speed | f64 | Yes | ironing_speed |
| 4 | spacing | f64 | Yes | ironing_spacing |
| 5 | angle | f64 | -- | ironing_angle |

### 2.11 ScarfJointConfig (13 fields)

| # | Field | Type | Upstream Mapped | OrcaSlicer Key |
|---|-------|------|:---:|----------------|
| 1 | enabled | bool | -- | seam_slope_type |
| 2 | scarf_joint_type | ScarfJointType | -- | seam_slope_conditional |
| 3 | conditional_scarf | bool | -- | seam_slope_conditional |
| 4 | scarf_speed | f64 | -- | -- |
| 5 | scarf_start_height | f64 | -- | seam_slope_start_height |
| 6 | scarf_around_entire_wall | bool | -- | seam_slope_entire_loop |
| 7 | scarf_length | f64 | -- | seam_slope_min_length |
| 8 | scarf_steps | u32 | -- | seam_slope_steps |
| 9 | scarf_flow_ratio | f64 | -- | -- |
| 10 | scarf_inner_walls | bool | -- | seam_slope_inner_walls |
| 11 | role_based_wipe_speed | bool | -- | -- |
| 12 | wipe_speed | f64 | -- | -- |
| 13 | wipe_on_loop | bool | -- | wipe_on_loops |

### 2.12 MultiMaterialConfig (7 fields)

| # | Field | Type | Upstream Mapped | OrcaSlicer Key |
|---|-------|------|:---:|----------------|
| 1 | enabled | bool | -- | -- |
| 2 | tool_count | u8 | -- | -- |
| 3 | tools | Vec<ToolConfig> | -- | -- |
| 4 | purge_tower_position | [f64; 2] | -- | wipe_tower_x / wipe_tower_y |
| 5 | purge_tower_width | f64 | -- | prime_tower_width |
| 6 | purge_volume | f64 | -- | prime_volume |
| 7 | wipe_length | f64 | -- | -- |

### 2.13 SequentialConfig (6 fields)

| # | Field | Type | Upstream Mapped | OrcaSlicer Key |
|---|-------|------|:---:|----------------|
| 1 | enabled | bool | -- | print_sequence |
| 2 | extruder_clearance_radius | f64 | Yes | extruder_clearance_radius |
| 3 | extruder_clearance_height | f64 | Yes | extruder_clearance_height_to_rod |
| 4 | gantry_width | f64 | Yes | gantry_width |
| 5 | gantry_depth | f64 | -- | -- |
| 6 | extruder_clearance_polygon | Vec<(f64,f64)> | -- | -- |

### 2.14 PerFeatureFlow (13 fields)

| # | Field | Type | Upstream Mapped | OrcaSlicer Key |
|---|-------|------|:---:|----------------|
| 1 | outer_perimeter | f64 | -- | -- |
| 2 | inner_perimeter | f64 | -- | -- |
| 3 | solid_infill | f64 | -- | -- |
| 4 | sparse_infill | f64 | -- | -- |
| 5 | support | f64 | -- | -- |
| 6 | support_interface | f64 | -- | -- |
| 7 | bridge | f64 | -- | -- |
| 8 | gap_fill | f64 | -- | -- |
| 9 | skirt | f64 | -- | -- |
| 10 | brim | f64 | -- | -- |
| 11 | variable_width_perimeter | f64 | -- | -- |
| 12 | ironing | f64 | -- | -- |
| 13 | purge_tower | f64 | -- | -- |

### 2.15 CustomGcodeHooks (5 fields)

| # | Field | Type | Upstream Mapped | OrcaSlicer Key |
|---|-------|------|:---:|----------------|
| 1 | before_layer_change | String | -- | before_layer_change_gcode |
| 2 | after_layer_change | String | -- | -- |
| 3 | tool_change_gcode | String | -- | change_filament_gcode |
| 4 | before_every_layer | String | -- | -- |
| 5 | custom_gcode_per_z | Vec<(f64, String)> | -- | -- |

### 2.16 PostProcessConfig (7 fields) + TimelapseConfig (6) + FanOverrideRule (3) + CustomGcodeRule (2)

| # | Field | Type | Upstream Mapped | OrcaSlicer Key |
|---|-------|------|:---:|----------------|
| 1 | enabled | bool | -- | -- |
| 2 | pause_at_layers | Vec<usize> | -- | -- |
| 3 | pause_command | String | -- | -- |
| 4 | timelapse.enabled | bool | -- | timelapse_type |
| 5 | timelapse.park_x | f64 | -- | -- |
| 6 | timelapse.park_y | f64 | -- | -- |
| 7 | timelapse.dwell_ms | u32 | -- | -- |
| 8 | timelapse.retract_distance | f64 | -- | -- |
| 9 | timelapse.retract_speed | f64 | -- | -- |
| 10 | fan_overrides | Vec<FanOverrideRule> | -- | -- |
| 11 | custom_gcode | Vec<CustomGcodeRule> | -- | -- |
| 12 | plugin_order | Vec<String> | -- | -- |

### 2.17 PaCalibrationConfig (13 fields)

Standalone calibration config, not part of PrintConfig. Not relevant to parity audit.

### 2.18 SettingOverrides (7 fields)

Per-region modifier config. Not relevant to profile import parity.

---

**Total typed fields across PrintConfig + all sub-structs: ~258**

---

## Section 3: Known Missing Fields (Gap Analysis)

Fields present in OrcaSlicer/BambuStudio/PrusaSlicer that have **no typed PrintConfig
representation** in libslic3r-rs. Derived from analysis of OrcaSlicer's config domains and
fields that currently go to passthrough.

### P0 -- Critical for Print Quality Parity

These fields directly affect print quality and are commonly set in mainstream profiles.

| # | OrcaSlicer Key | Description | Why Critical |
|---|----------------|-------------|--------------|
| 1 | `chamber_temperature` | Heated chamber temperature for enclosed printers | Required for ABS/ASA/PC printing; common in Bambu/Voron profiles |
| 2 | `xy_hole_compensation` | Inward compensation for circular holes (mm) | Dimensional accuracy for mechanical parts |
| 3 | `xy_contour_compensation` | Outward compensation for outer contours (mm) | Dimensional accuracy for mechanical parts |
| 4 | `extra_perimeters_on_overhangs` | Add extra walls on overhang regions | Print quality on overhangs without support |
| 5 | `top_surface_pattern` | Fill pattern for top surfaces (monotonic, etc.) | Surface finish quality -- separate from sparse infill |
| 6 | `bottom_surface_pattern` | Fill pattern for bottom surfaces | Surface finish quality on bottom faces |
| 7 | `internal_bridge_speed` | Speed for internal unsupported bridges | Quality of internal bridges (between infill) |
| 8 | `internal_bridge_support_enabled` | Enable support for internal bridges | Prevents sagging on internal unsupported spans |
| 9 | `filament_shrink` | Per-filament shrinkage compensation (%) | Compensates material shrinkage for accurate dimensions |
| 10 | `z_offset` | Global Z offset (mm) | First layer height fine-tuning per-printer |
| 11 | `curr_bed_type` | Bed surface type for temperature lookup | Temperature varies by bed surface (PEI, textured, etc.) |
| 12 | `min_length_factor_for_acceleration` | Minimum segment length for acceleration changes | Prevents micro-segments from triggering accel changes |
| 13 | `role_based_wipe_speed` | Wipe speed based on extrusion role | Wipe behavior differs for outer wall vs infill |
| 14 | `precise_z_height` | Precise Z positioning per layer | Prevents Z drift accumulation |
| 15 | `initial_layer_line_width` (from nozzle) | Auto-calculate initial layer width from nozzle | Common in profiles; we have the field but no auto-calc |

### P1 -- Important for Profile Fidelity

These fields are commonly present in OrcaSlicer profiles and affect imported profile accuracy.

| # | OrcaSlicer Key | Description | Category |
|---|----------------|-------------|----------|
| 1 | `accel_to_decel_enable` | Enable input shaping acceleration | Motion |
| 2 | `accel_to_decel_factor` | Accel-to-decel ratio for input shaping | Motion |
| 3 | `additional_cooling_fan_speed` | Auxiliary cooling fan speed | Cooling |
| 4 | `auxiliary_fan` | Enable auxiliary fan | Cooling |
| 5 | `precise_outer_wall` | Enable precise outer wall positioning | Quality |
| 6 | `internal_solid_infill_acceleration` | Acceleration for solid infill | Acceleration |
| 7 | `support_acceleration` | Support structure acceleration | Acceleration |
| 8 | `support_interface_acceleration` | Support interface acceleration | Acceleration |
| 9 | `retraction_distances_when_cut` | Retraction distance when cut (Bambu AMS) | Retraction |
| 10 | `long_retractions_when_cut` | Long retraction mode for Bambu AMS | Retraction |
| 11 | `filament_colour` | Filament color (hex) for preview | Filament |
| 12 | `skirt_height` | Skirt height in layers | Skirt/Brim |
| 13 | `brim_type` | Brim type (outer_only, inner_only, outer_and_inner) | Skirt/Brim |
| 14 | `brim_ears` | Enable brim ears (corners only) | Skirt/Brim |
| 15 | `brim_ears_max_angle` | Max corner angle for brim ears | Skirt/Brim |
| 16 | `min_bead_width` | Minimum bead width for Arachne | Wall gen |
| 17 | `min_feature_size` | Minimum feature size for Arachne | Wall gen |
| 18 | `draft_shield` | Enable draft shield around print | Enclosure |
| 19 | `ooze_prevention` | Enable ooze prevention on multi-tool | Multi-mat |
| 20 | `support_bottom_interface_layers` | Bottom interface layer count | Support |
| 21 | `support_interface_filament` | Filament index for support interface | Support |
| 22 | `support_filament` | Filament index for support body | Support |
| 23 | `fuzzy_skin` | Enable fuzzy skin effect | Surface |
| 24 | `fuzzy_skin_thickness` | Fuzzy skin amplitude (mm) | Surface |
| 25 | `fuzzy_skin_point_dist` | Fuzzy skin point distance (mm) | Surface |
| 26 | `enable_overhang_speed` | Master switch for overhang speed | Speed |
| 27 | `infill_combination` | Combine infill every N layers | Infill |
| 28 | `solid_infill_filament` | Filament index for solid infill | Multi-mat |
| 29 | `wall_filament` | Filament index for walls | Multi-mat |
| 30 | `infill_anchor_max` | Maximum infill anchor length (mm) | Infill |

### P2 -- Nice-to-Have / Niche

These fields serve specialized use cases or UI-only purposes.

| # | OrcaSlicer Key | Description | Category |
|---|----------------|-------------|----------|
| 1 | `timelapse_type` | Timelapse type (traditional vs smooth) | Timelapse |
| 2 | `thumbnails` | Thumbnail size array (e.g., "96x96,400x300") | Preview |
| 3 | `emit_machine_limits_to_gcode` | Write M201/M203/M204 to G-code | G-code |
| 4 | `max_travel_detour_length` | Max detour for travel optimization | Travel |
| 5 | `external_perimeter_extrusion_role` | Role assignment for outer wall | Internal |
| 6 | `slicing_tolerance` | Slicing tolerance mode (gauss/nearest) | Slice |
| 7 | `post_process` | Post-process script paths | Script |
| 8 | `silent_mode` | Enable silent/stealth mode | Machine |
| 9 | `nozzle_hrc` | Nozzle hardness rating (HRC) | Machine |
| 10 | `compatible_printers_condition_cummulative` | Cumulative condition expressions | Meta |
| 11 | `bed_custom_texture` | Custom bed texture path for preview | UI |
| 12 | `bed_custom_model` | Custom bed 3D model path for preview | UI |
| 13 | `extruder_offset` | XY offset per extruder | Multi-ext |
| 14 | `cooling_tube_length` | Cooling tube length (Bambu AMS) | Machine |
| 15 | `cooling_tube_retraction` | Cooling tube retraction (Bambu AMS) | Machine |
| 16 | `parking_pos_retraction` | Parking retraction distance (AMS) | Machine |
| 17 | `extra_loading_move` | Extra loading move distance (AMS) | Machine |
| 18 | `single_extruder_multi_material` | Multi-material via single extruder | Multi-mat |
| 19 | `wipe_tower_rotation_angle` | Purge tower rotation angle | Multi-mat |
| 20 | `inherits_group` | Profile inheritance grouping | Meta |

---

## Section 4: Mapping Coverage Statistics

| Sub-struct | Our Fields | Mapped from Upstream | Known Unmapped Upstream | Coverage |
|------------|:---------:|:--------------------:|:---------------------:|:--------:|
| PrintConfig (top-level) | 42 | 24 | ~8 | 57% |
| LineWidthConfig | 7 | 7 | 0 | 100% |
| SpeedConfig | 19 | 19 | ~3 | 100% (our fields) |
| CoolingConfig | 11 | 10 | ~3 | 91% |
| RetractionConfig | 9 | 9 | ~3 | 100% (our fields) |
| MachineConfig | 28 | 25 | ~6 | 89% |
| AccelerationConfig | 9 | 9 | ~3 | 100% (our fields) |
| FilamentPropsConfig | 16 | 16 | ~5 | 100% (our fields) |
| SupportConfig + subs | 27 | 0 | ~15 | 0% (deferred) |
| IroningConfig | 5 | 4 | ~1 | 80% |
| ScarfJointConfig | 13 | 0 | ~8 | 0% (deferred) |
| MultiMaterialConfig | 7 | 0 | ~6 | 0% (deferred) |
| SequentialConfig | 6 | 4 | ~1 | 67% |
| PerFeatureFlow | 13 | 0 | 0 | N/A (our feature) |
| CustomGcodeHooks | 5 | 0 | ~3 | 0% (deferred) |
| PostProcessConfig + subs | 12 | 0 | ~2 | 0% (deferred) |

**Overall typed field count:** ~258 fields across all sub-structs
**Overall upstream mapping coverage (of our fields):** ~127/258 = ~49%
**Upstream keys we can consume:** ~150 unique OrcaSlicer keys
**Estimated total OrcaSlicer keys:** ~400+
**Gap:** ~65 missing fields identified (P0: 15, P1: 30, P2: 20)

---

## Section 5: Recommended Phases

### Phase 30: Config Gap Closure -- P0 Fields

**Objective:** Add ~15 critical fields to PrintConfig sub-structs with upstream profile mapping.

**Scope:**
- Add fields: `chamber_temperature`, `xy_hole_compensation`, `xy_contour_compensation`,
  `extra_perimeters_on_overhangs`, `top_surface_pattern`, `bottom_surface_pattern`,
  `internal_bridge_speed`, `internal_bridge_support_enabled`, `filament_shrink`,
  `z_offset`, `curr_bed_type`, `min_length_factor_for_acceleration`, `precise_z_height`
- Add profile_import.rs mappings for each new field
- Add profile_import_ini.rs mappings for PrusaSlicer equivalents
- Update existing tests and add new field-specific tests

**Estimated effort:** 3-4 plans, 1 wave
- Plan 1: Add P0 fields to config structs with defaults
- Plan 2: Add upstream JSON mappings in profile_import.rs
- Plan 3: Add PrusaSlicer INI mappings in profile_import_ini.rs
- Plan 4: Integration tests with real profile snippets

### Phase 31: Config Gap Closure -- P1 Fields

**Objective:** Add ~30 fields for profile fidelity (input shaping, advanced fan, brim, fuzzy skin).

**Scope:**
- Input shaping: `accel_to_decel_enable`, `accel_to_decel_factor`
- Advanced fan: `additional_cooling_fan_speed`, `auxiliary_fan`
- Wall: `precise_outer_wall`, `min_bead_width`, `min_feature_size`
- Acceleration: `internal_solid_infill_acceleration`, `support_acceleration`,
  `support_interface_acceleration`
- Retraction: `retraction_distances_when_cut`, `long_retractions_when_cut`
- Brim: `skirt_height`, `brim_type`, `brim_ears`, `brim_ears_max_angle`
- Surface: `fuzzy_skin`, `fuzzy_skin_thickness`, `fuzzy_skin_point_dist`
- Support: `support_bottom_interface_layers`, `support_interface_filament`, `support_filament`
- Multi-mat: `solid_infill_filament`, `wall_filament`, `ooze_prevention`
- Misc: `draft_shield`, `enable_overhang_speed`, `infill_combination`, `infill_anchor_max`

**Estimated effort:** 4-5 plans, 1-2 waves
- Plan 1: Add acceleration and motion fields
- Plan 2: Add brim/skirt and surface fields (fuzzy skin)
- Plan 3: Add support and multi-material fields
- Plan 4: Add upstream JSON mappings for all new fields
- Plan 5: Integration tests and profile round-trip verification

### Phase 32: Support Config Profile Import

**Objective:** Map existing SupportConfig fields from upstream JSON profiles.

**Scope:**
- Map all 16 SupportConfig fields from OrcaSlicer keys
- Map tree support parameters
- Map scarf joint parameters from OrcaSlicer seam_slope keys
- Map multi-material config from OrcaSlicer wipe_tower keys
- Map CustomGcodeHooks from upstream gcode fields

**Estimated effort:** 2-3 plans, 1 wave
- Plan 1: Support, scarf joint, and multi-material mapping
- Plan 2: Custom gcode hooks and post-process mapping
- Plan 3: Integration tests with full OrcaSlicer profiles

### Phase 33: ConfigSchema System

**Objective:** Build SettingDefinition metadata system per PRD Section 7.

**Scope:**
- Per-field metadata: display_name, description, tier (0-4), category, value_type,
  default, constraints, affects/affected_by, units, tags
- Implementation via proc-macro derive or build-script code generation
- Runtime registry: `HashMap<SettingKey, SettingDefinition>`
- Outputs: JSON Schema, auto-generated docs, validation layer, UI form generation data
- Progressive disclosure tiers (0=AI auto, 1=simple ~15, 2=intermediate ~60,
  3=advanced ~200, 4=developer all)

**Estimated effort:** 5-6 plans, 2-3 waves

**Wave 1: Foundation**
- Plan 1: Core schema types (`SettingDefinition`, `SettingKey`, `ValueType`, `Tier`, `Category`)
- Plan 2: Derive macro `#[derive(SettingSchema)]` with `#[setting(...)]` attributes

**Wave 2: Application**
- Plan 3: Apply derive macro to all PrintConfig fields (258+ fields)
- Plan 4: Apply to all sub-struct fields with category grouping

**Wave 3: Output Generation**
- Plan 5: JSON Schema generation from registry
- Plan 6: Validation integration (setting constraints, dependency graph)

---

## Section 6: ConfigSchema System Design Notes

Reference: PRD Section 7 -- SettingDefinition Schema

### Proc-Macro Approach

```rust
#[derive(Debug, Clone, Serialize, Deserialize, SettingSchema)]
#[serde(default)]
pub struct SpeedConfig {
    /// Perimeter print speed (mm/s).
    #[setting(
        tier = 1,
        category = "speed",
        units = "mm/s",
        min = 1.0,
        max = 1000.0,
        affects = ["quality", "print_time"],
        display_name = "Outer Wall Speed"
    )]
    pub perimeter: f64,
    // ...
}
```

The `#[derive(SettingSchema)]` macro would:
1. Parse `#[setting(...)]` attributes on each field
2. Extract doc comments as `description`
3. Generate a `fn setting_definitions() -> Vec<SettingDefinition>` method
4. Register definitions in a global `SettingRegistry`

### Runtime Registry

```rust
pub struct SettingRegistry {
    definitions: HashMap<SettingKey, SettingDefinition>,
    categories: HashMap<String, Vec<SettingKey>>,
    tiers: HashMap<Tier, Vec<SettingKey>>,
}

impl SettingRegistry {
    /// Get all settings at or below the specified tier.
    pub fn settings_for_tier(&self, tier: Tier) -> Vec<&SettingDefinition>;

    /// Get all settings in a category.
    pub fn settings_in_category(&self, category: &str) -> Vec<&SettingDefinition>;

    /// Validate a config value against its definition constraints.
    pub fn validate(&self, key: &SettingKey, value: &str) -> Result<(), ValidationError>;

    /// Generate JSON Schema for all settings.
    pub fn to_json_schema(&self) -> serde_json::Value;
}
```

### Progressive Disclosure Tiers

| Tier | Name | Approx. Fields | Target User |
|------|------|:--------------:|-------------|
| 0 | AI Auto | 0 (all hidden) | AI suggests everything |
| 1 | Simple | ~15 | Beginners: layer height, infill, speed |
| 2 | Intermediate | ~60 | Hobbyists: temps, retraction, support |
| 3 | Advanced | ~200 | Experts: acceleration, fan curves, scarf |
| 4 | Developer | All (~400+) | Developers: all fields including internal |

### Output Formats

- **JSON Schema**: For external tool validation and IDE autocomplete
- **UI Form Metadata**: Category grouping, display names, constraints for form generation
- **Auto-generated Documentation**: Per-field docs with units, ranges, and descriptions
- **Setting Search**: Full-text search across display names, descriptions, and tags
- **Dependency Graph**: Which settings affect which other settings (for smart defaults)

---

## Section 7: Priority Matrix

| Phase | Name | Fields | Plans | Effort | Impact | Depends On |
|-------|------|:------:|:-----:|:------:|:------:|:----------:|
| 30 | Config Gap Closure P0 | ~15 | 3-4 | Medium | **High** -- print quality parity | None |
| 31 | Config Gap Closure P1 | ~30 | 4-5 | Medium-High | **Medium** -- profile fidelity | Phase 30 |
| 32 | Support Config Import | ~40 mappings | 2-3 | Medium | **Medium** -- full profile round-trip | None |
| 33 | ConfigSchema System | ~258+ | 5-6 | **High** | **High** -- UI, validation, AI tier | Phase 30, 31 |

### Recommended Execution Order

1. **Phase 30** (P0 fields) -- Highest impact per effort; unblocks profile accuracy
2. **Phase 32** (Support import) -- Independent; can run parallel with Phase 30
3. **Phase 31** (P1 fields) -- After Phase 30; many fields depend on P0 patterns
4. **Phase 33** (ConfigSchema) -- After Phase 31; needs most fields in place first

### Total Estimated Effort

- Phase 30: ~4 plans = ~1 session
- Phase 31: ~5 plans = ~1-2 sessions
- Phase 32: ~3 plans = ~1 session
- Phase 33: ~6 plans = ~2-3 sessions
- **Total: ~18 plans, ~5-7 sessions**

---

## Appendix A: Upstream Key Reference

Complete list of OrcaSlicer upstream keys that map to our fields (from profile_import.rs
`apply_field_mapping` + `apply_array_field_mapping`):

**Process keys (43):** layer_height, initial_layer_print_height, wall_loops,
sparse_infill_density, sparse_infill_pattern, top_shell_layers, bottom_shell_layers,
outer_wall_speed, sparse_infill_speed, travel_speed, initial_layer_speed, skirt_loops,
skirt_distance, brim_width, default_acceleration, travel_acceleration, enable_arc_fitting,
adaptive_layer_height, wall_generator, seam_position, bridge_speed, inner_wall_speed,
gap_infill_speed, top_surface_speed, internal_solid_infill_speed,
initial_layer_infill_speed, support_speed, support_interface_speed, small_perimeter_speed,
solid_infill_speed, overhang_1_4_speed, overhang_2_4_speed, overhang_3_4_speed,
overhang_4_4_speed, travel_speed_z, ironing_type, ironing_flow, ironing_speed,
ironing_spacing, bridge_flow, elefant_foot_compensation, infill_direction, resolution

**Line width keys (8):** line_width, outer_wall_line_width, inner_wall_line_width,
sparse_infill_line_width, top_surface_line_width, initial_layer_line_width,
internal_solid_infill_line_width, support_line_width

**Cooling keys (9):** fan_max_speed, fan_min_speed, slow_down_layer_time,
slow_down_min_speed, overhang_fan_speed, overhang_fan_threshold, full_fan_speed_layer,
slow_down_for_layer_cooling, close_fan_the_first_x_layers

**Retraction keys (7):** retraction_length, retraction_speed, z_hop,
retraction_minimum_travel, deretraction_speed, retract_before_wipe, wipe_distance

**Machine keys (20):** nozzle_diameter, machine_start_gcode, machine_end_gcode,
layer_change_gcode, printable_height, machine_max_acceleration_x/y/z/e,
machine_max_acceleration_extruding, machine_max_acceleration_retracting,
machine_max_acceleration_travel, machine_max_speed_x/y/z/e,
machine_max_jerk_x/y/z/e, nozzle_type, printer_model

**Acceleration keys (7):** outer_wall_acceleration, inner_wall_acceleration,
initial_layer_acceleration, initial_layer_travel_acceleration,
top_surface_acceleration, sparse_infill_acceleration, bridge_acceleration

**Filament keys (14):** nozzle_temperature, bed_temperature, hot_plate_temp,
nozzle_temperature_initial_layer, bed_temperature_initial_layer,
hot_plate_temp_initial_layer, filament_density, filament_diameter, filament_cost,
filament_flow_ratio, filament_type, filament_vendor, filament_max_volumetric_speed,
filament_retraction_length

**Misc keys (12):** infill_wall_overlap, spiral_mode, only_one_wall_top, raft_layers,
detect_thin_wall, bed_shape, gcode_flavor, retract_when_changing_layer, wipe,
fan_cooling_layer_time, extruder_clearance_radius, extruder_clearance_height_to_rod

**Total mapped upstream keys: ~120**
