# Phase 20: Expand PrintConfig Field Coverage and Profile Mapping - Context

**Gathered:** 2026-02-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Expand PrintConfig to include every mappable upstream slicer field from OrcaSlicer/BambuStudio JSON and PrusaSlicer INI formats. Update both JSON and INI profile mappers to map everything possible. Regenerate all ~21k profiles with expanded mapping. This enables meaningful apples-to-apples slicer output comparison.

</domain>

<decisions>
## Implementation Decisions

### Field prioritization
- Map EVERYTHING possible — not just output-affecting fields. Every field that has a reasonable PrintConfig representation gets mapped.
- Fields with no direct engine equivalent (AMS drying, timelapse_gcode, scan_first_layer) are stored as passthrough in PrintConfig. They round-trip through profiles and are available for G-code start/end templates. Future-proofs the config.
- PrusaSlicer INI mapper gets the same full treatment — all three sources (OrcaSlicer, BambuStudio, PrusaSlicer) map every possible field, including source-unique fields.
- Multi-extruder array fields (nozzle_diameter, jerk, temperatures) stored as full Vec<f64> arrays, not just first value. This is a change from current behavior.

### PrintConfig structure
- Organize new fields into nested sub-configs: LineWidths, SpeedConfig, CoolingConfig, RetractionConfig, MachineConfig, etc.
- Keeps PrintConfig manageable as it grows to 150+ fields.
- Existing flat fields should be migrated into sub-configs where it makes sense (breaking change to config format is acceptable).

### Profile re-conversion
- After expanding mappers, regenerate ALL ~21k profiles (full re-conversion).
- Clean slate ensures every profile benefits from expanded mapping.
- This includes orcaslicer, bambustudio, prusaslicer, and crealityprint sources.

### Claude's Discretion
- Default values per-field: Claude picks the most sensible default for each field (BambuStudio defaults where comparing, slicer-agnostic industry standards otherwise).
- Exact sub-config groupings and naming.
- Migration strategy for existing flat fields into nested sub-configs.

</decisions>

<specifics>
## Specific Ideas

- The immediate motivation is enabling BambuStudio X1C comparison: 0.20mm Standard process + Generic PLA filament + X1C 0.4mm machine profiles must contain all settings needed for a representative slice.
- Currently only ~13% of upstream fields are mapped (24/180 process, 9/100 machine, 10/109 filament). Target is >80% coverage.
- Vec<f64> for multi-extruder arrays is a significant structural change — affects TOML serialization format and all existing profile consumers.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 20-expand-printconfig-field-coverage-and-profile-mapping*
*Context gathered: 2026-02-24*
