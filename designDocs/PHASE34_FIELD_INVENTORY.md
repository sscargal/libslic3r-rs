# Phase 34 Field Inventory

**Date:** 2026-03-17
**Source:** Real upstream profile scanning + codebase cross-reference
**Purpose:** Definitive "map these" field list for Phase 34 Plans 02-06

---

## Methodology

1. Extracted all currently-mapped upstream keys from `profile_import.rs` and `profile_import_ini.rs`
2. Scanned `/home/steve/slicer-analysis/OrcaSlicer/resources/profiles/` for all process/filament/machine JSON keys
3. Scanned `/home/steve/slicer-analysis/PrusaSlicer/` for all INI keys with `support_material*` prefix
4. Grepped real profiles for `seam_slope_*`, `wipe_tower_*`, `prime_*`, `*gcode*`, and `{variable}` patterns
5. Cross-referenced against our typed config fields in `config.rs`, `support/config.rs`, `custom_gcode.rs`
6. Enumerated P2 niche fields from CONFIG_PARITY_AUDIT.md Section 3

---

## Support Config Fields

Fields for `SupportConfig`, `BridgeConfig`, and `TreeSupportConfig` mapping from upstream profiles.

| # | OrcaSlicer Key | PrusaSlicer Key | Our Field Path | Status |
|---|---------------|-----------------|----------------|--------|
| 1 | `enable_support` | `support_material` | `support.enabled` | needs-mapping |
| 2 | `support_type` | `support_material_style` | `support.support_type` | needs-mapping + enum mapper |
| 3 | `support_threshold_angle` / `support_angle` | `support_material_threshold` | `support.overhang_angle` | needs-mapping |
| 4 | `support_base_pattern` | `support_material_pattern` | `support.support_pattern` | needs-mapping + enum mapper |
| 5 | `support_base_pattern_spacing` | `support_material_spacing` | `support.support_density` | needs-mapping (derived: line_width/spacing) |
| 6 | `support_interface_top_layers` | `support_material_interface_layers` | `support.interface_layers` | needs-mapping |
| 7 | `support_interface_spacing` | `support_material_interface_spacing` | `support.interface_density` | needs-mapping (derived) |
| 8 | `support_interface_pattern` | `support_material_interface_pattern` | `support.interface_pattern` | needs-mapping + enum mapper |
| 9 | `support_top_z_distance` | `support_material_contact_distance` | `support.z_gap` | needs-mapping |
| 10 | `support_object_xy_distance` | `support_material_xy_spacing` | `support.xy_gap` | needs-mapping |
| 11 | `support_on_build_plate_only` | `support_material_buildplate_only` | `support.build_plate_only` | needs-mapping |
| 12 | `support_interface_bottom_layers` | `support_material_bottom_interface_layers` | `support.support_bottom_interface_layers` | needs-mapping |
| 13 | `support_filament` | `support_material_extruder` | `multi_material.support_filament` | needs-mapping (1-based to 0-based) |
| 14 | `support_interface_filament` | `support_material_interface_extruder` | `multi_material.support_interface_filament` | needs-mapping (1-based to 0-based) |
| 15 | `support_bottom_z_distance` | `support_material_bottom_contact_distance` | `support.z_gap` (bottom) | needs-new-field OR map to z_gap |
| 16 | `support_expansion` | -- | -- | needs-new-field: `support.expansion` |
| 17 | `support_speed` | `support_material_speed` | `speeds.support` | existing-mapping |
| 18 | `support_interface_speed` | `support_material_interface_speed` | `speeds.support_interface` | existing-mapping |
| 19 | `support_line_width` | `support_material_extrusion_width` | `line_widths.support` | existing-mapping |
| 20 | `support_critical_regions_only` | -- | -- | needs-new-field: `support.critical_regions_only` |
| 21 | `support_remove_small_overhang` | -- | -- | needs-new-field: `support.remove_small_overhang` |
| 22 | `support_style` | -- | `support.support_type` | alt key for support_type |
| 23 | `support_interface_loop_pattern` | -- | -- | needs-new-field: `support.interface_loop_pattern` |
| 24 | `support_interface_not_for_body` | -- | -- | needs-new-field (bool) |
| 25 | `support_flow_ratio` | `support_material_flow` | -- | needs-new-field: `support.flow_ratio` |
| 26 | `support_interface_flow_ratio` | `support_material_interface_flow` | -- | needs-new-field: `support.interface_flow_ratio` |
| 27 | `support_material_synchronize_layers` | `support_material_synchronize_layers` | -- | needs-new-field: `support.synchronize_layers` |
| 28 | `support_threshold_overlap` | -- | -- | needs-new-field (advanced, low priority) |
| 29 | `enforce_support_layers` | `support_material_enforce_layers` | -- | needs-new-field: `support.enforce_layers` |
| 30 | `support_closing_radius` | `support_material_closing_radius` | -- | needs-new-field: `support.closing_radius` |

