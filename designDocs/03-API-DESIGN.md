# LibSlic3r-RS: API Design Document

**Version:** 1.0.0-draft
**Author:** Steve Scargall / SliceCore-RS Architecture Team
**Date:** 2026-02-14
**Status:** Draft — Review & Iterate

---

## 1. API Philosophy

### 1.1 Design Principles

1. **Zero-Cost Abstractions:** Traits and generics compile away at monomorphization; the API incurs no runtime overhead compared to hand-written code. Dynamic dispatch (`dyn Trait`) is used only at plugin boundaries.
2. **Builder Pattern for Configuration:** All complex structures use type-safe builders with compile-time validation where possible. Invalid states are unrepresentable.
3. **Streaming Results:** Large outputs (G-code, toolpaths, preview data) stream via iterators and async channels rather than accumulating in memory.
4. **Progressive Disclosure:** Simple operations require minimal code. Advanced features are available without cluttering the common path.
5. **Deterministic by Default:** Same inputs always produce identical outputs. Non-deterministic features (AI suggestions, parallel ordering) are opt-in and explicitly marked.
6. **Fail Fast, Fail Clearly:** Errors carry structured context (layer number, setting key, file position). No stringly-typed errors escape the public API.
7. **Cancel Anywhere:** Every long-running operation accepts a `CancellationToken` and checks it at meaningful boundaries (per-layer, per-stage).

### 1.2 API Surface Overview

```
                    +--------------------------+
                    |   slicecore-engine       |  <-- Primary Rust API
                    |   Engine, SliceJob,      |
                    |   SliceResult, Config    |
                    +-----------+--------------+
                                |
          +---------------------+---------------------+
          |            |            |          |       |
     +---------+ +---------+ +---------+ +-------+ +------+
     |   CLI   | |  REST   | |  C FFI  | | PyO3  | | WASM |
     |  clap   | |  axum   | | cbindgen| | pyo3  | | wasm |
     |         | |         | |         | |       | | bind |
     +---------+ +---------+ +---------+ +-------+ +------+
     slicecore   slicecore   slicecore   slicecore  slicecore
     -cli        -server     -ffi        -python    -wasm
```

All external interfaces are thin wrappers around the same `slicecore-engine` API. No business logic lives in the interface layer.

### 1.3 Versioning Contract

- The Rust library API follows SemVer strictly. Breaking changes increment the major version.
- The REST API uses URL-path versioning (`/api/v1/`, `/api/v2/`). Old versions remain available for at least 12 months after deprecation.
- The C FFI maintains ABI stability within a major version. New functions are additive.
- The Python bindings mirror the Rust API structure and version in lockstep.
- The WASM interface uses the same version as the Rust crate.

---

## 2. Rust Library API (`slicecore-engine`)

This is the primary public API. All other interfaces (CLI, REST, FFI, Python, WASM) are projections of this API.

### 2.1 Engine Lifecycle

```rust
use slicecore_engine::{Engine, EngineConfig};

// Create an engine with default configuration
let engine = Engine::new(EngineConfig::default())?;

// Create an engine with custom thread pool and plugin directory
let engine = Engine::builder()
    .threads(8)
    .plugin_dir("/usr/lib/slicecore/plugins")
    .progress_reporter(my_reporter)
    .build()?;

// Engine is Send + Sync — safe to share across threads via Arc
let engine = Arc::new(engine);
```

#### `EngineConfig`

```rust
pub struct EngineConfig {
    /// Number of worker threads for parallel slicing. None = auto-detect.
    pub thread_count: Option<usize>,

    /// Directory to scan for dynamic plugins.
    pub plugin_dir: Option<PathBuf>,

    /// Memory limit for slicing operations (soft limit, advisory).
    pub memory_limit: Option<usize>,

    /// Progress reporter for all operations dispatched by this engine.
    pub progress_reporter: Option<Arc<dyn ProgressReporter>>,

    /// Base directory for resolving relative config/profile paths.
    pub config_search_path: Vec<PathBuf>,
}
```

#### `Engine`

```rust
pub struct Engine { /* ... */ }

impl Engine {
    /// Create a new engine with the given configuration.
    pub fn new(config: EngineConfig) -> Result<Self, EngineError>;

    /// Builder pattern for ergonomic construction.
    pub fn builder() -> EngineBuilder;

    /// Synchronous slice — blocks until complete or cancelled.
    pub fn slice(&self, job: &SliceJob) -> Result<SliceResult, SliceError>;

    /// Async slice with progress streaming.
    /// Returns a handle that can be polled for progress and awaited for the result.
    pub fn slice_async(
        &self,
        job: SliceJob,
        cancel: CancellationToken,
    ) -> SliceHandle;

    /// Analyze a model without slicing it.
    pub fn analyze(&self, model: &ModelInput) -> Result<AnalysisReport, AnalyzeError>;

    /// Validate a configuration against the schema.
    pub fn validate_config(&self, config: &PrintConfig) -> ValidationReport;

    /// Return the full settings schema (for UI generation, documentation).
    pub fn schema(&self) -> &ConfigSchema;

    /// List available printer/filament/quality profiles.
    pub fn list_profiles(&self, filter: ProfileFilter) -> Vec<ProfileSummary>;

    /// Load a profile by name or path.
    pub fn load_profile(&self, id: &ProfileId) -> Result<PrintConfig, ConfigError>;

    /// Diff two configurations, returning all differences.
    pub fn diff_configs(
        &self,
        a: &PrintConfig,
        b: &PrintConfig,
    ) -> Vec<ConfigDiff>;

    /// Registered plugin metadata.
    pub fn plugins(&self) -> &[PluginMetadata];

    /// Engine version and build info.
    pub fn version(&self) -> &VersionInfo;
}
```

### 2.2 SliceJob — Input Specification

A `SliceJob` describes everything needed to produce G-code: models, configuration, and per-object/per-region overrides.

```rust
pub struct SliceJob {
    /// One or more models to slice. Multiple models are arranged on the bed.
    pub models: Vec<ModelInput>,

    /// Print configuration (merged hierarchy: defaults + printer + filament + quality).
    pub config: PrintConfig,

    /// Per-object setting overrides, keyed by model index.
    pub object_overrides: HashMap<usize, PrintConfig>,

    /// Modifier volumes with region-specific overrides.
    pub modifiers: Vec<ModifierVolume>,

    /// Output preferences.
    pub output: OutputOptions,
}
```

#### `ModelInput`

```rust
/// Specifies a model to be sliced. Supports multiple input methods.
pub enum ModelInput {
    /// Load from a file path (STL, 3MF, OBJ, STEP, AMF).
    File(PathBuf),

    /// Load from in-memory bytes with format hint.
    Bytes {
        data: Vec<u8>,
        format: ModelFormat,
    },

    /// Use a pre-loaded and optionally pre-repaired mesh.
    Mesh(TriangleMesh),
}

pub enum ModelFormat {
    Stl,
    StlAscii,
    ThreeMf,
    Obj,
    Step,
    Amf,
    Auto, // Detect from magic bytes / extension
}
```

#### `OutputOptions`

```rust
pub struct OutputOptions {
    /// Where to write G-code. None = return in memory only.
    pub gcode_path: Option<PathBuf>,

    /// Whether to include structured metadata alongside G-code.
    pub include_metadata: bool,

    /// Whether to generate preview/visualization data (layer outlines, moves).
    pub include_preview: bool,

    /// G-code comment verbosity.
    pub comment_level: CommentLevel,

    /// Whether to embed thumbnails in the G-code (for printer LCD previews).
    pub embed_thumbnails: Vec<ThumbnailSize>,
}

pub enum CommentLevel {
    /// No comments — smallest file size.
    None,
    /// Layer markers and basic annotations.
    Minimal,
    /// Feature type annotations on every extrusion group.
    Normal,
    /// Full debug info: speeds, flows, region types, coordinates.
    Verbose,
}

pub struct ThumbnailSize {
    pub width: u32,
    pub height: u32,
    pub format: ThumbnailFormat, // Png, Qoi
}
```

#### `ModifierVolume`

```rust
/// A region in 3D space that overrides settings for geometry inside it.
pub struct ModifierVolume {
    /// The volume (mesh or primitive shape).
    pub volume: ModifierShape,

    /// Settings to override within this volume.
    pub overrides: PrintConfig,
}

pub enum ModifierShape {
    Mesh(TriangleMesh),
    Box { min: Point3, max: Point3 },
    Cylinder { center: Point3, radius: f64, height: f64 },
    Sphere { center: Point3, radius: f64 },
    HeightRange { z_min: f64, z_max: f64 },
}
```

#### Builder Pattern for `SliceJob`

```rust
// Ergonomic construction via builder
let job = SliceJob::builder()
    .model(ModelInput::File("model.stl".into()))
    .config(PrintConfig::from_file("profiles/pla_standard.toml")?)
    .override_object(0, |c| {
        c.set("infill.density", 0.30)?;
        c.set("perimeters.wall_count", 4)?;
        Ok(())
    })
    .modifier(ModifierVolume {
        volume: ModifierShape::HeightRange { z_min: 0.0, z_max: 5.0 },
        overrides: PrintConfig::from_overrides(&[
            ("speed.first_layer_speed", "20"),
        ])?,
    })
    .output(OutputOptions {
        gcode_path: Some("output.gcode".into()),
        include_metadata: true,
        include_preview: false,
        comment_level: CommentLevel::Normal,
        embed_thumbnails: vec![ThumbnailSize::default()],
    })
    .build()?;
```

### 2.3 SliceResult — Output

