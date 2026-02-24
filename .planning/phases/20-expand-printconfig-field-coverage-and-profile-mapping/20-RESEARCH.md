# Phase 20: Expand PrintConfig Field Coverage and Profile Mapping - Research

**Researched:** 2026-02-24
**Domain:** Slicer configuration data model expansion and upstream profile field mapping
**Confidence:** HIGH

## Summary

Phase 20 expands PrintConfig from ~55 flat fields to ~150+ fields organized into nested sub-configs, and expands the JSON/INI profile mappers from ~43 mapped fields to 100+ mapped fields covering process, machine, and filament upstream sources. The existing codebase has clean separation between config (`config.rs`), JSON mapper (`profile_import.rs`), INI mapper (`profile_import_ini.rs`), TOML converter (`profile_convert.rs`), and batch converter (`profile_library.rs`). All profile data flows through the same `ImportResult` -> `convert_to_toml()` pipeline.

Analysis of the BambuStudio BBL X1C inheritance chain reveals 180 unique process fields, 121 unique filament fields, and 103 unique machine fields. PrusaSlicer has a similar scale: ~188 print fields, ~82 filament fields, ~85 printer fields. Currently the JSON mapper handles 24 process fields, 10 filament fields, and 9 machine fields (43 total). The INI mapper handles roughly the same count with PrusaSlicer-specific key names. The target is to map every field that has a reasonable PrintConfig representation -- the user decision specifies "map EVERYTHING possible" with passthrough storage for fields that have no engine equivalent.

**Primary recommendation:** Organize new fields into nested sub-config structs (LineWidths, SpeedConfig, CoolingConfig, RetractionConfig, MachineConfig, AccelerationConfig, etc.), expand both mappers in parallel, then regenerate all ~21k profiles. The existing `extract_string_value` / `parse_and_set_f64` helpers and the `serde(default)` pattern make this straightforward -- the main effort is enumeration of fields, not architectural change.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Map EVERYTHING possible -- not just output-affecting fields. Every field that has a reasonable PrintConfig representation gets mapped.
- Fields with no direct engine equivalent (AMS drying, timelapse_gcode, scan_first_layer) are stored as passthrough in PrintConfig. They round-trip through profiles and are available for G-code start/end templates. Future-proofs the config.
- PrusaSlicer INI mapper gets the same full treatment -- all three sources (OrcaSlicer, BambuStudio, PrusaSlicer) map every possible field, including source-unique fields.
- Multi-extruder array fields (nozzle_diameter, jerk, temperatures) stored as full Vec<f64> arrays, not just first value. This is a change from current behavior.
- Organize new fields into nested sub-configs: LineWidths, SpeedConfig, CoolingConfig, RetractionConfig, MachineConfig, etc.
- Keeps PrintConfig manageable as it grows to 150+ fields.
- Existing flat fields should be migrated into sub-configs where it makes sense (breaking change to config format is acceptable).
- After expanding mappers, regenerate ALL ~21k profiles (full re-conversion).
- Clean slate ensures every profile benefits from expanded mapping.
- This includes orcaslicer, bambustudio, prusaslicer, and crealityprint sources.

### Claude's Discretion
- Default values per-field: Claude picks the most sensible default for each field (BambuStudio defaults where comparing, slicer-agnostic industry standards otherwise).
- Exact sub-config groupings and naming.
- Migration strategy for existing flat fields into nested sub-configs.

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1.x | Serialize/Deserialize derive for all new sub-config structs | Already in use, `#[serde(default)]` pattern |
| toml | 0.8.x | TOML serialization for profile output | Already in use for `convert_to_toml` |
| serde_json | 1.x | JSON parsing for upstream profiles | Already in use for `import_upstream_profile` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| walkdir | 2.x | Directory traversal for batch re-conversion | Already in use in `profile_library.rs` |

### Alternatives Considered
No new libraries needed. All work is data model expansion within the existing stack.

## Architecture Patterns

### Recommended Sub-Config Structure

