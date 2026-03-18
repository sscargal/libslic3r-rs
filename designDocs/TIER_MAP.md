# Tier Map: Progressive Disclosure of Config Settings

## Tier System

| Tier | Name         | Description                                                    | Target Count |
|------|--------------|----------------------------------------------------------------|--------------|
| 0    | AI Auto      | Reserved for AI-managed settings (populated in AI phase)       | 0            |
| 1    | Simple       | Essential settings every beginner needs                        | ~15          |
| 2    | Intermediate | Settings for users who want more control                       | ~60          |
| 3    | Advanced     | Settings for power users and fine-tuning                       | ~200         |
| 4    | Developer    | Debug, calibration, niche, and machine-specific settings       | Rest         |

## Methodology

Tier assignments use OrcaSlicer's UI tab placement as baseline:

- **OrcaSlicer Simple tab** -> Tier 1
- **OrcaSlicer Advanced tab** -> Tier 2
- **OrcaSlicer Expert tab** -> Tier 3
- **Hidden / Debug / Niche** -> Tier 4

Adjustments are made where our engine's field organization differs from OrcaSlicer, or where 3D printing domain knowledge suggests a different tier is more appropriate for progressive disclosure.

---

## Quality (PrintConfig top-level)

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| layer_height | Layer Height | 1 | Simple | Most fundamental print quality setting |
| first_layer_height | First Layer Height | 1 | Simple | Critical for bed adhesion |
| wall_count | Wall Loops | 1 | Simple | Core structural parameter |
| wall_order | Wall Order | 2 | Advanced | Affects surface quality but not essential for beginners |
| seam_position | Seam Position | 2 | Advanced | Cosmetic tuning |
| top_solid_layers | Top Shell Layers | 1 | Simple | Essential for closed top surfaces |
| bottom_solid_layers | Bottom Shell Layers | 1 | Simple | Essential for closed bottom surfaces |
| top_surface_pattern | Top Surface Pattern | 2 | Advanced | Surface finish tuning |
| bottom_surface_pattern | Bottom Surface Pattern | 3 | Expert | Rarely changed from default |
| solid_infill_pattern | Solid Infill Pattern | 3 | Expert | Internal solid layer pattern, rarely adjusted |
| extra_perimeters_on_overhangs | Extra Perimeters On Overhangs | 3 | Expert | Overhang quality enhancement |
| internal_bridge_support | Internal Bridge Support | 3 | Expert | Niche bridge quality setting |
| z_offset | Z Offset | 2 | Advanced | Common first-layer calibration adjustment |
| precise_z_height | Precise Z Height | 4 | Hidden | Niche firmware feature |
| extrusion_multiplier | Extrusion Multiplier | 2 | Advanced | Flow tuning for dimensional accuracy |
| bridge_flow | Bridge Flow Ratio | 3 | Expert | Fine-tune bridge extrusion |
| resolution | G-code Resolution | 3 | Expert | Controls G-code point density |
| detect_thin_wall | Detect Thin Wall | 3 | Expert | Thin wall detection toggle |
| only_one_wall_top | Only One Wall Top | 3 | Expert | Top surface wall optimization |
| spiral_mode | Spiral Vase Mode | 2 | Advanced | Popular special print mode |
| precise_outer_wall | Precise Outer Wall | 3 | Expert | Dimensional accuracy tuning |
| draft_shield | Draft Shield | 3 | Expert | Environmental protection for ABS/ASA |
| ooze_prevention | Ooze Prevention | 3 | Expert | Multi-tool ooze management |
| slicing_tolerance | Slicing Tolerance | 4 | Hidden | Niche dimensional accuracy mode |
| arachne_enabled | Arachne Variable Width | 3 | Expert | Advanced wall generation algorithm |
| min_bead_width | Min Bead Width | 3 | Expert | Arachne parameter |
| min_feature_size | Min Feature Size | 3 | Expert | Arachne parameter |
| adaptive_layer_height | Adaptive Layer Height | 2 | Advanced | Quality/speed optimization |
| adaptive_min_layer_height | Adaptive Min Layer Height | 3 | Expert | Adaptive layer parameter |
| adaptive_max_layer_height | Adaptive Max Layer Height | 3 | Expert | Adaptive layer parameter |
| adaptive_layer_quality | Adaptive Layer Quality | 3 | Expert | Adaptive layer parameter |
| gap_fill_enabled | Gap Fill Enabled | 3 | Expert | Gap fill toggle |
| gap_fill_min_width | Gap Fill Min Width | 3 | Expert | Gap fill threshold |
| polyhole_enabled | Polyhole Conversion | 3 | Expert | Dimensional accuracy for holes |
| polyhole_min_diameter | Polyhole Min Diameter | 3 | Expert | Polyhole threshold |
| thumbnail_resolution | Thumbnail Resolution | 4 | Hidden | Preview image resolution |
| thumbnails | Thumbnail Sizes | 4 | Hidden | Thumbnail size list for 3MF/G-code |
| compatible_printers_condition | Compatible Printers Condition | 4 | Hidden | Profile management metadata |
| inherits_group | Inherits Group | 4 | Hidden | Profile inheritance metadata |
| exclude_object | Exclude Object | 3 | Expert | Cancel object support |