```rust
pub struct SliceResult {
    /// The generated G-code as a string (or path if written to file).
    pub gcode: GcodeOutput,

    /// Structured metadata about the sliced output.
    pub metadata: SliceMetadata,

    /// Warnings generated during slicing (non-fatal issues).
    pub warnings: Vec<SliceWarning>,

    /// Per-layer preview data (if requested).
    pub preview: Option<PreviewData>,
}

pub enum GcodeOutput {
    /// G-code returned in memory.
    InMemory(String),

    /// G-code written to file; this variant holds the path and byte count.
    File { path: PathBuf, size_bytes: u64 },
}

pub struct SliceMetadata {
    /// Total estimated print time.
    pub estimated_time: Duration,

    /// Per-stage time breakdown.
    pub time_breakdown: TimeBreakdown,

    /// Filament usage per extruder.
    pub filament_usage: Vec<FilamentUsage>,

    /// Total layer count.
    pub layer_count: u32,

    /// Layer height range (min, max) — differs when adaptive layers are used.
    pub layer_height_range: (f64, f64),

    /// Bounding box of the printed object on the bed.
    pub print_bounds: BBox3,

    /// Slicing engine version that produced this output.
    pub engine_version: String,

    /// Configuration fingerprint (hash of merged config).
    pub config_hash: String,

    /// Total extrusion moves, travel moves, retractions.
    pub move_counts: MoveCounts,

    /// Time spent in each pipeline stage (for profiling).
    pub pipeline_timing: PipelineTiming,
}

pub struct TimeBreakdown {
    pub perimeters: Duration,
    pub infill: Duration,
    pub supports: Duration,
    pub travel: Duration,
    pub retraction: Duration,
    pub other: Duration,
}

pub struct FilamentUsage {
    pub extruder_index: u8,
    pub length_mm: f64,
    pub volume_mm3: f64,
    pub weight_grams: Option<f64>,   // Requires filament density in config
    pub cost: Option<f64>,           // Requires filament cost in config
}

pub struct MoveCounts {
    pub extrusion_moves: u64,
    pub travel_moves: u64,
    pub retractions: u64,
    pub z_hops: u64,
    pub wipes: u64,
    pub tool_changes: u32,
}

pub struct PipelineTiming {
    pub mesh_loading: Duration,
    pub mesh_repair: Duration,
    pub slicing: Duration,
    pub perimeters: Duration,
    pub infill: Duration,
    pub supports: Duration,
    pub pathing: Duration,
    pub planning: Duration,
    pub gcode_generation: Duration,
    pub post_processing: Duration,
    pub total: Duration,
}
```

#### Preview Data

```rust
/// Layer-by-layer visualization data for UI rendering.
pub struct PreviewData {
    pub layers: Vec<PreviewLayer>,
}

pub struct PreviewLayer {
    pub z: f64,
    pub layer_height: f64,
    pub moves: Vec<PreviewMove>,
}

pub struct PreviewMove {
    pub start: Point2,
    pub end: Point2,
    pub move_type: PreviewMoveType,
    pub width: f64,
    pub speed: f64,
    pub flow: f64,
}

pub enum PreviewMoveType {
    OuterWall,
    InnerWall,
    TopSurface,
    BottomSurface,
    Infill,
    Support,
    SupportInterface,
    Bridge,
    GapFill,
    Skirt,
    Travel,
    Retract,
    Wipe,
    Custom(String),
}
```

### 2.4 PrintConfig — Configuration System

```rust
use slicecore_config::{PrintConfig, ConfigSchema, SettingKey};

// Load from a TOML profile file
let config = PrintConfig::from_file("profiles/pla_standard.toml")?;

// Load with hierarchical merging (defaults <- printer <- filament <- quality)
let config = PrintConfig::builder()
    .defaults()
    .printer("profiles/printer/ender3_v3.toml")?
    .filament("profiles/filament/generic_pla.toml")?
    .quality("profiles/quality/standard.toml")?
    .override_setting("layer_height", "0.16")?
    .override_setting("infill.density", "0.20")?
    .build()?;

// Programmatic access
let layer_height: f64 = config.get("layer_height")?;
let wall_count: i64 = config.get("perimeters.wall_count")?;
let infill_pattern: InfillPattern = config.get_enum("infill.pattern")?;

// Type-safe setters with validation
config.set("layer_height", 0.28)?;        // OK
config.set("layer_height", -1.0)?;         // Err: below minimum (0.01)
config.set("layer_height", "not_a_number")?; // Err: type mismatch

// Import from PrusaSlicer/OrcaSlicer INI format
let config = PrintConfig::from_legacy_ini("PLA_profile.ini")?;

// Export as JSON for API consumers
let json = config.to_json()?;

// Diff two configs
let diffs = config_a.diff(&config_b);
for diff in &diffs {
    println!("{}: {:?} -> {:?}", diff.key, diff.old_value, diff.new_value);
}
```

#### Schema Introspection

```rust
let schema = engine.schema();

// Iterate all settings
for setting in schema.settings() {
    println!("{}: {} ({})", setting.key, setting.display_name, setting.value_type);
}

// Filter by tier
let simple_settings: Vec<_> = schema.settings()
    .filter(|s| s.tier == Tier::Simple)
    .collect();

// Filter by category
let speed_settings: Vec<_> = schema.settings()
    .filter(|s| s.category == Category::Speed)
    .collect();

// Get setting definition with full metadata
let def = schema.get("layer_height").unwrap();
assert_eq!(def.value_type, ValueType::Float { min: 0.01, max: 1.0, precision: 2 });
assert_eq!(def.units, Some("mm".to_string()));
assert_eq!(def.tier, Tier::Simple);
```

### 2.5 Progress Reporting

```rust
/// Trait for receiving progress updates during slicing.
/// Implementations must be Send + Sync (called from worker threads).
pub trait ProgressReporter: Send + Sync {
    /// Called when progress changes. May be called from any thread.
    fn on_progress(&self, progress: &Progress);

    /// Called when a warning is emitted during slicing.
    fn on_warning(&self, warning: &SliceWarning);

    /// Check if the operation should be cancelled.
    /// Called at stage boundaries and periodically within stages.
    fn is_cancelled(&self) -> bool;
}

pub struct Progress {
    /// Current pipeline stage.
    pub stage: SliceStage,

    /// Progress within the current stage (0.0 to 1.0).
    pub stage_progress: f64,

    /// Overall progress across all stages (0.0 to 1.0).
    pub overall_progress: f64,

    /// Current layer being processed (if applicable).
    pub current_layer: Option<u32>,

    /// Total layers (known after slicing stage completes).
    pub total_layers: Option<u32>,

    /// Human-readable status message.
    pub message: String,

    /// Time elapsed since operation started.
    pub elapsed: Duration,

    /// Estimated time remaining (None if not yet estimable).
    pub estimated_remaining: Option<Duration>,

    /// Items processed in current stage (layers, moves, etc.).
    pub items_processed: u64,

    /// Total items in current stage (if known).
    pub items_total: Option<u64>,
}

/// Pipeline stages in execution order, matching the C++ PrintObjectStep enum
/// extended with LibSlic3r-RS-specific stages.
pub enum SliceStage {
    /// Loading and parsing model files.
    Loading,
    /// Detecting and repairing mesh issues.
    Repairing,
    /// Extracting 2D contours from 3D mesh at each layer height.
    Slicing,
    /// Generating perimeter/wall toolpaths.
    Perimeters,
    /// Classifying regions and preparing fill areas.
    PreparingInfill,
    /// Generating infill toolpaths.
    Infill,
    /// Detecting overhangs and generating support structures.
    SupportMaterial,
    /// Optimizing toolpath ordering and travel moves.
    Pathing,
    /// Planning speeds, accelerations, temperatures, and cooling.
    Planning,
    /// Emitting G-code from planned moves.
    GcodeGeneration,
    /// Running post-processors (plugins, custom G-code).
    PostProcessing,
    /// Computing metadata, thumbnails, and preview data.
    Finalizing,
    /// Operation complete.
    Done,
}
```

#### Built-in Progress Reporters

```rust
/// Logs progress to the `tracing` framework.
pub struct TracingProgressReporter;

/// Collects all progress events for later inspection (testing).
pub struct CollectingProgressReporter {
    events: Mutex<Vec<Progress>>,
}

/// Calls a closure on each progress event.
pub struct CallbackProgressReporter<F: Fn(&Progress) + Send + Sync> {
    callback: F,
    cancelled: AtomicBool,
}

impl<F: Fn(&Progress) + Send + Sync> CallbackProgressReporter<F> {
    pub fn new(callback: F) -> Self { /* ... */ }
    pub fn cancel(&self) { self.cancelled.store(true, Ordering::Relaxed); }
}
```

### 2.6 Async Slicing & Cancellation

```rust
use slicecore_engine::{Engine, SliceJob, CancellationToken, SliceHandle};

let engine = Arc::new(Engine::new(EngineConfig::default())?);
let cancel = CancellationToken::new();

// Start async slice — returns immediately
let handle: SliceHandle = engine.slice_async(job, cancel.clone());

// Poll progress from another thread or async task
tokio::spawn({
    let handle = handle.clone();
    async move {
        loop {
            if let Some(progress) = handle.progress() {
                println!(
                    "[{:.0}%] {} — Layer {}/{}",
                    progress.overall_progress * 100.0,
                    progress.stage,
                    progress.current_layer.unwrap_or(0),
                    progress.total_layers.unwrap_or(0),
                );
            }
            if handle.is_complete() { break; }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
});

// Cancel from anywhere (e.g., user presses Ctrl+C)
cancel.cancel();

// Await the final result
match handle.await {
    Ok(result) => println!("Done! {} layers", result.metadata.layer_count),
    Err(SliceError::Cancelled) => println!("Slice was cancelled"),
    Err(e) => eprintln!("Slice failed: {}", e),
}
```

#### `CancellationToken`

```rust
/// Thread-safe cancellation signal. Clone to share across threads.
#[derive(Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self;

    /// Signal cancellation. All clones observe this immediately.
    pub fn cancel(&self);

    /// Check if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool;

    /// Create a child token that is cancelled when either parent or child is cancelled.
    pub fn child(&self) -> CancellationToken;
}
```

#### `SliceHandle`

```rust
/// Handle to an in-progress async slice operation.
/// Implements Future<Output = Result<SliceResult, SliceError>>.
pub struct SliceHandle { /* ... */ }

impl SliceHandle {
    /// Get the latest progress snapshot (non-blocking).
    pub fn progress(&self) -> Option<Progress>;

    /// Check if the operation has completed (success, error, or cancelled).
    pub fn is_complete(&self) -> bool;

    /// Block the current thread until complete (for non-async contexts).
    pub fn wait(self) -> Result<SliceResult, SliceError>;
}

impl Future for SliceHandle {
    type Output = Result<SliceResult, SliceError>;
    // ...
}
```

### 2.7 Streaming G-code Output

For very large models or real-time printing scenarios, G-code can be consumed as a stream rather than waiting for full completion.

```rust
/// Stream G-code layers as they are generated.
/// Each item is a complete layer's worth of G-code.
let stream = engine.slice_streaming(job, cancel.clone());

// Consume as an async stream
while let Some(chunk) = stream.next().await {
    match chunk? {
        GcodeChunk::Header(header) => write_to_printer(&header)?,
        GcodeChunk::Layer { z, gcode, metadata } => {
            write_to_printer(&gcode)?;
            update_progress_bar(metadata.layer_index, metadata.total_layers);
        }
        GcodeChunk::Footer(footer) => write_to_printer(&footer)?,
    }
}
```

```rust
pub enum GcodeChunk {
    /// Initialization G-code (start sequence, header comments).
    Header(String),

    /// One complete layer of G-code.
    Layer {
        z: f64,
        gcode: String,
        metadata: LayerMetadata,
    },

    /// Finalization G-code (end sequence, statistics comments).
    Footer(String),
}

pub struct LayerMetadata {
    pub layer_index: u32,
    pub total_layers: u32,
    pub z: f64,
    pub layer_height: f64,
    pub estimated_layer_time: Duration,
    pub filament_used_mm: f64,
    pub move_count: u32,
}
```