```
PrintConfig {
    // Existing simple fields (layer_height, first_layer_height, wall_count, etc.)

    // NEW nested sub-configs:
    line_widths: LineWidthConfig,       // outer_wall, inner_wall, infill, top_surface, initial_layer, support
    speeds: SpeedConfig,                // bridge, inner_wall, gap_fill, top_surface, initial_layer_infill,
                                        // internal_solid_infill, support, support_interface, small_perimeter,
                                        // solid_infill, overhang speeds (4 tiers + totally)
    cooling: CoolingConfig,             // fan_max_speed, fan_min_speed, slow_down_layer_time, slow_down_min_speed,
                                        // overhang_fan_speed, overhang_fan_threshold, full_fan_speed_layer,
                                        // slow_down_for_layer_cooling
    retraction: RetractionConfig,       // (migrate existing retract_* fields here) + deretraction_speed,
                                        // retract_before_wipe, retract_when_changing_layer, wipe, wipe_distance
    machine: MachineConfig,             // printable_area, printable_height, max_acceleration_{x,y,z,e,extruding,retracting,travel},
                                        // max_speed_{x,y,z,e}, start_gcode, end_gcode, layer_change_gcode,
                                        // nozzle_type, printer_model, bed_shape
    acceleration: AccelerationConfig,   // (migrate existing print_acceleration, travel_acceleration here) +
                                        // outer_wall_acceleration, inner_wall_acceleration, initial_layer_acceleration,
                                        // initial_layer_travel_acceleration, top_surface_acceleration,
                                        // sparse_infill_acceleration, bridge_acceleration
    filament_props: FilamentPropsConfig,// filament_type, filament_vendor, max_volumetric_speed,
                                        // nozzle_temperature_range_{low,high}, filament_start_gcode, filament_end_gcode,
                                        // filament_retraction_length (override), filament_retraction_speed (override)
    passthrough: HashMap<String, String>,// ALL unmapped fields stored verbatim for round-trip and template access
}
```

### Pattern 1: Nested Sub-Config with serde(default)

**What:** Each sub-config struct derives `Debug, Clone, Serialize, Deserialize` with `#[serde(default)]` and a custom `Default` impl.
**When to use:** For every new config group.
**Example:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LineWidthConfig {
    /// Outer wall line width in mm (0 = auto from nozzle_diameter).
    pub outer_wall: f64,
    /// Inner wall line width in mm.
    pub inner_wall: f64,
    /// Sparse infill line width in mm.
    pub infill: f64,
    /// Top surface line width in mm.
    pub top_surface: f64,
    /// Initial layer line width in mm.
    pub initial_layer: f64,
    /// Internal solid infill line width in mm.
    pub internal_solid_infill: f64,
    /// Support line width in mm.
    pub support: f64,
}

impl Default for LineWidthConfig {
    fn default() -> Self {
        Self {
            outer_wall: 0.42,
            inner_wall: 0.45,
            infill: 0.45,
            top_surface: 0.42,
            initial_layer: 0.5,
            internal_solid_infill: 0.42,
            support: 0.42,
        }
    }
}
```

### Pattern 2: Vec<f64> for Multi-Extruder Array Fields

**What:** Fields that upstream stores as JSON arrays (nozzle_diameter, jerk, temperatures) become `Vec<f64>` in PrintConfig, replacing the current single `f64`.
**When to use:** For all fields documented as multi-extruder in upstream profiles.
**Critical consideration:** This is a **breaking change** to the TOML serialization format. Current TOML has `nozzle_diameter = 0.4`. New TOML will have `nozzle_diameter = [0.4]`. The `extract_string_value` helper currently extracts only `arr[0]` for the JSON mapper. A new `extract_array_values` helper is needed.

**Migration approach:**
- Keep the primary scalar field (e.g., `nozzle_diameter: f64`) as the main field for engine use.
- Add a `nozzle_diameters: Vec<f64>` (plural) for the full array storage.
- OR: Switch to `Vec<f64>` and add a `nozzle_diameter()` convenience method returning `self.nozzle_diameters[0]`.
- The second approach is cleaner for new code but breaks all 125+ call sites that reference `config.nozzle_diameter`. A wrapper method minimizes the blast radius.

**Recommended approach:** Store as `Vec<f64>` but with `#[serde(default)]` defaulting to a single-element vec. Add accessor methods `fn nozzle_diameter(&self) -> f64` that returns `self.nozzle_diameter[0]`. This keeps the existing engine call sites working with a method call change from field access to method call.

