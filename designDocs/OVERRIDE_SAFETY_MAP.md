# Override Safety Map

Classification of all 374 registered settings for per-object/per-region override safety.

## Summary

| Classification | Count | Description |
|---------------|-------|-------------|
| safe | 190 | Safe to override in any context (per-object, per-region) |
| warn | 106 | Nonsensical in some override contexts but allowed (warns) |
| ignored | 78 | Has no effect as a per-region override (silently ignored) |

## Classifications

### Safe (190 fields)

Settings that make sense per-object and per-region (e.g., modifier meshes).

| Field | Safety | Reason |
|-------|--------|--------|
| accel.bridge | safe | Acceleration can differ per object/region |
| accel.default_jerk | safe | Acceleration can differ per object/region |
| accel.infill_jerk | safe | Acceleration can differ per object/region |
| accel.initial_layer | safe | Acceleration can differ per object/region |
| accel.initial_layer_jerk | safe | Acceleration can differ per object/region |
| accel.initial_layer_travel | safe | Acceleration can differ per object/region |
| accel.inner_wall | safe | Acceleration can differ per object/region |
| accel.inner_wall_jerk | safe | Acceleration can differ per object/region |
| accel.internal_solid_infill_acceleration | safe | Acceleration can differ per object/region |
| accel.min_length_factor | safe | Acceleration can differ per object/region |
| accel.outer_wall | safe | Acceleration can differ per object/region |
| accel.outer_wall_jerk | safe | Acceleration can differ per object/region |
| accel.print | safe | Acceleration can differ per object/region |
| accel.sparse_infill | safe | Acceleration can differ per object/region |
| accel.support_acceleration | safe | Acceleration can differ per object/region |
| accel.support_interface_acceleration | safe | Acceleration can differ per object/region |
| accel.top_surface | safe | Acceleration can differ per object/region |
| accel.top_surface_jerk | safe | Acceleration can differ per object/region |
| accel.travel | safe | Acceleration can differ per object/region |
| accel.travel_jerk | safe | Acceleration can differ per object/region |
| adaptive_layer_height | safe | Print quality parameter, per-object meaningful |
| adaptive_layer_quality | safe | Print quality parameter, per-object meaningful |
| adaptive_max_layer_height | safe | Print quality parameter, per-object meaningful |
| adaptive_min_layer_height | safe | Print quality parameter, per-object meaningful |
| arachne_enabled | safe | Print quality parameter, per-object meaningful |
| arc_fitting_enabled | safe | Print quality parameter, per-object meaningful |
| arc_fitting_min_points | safe | Print quality parameter, per-object meaningful |
| arc_fitting_tolerance | safe | Print quality parameter, per-object meaningful |
| bottom_solid_layers | safe | Print quality parameter, per-object meaningful |
| bottom_surface_pattern | safe | Print quality parameter, per-object meaningful |
| bridge_flow | safe | Print quality parameter, per-object meaningful |
| detect_thin_wall | safe | Print quality parameter, per-object meaningful |
| dimensional_compensation.elephant_foot_compensation | safe | Compensation can differ per object |
| dimensional_compensation.xy_contour_compensation | safe | Compensation can differ per object |
| dimensional_compensation.xy_hole_compensation | safe | Compensation can differ per object |
| extra_perimeters_on_overhangs | safe | Print quality parameter, per-object meaningful |
| extrusion_multiplier | safe | Print quality parameter, per-object meaningful |
| first_layer_height | safe | Print quality parameter, per-object meaningful |
| fuzzy_skin.enabled | safe | Fuzzy skin can differ per object/region |
| fuzzy_skin.point_distance | safe | Fuzzy skin can differ per object/region |
| fuzzy_skin.thickness | safe | Fuzzy skin can differ per object/region |
| gap_fill_enabled | safe | Print quality parameter, per-object meaningful |
| gap_fill_min_width | safe | Print quality parameter, per-object meaningful |
| infill_anchor_max | safe | Print quality parameter, per-object meaningful |
| infill_combination | safe | Print quality parameter, per-object meaningful |
| infill_density | safe | Print quality parameter, per-object meaningful |
| infill_direction | safe | Print quality parameter, per-object meaningful |
| infill_pattern | safe | Print quality parameter, per-object meaningful |
| infill_wall_overlap | safe | Print quality parameter, per-object meaningful |
| internal_bridge_support | safe | Print quality parameter, per-object meaningful |
| ironing.angle | safe | Ironing can differ per object/region |
| ironing.enabled | safe | Ironing can differ per object/region |
| ironing.flow_rate | safe | Ironing can differ per object/region |
| ironing.spacing | safe | Ironing can differ per object/region |
| ironing.speed | safe | Ironing can differ per object/region |
| layer_height | safe | Print quality parameter, per-object meaningful |
| line_widths.infill | safe | Line width can differ per object/region |
| line_widths.initial_layer | safe | Line width can differ per object/region |
| line_widths.inner_wall | safe | Line width can differ per object/region |
| line_widths.internal_solid_infill | safe | Line width can differ per object/region |
| line_widths.outer_wall | safe | Line width can differ per object/region |
| line_widths.support | safe | Line width can differ per object/region |
| line_widths.top_surface | safe | Line width can differ per object/region |
| max_travel_detour_length | safe | Print quality parameter, per-object meaningful |
| min_bead_width | safe | Print quality parameter, per-object meaningful |
| min_feature_size | safe | Print quality parameter, per-object meaningful |
| only_one_wall_top | safe | Print quality parameter, per-object meaningful |
| per_feature_flow.bridge | safe | Flow multipliers can differ per object/region |
| per_feature_flow.brim | safe | Flow multipliers can differ per object/region |
| per_feature_flow.gap_fill | safe | Flow multipliers can differ per object/region |
| per_feature_flow.inner_perimeter | safe | Flow multipliers can differ per object/region |
| per_feature_flow.ironing | safe | Flow multipliers can differ per object/region |
| per_feature_flow.outer_perimeter | safe | Flow multipliers can differ per object/region |
| per_feature_flow.purge_tower | safe | Flow multipliers can differ per object/region |
| per_feature_flow.skirt | safe | Flow multipliers can differ per object/region |
| per_feature_flow.solid_infill | safe | Flow multipliers can differ per object/region |
| per_feature_flow.sparse_infill | safe | Flow multipliers can differ per object/region |
| per_feature_flow.support | safe | Flow multipliers can differ per object/region |
| per_feature_flow.support_interface | safe | Flow multipliers can differ per object/region |
| per_feature_flow.variable_width_perimeter | safe | Flow multipliers can differ per object/region |
| polyhole_enabled | safe | Print quality parameter, per-object meaningful |
| polyhole_min_diameter | safe | Print quality parameter, per-object meaningful |
| precise_outer_wall | safe | Print quality parameter, per-object meaningful |
| reduce_crossing_wall | safe | Print quality parameter, per-object meaningful |
| reduce_infill_retraction | safe | Print quality parameter, per-object meaningful |
| resolution | safe | Print quality parameter, per-object meaningful |
| retraction.deretraction_speed | safe | Retraction can differ per object/region |
| retraction.length | safe | Retraction can differ per object/region |
| retraction.min_travel | safe | Retraction can differ per object/region |
| retraction.retract_before_wipe | safe | Retraction can differ per object/region |
| retraction.retract_when_changing_layer | safe | Retraction can differ per object/region |
| retraction.speed | safe | Retraction can differ per object/region |
| retraction.wipe | safe | Retraction can differ per object/region |
| retraction.wipe_distance | safe | Retraction can differ per object/region |
| retraction.z_hop | safe | Retraction can differ per object/region |
| scarf_joint.conditional_scarf | safe | Scarf seam can differ per object/region |
| scarf_joint.enabled | safe | Scarf seam can differ per object/region |
| scarf_joint.override_filament_setting | safe | Scarf seam can differ per object/region |
| scarf_joint.role_based_wipe_speed | safe | Scarf seam can differ per object/region |
| scarf_joint.scarf_angle_threshold | safe | Scarf seam can differ per object/region |
| scarf_joint.scarf_around_entire_wall | safe | Scarf seam can differ per object/region |
| scarf_joint.scarf_flow_ratio | safe | Scarf seam can differ per object/region |
| scarf_joint.scarf_inner_walls | safe | Scarf seam can differ per object/region |
| scarf_joint.scarf_joint_type | safe | Scarf seam can differ per object/region |
| scarf_joint.scarf_length | safe | Scarf seam can differ per object/region |
| scarf_joint.scarf_overhang_threshold | safe | Scarf seam can differ per object/region |
| scarf_joint.scarf_speed | safe | Scarf seam can differ per object/region |
| scarf_joint.scarf_start_height | safe | Scarf seam can differ per object/region |
| scarf_joint.scarf_steps | safe | Scarf seam can differ per object/region |
| scarf_joint.seam_gap | safe | Scarf seam can differ per object/region |
| scarf_joint.wipe_on_loop | safe | Scarf seam can differ per object/region |
| scarf_joint.wipe_speed | safe | Scarf seam can differ per object/region |
| seam_position | safe | Print quality parameter, per-object meaningful |
| slicing_tolerance | safe | Print quality parameter, per-object meaningful |
| solid_infill_pattern | safe | Print quality parameter, per-object meaningful |
| speeds.bridge | safe | Speed can differ per object/region |
| speeds.enable_overhang_speed | safe | Speed can differ per object/region |
| speeds.first_layer | safe | Speed can differ per object/region |
| speeds.gap_fill | safe | Speed can differ per object/region |
| speeds.infill | safe | Speed can differ per object/region |
| speeds.initial_layer_infill | safe | Speed can differ per object/region |
| speeds.inner_wall | safe | Speed can differ per object/region |
| speeds.internal_bridge_speed | safe | Speed can differ per object/region |
| speeds.internal_solid_infill | safe | Speed can differ per object/region |
| speeds.overhang_1_4 | safe | Speed can differ per object/region |
| speeds.overhang_2_4 | safe | Speed can differ per object/region |
| speeds.overhang_3_4 | safe | Speed can differ per object/region |
| speeds.overhang_4_4 | safe | Speed can differ per object/region |
| speeds.perimeter | safe | Speed can differ per object/region |
| speeds.small_perimeter | safe | Speed can differ per object/region |
| speeds.solid_infill | safe | Speed can differ per object/region |
| speeds.support | safe | Speed can differ per object/region |
| speeds.support_interface | safe | Speed can differ per object/region |
| speeds.top_surface | safe | Speed can differ per object/region |
| speeds.travel | safe | Speed can differ per object/region |
| speeds.travel_z | safe | Speed can differ per object/region |
| spiral_mode | safe | Print quality parameter, per-object meaningful |
| support.bottom_z_gap | safe | Support settings can differ per object |
| support.bridge.acceleration | safe | Bridge settings can differ per object/region |
| support.bridge.density | safe | Bridge settings can differ per object/region |
| support.bridge.flow_ratio | safe | Bridge settings can differ per object/region |
| support.bridge.line_width_ratio | safe | Bridge settings can differ per object/region |
| support.bridge.no_support | safe | Bridge settings can differ per object/region |
| support.bridge.speed | safe | Bridge settings can differ per object/region |
| support.bridge.thick_bridges | safe | Bridge settings can differ per object/region |
| support.bridge_detection | safe | Support settings can differ per object |
| support.build_plate_only | safe | Support settings can differ per object |
| support.closing_radius | safe | Support settings can differ per object |
| support.conflict_resolution | safe | Support settings can differ per object |
| support.critical_regions_only | safe | Support settings can differ per object |
| support.enabled | safe | Support settings can differ per object |
| support.enforce_layers | safe | Support settings can differ per object |
| support.expansion | safe | Support settings can differ per object |
| support.flow_ratio | safe | Support settings can differ per object |
| support.interface_density | safe | Support settings can differ per object |
| support.interface_flow_ratio | safe | Support settings can differ per object |
| support.interface_layers | safe | Support settings can differ per object |
| support.interface_pattern | safe | Support settings can differ per object |
| support.min_support_area | safe | Support settings can differ per object |
| support.overhang_angle | safe | Support settings can differ per object |
| support.quality_preset | safe | Support settings can differ per object |
| support.raft_expansion | safe | Support settings can differ per object |
| support.raft_layers | safe | Support settings can differ per object |
| support.remove_small_overhang | safe | Support settings can differ per object |
| support.support_density | safe | Support settings can differ per object |
| support.support_pattern | safe | Support settings can differ per object |
| support.support_type | safe | Support settings can differ per object |
| support.synchronize_layers | safe | Support settings can differ per object |
| support.tree.adaptive_layer_height | safe | Tree support can differ per object |
| support.tree.angle_slow | safe | Tree support can differ per object |
| support.tree.auto_brim | safe | Tree support can differ per object |
| support.tree.branch_angle | safe | Tree support can differ per object |
| support.tree.branch_diameter_angle | safe | Tree support can differ per object |
| support.tree.branch_distance | safe | Tree support can differ per object |
| support.tree.branch_style | safe | Tree support can differ per object |
| support.tree.brim_width | safe | Tree support can differ per object |
| support.tree.max_trunk_diameter | safe | Tree support can differ per object |
| support.tree.merge_distance_factor | safe | Tree support can differ per object |
| support.tree.min_branch_angle | safe | Tree support can differ per object |
| support.tree.taper_method | safe | Tree support can differ per object |
| support.tree.tip_diameter | safe | Tree support can differ per object |
| support.tree.top_rate | safe | Tree support can differ per object |
| support.tree.wall_count | safe | Tree support can differ per object |
| support.tree.with_infill | safe | Tree support can differ per object |
| support.xy_gap | safe | Support settings can differ per object |
| support.z_gap | safe | Support settings can differ per object |
| top_solid_layers | safe | Print quality parameter, per-object meaningful |
| top_surface_pattern | safe | Print quality parameter, per-object meaningful |
| wall_count | safe | Print quality parameter, per-object meaningful |
| wall_order | safe | Print quality parameter, per-object meaningful |