### 2.8 Complete Example — Minimal Slice

```rust
use slicecore_engine::{Engine, EngineConfig, SliceJob, ModelInput, OutputOptions};
use slicecore_config::PrintConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create engine
    let engine = Engine::new(EngineConfig::default())?;

    // 2. Build job
    let job = SliceJob::builder()
        .model(ModelInput::File("calibration_cube.stl".into()))
        .config(PrintConfig::from_file("profiles/pla_standard.toml")?)
        .output(OutputOptions {
            gcode_path: Some("output.gcode".into()),
            include_metadata: true,
            ..Default::default()
        })
        .build()?;

    // 3. Slice
    let result = engine.slice(&job)?;

    // 4. Report
    println!("Sliced {} layers in {:.1}s",
        result.metadata.layer_count,
        result.metadata.pipeline_timing.total.as_secs_f64());
    println!("Filament: {:.1}m ({:.1}g)",
        result.metadata.filament_usage[0].length_mm / 1000.0,
        result.metadata.filament_usage[0].weight_grams.unwrap_or(0.0));
    println!("Estimated print time: {}",
        humantime::format_duration(result.metadata.estimated_time));

    // 5. Report warnings
    for warning in &result.warnings {
        eprintln!("[{}] {}", warning.severity, warning.message);
    }

    Ok(())
}
```

---

## 3. CLI Interface (`slicecore-cli`)

The CLI is built with `clap` (derive mode) and serves as both a user tool and an automation target. All output defaults to human-readable format; machine-readable JSON is available via `--json`.

### 3.1 Command Structure

```
slicecore <COMMAND> [OPTIONS]

COMMANDS:
    slice       Slice model(s) to G-code
    analyze     Analyze model geometry and printability
    validate    Validate configuration files
    profile     Profile management (list, show, diff, convert, create)
    info        Display model information
    schema      Export settings schema
    version     Version and build information
    completions Generate shell completions
```

### 3.2 `slice` — Primary Slicing Command

```bash
# Minimal — slice with defaults
slicecore slice model.stl

# Full-featured
slicecore slice model.stl \
    --output output.gcode \
    --printer profiles/printer/ender3_v3.toml \
    --filament profiles/filament/generic_pla.toml \
    --quality profiles/quality/standard.toml \
    --set layer_height=0.16 \
    --set infill.density=0.20 \
    --set infill.pattern=gyroid \
    --metadata output_metadata.json \
    --preview output_preview.json \
    --thumbnails 300x300,32x32 \
    --comment-level normal \
    --threads 8 \
    --progress

# Batch slicing — multiple models
slicecore slice *.stl --output-dir ./gcode/ --config batch.toml

# Pipe-friendly (read STL from stdin, write G-code to stdout)
cat model.stl | slicecore slice --format stl --config pla.toml > output.gcode

# Sequential printing — multiple models printed one at a time
slicecore slice part_a.stl part_b.stl part_c.stl \
    --sequential \
    --output plate.gcode
```

**`slice` options:**

| Flag | Short | Description |
|------|-------|-------------|
| `--output <PATH>` | `-o` | Output G-code file path (default: `<model>.gcode`) |
| `--output-dir <DIR>` | | Output directory for batch mode |
| `--config <PATH>` | `-c` | Merged TOML configuration file |
| `--printer <PATH>` | `-p` | Printer profile |
| `--filament <PATH>` | `-f` | Filament profile |
| `--quality <PATH>` | `-q` | Quality preset |
| `--set <KEY=VALUE>` | `-s` | Override individual settings (repeatable) |
| `--metadata <PATH>` | `-m` | Write structured metadata JSON to file |
| `--preview <PATH>` | | Write preview data JSON to file |
| `--thumbnails <WxH,...>` | | Embed thumbnails (comma-separated sizes) |
| `--comment-level <LEVEL>` | | G-code comment verbosity: none, minimal, normal, verbose |
| `--format <FMT>` | | Input format hint: auto, stl, 3mf, obj, step, amf |
| `--threads <N>` | `-t` | Worker thread count (default: auto) |
| `--progress` | | Show progress bar on stderr |
| `--json` | `-j` | Output results as JSON (for scripting) |
| `--quiet` | | Suppress all output except errors |
| `--sequential` | | Print objects one at a time (sequential mode) |
| `--dry-run` | | Validate inputs and config without slicing |

**Exit codes:**

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Slicing error (check stderr) |
| 2 | Configuration error (invalid settings) |
| 3 | I/O error (file not found, permission denied) |
| 4 | Model error (unparseable, non-manifold with no auto-repair) |
| 130 | Cancelled (SIGINT / Ctrl+C) |

### 3.3 `analyze` — Model Analysis

```bash
# Human-readable analysis
slicecore analyze model.stl

# JSON output for automation
slicecore analyze model.stl --json

# Analyze with a specific printer profile (for bed size context)
slicecore analyze model.stl --printer ender3_v3.toml

# Check if model fits on bed
slicecore analyze model.stl --printer ender3_v3.toml --check-fit
```

**Example output:**

```
Model: model.stl
  Triangles:    48,204
  Vertices:     24,106
  Volume:       15.3 cm3
  Surface area: 82.7 cm2
  Bounding box: 40.0 x 35.0 x 60.0 mm
  Manifold:     Yes
  Watertight:   Yes
  Components:   1

Printability Analysis:
  Overhangs:    3 regions (max 62 degrees)
  Bridges:      1 span (12.5 mm)
  Thin walls:   2 regions (min 0.38 mm)
  Complexity:   0.45 / 1.0 (moderate)

Recommendations:
  - Supports recommended for 62-degree overhang at Z=32.4mm
  - Thin wall at X=12.1 Y=8.3 may require 0.25mm nozzle or Arachne perimeters
  - Bridge at Z=45.0mm spans 12.5mm — consider bridge speed/fan settings
```

### 3.4 `validate` — Configuration Validation

```bash
# Validate a single config file
slicecore validate config.toml

# Validate merged config hierarchy
slicecore validate \
    --printer printer.toml \
    --filament filament.toml \
    --quality quality.toml

# Validate with overrides (check if overrides are valid)
slicecore validate config.toml --set layer_height=2.0

# Output as JSON
slicecore validate config.toml --json
```

**Example output:**

```
Validating: config.toml
  Settings:  142 defined, 142 valid
  Warnings:
    - layer_height (0.32) exceeds 80% of nozzle_diameter (0.4): may cause poor adhesion
    - infill.density (0.05) is very low: part may be fragile
    - speed.outer_wall (250) exceeds recommended max for this printer (200)
  Errors:
    (none)

Result: VALID (3 warnings)
```

### 3.5 `profile` — Profile Management

```bash
# List available profiles
slicecore profile list
slicecore profile list --type printer
slicecore profile list --type filament --json

# Show a profile's settings
slicecore profile show pla_standard

# Diff two profiles
slicecore profile diff pla_standard pla_fast

# Convert legacy PrusaSlicer/OrcaSlicer INI to TOML
slicecore profile convert PrusaSlicer_config_bundle.ini --output-dir profiles/

# Create a new profile interactively
slicecore profile create --type quality --name "my_fast_draft"

# Export profile as JSON (for API consumption)
slicecore profile export pla_standard --format json
```

**Example `profile diff` output:**

```
Comparing: pla_standard vs pla_fast

  Setting                    | pla_standard | pla_fast
  ---------------------------+--------------+---------
  layer_height               | 0.20 mm      | 0.28 mm
  perimeters.wall_count      | 3            | 2
  infill.density             | 0.20         | 0.15
  speed.outer_wall           | 150 mm/s     | 200 mm/s
  speed.inner_wall           | 180 mm/s     | 250 mm/s
  speed.infill               | 200 mm/s     | 300 mm/s
  quality.top_surface_layers | 4            | 3

  7 differences found (135 settings identical)
```

### 3.6 `info` — Model Information

```bash
# Quick model stats (no analysis, just parsing)
slicecore info model.stl
slicecore info model.3mf --json

# Multiple files
slicecore info *.stl --json
```

**Example output:**

```
File:       model.stl
Format:     STL (binary)
Size:       2.4 MiB
Triangles:  48,204
Vertices:   24,106
Bounding box:
  X: 0.000 to 40.000 mm (40.000 mm)
  Y: 0.000 to 35.000 mm (35.000 mm)
  Z: 0.000 to 60.000 mm (60.000 mm)
Volume:     15.3 cm3
Manifold:   Yes
```

### 3.7 `schema` — Settings Schema Export

```bash
# Export full schema as JSON (for UI generation, documentation)
slicecore schema --format json > schema.json

# Export as JSON Schema (for validation in other tools)
slicecore schema --format json-schema > schema.jsonschema

# Filter by category
slicecore schema --category speed --format json

# Filter by tier
slicecore schema --tier simple --format json
```

---

## 4. REST API (`slicecore-server`)

The REST API server is built with `axum` on Tokio. It exposes the full engine capability over HTTP with JSON request/response bodies and multipart file uploads.

### 4.1 Server Configuration

```bash
# Start server with defaults (localhost:3000)
slicecore-server

# Custom bind address and thread pool
slicecore-server --bind 0.0.0.0:8080 --threads 16

# With API key authentication
slicecore-server --bind 0.0.0.0:8080 --api-key-file /etc/slicecore/api_keys.toml

# Environment variable configuration
export SLICECORE_BIND=0.0.0.0:8080
export SLICECORE_THREADS=16
export SLICECORE_MAX_UPLOAD_SIZE=100MB
export SLICECORE_API_KEY=sk-...
slicecore-server
```

### 4.2 Endpoints

#### `POST /api/v1/slice` — Submit Slice Job

Accepts multipart form data with model file(s) and JSON configuration.

**Request:**

```http
POST /api/v1/slice HTTP/1.1
Content-Type: multipart/form-data; boundary=----boundary

------boundary
Content-Disposition: form-data; name="model"; filename="part.stl"
Content-Type: application/octet-stream

<binary STL data>
------boundary
Content-Disposition: form-data; name="config"
Content-Type: application/json

{
  "printer": "ender3_v3",
  "filament": "generic_pla",
  "quality": "standard",
  "overrides": {
    "layer_height": 0.16,
    "infill.density": 0.20,
    "infill.pattern": "gyroid"
  },
  "output": {
    "include_metadata": true,
    "include_preview": false,
    "comment_level": "normal",
    "thumbnails": ["300x300"]
  }
}
------boundary--
```

**Response (202 Accepted):**

```json
{
  "job_id": "slice_01HXYZ...",
  "status": "queued",
  "created_at": "2026-02-14T10:30:00Z",
  "poll_url": "/api/v1/slice/slice_01HXYZ...",
  "cancel_url": "/api/v1/slice/slice_01HXYZ.../cancel",
  "websocket_url": "/api/v1/slice/slice_01HXYZ.../ws"
}
```