## Infill

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| infill_pattern | Infill Pattern | 1 | Simple | Core structural choice |
| infill_density | Infill Density | 1 | Simple | Most visible infill parameter |
| infill_direction | Infill Direction | 3 | Expert | Infill angle tuning |
| infill_wall_overlap | Infill Wall Overlap | 3 | Expert | Infill-wall bonding tuning |
| infill_combination | Infill Combination | 3 | Expert | Combine infill every N layers |
| infill_anchor_max | Infill Anchor Max | 3 | Expert | Infill anchor length limit |
| reduce_infill_retraction | Reduce Infill Retraction | 3 | Expert | Travel optimization over infill |

## Speed

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| speeds.perimeter | Perimeter Speed | 1 | Simple | Core speed setting |
| speeds.infill | Infill Speed | 1 | Simple | Core speed setting |
| speeds.travel | Travel Speed | 2 | Advanced | Non-extrusion movement speed |
| speeds.first_layer | First Layer Speed | 2 | Advanced | First layer adhesion tuning |
| speeds.bridge | Bridge Speed | 2 | Advanced | Bridge quality tuning |
| speeds.inner_wall | Inner Wall Speed | 2 | Advanced | Inner wall speed override |
| speeds.gap_fill | Gap Fill Speed | 3 | Expert | Gap fill speed override |
| speeds.top_surface | Top Surface Speed | 2 | Advanced | Surface quality speed |
| speeds.internal_solid_infill | Internal Solid Infill Speed | 3 | Expert | Internal solid layer speed |
| speeds.initial_layer_infill | Initial Layer Infill Speed | 3 | Expert | First layer infill speed |
| speeds.support | Support Speed | 2 | Advanced | Support printing speed |
| speeds.support_interface | Support Interface Speed | 3 | Expert | Support interface layer speed |
| speeds.small_perimeter | Small Perimeter Speed | 3 | Expert | Speed for small features |
| speeds.solid_infill | Solid Infill Speed | 3 | Expert | Solid infill speed override |
| speeds.overhang_1_4 | Overhang Speed 0-25% | 3 | Expert | Overhang speed tuning |
| speeds.overhang_2_4 | Overhang Speed 25-50% | 3 | Expert | Overhang speed tuning |
| speeds.overhang_3_4 | Overhang Speed 50-75% | 3 | Expert | Overhang speed tuning |
| speeds.overhang_4_4 | Overhang Speed 75-100% | 3 | Expert | Overhang speed tuning |
| speeds.travel_z | Z Travel Speed | 3 | Expert | Z-axis movement speed |
| speeds.internal_bridge_speed | Internal Bridge Speed | 3 | Expert | Internal bridge speed override |
| speeds.enable_overhang_speed | Enable Overhang Speed | 3 | Expert | Master switch for overhang speeds |

## Acceleration

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| accel.print | Print Acceleration | 2 | Advanced | Base print acceleration |
| accel.travel | Travel Acceleration | 2 | Advanced | Travel move acceleration |
| accel.outer_wall | Outer Wall Acceleration | 3 | Expert | Per-feature acceleration |
| accel.inner_wall | Inner Wall Acceleration | 3 | Expert | Per-feature acceleration |
| accel.initial_layer | Initial Layer Acceleration | 3 | Expert | First layer acceleration |
| accel.initial_layer_travel | Initial Layer Travel Acceleration | 3 | Expert | First layer travel acceleration |
| accel.top_surface | Top Surface Acceleration | 3 | Expert | Surface quality acceleration |
| accel.sparse_infill | Sparse Infill Acceleration | 3 | Expert | Infill acceleration |
| accel.bridge | Bridge Acceleration | 3 | Expert | Bridge acceleration |
| accel.min_length_factor | Min Length Factor | 4 | Hidden | Minimum segment length for accel changes |
| accel.internal_solid_infill_acceleration | Internal Solid Infill Acceleration | 3 | Expert | Internal solid acceleration |
| accel.support_acceleration | Support Acceleration | 3 | Expert | Support structure acceleration |
| accel.support_interface_acceleration | Support Interface Acceleration | 3 | Expert | Support interface acceleration |
| accel.default_jerk | Default Jerk | 3 | Expert | Base jerk value |
| accel.outer_wall_jerk | Outer Wall Jerk | 3 | Expert | Per-feature jerk |
| accel.inner_wall_jerk | Inner Wall Jerk | 3 | Expert | Per-feature jerk |
| accel.top_surface_jerk | Top Surface Jerk | 3 | Expert | Per-feature jerk |
| accel.infill_jerk | Infill Jerk | 3 | Expert | Per-feature jerk |
| accel.travel_jerk | Travel Jerk | 3 | Expert | Per-feature jerk |
| accel.initial_layer_jerk | Initial Layer Jerk | 3 | Expert | Per-feature jerk |

## Cooling

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| cooling.fan_speed | Fan Speed | 2 | Advanced | Primary fan control |
| cooling.fan_below_layer_time | Fan Below Layer Time | 2 | Advanced | Auto-fan threshold |
| cooling.disable_fan_first_layers | Disable Fan First Layers | 2 | Advanced | First layer fan control |
| cooling.fan_max_speed | Fan Max Speed | 2 | Advanced | Maximum fan speed limit |
| cooling.fan_min_speed | Fan Min Speed | 2 | Advanced | Minimum fan speed limit |
| cooling.slow_down_layer_time | Slow Down Layer Time | 3 | Expert | Layer time slowdown threshold |
| cooling.slow_down_min_speed | Slow Down Min Speed | 3 | Expert | Minimum speed during slowdown |
| cooling.overhang_fan_speed | Overhang Fan Speed | 3 | Expert | Overhang-specific fan speed |
| cooling.overhang_fan_threshold | Overhang Fan Threshold | 3 | Expert | Overhang angle for fan override |
| cooling.full_fan_speed_layer | Full Fan Speed Layer | 3 | Expert | Layer at which fan reaches full speed |
| cooling.slow_down_for_layer_cooling | Slow Down For Layer Cooling | 3 | Expert | Auto-slowdown toggle |
| cooling.additional_cooling_fan_speed | Additional Cooling Fan Speed | 3 | Expert | Auxiliary fan speed |
| cooling.auxiliary_fan | Auxiliary Fan | 3 | Expert | Enable auxiliary fan |

