# Phase 46: Job Output Directories for Isolated Slice Execution - Research

**Researched:** 2026-03-24
**Domain:** CLI structured output directories, file locking, manifest generation
**Confidence:** HIGH

## Summary

Phase 46 adds a `--job-dir` flag to the `slice` command that creates a structured output directory containing all slice artifacts (G-code, config snapshot, log, thumbnail, manifest). The implementation is primarily CLI-level orchestration -- routing existing output paths into a single directory and adding manifest generation. The codebase already has all the building blocks: SHA-256 checksums (`sha2` in slicecore-engine), thumbnail rendering (`slicecore-render`), config serialization (`PrintConfig::from_file` / TOML serialization), statistics collection (`SliceResult.statistics`), and reproduce command generation (`gcode_gen::reproduce_command`).

The main new work is: (1) a `JobDir` module that manages directory creation, locking, and artifact routing; (2) manifest JSON generation with lifecycle states; (3) integrating `--job-dir` / `--job-dir auto` / `--job-base` / `SLICECORE_JOB_DIR` into the clap argument parser with proper conflict groups; and (4) wiring the existing `cmd_slice` output paths through the job directory.

**Primary recommendation:** Create a new `job_dir` module in `slicecore-cli/src/` that encapsulates all job directory logic (creation, locking, manifest I/O, artifact path resolution), then modify `cmd_slice` and `cmd_slice_plate` to delegate output routing through it when `--job-dir` is active.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Flat layout -- all files at top level of job directory
- G-code filename derived from input model name (benchy.stl -> benchy.gcode)
- Plate mode produces single combined G-code file named `plate.gcode`
- All artifacts always written when --job-dir is used -- config.toml, slice.log, thumbnail.png, manifest.json are automatic
- Fixed artifact filenames: `manifest.json`, `config.toml`, `slice.log`, `thumbnail.png`, plus model-derived `.gcode`
- JSON manifest format with integer `schema_version` starting at 1
- Manifest written twice: initially "running", overwritten with final status on completion/failure
- On failure: "failed" status with error details, partial artifacts remain
- `--job-dir <path>` creates with mkdir -p semantics
- Error if target directory non-empty (use `--force` to override)
- `--job-dir` mutually exclusive with `--output`, `--log-file`, `--save-config`
- On success, print job directory path to stdout
- `--job-dir auto` generates UUID-named directory
- `--job-base <path>` sets parent for auto-generated dirs (default: CWD)
- `SLICECORE_JOB_DIR` env var sets default base directory
- Priority: `--job-base` > `SLICECORE_JOB_DIR` > CWD
- Config file option `job_base_dir` for persistent base directory
- File lock (`.lock` file with PID) to prevent concurrent writes
- Lock released on completion (success or failure)

### Claude's Discretion
- Lock file implementation details (flock vs advisory vs PID file)
- Config file format and location for `job_base_dir` setting
- Exact manifest JSON structure and field ordering
- How config diff from defaults is computed and represented
- Thumbnail format/size selection within job dir context
- Internal code organization (separate module vs inline)
- Error message wording

### Deferred Ideas (OUT OF SCOPE)
- Job management CLI subcommands (inspect, list, clean, reslice)
- Input model copy (--include-model)
- Model + timestamp naming for auto-generated directories
- Model + short hash naming for auto-generated directories
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| API-02 | Full-featured CLI interface (slice, validate, analyze commands) | Job directory extends the slice command with structured output management, adding --job-dir, --job-dir auto, --job-base flags and manifest generation |
</phase_requirements>

## Standard Stack

### Core (already in workspace)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde + serde_json | 1.x / 1.x | Manifest JSON serialization | Already workspace deps |
| sha2 | 0.10 | SHA-256 checksums for artifact integrity | Already in slicecore-engine |
| toml | 0.8 | Config snapshot serialization | Already workspace dep |
| clap | 4.5 | CLI argument parsing with conflict groups | Already in slicecore-cli |

### New Dependencies
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| uuid | 1.22 | UUID v4 generation for `--job-dir auto` | Auto-generated directory names |
| chrono | 0.4.44 | ISO 8601 timestamps in manifest | Created/started/completed fields |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| uuid crate | Random bytes formatted as hex | uuid is standard, well-tested, 1 dep |
| chrono | std::time + manual formatting | chrono gives RFC 3339 formatting for free |
| fs2 for file locking | PID file (manual) | PID file is simpler, cross-platform, no extra dep -- RECOMMENDED |

**Installation:**
```bash
cargo add uuid --features v4 -p slicecore-cli
cargo add chrono --features serde -p slicecore-cli
```

**Version verification:** uuid 1.22.0 and chrono 0.4.44 confirmed current on crates.io as of 2026-03-24.

## Architecture Patterns

