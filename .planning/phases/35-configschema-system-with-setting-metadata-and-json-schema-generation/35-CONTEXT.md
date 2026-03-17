# Phase 35: ConfigSchema System with Setting Metadata and JSON Schema Generation - Context

**Gathered:** 2026-03-17
**Status:** Ready for planning

<domain>
## Phase Boundary

Build a per-field metadata system for all config settings using a proc-macro derive, populate a runtime SettingRegistry, and generate JSON Schema + flat metadata JSON output. Replace ad-hoc validation with schema-driven validation. Deliver a CLI `slicecore schema` command for querying and exporting. Annotate ALL ~400+ fields with tier, description, units, constraints, affects, and category. Engine behavior changes are NOT in scope — this is metadata infrastructure.

</domain>

<decisions>
## Implementation Decisions

### Derive Macro Approach
- Custom `#[derive(SettingSchema)]` proc-macro with `#[setting(...)]` attributes on each field
- New crate: `slicecore-config-derive` (proc-macro crate)
- New crate: `slicecore-config-schema` (runtime types: SettingDefinition, SettingRegistry, ValueType, etc.)
- `slicecore-engine` depends on both
- Explicit `description = "..."` attribute required (NOT auto-extracted from doc comments) — clean separation between Rust docs and user-facing descriptions
- `display_name` auto-generated from field name (snake_case → Title Case), overridable with explicit `display_name = "..."`
- `#[setting(flatten)]` on sub-struct fields auto-prefixes child keys with parent field name (e.g., `speed.perimeter`). Overridable with `#[setting(flatten, prefix = "custom")]`
- Enums also derive `SettingSchema` — each variant gets `#[setting(display = "...", description = "...")]`. Macro generates `ValueType::Enum { variants }` automatically
- `ValueType` inferred from Rust types: f64 → Float, bool → Bool, Vec<f64> → FloatVec, Option<T> → marks field as optional. Override with explicit `value_type` if needed
- Fields WITHOUT `#[setting()]` still get registered with sensible defaults (tier=4 Developer, empty description, inferred type). Compile warning for missing tier/description — allows incremental annotation
- `#[setting(skip)]` excludes fields from schema (for passthrough BTreeMap, internal caches, computed fields)
- Struct-level `#[setting(category = "...")]` sets default category for all fields; individual fields can override

### Tier System
- 5 tiers: 0=AI Auto, 1=Simple (~15), 2=Intermediate (~60), 3=Advanced (~200), 4=Developer (rest)
- Claude assigns all tiers using OrcaSlicer's Simple/Advanced/Expert tab placement as baseline: Simple → Tier 1, Advanced → Tier 2, Expert → Tier 3, Hidden/Debug → Tier 4
- Tier 0 (AI Auto) left empty — populated in AI integration phase
- Reviewable `designDocs/TIER_MAP.md` artifact produced during research/planning, grouped by category and sorted by tier within each category, with rationale column
- User reviews TIER_MAP.md before annotation implementation begins
- `SettingCategory` is a fixed Rust enum (~15 variants): Quality, Speed, Cooling, Retraction, Support, Infill, Adhesion, Advanced, Machine, Filament, etc.
- No strong preferences on specific field tier assignments — trust OrcaSlicer baseline + Claude's judgment

### Constraints & Dependencies
- Constraint types this phase: Range { min, max }, Enum { variants }, DependsOn { key, condition }
- `depends_on` uses simple equality checks: `depends_on = "support.enable"` (bool true), `depends_on = "support.type == Tree"` (enum equality)
- Full `affects` dependency graph populated for ALL fields — not deferred
- `affected_by` auto-generated as the inverse of `affects` — only `affects` is specified in annotations
- `SettingKey` is a newtype around String with dotted path format: `SettingKey("speed.perimeter")`
- Both compile-time AND runtime validation:
  - Compile-time: macro validates min < max, referenced depends_on/affects keys exist
  - Runtime: `SettingRegistry.validate()` checks actual config values against constraints