## Retraction

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| retraction.length | Retraction Length | 2 | Advanced | Core retraction parameter |
| retraction.speed | Retraction Speed | 2 | Advanced | Core retraction parameter |
| retraction.z_hop | Z Hop | 2 | Advanced | Common retraction enhancement |
| retraction.min_travel | Min Travel For Retract | 2 | Advanced | Retraction trigger threshold |
| retraction.deretraction_speed | Deretraction Speed | 3 | Expert | Unretract speed override |
| retraction.retract_before_wipe | Retract Before Wipe | 3 | Expert | Pre-wipe retraction percentage |
| retraction.retract_when_changing_layer | Retract When Changing Layer | 3 | Expert | Layer change retraction toggle |
| retraction.wipe | Wipe | 3 | Expert | Enable wipe move |
| retraction.wipe_distance | Wipe Distance | 3 | Expert | Wipe move length |
| reduce_crossing_wall | Reduce Crossing Wall | 3 | Expert | Travel path optimization |

## Line Width

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| line_widths.outer_wall | Outer Wall Line Width | 2 | Advanced | Per-feature line width |
| line_widths.inner_wall | Inner Wall Line Width | 2 | Advanced | Per-feature line width |
| line_widths.infill | Infill Line Width | 2 | Advanced | Per-feature line width |
| line_widths.top_surface | Top Surface Line Width | 2 | Advanced | Per-feature line width |
| line_widths.initial_layer | Initial Layer Line Width | 2 | Advanced | Per-feature line width |
| line_widths.internal_solid_infill | Internal Solid Infill Line Width | 3 | Expert | Internal solid line width |
| line_widths.support | Support Line Width | 3 | Expert | Support structure line width |

## Filament

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| filament.diameter | Filament Diameter | 2 | Advanced | Filament physical property |
| filament.density | Filament Density | 3 | Expert | Cost/weight estimation |
| filament.cost_per_kg | Filament Cost Per Kg | 3 | Expert | Cost estimation |
| filament.filament_type | Filament Type | 1 | Simple | Material selection |
| filament.filament_vendor | Filament Vendor | 3 | Expert | Material metadata |
| filament.max_volumetric_speed | Max Volumetric Speed | 3 | Expert | Flow rate limit |
| filament.nozzle_temperature_range_low | Nozzle Temp Range Low | 3 | Expert | Temperature range metadata |
| filament.nozzle_temperature_range_high | Nozzle Temp Range High | 3 | Expert | Temperature range metadata |
| filament.nozzle_temperatures | Nozzle Temperature | 1 | Simple | Core temperature setting |
| filament.bed_temperatures | Bed Temperature | 1 | Simple | Core temperature setting |
| filament.first_layer_nozzle_temperatures | First Layer Nozzle Temperature | 2 | Advanced | First layer temp override |
| filament.first_layer_bed_temperatures | First Layer Bed Temperature | 2 | Advanced | First layer temp override |
| filament.filament_retraction_length | Filament Retraction Length | 3 | Expert | Per-filament retraction override |
| filament.filament_retraction_speed | Filament Retraction Speed | 3 | Expert | Per-filament retraction override |
| filament.filament_start_gcode | Filament Start G-code | 4 | Hidden | Filament-specific startup G-code |
| filament.filament_end_gcode | Filament End G-code | 4 | Hidden | Filament-specific shutdown G-code |
| filament.chamber_temperature | Chamber Temperature | 3 | Expert | Enclosed printer chamber temp |
| filament.filament_shrink | Filament Shrinkage | 4 | Hidden | Shrinkage compensation percentage |
| filament.z_offset | Filament Z Offset | 3 | Expert | Per-filament Z offset |
| filament.filament_colour | Filament Colour | 2 | Advanced | Preview color |
| filament.hot_plate_temp | Hot Plate Temperature | 3 | Expert | Per-bed-type temperature |
| filament.cool_plate_temp | Cool Plate Temperature | 3 | Expert | Per-bed-type temperature |
| filament.eng_plate_temp | Engineering Plate Temperature | 3 | Expert | Per-bed-type temperature |
| filament.textured_plate_temp | Textured Plate Temperature | 3 | Expert | Per-bed-type temperature |
| filament.hot_plate_temp_initial_layer | Hot Plate Initial Layer Temp | 3 | Expert | Per-bed-type first layer temp |
| filament.cool_plate_temp_initial_layer | Cool Plate Initial Layer Temp | 3 | Expert | Per-bed-type first layer temp |
| filament.eng_plate_temp_initial_layer | Eng Plate Initial Layer Temp | 3 | Expert | Per-bed-type first layer temp |
| filament.textured_plate_temp_initial_layer | Textured Plate Initial Layer Temp | 3 | Expert | Per-bed-type first layer temp |