### Warn (106 fields)

Settings that are machine/plate-level and nonsensical per-region, but allowed with warning.

| Field | Safety | Reason |
|-------|--------|--------|
| acceleration_enabled | warn | Plate/machine-level setting |
| brim_skirt.brim_ears | warn | Adhesion aid is plate-level |
| brim_skirt.brim_ears_max_angle | warn | Adhesion aid is plate-level |
| brim_skirt.brim_type | warn | Adhesion aid is plate-level |
| brim_skirt.skirt_height | warn | Adhesion aid is plate-level |
| brim_width | warn | Plate/machine-level setting |
| cooling.additional_cooling_fan_speed | warn | Fan control is machine-level, odd per-region |
| cooling.auxiliary_fan | warn | Fan control is machine-level, odd per-region |
| cooling.disable_fan_first_layers | warn | Fan control is machine-level, odd per-region |
| cooling.fan_below_layer_time | warn | Fan control is machine-level, odd per-region |
| cooling.fan_max_speed | warn | Fan control is machine-level, odd per-region |
| cooling.fan_min_speed | warn | Fan control is machine-level, odd per-region |
| cooling.fan_speed | warn | Fan control is machine-level, odd per-region |
| cooling.full_fan_speed_layer | warn | Fan control is machine-level, odd per-region |
| cooling.overhang_fan_speed | warn | Fan control is machine-level, odd per-region |
| cooling.overhang_fan_threshold | warn | Fan control is machine-level, odd per-region |
| cooling.slow_down_for_layer_cooling | warn | Fan control is machine-level, odd per-region |
| cooling.slow_down_layer_time | warn | Fan control is machine-level, odd per-region |
| cooling.slow_down_min_speed | warn | Fan control is machine-level, odd per-region |
| custom_gcode.after_layer_change | warn | G-code hooks are machine/plate-level |
| custom_gcode.before_every_layer | warn | G-code hooks are machine/plate-level |
| custom_gcode.before_layer_change | warn | G-code hooks are machine/plate-level |
| custom_gcode.between_objects | warn | G-code hooks are machine/plate-level |
| custom_gcode.color_change | warn | G-code hooks are machine/plate-level |
| custom_gcode.pause_print | warn | G-code hooks are machine/plate-level |
| custom_gcode.tool_change_gcode | warn | G-code hooks are machine/plate-level |
| draft_shield | warn | Plate/machine-level setting |
| exclude_object | warn | Plate/machine-level setting |
| filament.bed_temperatures | warn | Filament properties are per-extruder, not per-region |
| filament.chamber_temperature | warn | Filament properties are per-extruder, not per-region |
| filament.cool_plate_temp | warn | Filament properties are per-extruder, not per-region |
| filament.cool_plate_temp_initial_layer | warn | Filament properties are per-extruder, not per-region |
| filament.cost_per_kg | warn | Filament properties are per-extruder, not per-region |
| filament.density | warn | Filament properties are per-extruder, not per-region |
| filament.diameter | warn | Filament properties are per-extruder, not per-region |
| filament.eng_plate_temp | warn | Filament properties are per-extruder, not per-region |
| filament.eng_plate_temp_initial_layer | warn | Filament properties are per-extruder, not per-region |
| filament.filament_colour | warn | Filament properties are per-extruder, not per-region |
| filament.filament_end_gcode | warn | Filament properties are per-extruder, not per-region |
| filament.filament_retraction_length | warn | Filament properties are per-extruder, not per-region |
| filament.filament_retraction_speed | warn | Filament properties are per-extruder, not per-region |
| filament.filament_shrink | warn | Filament properties are per-extruder, not per-region |
| filament.filament_start_gcode | warn | Filament properties are per-extruder, not per-region |
| filament.filament_type | warn | Filament properties are per-extruder, not per-region |
| filament.filament_vendor | warn | Filament properties are per-extruder, not per-region |
| filament.first_layer_bed_temperatures | warn | Filament properties are per-extruder, not per-region |
| filament.first_layer_nozzle_temperatures | warn | Filament properties are per-extruder, not per-region |
| filament.hot_plate_temp | warn | Filament properties are per-extruder, not per-region |
| filament.hot_plate_temp_initial_layer | warn | Filament properties are per-extruder, not per-region |
| filament.max_volumetric_speed | warn | Filament properties are per-extruder, not per-region |
| filament.nozzle_temperature_range_high | warn | Filament properties are per-extruder, not per-region |
| filament.nozzle_temperature_range_low | warn | Filament properties are per-extruder, not per-region |
| filament.nozzle_temperatures | warn | Filament properties are per-extruder, not per-region |
| filament.textured_plate_temp | warn | Filament properties are per-extruder, not per-region |
| filament.textured_plate_temp_initial_layer | warn | Filament properties are per-extruder, not per-region |
| filament.z_offset | warn | Filament properties are per-extruder, not per-region |
| input_shaping.accel_to_decel_enable | warn | Input shaping is machine-level |
| input_shaping.accel_to_decel_factor | warn | Input shaping is machine-level |
| multi_material.enabled | warn | Multi-material is plate/machine-level |
| multi_material.flush_into_infill | warn | Multi-material is plate/machine-level |
| multi_material.flush_into_objects | warn | Multi-material is plate/machine-level |
| multi_material.flush_into_support | warn | Multi-material is plate/machine-level |
| multi_material.purge_in_prime_tower | warn | Multi-material is plate/machine-level |
| multi_material.purge_tower_position | warn | Multi-material is plate/machine-level |
| multi_material.purge_tower_width | warn | Multi-material is plate/machine-level |
| multi_material.purge_volume | warn | Multi-material is plate/machine-level |
| multi_material.single_extruder_mmu | warn | Multi-material is plate/machine-level |
| multi_material.solid_infill_filament | warn | Multi-material is plate/machine-level |
| multi_material.support_filament | warn | Multi-material is plate/machine-level |
| multi_material.support_interface_filament | warn | Multi-material is plate/machine-level |
| multi_material.tool_change_retraction.long_retraction_when_cut | warn | Tool change retraction is machine-level |
| multi_material.tool_change_retraction.retraction_distance_when_cut | warn | Tool change retraction is machine-level |
| multi_material.tool_count | warn | Multi-material is plate/machine-level |
| multi_material.wall_filament | warn | Multi-material is plate/machine-level |
| multi_material.wipe_length | warn | Multi-material is plate/machine-level |
| multi_material.wipe_tower_bridging | warn | Multi-material is plate/machine-level |
| multi_material.wipe_tower_cone_angle | warn | Multi-material is plate/machine-level |
| multi_material.wipe_tower_no_sparse_layers | warn | Multi-material is plate/machine-level |
| multi_material.wipe_tower_rotation_angle | warn | Multi-material is plate/machine-level |
| ooze_prevention | warn | Plate/machine-level setting |
| post_process.enabled | warn | Post-processing is plate-level |
| post_process.gcode_label_objects | warn | Post-processing is plate-level |
| post_process.pause_at_layers | warn | Post-processing is plate-level |
| post_process.pause_command | warn | Post-processing is plate-level |
| post_process.timelapse.dwell_ms | warn | Timelapse is machine/plate-level |
| post_process.timelapse.enabled | warn | Timelapse is machine/plate-level |
| post_process.timelapse.park_x | warn | Timelapse is machine/plate-level |
| post_process.timelapse.park_y | warn | Timelapse is machine/plate-level |
| post_process.timelapse.retract_distance | warn | Timelapse is machine/plate-level |
| post_process.timelapse.retract_speed | warn | Timelapse is machine/plate-level |
| precise_z_height | warn | Plate/machine-level setting |
| pressure_advance | warn | Plate/machine-level setting |
| raft_layers | warn | Plate/machine-level setting |
| sequential.enabled | warn | Sequential printing is plate-level |
| sequential.extruder_clearance_height | warn | Sequential printing is plate-level |
| sequential.extruder_clearance_radius | warn | Sequential printing is plate-level |
| sequential.gantry_depth | warn | Sequential printing is plate-level |
| sequential.gantry_width | warn | Sequential printing is plate-level |
| skirt_distance | warn | Plate/machine-level setting |
| skirt_loops | warn | Plate/machine-level setting |
| travel_opt.algorithm | warn | Travel optimization is plate-level |
| travel_opt.enabled | warn | Travel optimization is plate-level |
| travel_opt.max_iterations | warn | Travel optimization is plate-level |
| travel_opt.optimize_cross_object | warn | Travel optimization is plate-level |
| travel_opt.print_order | warn | Travel optimization is plate-level |
| z_offset | warn | Plate/machine-level setting |

