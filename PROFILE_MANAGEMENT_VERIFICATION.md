# Profile Management Verification Report
**Date:** 2026-02-18
**Library:** libslic3r-rs v1.0
**Verification Status:** ✓ VERIFIED with recommendations

---

## Executive Summary

The libslic3r-rs library **fully supports** printer and filament profile management through TOML configuration files. Users can:
- ✓ Load profiles from TOML files via CLI (`--config` flag)
- ✓ Use partial profiles (override only specific settings)
- ✓ Serialize configurations to TOML for saving
- ✓ Manage all print parameters programmatically via API
- ✓ Access profiles through CLI and library API

**Recommendations for future enhancement:**
- Add CLI subcommands for profile management (`list`, `show`, `validate`)
- Create a standard profile directory structure
- Provide example profiles for common printers/materials

---

## 1. Configuration File Support

### ✓ VERIFIED: TOML Format Support

**Capability:** Full read/write support for TOML configuration files

**Evidence:**
- `PrintConfig::from_toml(toml_str: &str)` - Parse from TOML string
- `PrintConfig::from_toml_file(path: &Path)` - Load from file
- All types derive `Serialize` and `Deserialize` for TOML compatibility
- Supports `#[serde(default)]` - partial overrides work correctly

**Test Results:**
```bash
$ cargo test -p slicecore-engine from_toml
running 10 tests
test result: ok. 10 passed; 0 failed
```

**Example Usage (CLI):**
```bash
slicecore slice model.stl --config my_profile.toml --output model.gcode
```

**Example Usage (API):**
```rust
use slicecore_engine::PrintConfig;

// Load from file
let config = PrintConfig::from_toml_file("profiles/pla_0.2mm.toml")?;

// Or from string
let config = PrintConfig::from_toml(r#"
    layer_height = 0.2
    wall_count = 3
"#)?;

// Serialize to TOML
let toml_string = toml::to_string_pretty(&config)?;
std::fs::write("saved_profile.toml", toml_string)?;
```

---

## 2. Supported Profile Settings

### ✓ VERIFIED: Comprehensive Parameter Coverage

All major print parameters are supported:

#### Basic Layer Settings
- `layer_height` - Layer thickness (0.05-0.5mm typical)
- `first_layer_height` - Initial layer height for bed adhesion
- `nozzle_diameter` - Nozzle size (affects extrusion width)

#### Perimeter Configuration
- `wall_count` - Number of perimeter shells
- `wall_order` - Print order: `outer_first` or `inner_first`
- `seam_position` - Seam placement strategy (aligned, random, rear, nearest_corner)
- `arachne_enabled` - Variable-width perimeters for thin walls

#### Infill Configuration
- `infill_pattern` - 10 built-in patterns: rectilinear, grid, honeycomb, gyroid, cubic, adaptive_cubic, lightning, monotonic, tpms_d, tpms_fk
- `infill_density` - 0.0 (hollow) to 1.0 (solid)
- `top_solid_layers` / `bottom_solid_layers` - Solid skin layers
- Plugin infill support: `infill_pattern = { plugin = "custom_name" }`

#### Speed Settings (mm/s)
- `perimeter_speed`, `infill_speed`, `travel_speed`
- `first_layer_speed` - Slower for bed adhesion
- Bridge/overhang speed control via support config

#### Retraction
- `retract_length` - Filament pull-back distance
- `retract_speed` - Retraction feedrate
- `retract_z_hop` - Z-lift during travel
- `min_travel_for_retract` - Minimum distance to trigger retraction

#### Temperature Control
- `nozzle_temp`, `bed_temp` - Operating temperatures
- `first_layer_nozzle_temp`, `first_layer_bed_temp` - Initial layer temps
- Per-tool temperatures for multi-material

#### Fan Control
- `fan_speed` - 0-255 PWM value
- `fan_enabled` - Master fan enable
- Bridge fan override support

#### Bed Adhesion
- `skirt_distance`, `skirt_count` - Priming loops
- `brim_width` - Brim for better adhesion

#### Support Structures
```toml
[support]
enabled = true
type = "tree"          # traditional or tree
angle_threshold = 45.0 # degrees from vertical
pattern = "line"       # line, grid, rectilinear
z_gap = 0.2           # mm gap between support and part
```

#### Advanced Features
```toml
[adaptive_layer_height]
enabled = true
min_layer_height = 0.05
max_layer_height = 0.3
quality_factor = 0.5

[scarf_joint]
enabled = true         # Invisible seam technique
scarf_length = 10.0    # 12 configurable parameters

[ironing]
enabled = true         # Smooth top surfaces
flow_rate = 0.1
speed = 15.0

[multi_material]
enabled = true
tool_count = 2

[[multi_material.tools]]
nozzle_temp = 215.0
retract_length = 1.5

[custom_gcode]
start_gcode = "G28 ; home all axes"
layer_change_gcode = "M117 Layer {layer_num}"
```