## Machine

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| machine.bed_x | Bed X Size | 2 | Advanced | Printer bed dimension |
| machine.bed_y | Bed Y Size | 2 | Advanced | Printer bed dimension |
| machine.printable_height | Printable Height | 2 | Advanced | Maximum Z height |
| machine.max_acceleration_x | Max Acceleration X | 4 | Hidden | Machine motion limit |
| machine.max_acceleration_y | Max Acceleration Y | 4 | Hidden | Machine motion limit |
| machine.max_acceleration_z | Max Acceleration Z | 4 | Hidden | Machine motion limit |
| machine.max_acceleration_e | Max Acceleration E | 4 | Hidden | Machine motion limit |
| machine.max_acceleration_extruding | Max Acceleration Extruding | 4 | Hidden | Machine motion limit |
| machine.max_acceleration_retracting | Max Acceleration Retracting | 4 | Hidden | Machine motion limit |
| machine.max_acceleration_travel | Max Acceleration Travel | 4 | Hidden | Machine motion limit |
| machine.max_speed_x | Max Speed X | 4 | Hidden | Machine motion limit |
| machine.max_speed_y | Max Speed Y | 4 | Hidden | Machine motion limit |
| machine.max_speed_z | Max Speed Z | 4 | Hidden | Machine motion limit |
| machine.max_speed_e | Max Speed E | 4 | Hidden | Machine motion limit |
| machine.nozzle_diameters | Nozzle Diameter | 2 | Advanced | Hardware specification |
| machine.jerk_values_x | Jerk X | 4 | Hidden | Machine jerk limit |
| machine.jerk_values_y | Jerk Y | 4 | Hidden | Machine jerk limit |
| machine.jerk_values_z | Jerk Z | 4 | Hidden | Machine jerk limit |
| machine.jerk_values_e | Jerk E | 4 | Hidden | Machine jerk limit |
| machine.start_gcode | Start G-code | 3 | Expert | Machine startup sequence |
| machine.start_gcode_original | Start G-code Original | 4 | Hidden | Upstream verbatim G-code |
| machine.end_gcode | End G-code | 3 | Expert | Machine shutdown sequence |
| machine.end_gcode_original | End G-code Original | 4 | Hidden | Upstream verbatim G-code |
| machine.layer_change_gcode | Layer Change G-code | 3 | Expert | Per-layer G-code injection |
| machine.layer_change_gcode_original | Layer Change G-code Original | 4 | Hidden | Upstream verbatim G-code |
| machine.nozzle_type | Nozzle Type | 3 | Expert | Nozzle material descriptor |
| machine.printer_model | Printer Model | 3 | Expert | Printer model identifier |
| machine.bed_shape | Bed Shape | 4 | Hidden | Serialized bed geometry |
| machine.min_layer_height | Min Layer Height | 4 | Hidden | Printer capability limit |
| machine.max_layer_height | Max Layer Height | 4 | Hidden | Printer capability limit |
| machine.extruder_count | Extruder Count | 3 | Expert | Number of toolheads |
| machine.watts | Printer Power (Watts) | 4 | Hidden | Cost estimation input |
| machine.chamber_temperature | Max Chamber Temperature | 4 | Hidden | Machine capability limit |
| machine.curr_bed_type | Current Bed Type | 2 | Advanced | Active build plate selection |
| machine.silent_mode | Silent Mode | 3 | Expert | Stealth mode toggle |
| machine.nozzle_hrc | Nozzle Hardness (HRC) | 4 | Hidden | Nozzle wear metadata |
| machine.emit_machine_limits_to_gcode | Emit Machine Limits To G-code | 4 | Hidden | G-code machine limits output |
| machine.bed_custom_texture | Bed Custom Texture | 4 | Hidden | UI preview texture path |
| machine.bed_custom_model | Bed Custom Model | 4 | Hidden | UI preview model path |
| machine.extruder_offset | Extruder Offset | 4 | Hidden | Multi-extruder XY offset |
| machine.cooling_tube_length | Cooling Tube Length | 4 | Hidden | Bambu AMS parameter |
| machine.cooling_tube_retraction | Cooling Tube Retraction | 4 | Hidden | Bambu AMS parameter |
| machine.parking_pos_retraction | Parking Position Retraction | 4 | Hidden | Bambu AMS parameter |
| machine.extra_loading_move | Extra Loading Move | 4 | Hidden | Bambu AMS parameter |
| machine.retract_length_toolchange | Retract Length Toolchange | 4 | Hidden | Tool change retraction |
| machine.retract_restart_extra | Retract Restart Extra | 4 | Hidden | Extra prime after retraction |
| machine.retract_restart_extra_toolchange | Retract Restart Extra Toolchange | 4 | Hidden | Extra prime after tool change |
| machine.min_extruding_rate | Min Extruding Rate | 4 | Hidden | Machine minimum extrusion speed |
| machine.min_travel_rate | Min Travel Rate | 4 | Hidden | Machine minimum travel speed |