### Ignored (78 fields)

Machine properties that have no per-object meaning and are silently ignored as overrides.

| Field | Safety | Reason |
|-------|--------|--------|
| compatible_printers_condition | ignored | Infrastructure/metadata, not a print parameter |
| gcode_dialect | ignored | Infrastructure/metadata, not a print parameter |
| inherits_group | ignored | Infrastructure/metadata, not a print parameter |
| machine.bed_custom_model | ignored | Machine hardware property |
| machine.bed_custom_texture | ignored | Machine hardware property |
| machine.bed_shape | ignored | Machine hardware property |
| machine.bed_x | ignored | Machine hardware property |
| machine.bed_y | ignored | Machine hardware property |
| machine.chamber_temperature | ignored | Machine hardware property |
| machine.cooling_tube_length | ignored | Machine hardware property |
| machine.cooling_tube_retraction | ignored | Machine hardware property |
| machine.curr_bed_type | ignored | Machine hardware property |
| machine.emit_machine_limits_to_gcode | ignored | Machine hardware property |
| machine.end_gcode | ignored | Machine hardware property |
| machine.end_gcode_original | ignored | Machine hardware property |
| machine.extra_loading_move | ignored | Machine hardware property |
| machine.extruder_count | ignored | Machine hardware property |
| machine.extruder_offset | ignored | Machine hardware property |
| machine.jerk_values_e | ignored | Machine hardware property |
| machine.jerk_values_x | ignored | Machine hardware property |
| machine.jerk_values_y | ignored | Machine hardware property |
| machine.jerk_values_z | ignored | Machine hardware property |
| machine.layer_change_gcode | ignored | Machine hardware property |
| machine.layer_change_gcode_original | ignored | Machine hardware property |
| machine.max_acceleration_e | ignored | Machine hardware property |
| machine.max_acceleration_extruding | ignored | Machine hardware property |
| machine.max_acceleration_retracting | ignored | Machine hardware property |
| machine.max_acceleration_travel | ignored | Machine hardware property |
| machine.max_acceleration_x | ignored | Machine hardware property |
| machine.max_acceleration_y | ignored | Machine hardware property |
| machine.max_acceleration_z | ignored | Machine hardware property |
| machine.max_layer_height | ignored | Machine hardware property |
| machine.max_speed_e | ignored | Machine hardware property |
| machine.max_speed_x | ignored | Machine hardware property |
| machine.max_speed_y | ignored | Machine hardware property |
| machine.max_speed_z | ignored | Machine hardware property |
| machine.min_extruding_rate | ignored | Machine hardware property |
| machine.min_layer_height | ignored | Machine hardware property |
| machine.min_travel_rate | ignored | Machine hardware property |
| machine.nozzle_diameters | ignored | Machine hardware property |
| machine.nozzle_hrc | ignored | Machine hardware property |
| machine.nozzle_type | ignored | Machine hardware property |
| machine.parking_pos_retraction | ignored | Machine hardware property |
| machine.printable_height | ignored | Machine hardware property |
| machine.printer_model | ignored | Machine hardware property |
| machine.retract_length_toolchange | ignored | Machine hardware property |
| machine.retract_restart_extra | ignored | Machine hardware property |
| machine.retract_restart_extra_toolchange | ignored | Machine hardware property |
| machine.silent_mode | ignored | Machine hardware property |
| machine.start_gcode | ignored | Machine hardware property |
| machine.start_gcode_original | ignored | Machine hardware property |
| machine.watts | ignored | Machine hardware property |
| multi_material.tools.nozzle_temp | ignored | Per-tool machine property |
| multi_material.tools.retract_length | ignored | Per-tool machine property |
| multi_material.tools.retract_speed | ignored | Per-tool machine property |
| pa_calibration.bed_center_x | ignored | Standalone calibration tool, not per-object |
| pa_calibration.bed_center_y | ignored | Standalone calibration tool, not per-object |
| pa_calibration.bed_temp | ignored | Standalone calibration tool, not per-object |
| pa_calibration.fast_speed | ignored | Standalone calibration tool, not per-object |
| pa_calibration.filament_diameter | ignored | Standalone calibration tool, not per-object |
| pa_calibration.layer_height | ignored | Standalone calibration tool, not per-object |
| pa_calibration.line_width | ignored | Standalone calibration tool, not per-object |
| pa_calibration.nozzle_temp | ignored | Standalone calibration tool, not per-object |
| pa_calibration.pa_end | ignored | Standalone calibration tool, not per-object |
| pa_calibration.pa_start | ignored | Standalone calibration tool, not per-object |
| pa_calibration.pa_step | ignored | Standalone calibration tool, not per-object |
| pa_calibration.pattern_width | ignored | Standalone calibration tool, not per-object |
| pa_calibration.slow_speed | ignored | Standalone calibration tool, not per-object |
| parallel_slicing | ignored | Infrastructure/metadata, not a print parameter |
| plugin_dir | ignored | Infrastructure/metadata, not a print parameter |
| post_process.filename_format | ignored | Output infrastructure, not a print parameter |
| post_process.gcode_add_line_number | ignored | Output infrastructure, not a print parameter |
| post_process.gcode_comments | ignored | Output infrastructure, not a print parameter |
| post_process.plugin_order | ignored | Output infrastructure, not a print parameter |
| post_process.scripts | ignored | Output infrastructure, not a print parameter |
| thread_count | ignored | Infrastructure/metadata, not a print parameter |
| thumbnail_resolution | ignored | Infrastructure/metadata, not a print parameter |
| thumbnails | ignored | Infrastructure/metadata, not a print parameter |