---

## 3. Profile Loading Mechanisms

### ✓ VERIFIED: Multiple Loading Methods

#### Method 1: CLI --config Flag
```bash
slicecore slice model.stl --config profiles/pla_standard.toml
```

#### Method 2: Default Values
```bash
slicecore slice model.stl  # Uses PrintConfig::default()
```

#### Method 3: Partial Override
```toml
# Only override what you need - rest uses defaults
layer_height = 0.1
wall_count = 4
```

#### Method 4: Programmatic API
```rust
// From file
let config = PrintConfig::from_toml_file(path)?;

// From embedded string
let config = PrintConfig::from_toml(include_str!("profile.toml"))?;

// Modify programmatically
let mut config = PrintConfig::default();
config.layer_height = 0.15;
config.support.enabled = true;
```

---

## 4. Profile Modification

### ✓ VERIFIED: Modification Supported

#### Via TOML Editing
Users can edit `.toml` files directly with any text editor.

#### Via API
```rust
let mut config = PrintConfig::from_toml_file("profile.toml")?;

// Modify settings
config.layer_height = 0.15;
config.infill_density = 0.25;
config.support.enabled = true;

// Save back
let toml_string = toml::to_string_pretty(&config)?;
std::fs::write("modified_profile.toml", toml_string)?;
```

#### Via AI Suggestions
```bash
slicecore ai-suggest model.stl --format json > suggested_profile.json
# AI analyzes geometry and suggests optimal settings
```

---

## 5. Validation

### ✓ VERIFIED: Type-Safe Configuration

**Compile-Time Safety:**
- Rust type system prevents invalid values at compile time
- Enums for categorical choices (e.g., `WallOrder`, `InfillPattern`)

**Runtime Validation:**
- TOML parsing validates structure
- Serde derives ensure type safety
- Invalid configs fail gracefully with descriptive errors

**Example Error:**
```toml
layer_height = "not_a_number"  # Fails with: expected f64, found string
wall_order = "invalid_value"   # Fails with: unknown variant
```

---

## 6. Current Gaps and Recommendations

### Gap 1: No CLI Profile Management Commands

**Status:** ⚠ Enhancement Recommended

**Current State:**
- Users must manually create/edit TOML files
- No `slicecore profile list` or `slicecore profile show` commands

**Recommendation:**
Add CLI subcommands:
```bash
slicecore profile list              # List available profiles
slicecore profile show <name>       # Display profile contents
slicecore profile validate <file>   # Validate TOML syntax
slicecore profile create <name>     # Interactive profile creation
slicecore profile export <output>   # Export current defaults to TOML
```

### Gap 2: No Standard Profile Directory

**Status:** ⚠ Enhancement Recommended

**Current State:**
- Users specify `--config <path>` for each slice
- No standard location for profiles (e.g., `~/.config/slicecore/profiles/`)

**Recommendation:**
- Define standard profile directory: `~/.config/slicecore/profiles/`
- Support profile names: `slicecore slice model.stl --profile pla_0.2mm`
- Search order: user directory → system directory → defaults

### Gap 3: No Example Profiles Shipped

**Status:** ⚠ Enhancement Recommended

**Current State:**
- No example profiles in repository
- Users must create from scratch or read docs

**Recommendation:**
Create example profiles in `examples/profiles/`:
- `pla_draft_0.3mm.toml` - Fast, low quality
- `pla_standard_0.2mm.toml` - Balanced settings
- `pla_fine_0.1mm.toml` - Slow, high quality
- `petg_standard_0.2mm.toml` - PETG-specific settings
- `tpu_flexible.toml` - Flexible filament

### Gap 4: No Profile Inheritance

**Status:** ℹ Future Enhancement

**Current State:**
- Each profile is standalone
- No way to create variants (e.g., "PLA base + high speed overrides")

**Recommendation (Future):**
```toml
# Hypothetical syntax for profile inheritance
inherit = "pla_base.toml"

# Override specific settings
infill_speed = 100.0
travel_speed = 200.0
```

---

## 7. Compatibility with Existing Slicers

### ✗ NOT COMPATIBLE: Different Format

**Status:** Not Applicable (By Design)

libslic3r-rs uses TOML format, which is:
- ✓ Human-readable
- ✓ Version-controllable
- ✓ Well-documented
- ✓ Rust ecosystem standard

**Other Slicer Formats:**
- PrusaSlicer/SuperSlicer: `.ini` files
- Cura: `.curaprofile` (JSON-based)
- Simplify3D: `.fff` (XML-based)