## Support

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| support.enabled | Enable Support | 1 | Simple | Core support toggle |
| support.support_type | Support Type | 2 | Advanced | Support generation strategy |
| support.overhang_angle | Overhang Angle | 2 | Advanced | Overhang detection threshold |
| support.min_support_area | Min Support Area | 3 | Expert | Small region filter |
| support.support_density | Support Density | 2 | Advanced | Support fill density |
| support.support_pattern | Support Pattern | 2 | Advanced | Support fill pattern |
| support.interface_layers | Interface Layers | 2 | Advanced | Dense interface layer count |
| support.interface_density | Interface Density | 3 | Expert | Interface fill density |
| support.interface_pattern | Interface Pattern | 3 | Expert | Interface fill pattern |
| support.z_gap | Support Z Gap | 2 | Advanced | Top gap for easy removal |
| support.xy_gap | Support XY Gap | 3 | Expert | Side gap between support and model |
| support.build_plate_only | Support Build Plate Only | 2 | Advanced | Common support restriction |
| support.bridge_detection | Bridge Detection | 3 | Expert | Auto-detect bridging spans |
| support.quality_preset | Quality Preset | 3 | Expert | Quick quality preset selector |
| support.conflict_resolution | Conflict Resolution | 4 | Hidden | Internal conflict resolution strategy |
| support.support_bottom_interface_layers | Support Bottom Interface Layers | 3 | Expert | Support floor interface layers |
| support.expansion | Support Expansion | 3 | Expert | Horizontal expansion distance |
| support.raft_layers | Support Raft Layers | 2 | Advanced | Raft layer count |
| support.raft_expansion | Raft Expansion | 3 | Expert | Raft first layer expansion |
| support.critical_regions_only | Critical Regions Only | 3 | Expert | Limit to critical overhangs |
| support.remove_small_overhang | Remove Small Overhang | 3 | Expert | Filter small overhang regions |
| support.flow_ratio | Support Flow Ratio | 3 | Expert | Support extrusion flow adjustment |
| support.interface_flow_ratio | Support Interface Flow Ratio | 3 | Expert | Interface extrusion flow adjustment |
| support.synchronize_layers | Synchronize Layers | 4 | Hidden | Sync support with object layers |
| support.enforce_layers | Enforce Support Layers | 4 | Hidden | Minimum enforced support layers |
| support.closing_radius | Closing Radius | 4 | Hidden | Support area closing radius |
| support.bottom_z_gap | Bottom Z Gap | 3 | Expert | Support floor Z gap |

## Bridge (support.bridge)

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| support.bridge.speed | Bridge Speed | 3 | Expert | Bridge extrusion speed |
| support.bridge.fan_speed | Bridge Fan Speed | 3 | Expert | Fan speed during bridging |
| support.bridge.flow_ratio | Bridge Flow Ratio | 3 | Expert | Bridge flow adjustment |
| support.bridge.acceleration | Bridge Acceleration | 3 | Expert | Bridge acceleration |
| support.bridge.line_width_ratio | Bridge Line Width Ratio | 3 | Expert | Bridge line width adjustment |
| support.bridge.angle | Bridge Angle | 3 | Expert | Bridge extrusion angle |
| support.bridge.density | Bridge Density | 3 | Expert | Bridge fill density |
| support.bridge.thick_bridges | Thick Bridges | 3 | Expert | Use thick bridges |
| support.bridge.no_support | Bridge No Support | 3 | Expert | Disable support under bridges |

## Tree Support (support.tree)

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| support.tree.branch_style | Branch Style | 3 | Expert | Tree branch growth style |
| support.tree.taper_method | Taper Method | 3 | Expert | Trunk diameter taper method |
| support.tree.branch_angle | Branch Angle | 3 | Expert | Maximum branch angle |
| support.tree.min_branch_angle | Min Branch Angle | 4 | Hidden | Minimum branch divergence angle |
| support.tree.max_trunk_diameter | Max Trunk Diameter | 3 | Expert | Maximum trunk diameter |
| support.tree.merge_distance_factor | Merge Distance Factor | 4 | Hidden | Branch merge distance factor |
| support.tree.tip_diameter | Tip Diameter | 3 | Expert | Contact point tip diameter |
| support.tree.branch_distance | Branch Distance | 3 | Expert | Distance between branches |
| support.tree.branch_diameter_angle | Branch Diameter Angle | 4 | Hidden | Diameter increase angle |
| support.tree.wall_count | Tree Wall Count | 3 | Expert | Walls around tree branches |
| support.tree.auto_brim | Tree Auto Brim | 3 | Expert | Auto-brim around tree base |
| support.tree.brim_width | Tree Brim Width | 3 | Expert | Brim width around tree base |
| support.tree.adaptive_layer_height | Tree Adaptive Layer Height | 4 | Hidden | Adaptive layers for tree support |
| support.tree.angle_slow | Tree Angle Slow | 4 | Hidden | Gradual angle change speed |
| support.tree.top_rate | Tree Top Rate | 4 | Hidden | Top contact rate |
| support.tree.with_infill | Tree With Infill | 4 | Hidden | Fill tree interior with infill |

## Adhesion (Brim/Skirt)

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| skirt_loops | Skirt Loops | 2 | Advanced | Skirt line count |
| skirt_distance | Skirt Distance | 2 | Advanced | Skirt gap from object |
| brim_width | Brim Width | 1 | Simple | Primary adhesion control |
| brim_skirt.brim_type | Brim Type | 2 | Advanced | Brim placement type |
| brim_skirt.brim_ears | Brim Ears | 3 | Expert | Corner-only brim |
| brim_skirt.brim_ears_max_angle | Brim Ears Max Angle | 3 | Expert | Brim ears angle threshold |
| brim_skirt.skirt_height | Skirt Height | 3 | Expert | Skirt height in layers |
| raft_layers | Raft Layers | 2 | Advanced | Raft layer count (PrintConfig level) |