### Tree Support Fields

| # | OrcaSlicer Key | PrusaSlicer Key | Our Field Path | Status |
|---|---------------|-----------------|----------------|--------|
| 1 | `tree_support_branch_angle` | -- | `support.tree.branch_angle` | needs-mapping |
| 2 | `tree_support_branch_diameter` | -- | `support.tree.max_trunk_diameter` | needs-mapping |
| 3 | `tree_support_tip_diameter` | -- | `support.tree.tip_diameter` | needs-mapping |
| 4 | `tree_support_branch_distance` | -- | -- | needs-new-field: `support.tree.branch_distance` |
| 5 | `tree_support_branch_diameter_angle` | -- | -- | needs-new-field: `support.tree.branch_diameter_angle` |
| 6 | `tree_support_wall_count` | -- | -- | needs-new-field: `support.tree.wall_count` |
| 7 | `tree_support_auto_brim` | -- | -- | needs-new-field: `support.tree.auto_brim` |
| 8 | `tree_support_brim_width` | -- | -- | needs-new-field: `support.tree.brim_width` |
| 9 | `tree_support_adaptive_layer_height` | -- | -- | needs-new-field: `support.tree.adaptive_layer_height` |
| 10 | `tree_support_angle_slow` | -- | -- | needs-new-field: `support.tree.angle_slow` |
| 11 | `tree_support_top_rate` | -- | -- | needs-new-field: `support.tree.top_rate` |
| 12 | `tree_support_with_infill` | -- | -- | needs-new-field: `support.tree.with_infill` |
| 13 | `tree_support_branch_angle_organic` | -- | -- | needs-new-field |
| 14 | `tree_support_branch_diameter_organic` | -- | -- | needs-new-field |
| 15 | `tree_support_branch_distance_organic` | -- | -- | needs-new-field |
| 16 | `tree_support_branch_diameter_double_wall` | -- | -- | needs-new-field |
| 17 | `independent_support_layer_height` | -- | -- | needs-new-field: `support.independent_layer_height` |

### Bridge Config Fields

| # | OrcaSlicer Key | PrusaSlicer Key | Our Field Path | Status |
|---|---------------|-----------------|----------------|--------|
| 1 | `bridge_speed` | `bridge_speed` | `speeds.bridge` | existing-mapping |
| 2 | `bridge_flow` | `bridge_flow` / `bridge_flow_ratio` | `bridge_flow` (top-level) | existing-mapping |
| 3 | `bridge_acceleration` | `bridge_acceleration` | `accel.bridge` | existing-mapping |
| 4 | `bridge_no_support` | -- | -- | needs-new-field (bool) |
| 5 | `bridge_angle` | -- | -- | needs-new-field: `support.bridge.angle` |
| 6 | `bridge_density` | -- | -- | needs-new-field: `support.bridge.density` |
| 7 | `thick_bridges` | -- | -- | needs-new-field: `support.bridge.thick_bridges` |
| 8 | `internal_bridge_speed` | -- | `speeds.internal_bridge_speed` | existing-mapping |
| 9 | `internal_bridge_flow` | -- | -- | needs-new-field |
| 10 | `internal_bridge_density` | -- | -- | needs-new-field |

### Distinct `support_type` Values Found in Profiles

- `"normal(auto)"` -- maps to `SupportType::Traditional` (with auto-placement)
- `"normal(manual)"` -- maps to `SupportType::Traditional` (manual-only)
- `"tree(auto)"` -- maps to `SupportType::Tree`
- PrusaSlicer: `"grid"`, `"snug"`, `"organic"` (via `support_material_style`)

---

## Scarf Joint Fields

Fields for `ScarfJointConfig` mapping from OrcaSlicer `seam_slope_*` keys.