### Pattern 3: Passthrough Storage for Unmapped Fields

**What:** Fields from upstream profiles that have no engine equivalent are stored in a `HashMap<String, String>` passthrough map, preserving them for round-trip serialization and G-code template variable substitution.
**When to use:** For all upstream fields that don't map to a typed PrintConfig field.
**Example:**
```rust
/// Passthrough fields from upstream profiles that have no engine equivalent.
/// Preserved for round-trip fidelity and G-code template variable access.
#[serde(default, skip_serializing_if = "HashMap::is_empty")]
pub passthrough: HashMap<String, String>,
```

### Pattern 4: Field Mapping Expansion in apply_field_mapping

**What:** The `apply_field_mapping()` function in both `profile_import.rs` (JSON) and `profile_import_ini.rs` (INI) grows from ~40 match arms to ~120+ match arms.
**When to use:** When adding new upstream field mappings.
**Organization tip:** Group match arms by sub-config target (line widths, speeds, cooling, etc.) with section comments matching the sub-config struct names.

### Anti-Patterns to Avoid
- **Flat field explosion:** Do NOT add 100+ new fields directly to PrintConfig without sub-config grouping. The struct is already ~55 fields; adding 100 more flat fields would make it unmaintainable.
- **Accessor explosion:** Do NOT create individual accessor methods for every sub-config field on PrintConfig. Instead, access via `config.speeds.bridge_speed` directly.
- **Default mismatch:** Do NOT use `0.0` as default for speed fields where the upstream slicer has a meaningful default. Use the BambuStudio common profile defaults as the reference.
- **Parallel data paths:** Do NOT create separate deserialization paths for sub-configs. The `#[serde(default)]` pattern with nested structs handles partial TOML deserialization automatically.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| TOML nested struct serialization | Custom serializer | `serde(default)` on nested structs | TOML crate handles nested tables natively |
| Array field extraction | Custom JSON parser | Extend `extract_string_value` -> `extract_array_f64` | Consistent with existing pattern |
| Profile re-conversion | New conversion tool | Existing `batch_convert_profiles` / `batch_convert_prusaslicer_profiles` | Already handles inheritance, index merge |

**Key insight:** The existing batch conversion pipeline (`profile_library.rs`) already handles inheritance resolution, TOML conversion, and index generation. Re-conversion is simply re-running the existing commands with the expanded mappers.

## Common Pitfalls

### Pitfall 1: Breaking Existing TOML Profiles
**What goes wrong:** Renaming flat fields to nested sub-config paths (e.g., `perimeter_speed` -> `speeds.perimeter`) breaks all existing TOML profiles and test configs.
**Why it happens:** `serde(default)` on the sub-config allows missing sections, but existing files with the old flat key names will have those keys silently ignored.
**How to avoid:** Use `#[serde(alias = "perimeter_speed")]` on the new nested field, OR add a `#[serde(flatten)]` compatibility layer, OR migrate gradually (keep old flat field as deprecated alias). Alternatively, accept the breaking change since CONTEXT.md says "breaking change to config format is acceptable" -- but ensure all tests and existing TOML configs are updated in the same commit.
**Warning signs:** Tests that deserialize TOML strings fail silently (config loads but fields stay at default).

### Pitfall 2: Vec<f64> Serialization Format Change
**What goes wrong:** Changing `nozzle_diameter: f64` to `nozzle_diameter: Vec<f64>` changes TOML format from `nozzle_diameter = 0.4` to `nozzle_diameter = [0.4]`, breaking all 21k converted profiles.
**Why it happens:** TOML arrays have different syntax from scalar values.
**How to avoid:** Since we're regenerating ALL profiles anyway, this is acceptable. But ensure the TOML round-trip tests and all inline test strings are updated. The key risk is forgetting to update test fixtures.
**Warning signs:** `from_toml` tests fail with "expected array, found float" or vice versa.