## Scarf Joint (Quality)

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| scarf_joint.enabled | Scarf Joint Enabled | 3 | Expert | Scarf seam toggle |
| scarf_joint.scarf_joint_type | Scarf Joint Type | 3 | Expert | Contour/hole selection |
| scarf_joint.conditional_scarf | Conditional Scarf | 3 | Expert | Smooth perimeter only |
| scarf_joint.scarf_speed | Scarf Speed | 3 | Expert | Speed during scarf region |
| scarf_joint.scarf_start_height | Scarf Start Height | 3 | Expert | Z offset at ramp start |
| scarf_joint.scarf_around_entire_wall | Scarf Around Entire Wall | 3 | Expert | Apply scarf to full wall |
| scarf_joint.scarf_length | Scarf Length | 3 | Expert | Horizontal ramp length |
| scarf_joint.scarf_steps | Scarf Steps | 3 | Expert | Discrete ramp step count |
| scarf_joint.scarf_flow_ratio | Scarf Flow Ratio | 3 | Expert | Scarf extrusion flow ratio |
| scarf_joint.scarf_inner_walls | Scarf Inner Walls | 3 | Expert | Apply scarf to inner walls |
| scarf_joint.role_based_wipe_speed | Role Based Wipe Speed | 4 | Hidden | Use role speed for wipe |
| scarf_joint.wipe_speed | Wipe Speed | 3 | Expert | Seam end wipe speed |
| scarf_joint.wipe_on_loop | Wipe On Loop | 3 | Expert | Inward wipe at seam close |
| scarf_joint.seam_gap | Seam Gap | 4 | Hidden | Gap between scarf end and next layer |
| scarf_joint.scarf_angle_threshold | Scarf Angle Threshold | 4 | Hidden | Min angle for scarf activation |
| scarf_joint.scarf_overhang_threshold | Scarf Overhang Threshold | 4 | Hidden | Overhang threshold to disable scarf |
| scarf_joint.override_filament_setting | Override Filament Scarf Setting | 4 | Hidden | Override filament-level scarf settings |

## Ironing

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| ironing.enabled | Ironing Enabled | 2 | Advanced | Top surface smoothing toggle |
| ironing.flow_rate | Ironing Flow Rate | 3 | Expert | Ironing extrusion flow |
| ironing.speed | Ironing Speed | 3 | Expert | Ironing pass speed |
| ironing.spacing | Ironing Spacing | 3 | Expert | Ironing line spacing |
| ironing.angle | Ironing Angle | 3 | Expert | Ironing pass angle |

## Per-Feature Flow

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| per_feature_flow.outer_perimeter | Outer Perimeter Flow | 3 | Expert | Per-feature flow multiplier |
| per_feature_flow.inner_perimeter | Inner Perimeter Flow | 3 | Expert | Per-feature flow multiplier |
| per_feature_flow.solid_infill | Solid Infill Flow | 3 | Expert | Per-feature flow multiplier |
| per_feature_flow.sparse_infill | Sparse Infill Flow | 3 | Expert | Per-feature flow multiplier |
| per_feature_flow.support | Support Flow | 3 | Expert | Per-feature flow multiplier |
| per_feature_flow.support_interface | Support Interface Flow | 3 | Expert | Per-feature flow multiplier |
| per_feature_flow.bridge | Bridge Flow | 3 | Expert | Per-feature flow multiplier |
| per_feature_flow.gap_fill | Gap Fill Flow | 3 | Expert | Per-feature flow multiplier |
| per_feature_flow.skirt | Skirt Flow | 4 | Hidden | Rarely adjusted |
| per_feature_flow.brim | Brim Flow | 4 | Hidden | Rarely adjusted |
| per_feature_flow.variable_width_perimeter | Variable Width Perimeter Flow | 4 | Hidden | Arachne-specific flow |
| per_feature_flow.ironing | Ironing Flow | 4 | Hidden | Ironing pass flow |
| per_feature_flow.purge_tower | Purge Tower Flow | 4 | Hidden | MMU purge tower flow |

## Fuzzy Skin (Advanced)

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| fuzzy_skin.enabled | Fuzzy Skin Enabled | 3 | Expert | Textured surface toggle |
| fuzzy_skin.thickness | Fuzzy Skin Thickness | 3 | Expert | Random displacement amplitude |
| fuzzy_skin.point_distance | Fuzzy Skin Point Distance | 3 | Expert | Displacement point spacing |

## Input Shaping (Advanced)

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| input_shaping.accel_to_decel_enable | Accel To Decel Enable | 3 | Expert | Input shaping toggle |
| input_shaping.accel_to_decel_factor | Accel To Decel Factor | 3 | Expert | Input shaping ratio |

## Dimensional Compensation (Advanced)

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| dimensional_compensation.xy_hole_compensation | XY Hole Compensation | 3 | Expert | Hole dimensional offset |
| dimensional_compensation.xy_contour_compensation | XY Contour Compensation | 3 | Expert | Contour dimensional offset |
| dimensional_compensation.elephant_foot_compensation | Elephant Foot Compensation | 2 | Advanced | First layer inward offset |