| # | OrcaSlicer Key | PrusaSlicer Key | Our Field Path | Status |
|---|---------------|-----------------|----------------|--------|
| 1 | `seam_slope_type` | -- | `scarf_joint.enabled` + type | needs-mapping |
| 2 | `seam_slope_conditional` | -- | `scarf_joint.conditional_scarf` | needs-mapping |
| 3 | `seam_slope_start_height` | -- | `scarf_joint.scarf_start_height` | needs-mapping |
| 4 | `seam_slope_entire_loop` | -- | `scarf_joint.scarf_around_entire_wall` | needs-mapping |
| 5 | `seam_slope_min_length` | -- | `scarf_joint.scarf_length` | needs-mapping |
| 6 | `seam_slope_steps` | -- | `scarf_joint.scarf_steps` | needs-mapping |
| 7 | `seam_slope_inner_walls` | -- | `scarf_joint.scarf_inner_walls` | needs-mapping |
| 8 | `seam_slope_gap` | -- | -- | needs-new-field: `scarf_joint.seam_gap` |
| 9 | `wipe_on_loops` | -- | `scarf_joint.wipe_on_loop` | needs-mapping |
| 10 | `role_based_wipe_speed` | -- | `scarf_joint.role_based_wipe_speed` | needs-mapping |
| 11 | `wipe_speed` | -- | `scarf_joint.wipe_speed` | needs-mapping |
| 12 | `scarf_joint_speed` | -- | `scarf_joint.scarf_speed` | needs-mapping |
| 13 | `scarf_joint_flow_ratio` | -- | `scarf_joint.scarf_flow_ratio` | needs-mapping |
| 14 | `scarf_angle_threshold` | -- | -- | needs-new-field |
| 15 | `scarf_overhang_threshold` | -- | -- | needs-new-field |
| 16 | `override_filament_scarf_seam_setting` | -- | -- | needs-new-field (bool) |

**Note:** PrusaSlicer does not have scarf joint equivalent fields. These are OrcaSlicer-only.

---

## Multi-Material Fields

Fields for `MultiMaterialConfig` mapping from upstream profiles.

| # | OrcaSlicer Key | PrusaSlicer Key | Our Field Path | Status |
|---|---------------|-----------------|----------------|--------|
| 1 | `wipe_tower_x` | `wipe_tower_x` | `multi_material.purge_tower_position[0]` | needs-mapping |
| 2 | `wipe_tower_y` | `wipe_tower_y` | `multi_material.purge_tower_position[1]` | needs-mapping |
| 3 | `prime_tower_width` / `wipe_tower_width` | `wipe_tower_width` | `multi_material.purge_tower_width` | needs-mapping |
| 4 | `prime_volume` | -- | `multi_material.purge_volume` | needs-mapping |
| 5 | `wipe_tower_rotation_angle` | `wipe_tower_rotation_angle` | -- | needs-new-field |
| 6 | `wipe_tower_bridging` | `wipe_tower_bridging` | -- | needs-new-field |
| 7 | `wipe_tower_cone_angle` | `wipe_tower_cone_angle` | -- | needs-new-field |
| 8 | `wipe_tower_extra_flow` | -- | -- | needs-new-field |
| 9 | `wipe_tower_extra_spacing` | -- | -- | needs-new-field |
| 10 | `wipe_tower_no_sparse_layers` | `wipe_tower_no_sparse_layers` | -- | needs-new-field (bool) |
| 11 | `wipe_tower_max_purge_speed` | -- | -- | needs-new-field |
| 12 | `wipe_tower_filament` | `wipe_tower_filament` | -- | needs-new-field |
| 13 | `wipe_tower_extruder` | `wipe_tower_extruder` | -- | needs-new-field |
| 14 | `prime_tower_brim_width` | `wipe_tower_brim_width` | -- | needs-new-field |
| 15 | `enable_prime_tower` | -- | `multi_material.enabled` | needs-mapping |
| 16 | `wall_filament` | -- | `multi_material.wall_filament` | needs-mapping (1-based to 0-based) |
| 17 | `solid_infill_filament` | -- | `multi_material.solid_infill_filament` | needs-mapping |
| 18 | `support_filament` | `support_material_extruder` | `multi_material.support_filament` | needs-mapping |
| 19 | `support_interface_filament` | `support_material_interface_extruder` | `multi_material.support_interface_filament` | needs-mapping |
| 20 | `retraction_distances_when_cut` | -- | `multi_material.tool_change_retraction.retraction_distance_when_cut` | needs-mapping |
| 21 | `long_retractions_when_cut` | -- | `multi_material.tool_change_retraction.long_retraction_when_cut` | needs-mapping |
| 22 | `single_extruder_multi_material` | `single_extruder_multi_material` | -- | needs-new-field (bool) |
| 23 | `purge_in_prime_tower` | -- | -- | needs-new-field (bool) |
| 24 | `flush_into_infill` | -- | -- | needs-new-field (bool) |
| 25 | `flush_into_objects` | -- | -- | needs-new-field (bool) |
| 26 | `flush_into_support` | -- | -- | needs-new-field (bool) |
| 27 | `wiping_volumes_extruders` | -- | -- | needs-new-field (Vec) |
| 28 | `prime_tower_enable_framework` | -- | -- | needs-new-field (bool) |
| 29 | `prime_volume_mode` | -- | -- | needs-new-field |