### Pitfall 3: extract_string_value Only Returns First Element
**What goes wrong:** The current `extract_string_value` function returns `arr[0]` for JSON arrays, which is correct for single-value extraction but loses multi-extruder data.
**Why it happens:** It was designed for the "take first value" approach used in phases 13-18.
**How to avoid:** Add a new `extract_array_f64` / `extract_array_string` helper that returns `Vec<f64>` / `Vec<String>` for array fields, while keeping `extract_string_value` for backward compatibility on scalar fields.
**Warning signs:** Multi-extruder profiles only contain first extruder's values.

### Pitfall 4: Passthrough HashMap Ordering in TOML Output
**What goes wrong:** `HashMap<String, String>` serialized to TOML produces keys in arbitrary order, making TOML diffs noisy between runs.
**Why it happens:** HashMap has no deterministic iteration order.
**How to avoid:** Use `BTreeMap<String, String>` instead, which produces sorted keys in TOML output.
**Warning signs:** Re-running conversion produces different TOML files (same content, different key order).

### Pitfall 5: config.extrusion_width() Blast Radius
**What goes wrong:** The `extrusion_width()` method on PrintConfig returns `nozzle_diameter * 1.1`. If `nozzle_diameter` is moved to a Vec or nested struct, all 22+ call sites break.
**Why it happens:** The method is used throughout the engine (engine.rs, toolpath.rs, perimeter.rs, gap_fill.rs, etc.).
**How to avoid:** Keep `extrusion_width()` as a method on PrintConfig that accesses the appropriate sub-config field. If nozzle_diameter moves, the method signature stays the same -- only its internal implementation changes.
**Warning signs:** Compilation errors across 15+ files after config restructuring.

### Pitfall 6: convert_to_toml Default Comparison Breaks with Nested Structs
**What goes wrong:** `convert_to_toml()` compares the serialized config against `PrintConfig::default()` to filter out default values. With nested sub-configs, the comparison logic (toml::Value equality) works automatically for nested tables -- but only if the nested default values are correctly set.
**Why it happens:** The existing code uses `round_floats_in_value` recursively, which already handles nested tables/arrays. No code change needed here.
**How to avoid:** Verify the round_floats_in_value recursive traversal handles new nested structures -- it already does since it processes `Table` and `Array` variants recursively.
**Warning signs:** Converted TOML profiles include default sub-config sections with all-default values.

## Code Examples

### Adding a New Sub-Config Struct

```rust
// In config.rs:
/// Per-feature speed configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SpeedConfig {
    /// Bridge print speed (mm/s).
    pub bridge: f64,
    /// Inner wall speed (mm/s).
    pub inner_wall: f64,
    /// Gap fill speed (mm/s).
    pub gap_fill: f64,
    /// Top surface speed (mm/s).
    pub top_surface: f64,
    /// Internal solid infill speed (mm/s).
    pub internal_solid_infill: f64,
    /// Initial layer infill speed (mm/s).
    pub initial_layer_infill: f64,
    /// Support structure speed (mm/s).
    pub support: f64,
    /// Support interface speed (mm/s).
    pub support_interface: f64,
    /// Small perimeter speed (mm/s, 0 = percentage of perimeter_speed).
    pub small_perimeter: f64,
    /// Solid infill speed (mm/s).
    pub solid_infill: f64,
}
```

### Expanding JSON Field Mapping

