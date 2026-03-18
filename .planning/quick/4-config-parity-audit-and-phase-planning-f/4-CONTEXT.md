# Quick Task 4: Config Parity Audit and Phase Planning - Context

**Gathered:** 2026-03-13
**Status:** Ready for planning

<domain>
## Task Boundary

Audit the current PrintConfig against OrcaSlicer/BambuStudio/PrusaSlicer to identify all missing fields and features. Produce a gap analysis document and recommend phases for systematic closure. Also recommend a phase for the ConfigSchema system from the PRD.

</domain>

<decisions>
## Implementation Decisions

### Feature Parity Strategy
- Systematic parity with OrcaSlicer/BambuStudio/PrusaSlicer FIRST
- Then innovate with features existing slicers don't have (AI, streaming, plugins)
- SaaS-specific features added later when that project starts

### Config Schema System
- Build the SettingDefinition schema system from PRD Section 7
- This enables: auto-generated docs, JSON Schema, UI forms, validation, setting tiers
- Should be its own phase

### Repository Strategy
- libslic3r-rs stays as library + CLI (monorepo)
- SaaS server, browser UI, slicer plugins are separate projects
- CLI is both developer tool and SaaS backend executor

### Current State
- ~255 total config fields across all sub-structs
- ~145 actively used in pipeline
- 217 upstream field mappings (OrcaSlicer/Bambu JSON → PrintConfig)
- ~8-12% gap vs OrcaSlicer feature set
- Missing: xy offset, chamber temp, vibration compensation, cooling fan curves, etc.

### Claude's Discretion
- Audit methodology: compare against OrcaSlicer source config definitions
- Gap categorization: group by priority (P0 critical / P1 important / P2 nice-to-have)
- Phase sizing: recommend appropriately-sized phases for gap closure

</decisions>

<specifics>
## Specific Ideas

- Reference OrcaSlicer's `PrintConfig` and `FullPrintConfig` definitions for field-by-field comparison
- Cross-reference with PrusaSlicer for fields unique to that fork
- Check BambuStudio-specific fields (AMS, chamber temp, etc.)
- The deliverable is a structured audit document + phase recommendations
- Phases needed: config gap closure, ConfigSchema system, profile composition CLI (already Phase 30)

</specifics>