#### `GET /api/v1/slice/:id` — Poll Status / Get Result

**Response (in progress):**

```json
{
  "job_id": "slice_01HXYZ...",
  "status": "running",
  "progress": {
    "stage": "infill",
    "stage_progress": 0.45,
    "overall_progress": 0.62,
    "current_layer": 142,
    "total_layers": 310,
    "message": "Generating infill for layer 142/310",
    "elapsed_seconds": 4.2,
    "estimated_remaining_seconds": 2.5
  }
}
```

**Response (complete):**

```json
{
  "job_id": "slice_01HXYZ...",
  "status": "complete",
  "result": {
    "gcode_url": "/api/v1/slice/slice_01HXYZ.../gcode",
    "gcode_size_bytes": 4821503,
    "metadata": {
      "estimated_time_seconds": 5420,
      "layer_count": 310,
      "filament_usage": [
        {
          "extruder": 0,
          "length_mm": 12450.3,
          "weight_grams": 37.2,
          "cost_usd": 0.74
        }
      ],
      "print_bounds": {
        "min": [80.0, 85.0, 0.0],
        "max": [120.0, 115.0, 60.0]
      },
      "config_hash": "sha256:a1b2c3d4..."
    },
    "warnings": [
      {
        "severity": "advisory",
        "message": "Overhang detected at 52 degrees, consider supports",
        "location": { "z": 32.4 }
      }
    ]
  },
  "timing": {
    "queued_at": "2026-02-14T10:30:00Z",
    "started_at": "2026-02-14T10:30:01Z",
    "completed_at": "2026-02-14T10:30:08Z"
  }
}
```

**Response (error):**

```json
{
  "job_id": "slice_01HXYZ...",
  "status": "failed",
  "error": {
    "code": "MESH_NOT_MANIFOLD",
    "message": "Mesh has 3 non-manifold edges; auto-repair failed",
    "details": {
      "non_manifold_edges": [
        { "v1": 1024, "v2": 1025 },
        { "v1": 2048, "v2": 2049 },
        { "v1": 3072, "v2": 3073 }
      ]
    }
  }
}
```

#### `GET /api/v1/slice/:id/gcode` — Download G-code

Returns the raw G-code file as `text/plain` or `application/gzip` (if `Accept-Encoding: gzip`).

```http
GET /api/v1/slice/slice_01HXYZ.../gcode HTTP/1.1
Accept-Encoding: gzip

HTTP/1.1 200 OK
Content-Type: text/plain; charset=utf-8
Content-Encoding: gzip
Content-Disposition: attachment; filename="part.gcode"
Content-Length: 1240512

<gzipped G-code data>
```

#### `POST /api/v1/slice/:id/cancel` — Cancel Running Job

```http
POST /api/v1/slice/slice_01HXYZ.../cancel HTTP/1.1

HTTP/1.1 200 OK
{ "job_id": "slice_01HXYZ...", "status": "cancelled" }
```

#### `WebSocket /api/v1/slice/:id/ws` — Real-time Progress

```javascript
const ws = new WebSocket("/api/v1/slice/slice_01HXYZ.../ws");
ws.onmessage = (event) => {
    const progress = JSON.parse(event.data);
    // { "type": "progress", "stage": "infill", "overall": 0.62, ... }
    // { "type": "warning", "severity": "advisory", "message": "..." }
    // { "type": "complete", "result": { ... } }
    // { "type": "error", "error": { ... } }
};
```

#### `POST /api/v1/analyze` — Analyze Model

```http
POST /api/v1/analyze HTTP/1.1
Content-Type: multipart/form-data; boundary=----boundary

------boundary
Content-Disposition: form-data; name="model"; filename="part.stl"
Content-Type: application/octet-stream

<binary STL data>
------boundary
Content-Disposition: form-data; name="options"
Content-Type: application/json

{
  "printer": "ender3_v3",
  "check_fit": true,
  "suggest_orientation": true
}
------boundary--
```

**Response:**

```json
{
  "geometry": {
    "triangles": 48204,
    "vertices": 24106,
    "volume_cm3": 15.3,
    "surface_area_cm2": 82.7,
    "bounding_box": {
      "min": [0.0, 0.0, 0.0],
      "max": [40.0, 35.0, 60.0]
    },
    "is_manifold": true,
    "is_watertight": true,
    "connected_components": 1
  },
  "printability": {
    "complexity_score": 0.45,
    "overhangs": [
      { "max_angle_degrees": 62, "z_range": [30.0, 35.0] }
    ],
    "bridges": [
      { "span_mm": 12.5, "z": 45.0 }
    ],
    "thin_walls": [
      { "min_width_mm": 0.38, "location": [12.1, 8.3] }
    ]
  },
  "fits_on_bed": true,
  "suggested_orientation": {
    "rotation_degrees": [0, 0, 45],
    "reason": "Minimizes overhang area by 35%"
  },
  "recommendations": [
    "Supports recommended for 62-degree overhang at Z=32.4mm",
    "Thin wall at X=12.1 Y=8.3 may require Arachne perimeters"
  ]
}
```

#### `GET /api/v1/schema` — Settings Schema

```http
GET /api/v1/schema HTTP/1.1
Accept: application/json

GET /api/v1/schema?tier=simple HTTP/1.1
GET /api/v1/schema?category=speed HTTP/1.1
```

**Response:**

```json
{
  "version": "0.1.0",
  "settings_count": 850,
  "settings": [
    {
      "key": "layer_height",
      "display_name": "Layer Height",
      "description": "Height of each printed layer in millimeters",
      "type": "float",
      "default": 0.2,
      "min": 0.01,
      "max": 1.0,
      "precision": 2,
      "units": "mm",
      "tier": "simple",
      "category": "layers_and_perimeters",
      "tags": ["quality", "speed"],
      "affects": ["perimeters.thin_wall_detection", "infill.line_spacing"],
      "affected_by": ["printer.nozzle_diameter"]
    }
  ],
  "categories": [
    { "id": "layers_and_perimeters", "display_name": "Layers & Perimeters" },
    { "id": "infill", "display_name": "Infill" },
    { "id": "speed", "display_name": "Speed" },
    { "id": "support", "display_name": "Support" },
    { "id": "cooling", "display_name": "Cooling" },
    { "id": "filament", "display_name": "Filament" },
    { "id": "printer", "display_name": "Printer" },
    { "id": "advanced", "display_name": "Advanced" }
  ],
  "tiers": ["auto", "simple", "intermediate", "advanced", "developer"]
}
```

#### `GET /api/v1/profiles` — List Profiles

```http
GET /api/v1/profiles HTTP/1.1
GET /api/v1/profiles?type=printer HTTP/1.1
GET /api/v1/profiles?type=filament&search=pla HTTP/1.1
```

**Response:**

```json
{
  "profiles": [
    {
      "id": "generic_pla",
      "name": "Generic PLA",
      "type": "filament",
      "description": "Standard PLA settings for most printers",
      "author": "slicecore-builtin",
      "version": "1.0.0",
      "compatible_printers": ["*"],
      "key_settings": {
        "temperature.nozzle": 210,
        "temperature.bed": 60,
        "speed.outer_wall": 150
      }
    }
  ],
  "total": 42
}
```

#### `GET /api/v1/profiles/:id` — Get Profile Details

```http
GET /api/v1/profiles/generic_pla HTTP/1.1
```

Returns the full configuration as JSON.

#### `GET /api/v1/health` — Health Check

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 3600,
  "active_jobs": 2,
  "completed_jobs": 47,
  "engine_threads": 8
}
```

### 4.3 Error Response Format

All errors follow a consistent structure.

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Invalid configuration: layer_height must be between 0.01 and 1.0",
    "details": {
      "field": "layer_height",
      "value": -0.5,
      "constraint": "min=0.01"
    },
    "request_id": "req_01HXYZ..."
  }
}
```

**Error codes:**

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `VALIDATION_ERROR` | 400 | Invalid request parameters or configuration |
| `MODEL_PARSE_ERROR` | 400 | Model file could not be parsed |
| `UNSUPPORTED_FORMAT` | 400 | Model format not supported |
| `JOB_NOT_FOUND` | 404 | Slice job ID does not exist |
| `PROFILE_NOT_FOUND` | 404 | Profile ID does not exist |
| `JOB_ALREADY_COMPLETE` | 409 | Cannot cancel a completed job |
| `UPLOAD_TOO_LARGE` | 413 | Model file exceeds size limit |
| `RATE_LIMITED` | 429 | Too many requests |
| `INTERNAL_ERROR` | 500 | Unexpected server error |
| `MESH_ERROR` | 422 | Mesh has issues that could not be auto-repaired |
| `SLICE_ERROR` | 422 | Slicing failed (degenerate geometry, etc.) |

### 4.4 Authentication

```http
# API key in header
GET /api/v1/profiles HTTP/1.1
Authorization: Bearer sk-slicecore-abc123...

# Or as query parameter (for WebSocket connections)
GET /api/v1/slice/id/ws?api_key=sk-slicecore-abc123...
```

Authentication is optional for local development and mandatory when deployed with `--api-key-file`.

---

## 5. C FFI (`slicecore-ffi`)

The C FFI provides a stable, header-based interface for embedding SliceCore-RS in C, C++, Swift, Zig, and other languages that can call C functions. Generated with `cbindgen`.

### 5.1 Design Principles

- All public functions are `extern "C"` with `#[no_mangle]`.
- Opaque pointer types for complex structures (`SlicecoreEngine*`).
- Errors returned as integer codes with a separate `slicecore_last_error()` function for detailed messages.
- Strings are null-terminated UTF-8 (`const char*`). Caller-owned strings must be freed with `slicecore_string_free()`.
- No exceptions, no panics across the FFI boundary (all panics caught and converted to error codes).

### 5.2 C Header (`slicecore.h`)