```rust
// In profile_import.rs apply_field_mapping():

// --- Speed sub-config fields ---
"bridge_speed" => parse_and_set_f64(value, &mut config.speeds.bridge),
"inner_wall_speed" => parse_and_set_f64(value, &mut config.speeds.inner_wall),
"gap_infill_speed" => parse_and_set_f64(value, &mut config.speeds.gap_fill),
"top_surface_speed" => parse_and_set_f64(value, &mut config.speeds.top_surface),
"internal_solid_infill_speed" => parse_and_set_f64(value, &mut config.speeds.internal_solid_infill),
"initial_layer_infill_speed" => parse_and_set_f64(value, &mut config.speeds.initial_layer_infill),
"support_speed" => parse_and_set_f64(value, &mut config.speeds.support),
"support_interface_speed" => parse_and_set_f64(value, &mut config.speeds.support_interface),

// --- Line width sub-config fields ---
"outer_wall_line_width" => parse_and_set_f64(value, &mut config.line_widths.outer_wall),
"inner_wall_line_width" => parse_and_set_f64(value, &mut config.line_widths.inner_wall),
"sparse_infill_line_width" => parse_and_set_f64(value, &mut config.line_widths.infill),
"top_surface_line_width" => parse_and_set_f64(value, &mut config.line_widths.top_surface),
"initial_layer_line_width" => parse_and_set_f64(value, &mut config.line_widths.initial_layer),
"internal_solid_infill_line_width" => parse_and_set_f64(value, &mut config.line_widths.internal_solid_infill),

// --- Machine sub-config fields ---
"machine_start_gcode" | "start_gcode" => {
    config.machine.start_gcode = value.to_string();
    true
}
"machine_end_gcode" | "end_gcode" => {
    config.machine.end_gcode = value.to_string();
    true
}
"printable_height" => parse_and_set_f64(value, &mut config.machine.printable_height),
"machine_max_acceleration_x" => parse_and_set_f64(value, &mut config.machine.max_acceleration_x),
```

### Array Field Extraction Helper

```rust
/// Extract all values from a JSON array as f64 Vec.
///
/// Handles:
/// - Array of strings: `["0.4", "0.4"]` -> `vec![0.4, 0.4]`
/// - Array of numbers: `[0.4, 0.4]` -> `vec![0.4, 0.4]`
/// - Single value: `"0.4"` -> `vec![0.4]`
/// - Nil sentinel: `"nil"` or `["nil"]` -> empty vec
fn extract_array_f64(value: &serde_json::Value) -> Vec<f64> {
    match value {
        serde_json::Value::Array(arr) => {
            arr.iter()
                .filter_map(|v| match v {
                    serde_json::Value::String(s) if s != "nil" => s.parse::<f64>().ok(),
                    serde_json::Value::Number(n) => n.as_f64(),
                    _ => None,
                })
                .collect()
        }
        serde_json::Value::String(s) if s != "nil" => {
            s.parse::<f64>().ok().into_iter().collect()
        }
        serde_json::Value::Number(n) => {
            n.as_f64().into_iter().collect()
        }
        _ => Vec::new(),
    }
}
```

### Passthrough Storage Pattern

```rust
// In apply_field_mapping, the default arm stores unmapped fields:
_ => {
    // Store in passthrough for round-trip fidelity.
    config.passthrough.insert(key.to_string(), value.to_string());
    // Return true to mark as "handled" (stored, not unmapped).
    true
}
```

**Note:** This changes the semantics of `unmapped_fields` in ImportResult. With passthrough, ALL fields are "mapped" (either to a typed field or to passthrough). The `unmapped_fields` list becomes empty. Consider renaming to `passthrough_fields` for clarity.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| First-value extraction for arrays | Full Vec<f64> array storage | Phase 20 | Multi-extruder fidelity |
| 43 mapped fields | 100+ mapped fields | Phase 20 | >80% field coverage |
| Flat PrintConfig | Nested sub-configs | Phase 20 | Maintainable at 150+ fields |
| Unmapped fields as comments in TOML | Passthrough HashMap | Phase 20 | Round-trip fidelity |

## Open Questions