## Tool Change Retraction

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| multi_material.tool_change_retraction.retraction_distance_when_cut | Retraction Distance When Cut | 4 | Hidden | Tool change retraction distance |
| multi_material.tool_change_retraction.long_retraction_when_cut | Long Retraction When Cut | 4 | Hidden | Long retraction toggle |

## Multi-Material

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| multi_material.enabled | Multi-Material Enabled | 3 | Expert | MMU toggle |
| multi_material.tool_count | Tool Count | 3 | Expert | Number of tools |
| multi_material.tools | Per-Tool Config | 4 | Hidden | Per-tool temp/retraction array |
| multi_material.purge_tower_position | Purge Tower Position | 3 | Expert | Tower XY placement |
| multi_material.purge_tower_width | Purge Tower Width | 3 | Expert | Tower width |
| multi_material.purge_volume | Purge Volume | 3 | Expert | Volume per tool change |
| multi_material.wipe_length | Wipe Length | 3 | Expert | Wipe across tower length |
| multi_material.wall_filament | Wall Filament | 3 | Expert | Filament index for walls |
| multi_material.solid_infill_filament | Solid Infill Filament | 3 | Expert | Filament index for solid infill |
| multi_material.support_filament | Support Filament | 3 | Expert | Filament index for support |
| multi_material.support_interface_filament | Support Interface Filament | 3 | Expert | Filament index for support interface |
| multi_material.wipe_tower_rotation_angle | Wipe Tower Rotation Angle | 4 | Hidden | Tower rotation angle |
| multi_material.wipe_tower_bridging | Wipe Tower Bridging | 4 | Hidden | Tower bridging flow |
| multi_material.wipe_tower_cone_angle | Wipe Tower Cone Angle | 4 | Hidden | Tapered tower cone angle |
| multi_material.wipe_tower_no_sparse_layers | Wipe Tower No Sparse Layers | 4 | Hidden | Skip empty tower layers |
| multi_material.single_extruder_mmu | Single Extruder MMU | 4 | Hidden | Single-extruder MMU mode |
| multi_material.flush_into_infill | Flush Into Infill | 3 | Expert | Purge into infill |
| multi_material.flush_into_objects | Flush Into Objects | 3 | Expert | Purge into objects |
| multi_material.flush_into_support | Flush Into Support | 3 | Expert | Purge into support |
| multi_material.purge_in_prime_tower | Purge In Prime Tower | 3 | Expert | Use prime tower for purging |

## Per-Tool Config (multi_material.tools[])

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| multi_material.tools[].nozzle_temp | Tool Nozzle Temperature | 4 | Hidden | Per-tool temperature |
| multi_material.tools[].retract_length | Tool Retract Length | 4 | Hidden | Per-tool retraction |
| multi_material.tools[].retract_speed | Tool Retract Speed | 4 | Hidden | Per-tool retraction speed |

## Sequential Printing (Advanced)

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| sequential.enabled | Sequential Printing | 3 | Expert | Object-by-object mode toggle |
| sequential.extruder_clearance_radius | Extruder Clearance Radius | 3 | Expert | Collision avoidance radius |
| sequential.extruder_clearance_height | Extruder Clearance Height | 3 | Expert | Collision avoidance height |
| sequential.gantry_width | Gantry Width | 4 | Hidden | Rectangular clearance model |
| sequential.gantry_depth | Gantry Depth | 4 | Hidden | Rectangular clearance model |
| sequential.extruder_clearance_polygon | Extruder Clearance Polygon | 4 | Hidden | Custom clearance polygon |

## Post-Processing

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| post_process.enabled | Post-Processing Enabled | 3 | Expert | Post-processing pipeline toggle |
| post_process.pause_at_layers | Pause At Layers | 3 | Expert | Layer pause insertion |
| post_process.pause_command | Pause Command | 4 | Hidden | Pause G-code command |
| post_process.plugin_order | Plugin Order | 4 | Hidden | Post-processor execution order |
| post_process.scripts | Post-Process Scripts | 4 | Hidden | External script paths |
| post_process.gcode_label_objects | G-code Label Objects | 3 | Expert | Exclude Object labels |
| post_process.gcode_comments | G-code Comments | 4 | Hidden | Include comments in G-code |
| post_process.gcode_add_line_number | G-code Line Numbers | 4 | Hidden | Add line numbers to G-code |
| post_process.filename_format | Filename Format | 4 | Hidden | Output filename template |

## Timelapse

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| post_process.timelapse.enabled | Timelapse Enabled | 3 | Expert | Camera timelapse toggle |
| post_process.timelapse.park_x | Timelapse Park X | 3 | Expert | Camera park X position |
| post_process.timelapse.park_y | Timelapse Park Y | 3 | Expert | Camera park Y position |
| post_process.timelapse.dwell_ms | Timelapse Dwell Time | 3 | Expert | Camera dwell milliseconds |
| post_process.timelapse.retract_distance | Timelapse Retract Distance | 4 | Hidden | Retraction before park |
| post_process.timelapse.retract_speed | Timelapse Retract Speed | 4 | Hidden | Retraction speed before park |

## Calibration

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| pressure_advance | Pressure Advance | 2 | Advanced | Common firmware tuning value |
| acceleration_enabled | Acceleration Commands Enabled | 3 | Expert | Emit acceleration commands |