---

## Custom G-code Hook Fields

Fields for `CustomGcodeHooks` and `MachineConfig` G-code mapping.

| # | OrcaSlicer Key | PrusaSlicer Key | Our Field Path | Status |
|---|---------------|-----------------|----------------|--------|
| 1 | `before_layer_change_gcode` | `before_layer_gcode` | `custom_gcode.before_layer_change` | needs-mapping |
| 2 | `change_filament_gcode` | `toolchange_gcode` | `custom_gcode.tool_change_gcode` | needs-mapping |
| 3 | `machine_start_gcode` | `start_gcode` | `machine.start_gcode` | existing-mapping |
| 4 | `machine_end_gcode` | `end_gcode` | `machine.end_gcode` | existing-mapping |
| 5 | `layer_change_gcode` | `layer_gcode` | `machine.layer_change_gcode` | existing-mapping |
| 6 | `machine_pause_gcode` | -- | -- | needs-new-field |
| 7 | `between_objects_gcode` | `between_objects_gcode` | -- | needs-new-field |
| 8 | `color_change_gcode` | `color_change_gcode` | -- | needs-new-field |
| 9 | `pause_print_gcode` | -- | -- | needs-new-field |
| 10 | `change_extrusion_role_gcode` | -- | -- | needs-new-field |
| 11 | `filament_start_gcode` | `start_filament_gcode` | `filament.filament_start_gcode` | existing-mapping |
| 12 | `filament_end_gcode` | `end_filament_gcode` | `filament.filament_end_gcode` | existing-mapping |

---

## PostProcess/Timelapse Fields

Fields for `PostProcessConfig` and `TimelapseConfig` mapping.

| # | OrcaSlicer Key | PrusaSlicer Key | Our Field Path | Status |
|---|---------------|-----------------|----------------|--------|
| 1 | `timelapse_type` | -- | `post_process.timelapse.enabled` | needs-mapping (value != "none" -> enabled) |
| 2 | `post_process` | `post_process` | -- | needs-new-field: `post_process.scripts` (Vec<String>) |
| 3 | `emit_machine_limits_to_gcode` | -- | -- | needs-new-field (bool) |
| 4 | `gcode_label_objects` | -- | -- | needs-new-field (bool) |
| 5 | `gcode_comments` | -- | -- | needs-new-field (bool) |
| 6 | `gcode_add_line_number` | -- | -- | needs-new-field (bool) |
| 7 | `filename_format` | -- | -- | needs-new-field (String) |

---

## P2 Niche Fields

From CONFIG_PARITY_AUDIT.md Section 3 (lines 425-451), verified against codebase.