1. **extrusion_width() method vs per-feature line widths**
   - What we know: Currently `extrusion_width()` returns `nozzle_diameter * 1.1` and is used in 22+ call sites. The new `LineWidthConfig` provides per-feature line widths.
   - What's unclear: Should existing engine code be updated to use per-feature widths (e.g., `config.line_widths.outer_wall` for perimeter generation), or should that be deferred to a future phase?
   - Recommendation: For Phase 20, keep the existing `extrusion_width()` method working as before. The per-feature line widths are stored for profile fidelity but not consumed by the engine yet. Engine consumption is a separate concern for future phases when per-feature extrusion control is implemented.

2. **Passthrough fields: store everything or just known-useful fields?**
   - What we know: CONTEXT.md says "map EVERYTHING possible" with passthrough for fields with no engine equivalent.
   - What's unclear: Should BambuStudio-specific internal fields (e.g., `filament_dev_ams_drying_temperature`, `counter_coef_1`, `hole_coef_1`) also go into passthrough? They are never useful for G-code templates.
   - Recommendation: Store everything in passthrough. The HashMap overhead is minimal (a few KB per profile), and it's simpler than maintaining an exclusion list. Users can grep passthrough keys if they ever need them.

3. **Exact field count target**
   - What we know: CONTEXT.md targets >80% coverage (from ~13% currently). Process has 180 fields, filament has 121, machine has 103 = 404 total.
   - What's unclear: Whether "80% coverage" means 80% as typed fields or 80% including passthrough.
   - Recommendation: Target 80%+ as typed+passthrough combined. Typed fields should cover all "output-affecting" settings (speeds, widths, temperatures, accelerations, retractions). Device-specific internals (AMS drying, counter_coef, hole_coef) go to passthrough. Estimate: ~100-120 new typed fields + ~200 passthrough = well over 80%.

4. **TOML section naming for sub-configs**
   - What we know: TOML serialization of nested structs produces `[section_name]` headers. Current config has `[scarf_joint]`, `[support]`, `[ironing]`, `[per_feature_flow]`, `[custom_gcode]`.
   - What's unclear: Whether new section names should follow existing style exactly.
   - Recommendation: Use consistent snake_case: `[line_widths]`, `[speeds]`, `[cooling]`, `[retraction]`, `[machine]`, `[acceleration]`, `[filament_props]`. Short but descriptive.

## Existing Codebase Impact Analysis

### Files That Need Modification

| File | Change Type | Scope |
|------|------------|-------|
| `config.rs` | Major: Add ~8 new sub-config structs, migrate existing fields, update Default impl | ~800 new lines |
| `profile_import.rs` | Major: Expand `apply_field_mapping` from ~40 to ~120 arms, add array extraction helper | ~400 new lines |
| `profile_import_ini.rs` | Major: Expand `apply_prusaslicer_field_mapping` similarly | ~350 new lines |
| `profile_convert.rs` | Minor: Verify `round_floats_in_value` handles new nested structures (it already does) | ~20 lines |
| `profile_library.rs` | Minor: No code changes needed -- re-conversion uses existing batch_convert pipelines | 0 lines |
| `engine.rs` | Medium: Update ~26 references to migrated fields (e.g., `config.retract_length` -> `config.retraction.length`) | ~60 changed lines |
| `gcode_gen.rs` | Small: Update references to migrated speed/retraction fields | ~10 changed lines |
| `planner.rs` | Small: Update references to migrated speed fields | ~15 changed lines |
| `toolpath.rs` | Small: Update `extrusion_width()` callers if method moves | ~5 changed lines |
| Tests (multiple) | Medium: Update inline TOML strings and assertions for new field paths | ~100 changed lines |

### Current Field Counts (for reference)

