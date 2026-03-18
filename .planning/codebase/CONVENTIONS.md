# Coding Conventions

**Analysis Date:** 2026-03-18

## Naming Patterns

**Files:**
- `snake_case` for all Rust source files: `profile_import.rs`, `gcode_gen.rs`, `triangle_mesh.rs`
- Submodule groups use a directory + `mod.rs` pattern: `crates/slicecore-engine/src/infill/mod.rs`, `crates/slicecore-engine/src/support/mod.rs`
- Test files in `tests/` are named by scope: `integration.rs`, `determinism.rs`, `cli_plugins.rs`, `phase34_integration.rs`
- Bench files in `benches/` named by subject: `geometry_benchmark.rs`, `csg_bench.rs`, `slice_benchmark.rs`
- Error types always live in a dedicated `error.rs` file within each crate

**Crates:**
- `kebab-case` for crate directory names: `slicecore-engine`, `slicecore-mesh`, `slicecore-gcode-io`

**Structs / Enums / Traits:**
- `PascalCase`: `TriangleMesh`, `PrintConfig`, `EngineError`, `WallOrder`, `CancellationToken`
- Enum variants: `PascalCase`: `InnerFirst`, `OuterFirst`, `EmptyMesh`, `NonManifold`

**Functions / Methods:**
- `snake_case`: `generate_perimeters`, `slice_mesh`, `compute_e_value`, `estimate_print_time`
- Constructor-style helpers often named `new`, `zero`, `default`
- Boolean-returning functions use `is_` prefix: `is_cancelled()`, `is_circular_hole()`

**Variables:**
- `snake_case` throughout: `wall_count`, `layer_height`, `extrusion_width`, `half_width`
- Single-letter loop vars acceptable for geometry math: `a`, `b`, `dx`, `dy`, `k`

**Constants / Statics:**
- `SCREAMING_SNAKE_CASE`: `EPSILON`, `GLOBAL_REGISTRY`, `COORD_SCALE`

## Code Style

**Formatting:**
- Standard `rustfmt` (no explicit `.rustfmt.toml` found — default settings apply)
- Trailing commas on multi-line structs and enums: `PrintConfig { layer_height: 0.2, ..PrintConfig::default() }`

**Linting:**
- `clippy.toml` sets `too-many-arguments-threshold = 8`
- No crate-level `#![deny(clippy::pedantic)]` or similar lint attributes found
- No `#[must_use]` annotations found in library code (not enforced)

## Module Structure

**Crate `lib.rs` pattern:**
- Module declarations are all `pub mod` except internal helpers (`mod parallel`)
- Extensive `pub use` re-exports at crate root for public API surface: `crates/slicecore-engine/src/lib.rs`
- Feature-gated re-exports using `#[cfg(feature = "...")]`: plugins, ai, parallel

**Import Organization:**

1. Standard library: `use std::...`
2. External crates: `use serde::...`, `use thiserror::...`
3. Sibling crates: `use slicecore_mesh::...`, `use slicecore_geo::...`
4. Local module: `use crate::...`

No path aliases in use; full module paths are spelled out.

## Error Handling

**Strategy:** All public APIs return `Result<T, SomeError>` — no panics in library code.

**Pattern:**
- Each crate defines its own error enum in `error.rs` using `thiserror`
- Error variants document their fields with `///` doc comments
- `#[from]` is used to convert imported crate errors: `GcodeError(#[from] slicecore_gcode_io::GcodeError)`
- Named struct variants for multi-field errors: `Plugin { plugin: String, message: String }`
- Display messages are lowercase, e.g. `"mesh has no vertices"`, `"failed to read config file"`

**Example from `crates/slicecore-engine/src/error.rs`:**
```rust
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("failed to read config file {0}: {1}")]
    ConfigIo(PathBuf, std::io::Error),

    #[error("Plugin error in '{plugin}': {message}")]
    Plugin { plugin: String, message: String },
}
```

**`.unwrap()` policy:**
- `unwrap()` is used freely in test code and inline `#[cfg(test)]` blocks
- In doc examples, `unwrap()` is acceptable
- Production library code avoids `unwrap()` — errors propagate via `?`
- The single TODO in `crates/slicecore-mesh/src/spatial.rs:41` is the only known usage

## Documentation

**Module-level:** Every `.rs` file opens with `//!` module doc comment explaining purpose, algorithm, and key types. Multi-paragraph docs reference types with `[TypeName]` links.

**Item-level:** Public structs, enums, functions, and methods have `///` doc comments on every item. Fields in public structs are always documented.

**Doc examples:** Key types include runnable `# Example` blocks in their doc comments (e.g., `CancellationToken` in `crates/slicecore-engine/src/engine.rs`).

**Comments in bodies:** Inline `//` comments explain non-obvious geometry math, phase references (e.g., `// SC3: ...`), and section separators using `// ---` or `// ===` divider lines.

## Struct Design

**`Default` trait:** Configuration structs always implement `Default` with sensible FDM values. Used with struct update syntax: `PrintConfig { layer_height: 0.1, ..PrintConfig::default() }`.

**`Serialize` / `Deserialize`:** All config and output types derive `serde::{Serialize, Deserialize}`.

**`Clone` / `Copy`:** Geometry primitives (`Point2`, `Point3`, `Vec3`) derive both. Larger structs derive `Clone` only.

**`Debug`:** Derived on all public types.

**`#[serde(rename_all = "snake_case")]`:** Used consistently on enums: `WallOrder`, `SurfacePattern`, `InfillPattern`.

**`#[serde(default)]`:** Used on config structs to allow partial TOML deserialization.

## Inline Tests

Unit tests live at the bottom of the source file they test, inside a `#[cfg(test)] mod tests { ... }` block. Example: `crates/slicecore-math/src/point.rs` lines 227–373. Test functions use `snake_case` names describing what they verify: `point2_distance_to`, `point3_to_point2_drops_z`.

## Geometry-Specific Conventions

- Coordinates are stored in integer micron units (`i64`, scaled by `COORD_SCALE = 1_000_000`); conversions via `mm_to_coord` / `coord_to_mm` from `slicecore-math`
- Floating-point equality uses a custom `EPSILON` (not `approx` crate) via `PartialEq` implementations on geometry types
- Mesh indices are `u32`; vertex coordinates are `f64`
- All mesh triangles use CCW winding order for outward-facing normals

---

*Convention analysis: 2026-03-18*