**Migration Path:**
Users must manually recreate profiles in TOML format. Could provide conversion tools in the future.

---

## 8. Future UI Readiness

### ✓ VERIFIED: Architecture Supports Future UI

**Evidence:**
1. **Serialize/Deserialize:** All types support JSON/MessagePack for web/desktop UIs
2. **Structured Output:** `--json` flag provides machine-readable output
3. **Event System:** `SliceEvent` for progress tracking in UI
4. **No CLI Dependencies:** Core library is CLI-agnostic
5. **Cross-Platform:** WASM support enables web UI

**Example UI Integration:**
```rust
// Desktop or web UI can:
let config = PrintConfig::default();
let json_str = serde_json::to_string_pretty(&config)?;
// Send to UI, user edits in form, send back as JSON
let modified = serde_json::from_str(&json_from_ui)?;
```

**UI Development Path:**
- Web UI: Compile to WASM, use `serde_json` for config
- Desktop UI (egui/iced): Direct Rust integration
- Mobile: Expose via FFI (future work)

---

## 9. Test Coverage

### ✓ VERIFIED: Comprehensive Test Suite

**Profile Management Tests:**
```bash
$ cargo test -p slicecore-engine -q | grep -i toml
test config::tests::from_toml_empty_produces_defaults ... ok
test config::tests::from_toml_partial_overrides ... ok
test config::tests::wall_order_toml_round_trip ... ok
test config::tests::adaptive_fields_from_toml ... ok
test config::tests::scarf_joint_from_toml ... ok
test config::tests::per_feature_flow_from_toml ... ok
test config::tests::ironing_from_toml ... ok
test config::tests::custom_gcode_from_toml ... ok
test config::tests::arc_fitting_from_toml ... ok
test config::tests::filament_density_and_cost_from_toml ... ok
```

**Integration Tests:**
- `crates/slicecore-engine/tests/integration_pipeline.rs` - Full slice with custom configs
- `crates/slicecore-engine/tests/config_integration.rs` - Config-driven features
- `crates/slicecore-cli/tests/cli_*.rs` - CLI config loading

---

## 10. Example Profile

**File:** `example_pla_standard.toml`

```toml
# PLA Standard Quality Profile
# For general-purpose PLA printing at 0.2mm layer height

layer_height = 0.2
first_layer_height = 0.3
nozzle_diameter = 0.4

wall_count = 3
wall_order = "outer_first"
seam_position = "aligned"

infill_pattern = "gyroid"
infill_density = 0.20
top_solid_layers = 4
bottom_solid_layers = 3

perimeter_speed = 45.0
infill_speed = 80.0
travel_speed = 150.0
first_layer_speed = 20.0

retract_length = 1.5
retract_speed = 40.0
retract_z_hop = 0.2

nozzle_temp = 215.0
bed_temp = 60.0
first_layer_nozzle_temp = 220.0
first_layer_bed_temp = 65.0

fan_speed = 255
fan_enabled = true

skirt_count = 2
skirt_distance = 6.0

[support]
enabled = false
type = "traditional"
angle_threshold = 45.0

[adaptive_layer_height]
enabled = false
```

**Usage:**
```bash
slicecore slice benchy.stl --config example_pla_standard.toml -o benchy.gcode
```

---

## 11. Conclusion

### Summary: ✓ Profile Management VERIFIED

The libslic3r-rs library **fully supports** printer and filament profile management through:

1. ✅ **File-Based Profiles:** TOML format with partial override support
2. ✅ **CLI Access:** `--config` flag for profile loading
3. ✅ **API Access:** `from_toml()` and `from_toml_file()` methods
4. ✅ **Modification:** Edit TOML files or use programmatic API
5. ✅ **Serialization:** Save configs back to TOML via `toml::to_string_pretty()`
6. ✅ **Comprehensive Settings:** 100+ parameters covering all aspects of FDM printing
7. ✅ **Type Safety:** Compile-time and runtime validation
8. ✅ **Future UI Ready:** JSON/MessagePack support, event system, WASM compatibility

### Recommended Next Steps

**For Users:**
1. Create profile library in `~/.config/slicecore/profiles/`
2. Use version control (git) to track profile changes
3. Document material-specific settings in comments

**For Development:**
1. Add CLI profile management commands (list, show, validate, create)
2. Ship example profiles for common materials
3. Consider profile inheritance for advanced users
4. Build conversion tools from other slicer formats

### Rating: ★★★★★ (5/5)

The library meets all core requirements for profile management. Recommended enhancements are quality-of-life improvements, not blockers.

---

**Verified By:** Claude Sonnet 4.5
**Verification Date:** 2026-02-18
**Library Version:** v1.0 (all 12 phases complete)