```c
#ifndef SLICECORE_H
#define SLICECORE_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ===== Opaque Types ===== */

typedef struct SlicecoreEngine SlicecoreEngine;
typedef struct SlicecoreJob    SlicecoreJob;
typedef struct SlicecoreResult SlicecoreResult;
typedef struct SlicecoreConfig SlicecoreConfig;

/* ===== Error Handling ===== */

typedef enum {
    SLICECORE_OK = 0,
    SLICECORE_ERR_NULL_POINTER = -1,
    SLICECORE_ERR_INVALID_CONFIG = -2,
    SLICECORE_ERR_IO = -3,
    SLICECORE_ERR_MESH = -4,
    SLICECORE_ERR_SLICE = -5,
    SLICECORE_ERR_CANCELLED = -6,
    SLICECORE_ERR_PLUGIN = -7,
    SLICECORE_ERR_INTERNAL = -99,
} SlicecoreError;

/**
 * Get the last error message (thread-local).
 * Returns a null-terminated UTF-8 string. Caller must NOT free this pointer.
 * The string is valid until the next slicecore_* call on the same thread.
 */
const char* slicecore_last_error(void);

/**
 * Get the last error code (thread-local).
 */
SlicecoreError slicecore_last_error_code(void);

/* ===== Engine Lifecycle ===== */

/**
 * Create a new engine with default configuration.
 * Returns NULL on failure (check slicecore_last_error).
 */
SlicecoreEngine* slicecore_engine_new(void);

/**
 * Create a new engine with the given number of threads.
 * Pass 0 for auto-detection.
 */
SlicecoreEngine* slicecore_engine_new_with_threads(uint32_t thread_count);

/**
 * Destroy an engine and free all associated resources.
 */
void slicecore_engine_free(SlicecoreEngine* engine);

/* ===== Configuration ===== */

/**
 * Create a config from a TOML file.
 */
SlicecoreConfig* slicecore_config_from_file(const char* path);

/**
 * Create a config from a JSON string.
 */
SlicecoreConfig* slicecore_config_from_json(const char* json);

/**
 * Set a single configuration value. Key and value are UTF-8 strings.
 * Returns SLICECORE_OK on success.
 */
SlicecoreError slicecore_config_set(
    SlicecoreConfig* config,
    const char* key,
    const char* value
);

/**
 * Get a configuration value as a string.
 * Caller must free the returned string with slicecore_string_free.
 * Returns NULL if the key does not exist.
 */
char* slicecore_config_get(const SlicecoreConfig* config, const char* key);

/**
 * Destroy a config object.
 */
void slicecore_config_free(SlicecoreConfig* config);

/* ===== Slicing ===== */

/**
 * Slice a model file with the given configuration.
 * Writes G-code to output_path.
 * Returns SLICECORE_OK on success.
 */
SlicecoreError slicecore_slice_file(
    SlicecoreEngine* engine,
    const char* model_path,
    const SlicecoreConfig* config,
    const char* output_path
);

/**
 * Slice model bytes in memory.
 * model_data: pointer to model file bytes (STL, 3MF, etc.)
 * model_len: length of model data in bytes
 * format: format hint string ("stl", "3mf", "auto") or NULL for auto-detection
 * Returns a SlicecoreResult or NULL on failure.
 */
SlicecoreResult* slicecore_slice_bytes(
    SlicecoreEngine* engine,
    const uint8_t* model_data,
    size_t model_len,
    const char* format,
    const SlicecoreConfig* config
);

/* ===== Progress Callback ===== */

/**
 * Progress callback function type.
 * stage: current pipeline stage name (UTF-8)
 * progress: overall progress (0.0 to 1.0)
 * message: human-readable status message (UTF-8)
 * user_data: opaque pointer passed to slicecore_set_progress_callback
 *
 * Return 0 to continue, non-zero to cancel the operation.
 */
typedef int (*SlicecoreProgressCallback)(
    const char* stage,
    double progress,
    const char* message,
    void* user_data
);

/**
 * Set a progress callback on the engine.
 * The callback will be invoked during slice operations.
 */
void slicecore_engine_set_progress_callback(
    SlicecoreEngine* engine,
    SlicecoreProgressCallback callback,
    void* user_data
);

/* ===== Result Access ===== */

/**
 * Get G-code from a slice result as a UTF-8 string.
 * Caller must free with slicecore_string_free.
 */
char* slicecore_result_gcode(const SlicecoreResult* result);

/**
 * Get G-code length in bytes.
 */
size_t slicecore_result_gcode_len(const SlicecoreResult* result);

/**
 * Get metadata as a JSON string.
 * Caller must free with slicecore_string_free.
 */
char* slicecore_result_metadata_json(const SlicecoreResult* result);

/**
 * Get estimated print time in seconds.
 */
double slicecore_result_estimated_time(const SlicecoreResult* result);

/**
 * Get layer count.
 */
uint32_t slicecore_result_layer_count(const SlicecoreResult* result);

/**
 * Get filament usage in millimeters for a given extruder.
 */
double slicecore_result_filament_mm(const SlicecoreResult* result, uint8_t extruder);

/**
 * Get warning count.
 */
uint32_t slicecore_result_warning_count(const SlicecoreResult* result);

/**
 * Get warning message at index.
 * Caller must free with slicecore_string_free.
 */
char* slicecore_result_warning(const SlicecoreResult* result, uint32_t index);

/**
 * Destroy a result object.
 */
void slicecore_result_free(SlicecoreResult* result);

/* ===== Utility ===== */

/**
 * Free a string returned by slicecore_* functions.
 */
void slicecore_string_free(char* s);

/**
 * Get the library version string (e.g., "0.1.0").
 * The returned pointer is static — do NOT free it.
 */
const char* slicecore_version(void);

#ifdef __cplusplus
}
#endif

#endif /* SLICECORE_H */
```

### 5.3 C Usage Example

```c
#include "slicecore.h"
#include <stdio.h>

int progress_cb(const char* stage, double progress, const char* message, void* data) {
    printf("[%3.0f%%] %s: %s\n", progress * 100.0, stage, message);
    return 0; /* 0 = continue, non-zero = cancel */
}

int main(void) {
    /* Create engine */
    SlicecoreEngine* engine = slicecore_engine_new();
    if (!engine) {
        fprintf(stderr, "Failed to create engine: %s\n", slicecore_last_error());
        return 1;
    }

    /* Set progress callback */
    slicecore_engine_set_progress_callback(engine, progress_cb, NULL);

    /* Load config */
    SlicecoreConfig* config = slicecore_config_from_file("profiles/pla_standard.toml");
    if (!config) {
        fprintf(stderr, "Config error: %s\n", slicecore_last_error());
        slicecore_engine_free(engine);
        return 1;
    }
    slicecore_config_set(config, "layer_height", "0.16");

    /* Slice */
    SlicecoreError err = slicecore_slice_file(engine, "model.stl", config, "output.gcode");
    if (err != SLICECORE_OK) {
        fprintf(stderr, "Slice failed: %s\n", slicecore_last_error());
    } else {
        printf("Slice complete! Output written to output.gcode\n");
    }

    /* Cleanup */
    slicecore_config_free(config);
    slicecore_engine_free(engine);
    return (err == SLICECORE_OK) ? 0 : 1;
}
```

### 5.4 Rust FFI Implementation Pattern

```rust
// crates/slicecore-ffi/src/lib.rs

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = RefCell::new(None);
}

fn set_last_error(err: impl std::fmt::Display) {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some(
            CString::new(err.to_string()).unwrap_or_default()
        );
    });
}

/// Catch panics at the FFI boundary and convert to error codes.
fn catch_panic<F, T>(default: T, f: F) -> T
where
    F: FnOnce() -> Result<T, Box<dyn std::error::Error>> + std::panic::UnwindSafe,
{
    match std::panic::catch_unwind(f) {
        Ok(Ok(val)) => val,
        Ok(Err(err)) => {
            set_last_error(err);
            default
        }
        Err(_) => {
            set_last_error("Internal panic caught at FFI boundary");
            default
        }
    }
}

#[no_mangle]
pub extern "C" fn slicecore_engine_new() -> *mut SlicecoreEngine {
    catch_panic(ptr::null_mut(), || {
        let engine = Engine::new(EngineConfig::default())?;
        Ok(Box::into_raw(Box::new(SlicecoreEngine { inner: engine })))
    })
}

#[no_mangle]
pub extern "C" fn slicecore_engine_free(engine: *mut SlicecoreEngine) {
    if !engine.is_null() {
        unsafe { drop(Box::from_raw(engine)); }
    }
}
```

---

## 6. Python Bindings (`slicecore-python`)

Python bindings are built with PyO3 and published to PyPI as `slicecore`. The API mirrors the Rust library API with Pythonic conventions (snake_case, context managers, exceptions).

### 6.1 Installation

```bash
pip install slicecore
```

### 6.2 Python API

```python
import slicecore

# Create engine
engine = slicecore.Engine()

# Or with options
engine = slicecore.Engine(threads=8, plugin_dir="/usr/lib/slicecore/plugins")

# Simple slice
result = engine.slice(
    model="model.stl",
    config="profiles/pla_standard.toml",
    output="output.gcode",
)

print(f"Layers: {result.layer_count}")
print(f"Time: {result.estimated_time}")
print(f"Filament: {result.filament_usage[0].length_mm:.1f} mm")
print(f"Warnings: {len(result.warnings)}")
```

### 6.3 Configuration

```python
# Load from file
config = slicecore.Config.from_file("profiles/pla_standard.toml")

# Build programmatically
config = slicecore.Config.builder() \
    .defaults() \
    .printer("profiles/printer/ender3_v3.toml") \
    .filament("profiles/filament/generic_pla.toml") \
    .quality("profiles/quality/standard.toml") \
    .set("layer_height", 0.16) \
    .set("infill.density", 0.20) \
    .set("infill.pattern", "gyroid") \
    .build()

# Access settings
print(config["layer_height"])          # 0.16
print(config.get("infill.density"))    # 0.2

# Modify
config["layer_height"] = 0.20

# Validate
report = config.validate()
if report.errors:
    for err in report.errors:
        print(f"ERROR: {err}")
for warn in report.warnings:
    print(f"WARNING: {warn}")

# Diff
diffs = slicecore.Config.diff(config_a, config_b)
for d in diffs:
    print(f"{d.key}: {d.old_value} -> {d.new_value}")

# Import legacy profile
config = slicecore.Config.from_legacy_ini("PrusaSlicer_config.ini")
```

### 6.4 Progress and Cancellation

```python
import slicecore
import signal

engine = slicecore.Engine()

# Progress callback
def on_progress(progress):
    print(f"[{progress.overall_progress:.0%}] {progress.stage}: {progress.message}")

# Cancellation
cancel = slicecore.CancellationToken()
signal.signal(signal.SIGINT, lambda *_: cancel.cancel())

# Slice with progress
result = engine.slice(
    model="model.stl",
    config=config,
    output="output.gcode",
    progress_callback=on_progress,
    cancel_token=cancel,
)
```

### 6.5 Async Support

```python
import asyncio
import slicecore

async def main():
    engine = slicecore.Engine()
    config = slicecore.Config.from_file("profiles/pla_standard.toml")

    # Async slice with progress streaming
    async with engine.slice_async(
        model="model.stl",
        config=config,
        output="output.gcode",
    ) as handle:
        async for progress in handle.progress_stream():
            print(f"[{progress.overall_progress:.0%}] {progress.message}")

        result = await handle.result()
        print(f"Done! {result.layer_count} layers")

asyncio.run(main())
```

### 6.6 Model Analysis