### Recommended Module Structure
```
crates/slicecore-cli/src/
  job_dir.rs          # JobDir struct: creation, locking, artifact paths, manifest I/O
  main.rs             # Modified: --job-dir flag, routing through JobDir
  slice_workflow.rs   # Unchanged
```

### Pattern 1: JobDir Struct
**What:** A `JobDir` struct that encapsulates all job directory operations.
**When to use:** Whenever `--job-dir` is specified on the CLI.
**Example:**
```rust
pub struct JobDir {
    path: PathBuf,
    lock_file: Option<std::fs::File>,
}

impl JobDir {
    /// Create job directory with mkdir -p, verify empty (or --force), acquire lock.
    pub fn create(path: PathBuf, force: bool) -> Result<Self, JobDirError> { ... }

    /// Create with auto-generated UUID name under base_dir.
    pub fn create_auto(base_dir: PathBuf) -> Result<Self, JobDirError> { ... }

    /// Resolve base directory: --job-base > SLICECORE_JOB_DIR > CWD.
    pub fn resolve_base(job_base: Option<&Path>) -> PathBuf { ... }

    /// Artifact paths.
    pub fn gcode_path(&self, model_name: &str) -> PathBuf { ... }
    pub fn manifest_path(&self) -> PathBuf { ... }
    pub fn config_path(&self) -> PathBuf { ... }
    pub fn log_path(&self) -> PathBuf { ... }
    pub fn thumbnail_path(&self) -> PathBuf { ... }

    /// Write initial manifest with "running" status.
    pub fn write_initial_manifest(&self, manifest: &Manifest) -> Result<(), JobDirError> { ... }

    /// Write final manifest with stats and checksums.
    pub fn write_final_manifest(&self, manifest: &Manifest) -> Result<(), JobDirError> { ... }
}

impl Drop for JobDir {
    fn drop(&mut self) {
        // Release lock file
    }
}
```

### Pattern 2: Manifest Struct
**What:** Strongly-typed manifest with serde serialization.
**When to use:** Always for manifest.json generation.
**Example:**
```rust
#[derive(Serialize)]
pub struct Manifest {
    pub schema_version: u32,            // Always 1
    pub slicecore_version: String,
    pub status: JobStatus,              // "running" | "success" | "failed"
    pub created: String,                // ISO 8601
    pub started: Option<String>,
    pub completed: Option<String>,
    pub duration_ms: Option<u64>,
    pub input: Vec<PathBuf>,
    pub output: Option<String>,         // gcode filename
    pub config: String,                 // "config.toml"
    pub log: String,                    // "slice.log"
    pub thumbnail: Option<String>,      // "thumbnail.png"
    pub reproduce_command: Option<String>,
    pub checksums: Option<ArtifactChecksums>,
    pub statistics: Option<PrintStats>,
    pub profile_provenance: Option<Vec<ProfileSource>>,
    pub input_metadata: Option<Vec<InputModelMeta>>,
    pub warnings: Vec<String>,
    pub environment: EnvironmentInfo,
    pub config_diff: Option<serde_json::Value>,
    pub error: Option<String>,          // Only when status == "failed"
    pub per_object_stats: Option<Vec<ObjectStats>>,
}
```

### Pattern 3: Clap Conflict Groups
**What:** Use clap's `conflicts_with_all` for mutual exclusivity.
**When to use:** `--job-dir` conflicts with `--output`, `--log-file`, `--save-config`.
**Example:**
```rust
/// Job output directory (creates structured output with all artifacts)
#[arg(long, value_name = "PATH_OR_AUTO",
      conflicts_with_all = ["output", "log_file", "save_config"])]
job_dir: Option<String>,  // String to accept "auto" as special value

/// Base directory for auto-generated job dirs (default: CWD)
#[arg(long, value_name = "DIR")]
job_base: Option<PathBuf>,
```

### Pattern 4: PID-based Lock File
**What:** Write a `.lock` file containing the current PID. Check on creation if stale.
**When to use:** Always when creating a job directory.
**Example:**
```rust
fn acquire_lock(dir: &Path) -> Result<std::fs::File, JobDirError> {
    let lock_path = dir.join(".lock");
    if lock_path.exists() {
        let content = std::fs::read_to_string(&lock_path)?;
        if let Ok(pid) = content.trim().parse::<u32>() {
            // Check if PID is still running
            if process_exists(pid) {
                return Err(JobDirError::Locked { pid });
            }
            // Stale lock -- remove it
        }
    }
    let mut file = std::fs::File::create(&lock_path)?;
    write!(file, "{}", std::process::id())?;
    Ok(file)
}

fn release_lock(dir: &Path) {
    let _ = std::fs::remove_file(dir.join(".lock"));
}
```