### Additional Metadata Fields
- `since_version` — semver string indicating when a setting was introduced (e.g., `since_version = "0.1.0"`)
- `deprecated` — optional deprecation message (e.g., `deprecated = "Use retract_length instead"`)
- `tags` — Vec of searchable string tags (e.g., `tags = ["retraction", "quality"]`) for categorization beyond the single category enum

### Output Formats & CLI
- JSON Schema (draft 2020-12) generation via `SettingRegistry.to_json_schema()`
- JSON Schema uses nested structure matching Rust struct hierarchy (with `$ref` to `$defs` for sub-schemas)
- Custom metadata included as `x-` extensions in JSON Schema: `x-tier`, `x-category`, `x-units`, `x-display-name`, `x-affects`, `x-tags`, `x-since-version`, `x-deprecated`
- Flat metadata JSON format: `--format json` outputs array of all SettingDefinitions with full metadata (key, display_name, description, tier, category, value_type, default, min, max, units, affects, depends_on, tags, since_version, deprecated) — for UI form generators and AI consumption
- Runtime generation (not committed to repo) — schema always matches code
- Full `slicecore schema` CLI subcommand:
  - `--format json-schema` — JSON Schema 2020-12 output
  - `--format json` — Flat metadata list
  - `--tier simple|intermediate|advanced|developer` — Filter by tier
  - `--category speed|quality|...` — Filter by category
  - `--search "retract"` — Full-text search across key, display_name, description, and tags
  - Output to stdout for piping

### Search API
- `SettingRegistry.search(query)` — case-insensitive substring matching across display_name, description, key path, and tags
- Returns ranked results
- No external search library — simple built-in matching

### Registry Initialization
- Lazy global singleton via `std::sync::LazyLock` (or `once_cell::sync::Lazy`)
- `SettingRegistry::global()` — zero-setup access from anywhere
- Derive macro-generated code registers definitions on first access

### Validation Upgrade
- Replace Phase 30's ad-hoc config validation with schema-driven validation
- `SettingRegistry.validate(&config)` becomes the single source of truth for valid ranges
- Remove duplicate hardcoded range definitions — constraints defined once in `#[setting()]` attributes
- Validation report: warnings (out-of-range), errors (dangerous values), info (depends_on conditions not met)

### Annotation Scope
- ALL ~400+ fields annotated in this phase — complete SettingRegistry
- All enums annotated with display names and descriptions
- Full dependency graph (affects) populated for all fields

### Claude's Discretion
- Exact SettingCategory enum variant list and naming
- Internal proc-macro implementation details (syn/quote patterns)
- TIER_MAP.md production methodology (how to extract OrcaSlicer UI grouping)
- Exact search ranking algorithm
- Compile warning implementation approach (proc-macro diagnostics vs build warnings)
- LazyLock vs once_cell choice based on MSRV
- JSON Schema structure details (how to represent depends_on as if/then/else)
- Test strategy for the derive macro
- Order of annotation work across config structs

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### ConfigSchema system design
- `designDocs/CONFIG_PARITY_AUDIT.md` §Section 6 (line 578-650) — ConfigSchema system design notes: proc-macro approach, runtime registry, progressive disclosure tiers, output formats
- `designDocs/CONFIG_PARITY_AUDIT.md` §Section 5 (line 549-575) — Phase plan breakdown: 6 plans in 3 waves (foundation, application, output generation)
- `designDocs/01-PRODUCT_REQUIREMENTS.md` §7.2 (line 273-291) — SettingDefinition schema: fields, types, constraints structure
- `designDocs/02-ARCHITECTURE.md` §6.2 (line 520-573) — ConfigSchema struct, SettingDefinition struct, ValueType enum, Constraint enum
- `designDocs/03-API-DESIGN.md` §2.4 (line 429-445) — PrintConfig API using ConfigSchema
- `designDocs/03-API-DESIGN.md` §3.7 (line 1016-1029) — CLI `schema` command design

### Current config implementation
- `crates/slicecore-engine/src/config.rs` — PrintConfig and all sub-structs (~2800 lines, ~140 doc comments with units)
- `crates/slicecore-engine/src/support/config.rs` — SupportConfig (~27 fields + BridgeConfig + TreeSupportConfig)