```python
import slicecore

engine = slicecore.Engine()

report = engine.analyze("model.stl")

print(f"Volume: {report.geometry.volume_cm3:.1f} cm3")
print(f"Manifold: {report.geometry.is_manifold}")
print(f"Complexity: {report.printability.complexity_score:.2f}")

for overhang in report.printability.overhangs:
    print(f"Overhang: {overhang.max_angle_degrees} degrees at Z={overhang.z_range}")

for rec in report.recommendations:
    print(f"  - {rec}")
```

### 6.7 Batch Processing

```python
import slicecore
from pathlib import Path

engine = slicecore.Engine(threads=16)
config = slicecore.Config.from_file("profiles/pla_standard.toml")

# Batch slice all STL files in a directory
models = list(Path("models/").glob("*.stl"))

results = engine.slice_batch(
    models=[str(m) for m in models],
    config=config,
    output_dir="gcode/",
    progress_callback=lambda p: print(f"[{p.overall_progress:.0%}] {p.message}"),
)

for model, result in zip(models, results):
    if result.is_ok():
        r = result.unwrap()
        print(f"{model.name}: {r.layer_count} layers, {r.estimated_time}")
    else:
        print(f"{model.name}: FAILED - {result.error()}")
```

### 6.8 Schema Introspection

```python
import slicecore

engine = slicecore.Engine()
schema = engine.schema()

# List all simple-tier settings
for setting in schema.settings(tier="simple"):
    print(f"{setting.key}: {setting.display_name} ({setting.value_type})")
    print(f"  Default: {setting.default}")
    print(f"  Range: {setting.min} - {setting.max}")

# Export as JSON
with open("schema.json", "w") as f:
    f.write(schema.to_json())
```

### 6.9 PyO3 Implementation Pattern

```rust
// crates/slicecore-python/src/lib.rs
use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;

#[pyclass(name = "Engine")]
struct PyEngine {
    inner: slicecore_engine::Engine,
}

#[pymethods]
impl PyEngine {
    #[new]
    #[pyo3(signature = (threads=None, plugin_dir=None))]
    fn new(threads: Option<usize>, plugin_dir: Option<String>) -> PyResult<Self> {
        let config = slicecore_engine::EngineConfig {
            thread_count: threads,
            plugin_dir: plugin_dir.map(PathBuf::from),
            ..Default::default()
        };
        let engine = slicecore_engine::Engine::new(config)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(PyEngine { inner: engine })
    }

    #[pyo3(signature = (model, config, output=None, progress_callback=None, cancel_token=None))]
    fn slice(
        &self,
        py: Python<'_>,
        model: &str,
        config: &PyConfig,
        output: Option<&str>,
        progress_callback: Option<PyObject>,
        cancel_token: Option<&PyCancellationToken>,
    ) -> PyResult<PySliceResult> {
        // Release the GIL during slicing so Python threads can run
        py.allow_threads(|| {
            let job = build_slice_job(model, &config.inner, output)?;
            let result = self.inner.slice(&job)
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            Ok(PySliceResult::from(result))
        })
    }

    fn analyze(&self, py: Python<'_>, model: &str) -> PyResult<PyAnalysisReport> {
        py.allow_threads(|| {
            let input = slicecore_engine::ModelInput::File(PathBuf::from(model));
            let report = self.inner.analyze(&input)
                .map_err(|e| PyValueError::new_err(e.to_string()))?;
            Ok(PyAnalysisReport::from(report))
        })
    }
}

#[pyclass(name = "Config")]
struct PyConfig {
    inner: slicecore_config::PrintConfig,
}

#[pyclass(name = "SliceResult")]
struct PySliceResult {
    #[pyo3(get)]
    layer_count: u32,
    #[pyo3(get)]
    estimated_time: f64,
    #[pyo3(get)]
    warnings: Vec<String>,
    inner: slicecore_engine::SliceResult,
}

#[pymodule]
fn slicecore(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyEngine>()?;
    m.add_class::<PyConfig>()?;
    m.add_class::<PySliceResult>()?;
    m.add_class::<PyAnalysisReport>()?;
    m.add_class::<PyCancellationToken>()?;
    Ok(())
}
```

---

## 7. WASM Interface (`slicecore-wasm`)

The WASM build compiles the core slicing engine to WebAssembly via `wasm-bindgen`, enabling browser-based slicing without a server round-trip.

### 7.1 Build Configuration

```toml
# crates/slicecore-wasm/Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
slicecore-engine = { path = "../slicecore-engine", default-features = false, features = ["wasm"] }
wasm-bindgen = "0.2"
serde-wasm-bindgen = "0.6"
js-sys = "0.3"
web-sys = { version = "0.3", features = ["console"] }
wasm-bindgen-futures = "0.4"

[profile.release]
opt-level = "z"       # Minimize binary size
lto = true
codegen-units = 1
strip = true
```

**Excluded from WASM build** (via feature flags):
- `slicecore-ai` (requires network I/O, external HTTP clients)
- `slicecore-plugin` (requires dynamic library loading)
- `slicecore-api` (requires TCP sockets)
- `rayon` (replaced with single-threaded execution or Web Workers)

### 7.2 JavaScript/TypeScript API

```typescript
// TypeScript type declarations (auto-generated by wasm-bindgen)
export class SlicerEngine {
    constructor();
    free(): void;

    /**
     * Slice a model from raw bytes.
     * @param modelBytes - ArrayBuffer or Uint8Array of the model file
     * @param format - Format hint: "stl", "3mf", "obj", or "auto"
     * @param configJson - JSON string of configuration overrides
     * @returns Promise<SliceResult> - resolves when slicing is complete
     */
    slice(
        modelBytes: Uint8Array,
        format: string,
        configJson: string
    ): Promise<SliceResult>;

    /**
     * Analyze a model without slicing.
     */
    analyze(modelBytes: Uint8Array, format: string): Promise<AnalysisResult>;

    /**
     * Get the current progress (0.0 to 1.0).
     * Call during slice() from a separate requestAnimationFrame loop.
     */
    progress(): number;

    /**
     * Get the current stage name.
     */
    currentStage(): string;

    /**
     * Cancel the current operation.
     */
    cancel(): void;

    /**
     * Get the full settings schema as a JSON string.
     */
    schema(): string;

    /**
     * Validate a configuration JSON string.
     * Returns a JSON string with validation results.
     */
    validateConfig(configJson: string): string;

    /**
     * Get the engine version.
     */
    static version(): string;
}

export interface SliceResult {
    /** G-code as a string */
    gcode: string;
    /** Structured metadata */
    metadata: SliceMetadata;
    /** Warnings generated during slicing */
    warnings: SliceWarning[];
    /** Per-layer preview data (if requested in config) */
    preview?: PreviewData;
}

export interface SliceMetadata {
    estimated_time_seconds: number;
    layer_count: number;
    filament_usage: FilamentUsage[];
    print_bounds: BoundingBox;
    move_counts: MoveCounts;
}

export interface AnalysisResult {
    geometry: GeometryInfo;
    printability: PrintabilityInfo;
    recommendations: string[];
}

export interface SliceWarning {
    severity: "info" | "advisory" | "warning" | "critical";
    message: string;
    location?: { z: number };
}
```

### 7.3 Rust WASM Implementation

```rust
// crates/slicecore-wasm/src/lib.rs
use wasm_bindgen::prelude::*;
use serde_wasm_bindgen;

#[wasm_bindgen]
pub struct SlicerEngine {
    engine: slicecore_engine::Engine,
    cancel: slicecore_engine::CancellationToken,
    progress: Arc<AtomicProgress>,
}

#[wasm_bindgen]
impl SlicerEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<SlicerEngine, JsError> {
        // Initialize panic hook for better error messages in the browser console
        console_error_panic_hook::set_once();

        let config = slicecore_engine::EngineConfig {
            thread_count: Some(1), // Single-threaded in WASM main thread
            ..Default::default()
        };
        let engine = slicecore_engine::Engine::new(config)
            .map_err(|e| JsError::new(&e.to_string()))?;

        Ok(SlicerEngine {
            engine,
            cancel: slicecore_engine::CancellationToken::new(),
            progress: Arc::new(AtomicProgress::new()),
        })
    }

    /// Slice model bytes with the given config.
    /// Returns a JsValue that deserializes to SliceResult.
    pub fn slice(
        &mut self,
        model_bytes: &[u8],
        format: &str,
        config_json: &str,
    ) -> Result<JsValue, JsError> {
        self.cancel = slicecore_engine::CancellationToken::new();
        self.progress.reset();

        let model_format = parse_format(format)?;
        let config: slicecore_config::PrintConfig = serde_json::from_str(config_json)
            .map_err(|e| JsError::new(&format!("Invalid config JSON: {}", e)))?;

        let job = slicecore_engine::SliceJob::builder()
            .model(slicecore_engine::ModelInput::Bytes {
                data: model_bytes.to_vec(),
                format: model_format,
            })
            .config(config)
            .build()
            .map_err(|e| JsError::new(&e.to_string()))?;

        let result = self.engine.slice(&job)
            .map_err(|e| JsError::new(&e.to_string()))?;

        serde_wasm_bindgen::to_value(&WasmSliceResult::from(result))
            .map_err(|e| JsError::new(&e.to_string()))
    }

    pub fn analyze(
        &self,
        model_bytes: &[u8],
        format: &str,
    ) -> Result<JsValue, JsError> {
        let model_format = parse_format(format)?;
        let input = slicecore_engine::ModelInput::Bytes {
            data: model_bytes.to_vec(),
            format: model_format,
        };

        let report = self.engine.analyze(&input)
            .map_err(|e| JsError::new(&e.to_string()))?;

        serde_wasm_bindgen::to_value(&WasmAnalysisReport::from(report))
            .map_err(|e| JsError::new(&e.to_string()))
    }

    pub fn progress(&self) -> f64 {
        self.progress.get()
    }

    #[wasm_bindgen(js_name = "currentStage")]
    pub fn current_stage(&self) -> String {
        self.progress.stage()
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub fn schema(&self) -> String {
        serde_json::to_string(self.engine.schema())
            .unwrap_or_else(|_| "{}".to_string())
    }

    #[wasm_bindgen(js_name = "validateConfig")]
    pub fn validate_config(&self, config_json: &str) -> String {
        match serde_json::from_str::<slicecore_config::PrintConfig>(config_json) {
            Ok(config) => {
                let report = self.engine.validate_config(&config);
                serde_json::to_string(&report).unwrap_or_else(|_| "{}".to_string())
            }
            Err(e) => {
                serde_json::json!({
                    "valid": false,
                    "errors": [{ "message": format!("Invalid JSON: {}", e) }]
                }).to_string()
            }
        }
    }

    #[wasm_bindgen]
    pub fn version() -> String {
        slicecore_engine::VERSION.to_string()
    }
}
```