| # | OrcaSlicer Key | PrusaSlicer Key | Our Field Path | Typed Field Exists? | Mapping Exists? |
|---|---------------|-----------------|----------------|:---:|:---:|
| 1 | `timelapse_type` | -- | `post_process.timelapse.enabled` | Yes (bool) | No |
| 2 | `thumbnails` | `thumbnails` | `thumbnail_resolution` | Yes ([u32;2]) | No -- needs mapping (parse "96x96,400x300" format) |
| 3 | `emit_machine_limits_to_gcode` | -- | -- | No | No |
| 4 | `max_travel_detour_distance` | -- | -- | No | No |
| 5 | `external_perimeter_extrusion_role` | -- | -- | No | No -- internal OrcaSlicer detail, skip |
| 6 | `slicing_tolerance` | `slicing_tolerance` | -- | No | No |
| 7 | `post_process` | `post_process` | -- | No | No |
| 8 | `silent_mode` | `silent_mode` | -- | No | No |
| 9 | `nozzle_hrc` | -- | -- | No | No |
| 10 | `compatible_printers_condition_cummulative` | -- | -- | No | No |
| 11 | `bed_custom_texture` | `bed_custom_texture` | -- | No | No |
| 12 | `bed_custom_model` | `bed_custom_model` | -- | No | No |
| 13 | `extruder_offset` | `extruder_offset` | -- | No | No |
| 14 | `cooling_tube_length` | `cooling_tube_length` | -- | No | No |
| 15 | `cooling_tube_retraction` | `cooling_tube_retraction` | -- | No | No |
| 16 | `parking_pos_retraction` | `parking_pos_retraction` | -- | No | No |
| 17 | `extra_loading_move` | `extra_loading_move` | -- | No | No |
| 18 | `single_extruder_multi_material` | `single_extruder_multi_material` | -- | No | No |
| 19 | `wipe_tower_rotation_angle` | `wipe_tower_rotation_angle` | -- | No | No |
| 20 | `inherits_group` | -- | -- | No | No |
| 21 | `print_sequence` | -- | `sequential.enabled` | Yes | No -- needs mapping (value != "by layer") |
| 22 | `exclude_object` | -- | -- | No | No |
| 23 | `reduce_infill_retraction` | -- | -- | No | No |
| 24 | `reduce_crossing_wall` | -- | -- | No | No |

---

## Straggler Fields (from partially-mapped sections)

Fields in existing sub-structs that have upstream equivalents but lack mapping.

### IroningConfig (80% coverage -> 100%)

| # | OrcaSlicer Key | PrusaSlicer Key | Our Field Path | Status |
|---|---------------|-----------------|----------------|--------|
| 1 | `ironing_angle` | `ironing_angle` | `ironing.angle` | needs-mapping (field exists, mapping missing) |

### SequentialConfig (67% -> higher)

| # | OrcaSlicer Key | PrusaSlicer Key | Our Field Path | Status |
|---|---------------|-----------------|----------------|--------|
| 1 | `print_sequence` | -- | `sequential.enabled` | needs-mapping (`"by object"` -> true) |
| 2 | `extruder_clearance_height_to_lid` | `extruder_clearance_height` | `sequential.extruder_clearance_height` | needs-mapping (INI alternate key) |

### CoolingConfig (91% -> 100%)

| # | OrcaSlicer Key | PrusaSlicer Key | Our Field Path | Status |
|---|---------------|-----------------|----------------|--------|
| 1 | `fan_cooling_layer_time` | `fan_below_layer_time` | `cooling.fan_below_layer_time` | existing-mapping (OrcaSlicer) |

### MachineConfig (89% -> higher)

| # | OrcaSlicer Key | PrusaSlicer Key | Our Field Path | Status |
|---|---------------|-----------------|----------------|--------|
| 1 | `printable_area` / `bed_shape` | `bed_shape` | `machine.bed_shape` | existing-mapping |
| 2 | `silent_mode` | `silent_mode` | -- | needs-new-field (bool) |
| 3 | `retract_length_toolchange` | `retract_length_toolchange` | -- | needs-new-field |
| 4 | `retract_restart_extra` | `retract_restart_extra` | -- | needs-new-field |
| 5 | `retract_restart_extra_toolchange` | `retract_restart_extra_toolchange` | -- | needs-new-field |
| 6 | `z_hop_types` | -- | -- | needs-new-field (String/enum) |
| 7 | `machine_min_extruding_rate` | -- | -- | needs-new-field |
| 8 | `machine_min_travel_rate` | -- | -- | needs-new-field |

### AccelerationConfig (additional process keys found)

| # | OrcaSlicer Key | PrusaSlicer Key | Our Field Path | Status |
|---|---------------|-----------------|----------------|--------|
| 1 | `default_jerk` | -- | -- | needs-new-field |
| 2 | `outer_wall_jerk` | -- | -- | needs-new-field |
| 3 | `inner_wall_jerk` | -- | -- | needs-new-field |
| 4 | `top_surface_jerk` | -- | -- | needs-new-field |
| 5 | `infill_jerk` | -- | -- | needs-new-field |
| 6 | `travel_jerk` | -- | -- | needs-new-field |
| 7 | `initial_layer_jerk` | -- | -- | needs-new-field |