### Prior config decisions
- `.planning/phases/32-p0-config-gap-closure-critical-missing-fields/32-CONTEXT.md` — Phase 32 patterns: sub-struct organization, validation, doc comments format, G-code template variables
- `.planning/phases/33-p1-config-gap-closure-profile-fidelity-fields/33-CONTEXT.md` — Phase 33 patterns: extended sub-structs, enum patterns
- `.planning/phases/34-support-config-and-advanced-feature-profile-import-mapping/34-CONTEXT.md` — Phase 34: 100% field coverage target, doc comment format for Phase 35 compatibility

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `PrintConfig` with ~15 nested sub-configs — each sub-struct gets `#[derive(SettingSchema)]`
- ~140 doc comments with units/ranges already in config.rs — reference for writing description attributes
- Existing enum types (InfillPattern, SurfacePattern, BedType, SupportType, etc.) — all need `#[derive(SettingSchema)]` with variant metadata
- Existing config validation logic (Phase 30) — to be replaced by schema-driven validation

### Established Patterns
- `#[serde(default)]` on all config structs — SettingSchema derive coexists with serde derive
- Sub-struct organization: SpeedConfig, CoolingConfig, RetractionConfig, etc. — natural category boundaries
- Vec<f64> for multi-extruder array fields — needs ValueType::FloatVec handling
- `passthrough` BTreeMap<String,String> on PrintConfig — gets `#[setting(skip)]`

### Integration Points
- `crates/slicecore-engine/src/config.rs` — add derive macros and #[setting()] attributes to all structs and fields
- `crates/slicecore-engine/src/support/config.rs` — add derive macros to support config structs
- `crates/slicecore-cli/` — add `schema` subcommand
- Existing config validation — replace with registry.validate()
- Cargo workspace — add two new crates (slicecore-config-derive, slicecore-config-schema)

</code_context>

<specifics>
## Specific Ideas

- TIER_MAP.md must be reviewed by user before annotation implementation — it's a gate, not a nice-to-have
- The 3-wave plan structure from CONFIG_PARITY_AUDIT.md §5 is a good starting point: Wave 1 (core types + derive macro), Wave 2 (annotate all fields), Wave 3 (JSON Schema + CLI + validation upgrade)
- Existing doc comments like `/// Perimeter print speed (mm/s).` are the reference for writing `description = "Perimeter print speed"` and `units = "mm/s"` — but description is a separate explicit attribute, not auto-extracted
- The dependency graph (affects) should be informed by OrcaSlicer's setting interactions and 3D printing domain knowledge
- Compile-time validation of key references (affects, depends_on) is important for catching typos in 400+ annotations

</specifics>

<deferred>
## Deferred Ideas

### Documentation & Tooling (future phases)
- **Auto-generated markdown docs** — `slicecore schema --format markdown` producing per-category documentation pages
- **Schema diff tool** — `slicecore schema --diff v1.json v2.json` comparing schema versions for migration guides
- **Interactive schema explorer** — TUI or web-based setting browser with search and dependency visualization
- **OpenAPI integration** — Embed config schema in OpenAPI spec for API consumers

### AI Integration (future phases)
- **AI prompt schema** — Export tier-filtered subset optimized for LLM consumption with dependency context
- **Tier 0 population** — Determine which fields AI should auto-configure
- **Smart defaults** — Use dependency graph to suggest optimal values based on other settings

### Config Infrastructure (future phases)
- **Config migration system** — Use since_version + deprecated fields to auto-migrate old configs between schema versions
- **UI form generator** — Consume flat metadata JSON to auto-generate settings UI forms
- **WASM-compatible registry** — Ensure SettingRegistry works without std::sync globals (may need alternative initialization pattern)

### Carried from Phase 34
- **Engine behavior for mapped fields** — Wiring config values into actual engine behavior (separate from metadata)
- **Profile management CLI** — import, update, list, search, validate commands
- **Upstream sync workflow** — CI tooling to track slicer profile changes

</deferred>

---

*Phase: 35-configschema-system-with-setting-metadata-and-json-schema-generation*
*Context gathered: 2026-03-17*