### 7.4 Browser Usage Example

```html
<script type="module">
import init, { SlicerEngine } from './slicecore_wasm.js';

async function sliceModel() {
    // Initialize WASM module
    await init();
    const engine = new SlicerEngine();

    // Load model file from user input
    const fileInput = document.getElementById('model-file');
    const file = fileInput.files[0];
    const modelBytes = new Uint8Array(await file.arrayBuffer());

    // Configure
    const config = JSON.stringify({
        layer_height: 0.2,
        "infill.density": 0.15,
        "infill.pattern": "gyroid",
        "perimeters.wall_count": 3,
    });

    // Show progress
    const progressBar = document.getElementById('progress-bar');
    const progressInterval = setInterval(() => {
        const p = engine.progress();
        progressBar.style.width = `${p * 100}%`;
        progressBar.textContent = `${engine.currentStage()} - ${(p * 100).toFixed(0)}%`;
    }, 100);

    try {
        // Slice (runs synchronously in the main thread; for large models,
        // use a Web Worker — see Section 7.5)
        const result = engine.slice(modelBytes, "auto", config);

        clearInterval(progressInterval);
        progressBar.style.width = '100%';
        progressBar.textContent = 'Complete!';

        // Display results
        console.log(`Layers: ${result.metadata.layer_count}`);
        console.log(`Time: ${result.metadata.estimated_time_seconds}s`);
        console.log(`G-code size: ${result.gcode.length} bytes`);

        // Download G-code
        const blob = new Blob([result.gcode], { type: 'text/plain' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = file.name.replace(/\.\w+$/, '.gcode');
        a.click();

        // Show warnings
        for (const w of result.warnings) {
            console.warn(`[${w.severity}] ${w.message}`);
        }
    } catch (e) {
        clearInterval(progressInterval);
        console.error('Slicing failed:', e.message);
    } finally {
        engine.free();
    }
}
</script>
```

### 7.5 Web Worker Pattern for Non-blocking Slicing

For large models, slicing should run in a Web Worker to avoid blocking the UI thread.

```javascript
// worker.js
import init, { SlicerEngine } from './slicecore_wasm.js';

let engine = null;

self.onmessage = async (event) => {
    const { type, payload } = event.data;

    if (type === 'init') {
        await init();
        engine = new SlicerEngine();
        self.postMessage({ type: 'ready' });
    }

    if (type === 'slice') {
        const { modelBytes, format, config } = payload;

        // Poll progress and send to main thread
        const progressInterval = setInterval(() => {
            self.postMessage({
                type: 'progress',
                progress: engine.progress(),
                stage: engine.currentStage(),
            });
        }, 50);

        try {
            const result = engine.slice(modelBytes, format, JSON.stringify(config));
            clearInterval(progressInterval);
            self.postMessage({ type: 'result', result });
        } catch (e) {
            clearInterval(progressInterval);
            self.postMessage({ type: 'error', message: e.message });
        }
    }

    if (type === 'cancel') {
        engine?.cancel();
    }
};
```

```javascript
// main.js
const worker = new Worker('./worker.js', { type: 'module' });

worker.postMessage({ type: 'init' });

worker.onmessage = (event) => {
    const { type, ...data } = event.data;

    switch (type) {
        case 'ready':
            console.log('WASM engine ready');
            break;
        case 'progress':
            updateProgressBar(data.progress, data.stage);
            break;
        case 'result':
            handleSliceResult(data.result);
            break;
        case 'error':
            handleSliceError(data.message);
            break;
    }
};

function startSlice(modelBytes, config) {
    worker.postMessage({
        type: 'slice',
        payload: { modelBytes, format: 'auto', config },
    });
}

function cancelSlice() {
    worker.postMessage({ type: 'cancel' });
}
```

### 7.6 WASM Size Budget

| Component | Estimated Size (gzipped) |
|-----------|--------------------------|
| Core math/geometry | ~200 KiB |
| Mesh processing | ~300 KiB |
| Slicing algorithms | ~400 KiB |
| Perimeter/infill | ~500 KiB |
| G-code generation | ~200 KiB |
| Config/schema | ~150 KiB |
| wasm-bindgen overhead | ~50 KiB |
| **Total** | **~1.8 MiB** (target < 5 MiB) |

---

## 8. Event System

The event system provides a unified pub/sub mechanism for progress, warnings, errors, and diagnostic information across all API surfaces. It decouples event producers (pipeline stages) from consumers (progress bars, log files, WebSocket streams).

### 8.1 Event Types

```rust
/// All events emitted by the slicing engine.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum SliceEvent {
    /// Pipeline stage transition.
    StageChanged {
        previous: Option<SliceStage>,
        current: SliceStage,
        timestamp: Instant,
    },

    /// Progress update within a stage.
    Progress(Progress),

    /// Non-fatal warning.
    Warning(SliceWarning),

    /// Layer completed (for streaming output).
    LayerComplete {
        layer_index: u32,
        z: f64,
        layer_time: Duration,
        extrusion_length_mm: f64,
    },

    /// Mesh repair action taken.
    MeshRepaired {
        action: RepairAction,
        affected_elements: u32,
    },

    /// Plugin event (from extension code).
    PluginEvent {
        plugin_name: String,
        event_name: String,
        data: serde_json::Value,
    },

    /// Diagnostic timing event (for profiling).
    Timing {
        stage: SliceStage,
        operation: String,
        duration: Duration,
    },

    /// Operation completed.
    Complete {
        total_time: Duration,
        layer_count: u32,
    },

    /// Operation failed.
    Failed {
        error: String,
        stage: Option<SliceStage>,
        elapsed: Duration,
    },

    /// Operation cancelled by user.
    Cancelled {
        stage: SliceStage,
        elapsed: Duration,
    },
}
```

### 8.2 Event Bus

```rust
/// Event bus for subscribing to engine events.
/// Multiple subscribers can be registered; events are dispatched to all.
pub struct EventBus {
    subscribers: Vec<Box<dyn EventSubscriber>>,
}

/// Trait for receiving events.
pub trait EventSubscriber: Send + Sync {
    /// Called for every event. Implementations should return quickly.
    fn on_event(&self, event: &SliceEvent);

    /// Optional filter — return false to skip events of this type.
    /// Default: accept all events.
    fn accepts(&self, event: &SliceEvent) -> bool { true }
}

impl EventBus {
    pub fn new() -> Self { Self { subscribers: Vec::new() } }

    pub fn subscribe(&mut self, subscriber: Box<dyn EventSubscriber>) {
        self.subscribers.push(subscriber);
    }

    pub fn emit(&self, event: &SliceEvent) {
        for sub in &self.subscribers {
            if sub.accepts(event) {
                sub.on_event(event);
            }
        }
    }
}
```

### 8.3 Built-in Subscribers

```rust
/// Logs events to the `tracing` crate at appropriate levels.
pub struct TracingSubscriber;

impl EventSubscriber for TracingSubscriber {
    fn on_event(&self, event: &SliceEvent) {
        match event {
            SliceEvent::Warning(w) => tracing::warn!("{}: {}", w.category, w.message),
            SliceEvent::Failed { error, .. } => tracing::error!("Slice failed: {}", error),
            SliceEvent::Progress(p) => tracing::debug!(
                stage = %p.stage, progress = p.overall_progress, "Progress"
            ),
            _ => tracing::trace!(?event, "Engine event"),
        }
    }
}

/// Sends events to a tokio broadcast channel (for WebSocket/SSE streaming).
pub struct ChannelSubscriber {
    sender: tokio::sync::broadcast::Sender<SliceEvent>,
}

/// Collects all events for later inspection (testing).
pub struct CollectingSubscriber {
    events: Mutex<Vec<SliceEvent>>,
}

/// Writes events as newline-delimited JSON to a writer (file, stderr).
pub struct NdjsonSubscriber<W: Write + Send + Sync> {
    writer: Mutex<W>,
}
```

### 8.4 Integration with Progress Reporting

The `ProgressReporter` trait (Section 2.5) is implemented on top of the event system. The engine emits `SliceEvent::Progress` events, and the `ProgressReporter` adapter subscribes to them.

```rust
/// Adapter: converts a ProgressReporter into an EventSubscriber.
pub struct ProgressReporterAdapter {
    reporter: Arc<dyn ProgressReporter>,
}

impl EventSubscriber for ProgressReporterAdapter {
    fn on_event(&self, event: &SliceEvent) {
        match event {
            SliceEvent::Progress(p) => self.reporter.on_progress(p),
            SliceEvent::Warning(w) => self.reporter.on_warning(w),
            _ => {}
        }
    }

    fn accepts(&self, event: &SliceEvent) -> bool {
        matches!(event, SliceEvent::Progress(_) | SliceEvent::Warning(_))
    }
}
```

---

## 9. Error Types

All public error types use `thiserror` for ergonomic error handling. Errors are structured with enough context for both human display and programmatic handling.

### 9.1 Error Hierarchy

```
SliceCoreError (top-level, re-exported from slicecore-engine)
  |
  +-- EngineError          (engine creation, lifecycle)
  +-- SliceError           (slicing pipeline failures)
  |     +-- MeshError      (mesh loading, repair, validation)
  |     +-- ConfigError    (configuration parsing, validation)
  |     +-- SlicingError   (contour extraction failures)
  |     +-- ToolpathError  (toolpath generation failures)
  |     +-- GcodeError     (G-code generation failures)
  |     +-- PluginError    (plugin execution failures)
  |     +-- IoError        (file I/O)
  |     +-- Cancelled      (user cancellation)
  |
  +-- AnalyzeError         (model analysis failures)
  +-- ConfigError          (configuration system errors)
  +-- ProfileError         (profile loading, merging)
```

### 9.2 Error Definitions