### Anti-Patterns to Avoid
- **Inlining job dir logic into cmd_slice:** Keep it in a separate module. The function is already 500+ lines.
- **Using flock/fs2 for locking:** Adds a dependency for something a PID file handles. PID files are simpler and work cross-platform including WASM scenarios.
- **Writing manifest only once at the end:** The CONTEXT.md specifies writing it twice (running -> final). This is important for external monitoring.
- **Forgetting to release lock on panic:** Use `Drop` impl on the `JobDir` struct.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| UUID generation | Random hex string formatting | `uuid` crate v4 | Proper RFC 4122 compliance, one line |
| ISO 8601 timestamps | `format!("{:04}-{:02}-{:02}T...")` | `chrono::Utc::now().to_rfc3339()` | Handles timezone, milliseconds correctly |
| SHA-256 checksums | Manual digest loop | `sha2::Sha256` (already available) | Already in slicecore-engine, use same pattern as `plate_checksum` |
| Config diff from defaults | Field-by-field comparison | Serialize both to TOML tables, diff keys | TOML `Value` comparison is reliable |
| Process existence check | Reading /proc | `std::process::Command` with kill(0) or platform check | Cross-platform |

**Key insight:** Nearly all artifacts already exist in the codebase. The job directory is a routing/orchestration layer, not new computation.

## Common Pitfalls

### Pitfall 1: Non-Empty Directory Check Race
**What goes wrong:** Check if directory is empty, then start writing -- another process creates it between check and write.
**Why it happens:** TOCTOU race condition.
**How to avoid:** Acquire lock FIRST, then check emptiness. The lock serializes access.
**Warning signs:** Intermittent test failures in parallel job creation scenarios.

### Pitfall 2: Forgetting to Update Manifest on Panic/Error
**What goes wrong:** Process crashes mid-slice, manifest stays "running" forever.
**Why it happens:** Error paths skip manifest update.
**How to avoid:** Use a guard pattern -- `JobDir::Drop` writes "failed" manifest if status is still "running". Or use `std::panic::catch_unwind` at the top level.
**Warning signs:** Test that kills mid-slice shows stale "running" manifest.

### Pitfall 3: Stdout Pollution
**What goes wrong:** `--job-dir` should print only the directory path to stdout for script consumption, but progress/stats also go to stdout.
**Why it happens:** Existing code mixes stdout and stderr.
**How to avoid:** When `--job-dir` is active, ALL informational output goes to stderr (via `CliOutput`). Only the final directory path goes to stdout.
**Warning signs:** `JOB=$(slicecore slice ... --job-dir auto)` captures extra text.

### Pitfall 4: cmd_slice Parameter Explosion
**What goes wrong:** Adding 3+ more parameters to the already 35-parameter `cmd_slice` function.
**Why it happens:** Following the existing pattern of passing everything as individual params.
**How to avoid:** Consider a `JobDirConfig` struct that bundles job-dir-related options (`job_dir`, `job_base`, `force`). Pass that as a single parameter. The existing function is already very long -- keep the growth manageable.
**Warning signs:** Clippy `too_many_arguments` lint firing even with existing `#[allow]`.

### Pitfall 5: Thumbnail Generation Not Forced
**What goes wrong:** `--job-dir` always writes thumbnail.png, but thumbnail generation requires `--thumbnails` flag.
**Why it happens:** Existing thumbnail pipeline is opt-in.
**How to avoid:** When `--job-dir` is active, implicitly enable thumbnail generation regardless of `--thumbnails` flag.
**Warning signs:** Empty or missing `thumbnail.png` in job directory.

### Pitfall 6: Config Diff Complexity
**What goes wrong:** Computing "config diff from defaults" becomes a rabbit hole of nested TOML comparison.
**Why it happens:** `PrintConfig` has dozens of nested fields.
**How to avoid:** Serialize both default and actual configs to `toml::Value`, then recursively diff the Value trees. Only include keys where values differ.
**Warning signs:** Missing nested diffs, or giant diff including all default values.

## Code Examples

### SHA-256 Checksum of a File (existing pattern)
```rust
// Source: crates/slicecore-engine/src/gcode_gen.rs:330
use sha2::{Digest, Sha256};

fn file_checksum(path: &Path) -> std::io::Result<String> {
    let data = std::fs::read(path)?;
    let hash = Sha256::digest(&data);
    Ok(format!("sha256:{hash:x}"))
}
```

### Reproduce Command (existing)
```rust
// Source: crates/slicecore-engine/src/gcode_gen.rs:357
use slicecore_engine::gcode_gen::reproduce_command;
let cmd = reproduce_command(&plate, Some(Path::new("plate.toml")), Path::new("out.gcode"));
```

### Clap Conflict Group (existing pattern)
```rust
// Source: main.rs:207 -- existing --config conflicts_with pattern
#[arg(short, long, conflicts_with_all = ["machine", "filament", "process"])]
config: Option<PathBuf>,
```