---

## G-code Template Variables

Variables found in actual G-code template fields in upstream profiles.

### OrcaSlicer Variables (from `{variable}` syntax in gcode fields)

| # | Upstream Variable | Our Variable | Status |
|---|------------------|-------------|--------|
| 1 | `{initial_layer_print_height}` | `{first_layer_height}` | needs-translation |
| 2 | `{bed_temperature_initial_layer_single}` | `{first_layer_bed_temp}` | needs-translation |
| 3 | `{curr_bed_type}` | `{bed_type}` | needs-translation |
| 4 | `{overall_chamber_temperature}` | `{chamber_temperature}` | needs-translation |
| 5 | `{initial_extruder}` | `{initial_tool}` | needs-translation (alias) |
| 6 | `{total_layer_count}` | `{total_layers}` | needs-translation |
| 7 | `{layer_num}` | `{layer_num}` | identity (already matches) |
| 8 | `{next_extruder}` | `{next_extruder}` | identity |
| 9 | `{new_filament_e_feedrate}` | -- | OrcaSlicer-specific, passthrough |
| 10 | `{old_filament_e_feedrate}` | -- | OrcaSlicer-specific, passthrough |
| 11 | `{flush_length}` | -- | OrcaSlicer-specific, passthrough |
| 12 | `{toolchange_z}` | -- | OrcaSlicer-specific, passthrough |
| 13 | `{retraction_distance_when_cut}` | -- | maps to our field |
| 14 | `{retraction_distance_when_ec}` | -- | OrcaSlicer-specific |
| 15 | `{initial_no_support_extruder}` | -- | OrcaSlicer-specific |
| 16 | `{initial_tool}` | `{initial_tool}` | identity |
| 17 | `{wipe_avoid_pos_x}` | -- | OrcaSlicer-specific |
| 18 | `{z_after_toolchange}` | -- | OrcaSlicer-specific |
| 19 | `{adaptive_bed_mesh_margin}` | -- | OrcaSlicer-specific |

### PrusaSlicer Variables (from `[variable]` syntax in gcode fields)

| # | Upstream Variable | Our Variable | Status |
|---|------------------|-------------|--------|
| 1 | `[first_layer_temperature]` | `{first_layer_nozzle_temp}` | needs-translation |
| 2 | `[first_layer_bed_temperature]` | `{first_layer_bed_temp}` | needs-translation |
| 3 | `[first_layer_height]` | `{first_layer_height}` | needs-translation (bracket -> brace) |
| 4 | `[layer_num]` | `{layer_num}` | needs-translation (bracket -> brace) |
| 5 | `[layer_z]` | `{layer_z}` | needs-translation (bracket -> brace) |
| 6 | `[chamber_temperature]` | `{chamber_temperature}` | needs-translation |
| 7 | `[current_extruder]` | `{current_tool}` | needs-translation |
| 8 | `[initial_extruder]` | `{initial_tool}` | needs-translation |
| 9 | `[initial_tool]` | `{initial_tool}` | needs-translation |
| 10 | `[nozzle_diameter]` | `{nozzle_diameter}` | needs-translation |
| 11 | `[filament_diameter]` | `{filament_diameter}` | needs-translation |
| 12 | `[layer_height]` | `{layer_height}` | needs-translation |
| 13 | `[max_layer_z]` | -- | PrusaSlicer-specific |
| 14 | `[has_wipe_tower]` | -- | PrusaSlicer-specific |
| 15 | `[default_acceleration]` | `{default_acceleration}` | needs-translation |
| 16 | `[first_layer_acceleration]` | `{initial_layer_acceleration}` | needs-translation |
| 17 | `[extrusion_width]` | `{line_width}` | needs-translation |
| 18 | `[machine_max_acceleration_*]` | `{machine_max_acceleration_*}` | needs-translation |
| 19 | `[machine_max_feedrate_*]` | `{machine_max_speed_*}` | needs-translation |
| 20 | `[machine_max_jerk_*]` | `{machine_max_jerk_*}` | needs-translation |
| 21 | `[retract_length]` | `{retraction_length}` | needs-translation |
| 22 | `[printer_model]` | `{printer_model}` | needs-translation |
| 23 | `[extruded_weight_total]` | -- | PrusaSlicer-specific |
| 24 | `[next_extruder]` | `{next_extruder}` | needs-translation |
| 25 | `[previous_extruder]` | `{previous_extruder}` | needs-translation |