```rust
/// Top-level error type for all slicecore operations.
#[derive(Debug, thiserror::Error)]
pub enum SliceCoreError {
    #[error("Engine error: {0}")]
    Engine(#[from] EngineError),

    #[error("Slice error: {0}")]
    Slice(#[from] SliceError),

    #[error("Analysis error: {0}")]
    Analyze(#[from] AnalyzeError),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Profile error: {0}")]
    Profile(#[from] ProfileError),
}

/// Errors during engine creation and lifecycle.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Failed to create thread pool with {threads} threads: {source}")]
    ThreadPool { threads: usize, source: Box<dyn std::error::Error + Send + Sync> },

    #[error("Plugin directory not found: {path}")]
    PluginDirNotFound { path: PathBuf },

    #[error("Plugin load error: {0}")]
    PluginLoad(#[from] PluginError),
}

/// Errors during the slicing pipeline.
#[derive(Debug, thiserror::Error)]
pub enum SliceError {
    #[error("Mesh error: {0}")]
    Mesh(#[from] MeshError),

    #[error("Slicing failed at layer {layer} (z={z:.3}mm): {message}")]
    Slicing { layer: u32, z: f64, message: String },

    #[error("Toolpath generation failed at layer {layer}: {message}")]
    Toolpath { layer: u32, message: String },

    #[error("G-code generation error: {message}")]
    Gcode { message: String },

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Plugin '{plugin}' failed during {stage}: {message}")]
    Plugin { plugin: String, stage: String, message: String },

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Memory limit exceeded: used {used_bytes} bytes, limit {limit_bytes} bytes")]
    MemoryLimit { used_bytes: usize, limit_bytes: usize },

    #[error("Timeout: stage '{stage}' exceeded {timeout:?}")]
    Timeout { stage: String, timeout: Duration },
}

/// Mesh-specific errors with diagnostic information.
#[derive(Debug, thiserror::Error)]
pub enum MeshError {
    #[error("Failed to parse {format} file: {message}")]
    ParseError { format: String, message: String },

    #[error("Unsupported model format: {format}")]
    UnsupportedFormat { format: String },

    #[error("Non-manifold mesh: {count} non-manifold edges detected")]
    NonManifold {
        count: u32,
        /// Sample of affected edge vertex pairs (up to 10).
        sample_edges: Vec<(u32, u32)>,
        /// Whether auto-repair is likely to succeed.
        auto_repairable: bool,
    },

    #[error("Self-intersecting mesh: {count} intersecting face pairs")]
    SelfIntersection {
        count: u32,
        sample_faces: Vec<(u32, u32)>,
        auto_repairable: bool,
    },

    #[error("{count} degenerate triangles (zero area)")]
    DegenerateTriangles {
        count: u32,
        face_indices: Vec<u32>,
    },

    #[error("Empty mesh: no triangles")]
    EmptyMesh,

    #[error("Mesh exceeds size limit: {triangles} triangles (limit: {limit})")]
    TooLarge { triangles: u32, limit: u32 },

    #[error("Mesh has {count} disconnected components; expected 1")]
    DisconnectedComponents { count: u32 },

    #[error("Mesh repair failed: {message}")]
    RepairFailed { message: String },

    #[error("Model does not fit on build plate ({model_size} vs {bed_size})")]
    DoesNotFit { model_size: String, bed_size: String },
}

/// Configuration-specific errors.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Unknown setting key: '{key}'")]
    UnknownKey { key: String },

    #[error("Type mismatch for '{key}': expected {expected}, got {actual}")]
    TypeMismatch { key: String, expected: String, actual: String },

    #[error("Value out of range for '{key}': {value} not in [{min}, {max}]")]
    OutOfRange { key: String, value: String, min: String, max: String },

    #[error("Invalid enum value for '{key}': '{value}' (valid: {valid:?})")]
    InvalidEnum { key: String, value: String, valid: Vec<String> },

    #[error("Constraint violation: {message}")]
    ConstraintViolation { message: String },

    #[error("Dependency not met: '{key}' requires '{dependency}' = {expected}")]
    DependencyNotMet { key: String, dependency: String, expected: String },

    #[error("Failed to parse config file '{path}': {message}")]
    ParseError { path: PathBuf, message: String },

    #[error("Config file not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error("Expression evaluation error in '{key}': {expression} -> {error}")]
    ExpressionError { key: String, expression: String, error: String },
}

/// Profile management errors.
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("Profile not found: '{name}'")]
    NotFound { name: String },

    #[error("Profile merge conflict: '{key}' differs between profiles")]
    MergeConflict { key: String },

    #[error("Incompatible profile version: got {got}, requires {requires}")]
    VersionMismatch { got: String, requires: String },

    #[error("Legacy profile import failed: {message}")]
    LegacyImportFailed { message: String },
}

/// Plugin errors.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin '{name}' not found")]
    NotFound { name: String },

    #[error("Plugin '{name}' version {version} incompatible with engine {engine_version}")]
    Incompatible { name: String, version: String, engine_version: String },

    #[error("Plugin '{name}' initialization failed: {message}")]
    InitFailed { name: String, message: String },

    #[error("Plugin '{name}' exceeded resource limit: {resource} ({used}/{limit})")]
    ResourceExceeded { name: String, resource: String, used: String, limit: String },

    #[error("Plugin '{name}' panicked: {message}")]
    Panicked { name: String, message: String },
}
```

### 9.3 Warning Types

Warnings are non-fatal issues discovered during slicing. They are emitted via the event system and collected in `SliceResult`.

```rust
#[derive(Debug, Clone, Serialize)]
pub struct SliceWarning {
    /// Severity level.
    pub severity: WarningSeverity,

    /// Warning category for filtering.
    pub category: WarningCategory,

    /// Human-readable message.
    pub message: String,

    /// Where in the model/print this warning applies.
    pub location: Option<WarningLocation>,

    /// Actionable suggestion to resolve the warning.
    pub suggestion: Option<String>,

    /// Machine-readable warning code (for programmatic handling).
    pub code: WarningCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum WarningSeverity {
    /// Informational — no action needed.
    Info,
    /// Advisory — consider adjusting settings.
    Advisory,
    /// Warning — print quality may be affected.
    Warning,
    /// Critical — print is likely to fail.
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum WarningCategory {
    Mesh,
    Overhang,
    Bridge,
    ThinWall,
    Support,
    Temperature,
    Speed,
    Flow,
    Retraction,
    Compatibility,
    Plugin,
}

#[derive(Debug, Clone, Serialize)]
pub enum WarningLocation {
    /// A specific Z height.
    Layer { z: f64, layer_index: u32 },
    /// A Z range.
    LayerRange { z_min: f64, z_max: f64 },
    /// A specific XY position on a layer.
    Point { x: f64, y: f64, z: f64 },
    /// A region of the model.
    Region { description: String },
    /// A specific configuration setting.
    Setting { key: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum WarningCode {
    MeshNonManifold,
    MeshAutoRepaired,
    OverhangDetected,
    OverhangSteep,
    BridgeLong,
    ThinWallBelowMinimum,
    UnsupportedOverhang,
    HighFlowRate,
    LowLayerAdhesion,
    ExcessiveRetraction,
    SlowLayer,
    IncompatibleFirmwareFeature,
    DeprecatedSetting,
    PluginWarning,
}
```

### 9.4 Result Pattern

All fallible operations return `Result<T, E>` where `E` is the appropriate error type. The crate re-exports a convenience `Result` alias.

```rust
/// Convenience alias used throughout the crate.
pub type Result<T, E = SliceCoreError> = std::result::Result<T, E>;
```

---

## 10. Cross-Cutting Concerns

### 10.1 Logging and Tracing

All crates use the `tracing` crate for structured, zero-cost logging. Log levels map to operations.

```rust
// Engine crate — span-based tracing for pipeline stages
let _span = tracing::info_span!("slice", model = %job.model_name()).entered();

tracing::info!(layers = layer_count, "Slicing complete");
tracing::warn!(layer = 42, angle = 65.0, "Steep overhang detected");
tracing::debug!(elapsed = ?duration, "Perimeter generation took {:?}", duration);
```

### 10.2 Feature Flags

```toml
[features]
default = ["native"]

# Full native build with all features
native = ["rayon", "tokio", "reqwest"]

# WASM-compatible build (excludes threading, networking, filesystem)
wasm = ["wasm-bindgen", "web-sys", "js-sys"]

# Enable AI integration features
ai = ["reqwest", "slicecore-ai"]

# Enable plugin system (dynamic loading)
plugins = ["libloading", "wasmtime", "slicecore-plugin"]

# Enable REST/gRPC server
server = ["axum", "tokio", "tower"]

# Enable Python bindings
python = ["pyo3"]

# Enable C FFI generation
ffi = ["cbindgen"]
```

### 10.3 Serde Serialization

All public types derive `Serialize` and `Deserialize` for JSON/TOML/MessagePack interchange.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SliceMetadata { /* ... */ }

// JSON output for REST API and CLI --json
let json = serde_json::to_string_pretty(&result.metadata)?;

// TOML output for config files
let toml = toml::to_string_pretty(&config)?;

// MessagePack for compact binary transfer (WASM, IPC)
let msgpack = rmp_serde::to_vec(&result.metadata)?;
```

### 10.4 Thread Safety Guarantees

| Type | `Send` | `Sync` | Notes |
|------|--------|--------|-------|
| `Engine` | Yes | Yes | Safe to share via `Arc<Engine>` |
| `SliceJob` | Yes | Yes | Immutable after construction |
| `SliceResult` | Yes | Yes | Immutable after construction |
| `PrintConfig` | Yes | Yes | Interior mutability via `set()` uses locks |
| `CancellationToken` | Yes | Yes | Atomic internally |
| `SliceHandle` | Yes | Yes | Progress polled via atomic reads |
| `EventBus` | Yes | No | Locked internally; subscribers must be `Send + Sync` |

---

## 11. API Mapping to C++ Pipeline

For reference, this table maps the C++ `PrintObjectStep` enum (the 9-step pipeline discovered in the C++ analysis) to the LibSlic3r-RS API stages and responsible crates.

| C++ PrintObjectStep | LibSlic3r-RS `SliceStage` | Responsible Crate | API Entry Point |
|---------------------|---------------------------|-------------------|-----------------|
| `posSlice` | `Slicing` | `slicecore-slicer` | `Engine::slice()` (internal) |
| `posPerimeters` | `Perimeters` | `slicecore-perimeters` | `Engine::slice()` (internal) |
| `posPrepareInfill` | `PreparingInfill` | `slicecore-engine` | `Engine::slice()` (internal) |
| `posInfill` | `Infill` | `slicecore-infill` | `Engine::slice()` (internal) |
| `posSupportMaterial` | `SupportMaterial` | `slicecore-supports` | `Engine::slice()` (internal) |
| N/A (implicit) | `Pathing` | `slicecore-pathing` | `Engine::slice()` (internal) |
| N/A (implicit) | `Planning` | `slicecore-planner` | `Engine::slice()` (internal) |
| `posEstimateCurledExtrusions` | (part of `Planning`) | `slicecore-planner` | `Engine::slice()` (internal) |
| `posIroning` | (part of `Perimeters`) | `slicecore-perimeters` | `Engine::slice()` (internal) |
| `posExport` (PrintStep) | `GcodeGeneration` | `slicecore-gcode-gen` | `Engine::slice()` (internal) |

**Additions in LibSlic3r-RS not present in C++ pipeline:**
- `Loading` — explicit mesh loading/parsing stage
- `Repairing` — explicit mesh repair stage (implicit in C++)
- `PostProcessing` — explicit plugin post-processing stage
- `Finalizing` — metadata, thumbnails, preview generation

---

*Next Document: [04-IMPLEMENTATION-GUIDE.md](./04-IMPLEMENTATION-GUIDE.md)*