## PA Calibration Pattern

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| pa_calibration.pa_start | PA Start Value | 4 | Hidden | Calibration pattern parameter |
| pa_calibration.pa_end | PA End Value | 4 | Hidden | Calibration pattern parameter |
| pa_calibration.pa_step | PA Step Increment | 4 | Hidden | Calibration pattern parameter |
| pa_calibration.slow_speed | PA Slow Speed | 4 | Hidden | Calibration pattern parameter |
| pa_calibration.fast_speed | PA Fast Speed | 4 | Hidden | Calibration pattern parameter |
| pa_calibration.line_width | PA Line Width | 4 | Hidden | Calibration pattern parameter |
| pa_calibration.layer_height | PA Layer Height | 4 | Hidden | Calibration pattern parameter |
| pa_calibration.bed_center_x | PA Bed Center X | 4 | Hidden | Calibration pattern parameter |
| pa_calibration.bed_center_y | PA Bed Center Y | 4 | Hidden | Calibration pattern parameter |
| pa_calibration.pattern_width | PA Pattern Width | 4 | Hidden | Calibration pattern parameter |
| pa_calibration.nozzle_temp | PA Nozzle Temperature | 4 | Hidden | Calibration pattern parameter |
| pa_calibration.bed_temp | PA Bed Temperature | 4 | Hidden | Calibration pattern parameter |
| pa_calibration.filament_diameter | PA Filament Diameter | 4 | Hidden | Calibration pattern parameter |

## G-code & Miscellaneous

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| gcode_dialect | G-code Dialect | 2 | Advanced | Firmware dialect selection |
| arc_fitting_enabled | Arc Fitting Enabled | 3 | Expert | G2/G3 arc conversion toggle |
| arc_fitting_tolerance | Arc Fitting Tolerance | 3 | Expert | Arc fitting deviation limit |
| arc_fitting_min_points | Arc Fitting Min Points | 4 | Hidden | Minimum points for arc detection |
| max_travel_detour_length | Max Travel Detour Length | 4 | Hidden | Travel optimization limit |
| parallel_slicing | Parallel Slicing | 4 | Hidden | Multi-threaded processing toggle |
| thread_count | Thread Count | 4 | Hidden | Number of processing threads |
| plugin_dir | Plugin Directory | 4 | Hidden | Plugin scan directory |
| passthrough | Passthrough Fields | 4 | Hidden | Upstream profile passthrough map |

## Custom G-code Hooks

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| custom_gcode.before_layer_change | Before Layer Change G-code | 3 | Expert | Custom G-code injection |
| custom_gcode.before_layer_change_original | Before Layer Change Original | 4 | Hidden | Upstream verbatim G-code |
| custom_gcode.after_layer_change | After Layer Change G-code | 3 | Expert | Custom G-code injection |
| custom_gcode.after_layer_change_original | After Layer Change Original | 4 | Hidden | Upstream verbatim G-code |
| custom_gcode.tool_change_gcode | Tool Change G-code | 3 | Expert | Tool change injection |
| custom_gcode.tool_change_gcode_original | Tool Change G-code Original | 4 | Hidden | Upstream verbatim G-code |
| custom_gcode.before_every_layer | Before Every Layer G-code | 4 | Hidden | Alias for before_layer_change |
| custom_gcode.custom_gcode_per_z | Custom G-code Per Z | 3 | Expert | Z-height triggered injection |

## Post-Process Rules (Compound)

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| post_process.fan_overrides | Fan Override Rules | 4 | Hidden | Layer-range fan overrides |
| post_process.custom_gcode | Custom G-code Rules | 4 | Hidden | Trigger-based G-code injection |

## Setting Overrides (Per-Region)

| Field Key | Display Name | Tier | OrcaSlicer Tab | Rationale |
|-----------|-------------|------|----------------|-----------|
| overrides.infill_density | Override Infill Density | 3 | Expert | Modifier mesh infill density |
| overrides.infill_pattern | Override Infill Pattern | 3 | Expert | Modifier mesh infill pattern |
| overrides.wall_count | Override Wall Count | 3 | Expert | Modifier mesh wall count |
| overrides.perimeter_speed | Override Perimeter Speed | 3 | Expert | Modifier mesh perimeter speed |
| overrides.infill_speed | Override Infill Speed | 3 | Expert | Modifier mesh infill speed |
| overrides.top_solid_layers | Override Top Solid Layers | 3 | Expert | Modifier mesh top layers |
| overrides.bottom_solid_layers | Override Bottom Solid Layers | 3 | Expert | Modifier mesh bottom layers |

---

## Summary Counts

| Tier | Name         | Count | Target |
|------|--------------|-------|--------|
| 0    | AI Auto      | 0     | 0      |
| 1    | Simple       | 14    | ~15    |
| 2    | Intermediate | 54    | ~60    |
| 3    | Advanced     | 202   | ~200   |
| 4    | Developer    | 115   | Rest   |
| **Total** |         | **385** |      |

### Tier 1 Fields (14)

1. `layer_height` - Layer Height
2. `first_layer_height` - First Layer Height
3. `wall_count` - Wall Loops
4. `top_solid_layers` - Top Shell Layers
5. `bottom_solid_layers` - Bottom Shell Layers
6. `infill_pattern` - Infill Pattern
7. `infill_density` - Infill Density
8. `speeds.perimeter` - Perimeter Speed
9. `speeds.infill` - Infill Speed
10. `support.enabled` - Enable Support
11. `brim_width` - Brim Width
12. `filament.filament_type` - Filament Type
13. `filament.nozzle_temperatures` - Nozzle Temperature
14. `filament.bed_temperatures` - Bed Temperature