### Config Serialization (existing)
```rust
// PrintConfig already implements Serialize
let config_toml = toml::to_string_pretty(&print_config).expect("config serialize");
std::fs::write(job.config_path(), &config_toml)?;
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Individual output flags | Structured job directory | This phase | Single flag replaces --output + --log-file + --save-config + --thumbnails |
| No manifest | JSON manifest with lifecycle | This phase | External tooling can monitor and inspect job state |

**Existing assets to reuse:**
- `gcode_gen::reproduce_command` -- generates the CLI invocation for manifest
- `gcode_gen::plate_checksum` -- SHA-256 pattern for checksums
- `SliceResult.statistics` -- all print stats for manifest
- `SliceResult.filament_usage` -- filament length/weight/cost
- Profile provenance system (Phase 30) -- `FieldSource` tracking
- Thumbnail pipeline -- `slicecore-render` crate
- `PrintConfig` TOML serialization -- config snapshot

## Open Questions

1. **Process existence check cross-platform**
   - What we know: On Linux, `kill(pid, 0)` checks if process exists. On Windows, `OpenProcess` with limited access.
   - What's unclear: Whether this project needs Windows lock support now.
   - Recommendation: Use `#[cfg(unix)]` for kill-based check, with a simple "always assume alive" fallback on other platforms. Keeps it simple.

2. **Config file location for `job_base_dir`**
   - What we know: The project already uses `~/.slicecore/` for override-sets and enabled profiles.
   - What's unclear: Whether there's an existing global config file.
   - Recommendation: Add `job_base_dir` to a `~/.slicecore/config.toml` file. If no global config exists yet, create the pattern. This is Claude's discretion.

3. **Thumbnail forced in job-dir mode**
   - What we know: Thumbnails require mesh rendering pipeline. Job dir CONTEXT says "all artifacts always written."
   - What's unclear: Whether thumbnail generation is expensive enough to be a concern.
   - Recommendation: Always generate thumbnail in job-dir mode. The render pipeline already exists and handles a single isometric view quickly.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml [dev-dependencies] |
| Quick run command | `cargo test -p slicecore-cli --lib -- job_dir` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| API-02-a | --job-dir creates directory with all artifacts | integration | `cargo test -p slicecore-cli -- job_dir` | No - Wave 0 |
| API-02-b | --job-dir auto generates UUID directory | unit | `cargo test -p slicecore-cli -- job_dir::auto` | No - Wave 0 |
| API-02-c | --job-dir conflicts with --output/--log-file/--save-config | integration | `cargo test -p slicecore-cli -- job_dir::conflicts` | No - Wave 0 |
| API-02-d | Non-empty directory error without --force | unit | `cargo test -p slicecore-cli -- job_dir::nonempty` | No - Wave 0 |
| API-02-e | Lock file prevents concurrent writes | unit | `cargo test -p slicecore-cli -- job_dir::lock` | No - Wave 0 |
| API-02-f | Manifest lifecycle (running -> success/failed) | unit | `cargo test -p slicecore-cli -- manifest` | No - Wave 0 |
| API-02-g | --job-base and SLICECORE_JOB_DIR priority | unit | `cargo test -p slicecore-cli -- job_dir::base_priority` | No - Wave 0 |
| API-02-h | Stdout contains only job dir path | integration | `cargo test -p slicecore-cli -- job_dir::stdout` | No - Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p slicecore-cli -- job_dir`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before verification

### Wave 0 Gaps
- [ ] `crates/slicecore-cli/src/job_dir.rs` -- new module with unit tests
- [ ] `crates/slicecore-cli/tests/cli_job_dir.rs` -- integration tests for job directory CLI behavior
- [ ] Add `uuid` and `chrono` to slicecore-cli Cargo.toml

## Sources

### Primary (HIGH confidence)
- `crates/slicecore-cli/src/main.rs` -- current `cmd_slice` implementation (lines 1197-1764), clap args (lines 176-317)
- `crates/slicecore-engine/src/gcode_gen.rs` -- `reproduce_command` (line 357), `plate_checksum` (SHA-256 pattern)
- `crates/slicecore-engine/src/engine.rs` -- `SliceResult` struct (line 123), `PlateSliceResult` (line 666)
- `crates/slicecore-cli/Cargo.toml` -- current dependencies
- `Cargo.toml` -- workspace dependencies, no uuid/chrono yet

### Secondary (MEDIUM confidence)
- crates.io: uuid 1.22.0, chrono 0.4.44, fs2 0.4.3 -- versions verified 2026-03-24

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all core deps already in workspace, only 2 new (uuid, chrono)
- Architecture: HIGH -- clear integration points in existing cmd_slice, well-defined artifact list
- Pitfalls: HIGH -- based on direct code reading of 4400-line main.rs

**Research date:** 2026-03-24
**Valid until:** 2026-04-24 (stable domain, no fast-moving dependencies)