**Currently mapped in JSON mapper (profile_import.rs):**
- Process: layer_height, initial_layer_print_height, wall_loops, sparse_infill_density, sparse_infill_pattern, top_shell_layers, bottom_shell_layers, outer_wall_speed, sparse_infill_speed, travel_speed, initial_layer_speed, skirt_loops, skirt_distance, brim_width, default_acceleration, travel_acceleration, enable_arc_fitting, adaptive_layer_height, ironing_type, ironing_flow, ironing_speed, ironing_spacing, wall_generator, seam_position (24 fields)
- Filament: nozzle_temperature, nozzle_temperature_initial_layer, hot_plate_temp, hot_plate_temp_initial_layer, filament_density, filament_diameter, filament_cost, filament_flow_ratio, close_fan_the_first_x_layers, fan_cooling_layer_time (10 fields)
- Machine: nozzle_diameter, retraction_length, retraction_speed, z_hop, retraction_minimum_travel, gcode_flavor, machine_max_jerk_x, machine_max_jerk_y, machine_max_jerk_z (9 fields)
- **Total: 43 mapped fields**

**Target new typed fields to add (high-impact, output-affecting):**

Process speeds: bridge_speed, inner_wall_speed, gap_infill_speed, top_surface_speed, internal_solid_infill_speed, initial_layer_infill_speed, support_speed, support_interface_speed, small_perimeter_speed, overhang_1_4_speed, overhang_2_4_speed, overhang_3_4_speed, overhang_4_4_speed, overhang_totally_speed, travel_speed_z (~15)

Line widths: line_width, outer_wall_line_width, inner_wall_line_width, sparse_infill_line_width, top_surface_line_width, initial_layer_line_width, internal_solid_infill_line_width, support_line_width (~8)

Accelerations: outer_wall_acceleration, inner_wall_acceleration, initial_layer_acceleration, initial_layer_travel_acceleration, top_surface_acceleration, sparse_infill_acceleration, bridge_acceleration (~7)

Process misc: bridge_flow, elefant_foot_compensation, infill_direction, infill_wall_overlap, spiral_mode, only_one_wall_top, resolution, raft_layers, detect_thin_wall, print_sequence, initial_layer_print_height (~11)

Cooling: fan_max_speed, fan_min_speed, slow_down_layer_time, slow_down_min_speed, overhang_fan_speed, overhang_fan_threshold, slow_down_for_layer_cooling, full_fan_speed_layer (~8)

Machine: printable_area, printable_height, machine_start_gcode, machine_end_gcode, layer_change_gcode, machine_max_acceleration_x, machine_max_acceleration_y, machine_max_acceleration_z, machine_max_acceleration_e, machine_max_acceleration_extruding, machine_max_acceleration_retracting, machine_max_acceleration_travel, machine_max_speed_x, machine_max_speed_y, machine_max_speed_z, machine_max_speed_e, machine_max_jerk_e, deretraction_speed, retract_before_wipe, retract_when_changing_layer, wipe, wipe_distance, printer_model, nozzle_type, min_layer_height, max_layer_height (~26)

Filament: filament_type, filament_vendor, filament_max_volumetric_speed, nozzle_temperature_range_low, nozzle_temperature_range_high, filament_retraction_length, filament_retraction_speed, filament_start_gcode, filament_end_gcode, slow_down_layer_time, slow_down_min_speed (~11)

**Estimated new typed fields: ~86**
**Estimated total typed fields: 43 + 86 = ~129**
**Remaining upstream fields -> passthrough: ~275**

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `config.rs` (1030 lines, 55 flat fields), `profile_import.rs` (43 mapped fields), `profile_import_ini.rs` (30 mapped fields)
- BambuStudio profile data: `resources/profiles/BBL/` (180 process, 121 filament, 103 machine unique fields)
- PrusaSlicer profile data: `resources/profiles/PrusaResearch.ini` (~188 print, ~82 filament, ~85 printer unique fields)

### Secondary (MEDIUM confidence)
- Project CONTEXT.md decisions (user-locked constraints on coverage, structure, re-conversion)
- Existing test suite patterns (16 integration test files in `tests/`)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - No new dependencies, all existing patterns
- Architecture: HIGH - Sub-config pattern already proven by ScarfJointConfig, SupportConfig, IroningConfig, etc.
- Pitfalls: HIGH - Based on direct codebase analysis of call sites and serialization paths

**Research date:** 2026-02-24
**Valid until:** 2026-03-24 (stable domain, no external dependency changes)