---

## Passthrough Promotion Candidates

Keys that currently go to passthrough but should be promoted to typed fields, based on profile scanning.

| # | Passthrough Key | Promotion Target | Rationale |
|---|----------------|-----------------|-----------|
| 1 | `print_sequence` | `sequential.enabled` | Common in OrcaSlicer process profiles |
| 2 | `timelapse_type` | `post_process.timelapse.enabled` | Common in BambuStudio profiles |
| 3 | `enable_support` | `support.enabled` | Core support toggle |
| 4 | `support_type` | `support.support_type` | Core support type |
| 5 | All `support_*` keys | `support.*` fields | All support keys currently passthrough |
| 6 | All `seam_slope_*` keys | `scarf_joint.*` fields | All scarf keys currently passthrough |
| 7 | All `wipe_tower_*` / `prime_*` keys | `multi_material.*` | All multi-mat keys passthrough |
| 8 | `before_layer_change_gcode` | `custom_gcode.before_layer_change` | G-code hook key |
| 9 | `change_filament_gcode` | `custom_gcode.tool_change_gcode` | G-code hook key |
| 10 | `thumbnails` | `thumbnail_resolution` | Preview config |
| 11 | `post_process` | `post_process.scripts` | Script paths |
| 12 | `ironing_angle` | `ironing.angle` | Missing mapping for existing field |
| 13 | `emit_machine_limits_to_gcode` | new field | Controls M20x output |
| 14 | `gcode_label_objects` | new field | Object labeling |

---

## Summary Statistics

### Fields by Category

| Category | Total Fields | Existing Typed | Need New Field | Need Mapping Only |
|----------|:-----------:|:--------------:|:--------------:|:-----------------:|
| Support Config (body) | 30 | 18 | 12 | 18 |
| Tree Support | 17 | 7 | 10 | 3 |
| Bridge Config | 10 | 5 | 5 | 0 |
| Scarf Joint | 16 | 13 | 3 | 13 |
| Multi-Material | 29 | 12 | 17 | 6 |
| Custom G-code Hooks | 12 | 5 | 5 | 2 |
| PostProcess/Timelapse | 7 | 2 | 5 | 1 |
| P2 Niche | 24 | 2 | 22 | 2 |
| Straggler (IroningConfig) | 1 | 1 | 0 | 1 |
| Straggler (SequentialConfig) | 2 | 2 | 0 | 2 |
| Straggler (MachineConfig) | 8 | 0 | 8 | 0 |
| Straggler (AccelerationConfig/Jerk) | 7 | 0 | 7 | 0 |
| G-code Template Variables | 44 | -- | -- | 44 translations |
| **TOTAL** | **207** | **67** | **94** | **92** |

### Coverage Impact

- **Before Phase 34:** ~150 upstream keys mapped, ~120 typed fields with mappings
- **After Phase 34 (target):** ~250+ upstream keys mapped, ~190+ typed fields with mappings
- **Passthrough reduction:** From ~40% to <5% for representative profiles

### Priority Grouping for Plans 02-06

- **Plan 02 (Support Config Mapping):** Support fields #1-30, Tree fields #1-17, Bridge fields #4-10 = ~57 fields
- **Plan 03 (Scarf Joint + Multi-Material Mapping):** Scarf #1-16, Multi-Material #1-29 = ~45 fields
- **Plan 04 (G-code Hooks + PostProcess + P2):** G-code hooks #1-12, PostProcess #1-7, P2 niche = ~43 fields
- **Plan 05 (Straggler + G-code Template Translation):** Straggler fields, G-code variable table = ~62 items
- **Plan 06 (Re-conversion + Validation):** Run full sweep, coverage report, threshold test

---

*Generated from real profile scanning of OrcaSlicer, BambuStudio, and PrusaSlicer profile repositories.*
*Cross-referenced against codebase: config.rs, support/config.rs, profile_import.rs, profile_import_ini.rs, custom_gcode.rs*
