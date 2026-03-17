# LibSlic3r-RS: C++ LibSlic3r Analysis Guide

**Version:** 1.0.0-draft
**Author:** Steve Scargall / SliceCore-RS Architecture Team
**Date:** 2026-02-13
**Status:** Draft — Review & Iterate

---

## 1. Purpose

Before we can achieve feature parity with the C++ LibSlic3r ecosystem, we must systematically understand what the existing slicers do. This document provides:

1. **Instructions** for cloning and analyzing the open-source slicers
2. **Meta-prompts** for Claude Code to perform automated code analysis
3. **Mapping templates** to catalog features, settings, and algorithms
4. **Comparison methodology** to identify what's shared vs. divergent across forks

---

## 2. Repository Setup

### 2.1 Clone All Slicers

```bash
mkdir -p ~/slicer-analysis && cd ~/slicer-analysis

# Clone all four major forks
git clone --depth 1 https://github.com/prusa3d/PrusaSlicer.git
git clone --depth 1 https://github.com/bambulab/BambuStudio.git
git clone --depth 1 https://github.com/OrcaSlicer/OrcaSlicer.git
git clone --depth 1 https://github.com/CrealityOfficial/CrealityPrint.git

# Verify the libslic3r directories exist
ls PrusaSlicer/src/slic3r/
ls BambuStudio/src/slic3r/
ls OrcaSlicer/src/slic3r/
ls CrealityPrint/src/slic3r/
```

### 2.2 Directory Structure Overview

The LibSlic3r source in each project typically lives under `src/slic3r/` or `src/libslic3r/`. Key subdirectories:

```
src/slic3r/  (or src/libslic3r/)
├── BridgeDetector.cpp/.hpp       # Bridge span detection
├── ClipperUtils.cpp/.hpp         # Polygon clipping wrappers
├── Config.cpp/.hpp               # Configuration/settings system
├── ExPolygon.cpp/.hpp            # Extended polygons (outer + holes)
├── ExtrusionEntity.cpp/.hpp      # Extrusion path types
├── Fill/                         # Infill pattern implementations
│   ├── FillBase.cpp/.hpp
│   ├── FillGyroid.cpp/.hpp
│   ├── FillRectilinear.cpp/.hpp
│   ├── FillLightning.cpp/.hpp
│   └── ...
├── Flow.cpp/.hpp                 # Flow rate calculations
├── GCode.cpp/.hpp                # G-code generation
├── GCode/                        # G-code sub-modules
│   ├── AvoidCrossingPerimeters.cpp
│   ├── CoolingBuffer.cpp
│   ├── PressureEqualizer.cpp
│   ├── RetractWhenCrossingPerimeters.cpp
│   ├── SeamPlacer.cpp
│   ├── SpiralVase.cpp
│   ├── ThumbnailData.cpp
│   └── WipeTower.cpp
├── Geometry.cpp/.hpp             # Geometric utilities
├── Layer.cpp/.hpp                # Layer data structures
├── Model.cpp/.hpp                # Model/object data structures
├── PerimeterGenerator.cpp/.hpp   # Perimeter/wall generation
├── Point.cpp/.hpp                # Point types
├── Polygon.cpp/.hpp              # Polygon type
├── Polyline.cpp/.hpp             # Polyline type
├── Print.cpp/.hpp                # Print job orchestration
├── PrintConfig.cpp/.hpp          # Print configuration definitions
├── SLA/                          # SLA/resin (out of scope)
├── Slicing.cpp/.hpp              # Slicing algorithm
├── SupportMaterial.cpp/.hpp      # Support generation
├── Surface.cpp/.hpp              # Surface type classification
├── TreeSupport.cpp/.hpp          # Tree support algorithm
├── TriangleMesh.cpp/.hpp         # Mesh data structures
├── clipper/                      # Bundled Clipper library
└── ...
```

---

## 3. Analysis Tasks

### 3.1 Task 1: Settings/Configuration Extraction

**Goal:** Extract every configurable setting, its type, default value, range, and which UI category it appears in.

**Where to look:**
- `PrintConfig.cpp/.hpp` — Main settings definitions
- `Preset.cpp/.hpp` — Preset management
- Any JSON/INI files in `resources/` directories

### 3.2 Task 2: Algorithm Mapping

**Goal:** For each slicing stage, identify the algorithm used, its inputs/outputs, and performance characteristics.

| Stage | Files to Examine |
|-------|-----------------|
| Mesh loading | `TriangleMesh.cpp`, `Format/` directory |
| Mesh repair | `TriangleMesh.cpp`, `TriangleMeshSlicer.cpp` |
| Slicing | `Slicing.cpp`, `TriangleMeshSlicer.cpp` |
| Perimeters | `PerimeterGenerator.cpp` |
| Infill | `Fill/` directory (all files) |
| Supports | `SupportMaterial.cpp`, `TreeSupport.cpp` |
| Bridge detection | `BridgeDetector.cpp` |
| G-code generation | `GCode.cpp`, `GCode/` directory |
| Seam placement | `GCode/SeamPlacer.cpp` |
| Cooling | `GCode/CoolingBuffer.cpp` |
| Retraction | `GCode.cpp` (retraction logic inline) |
| Travel optimization | `GCode/AvoidCrossingPerimeters.cpp` |
| Time estimation | `GCode/GCodeProcessor.cpp` |

### 3.3 Task 3: Fork Divergence Analysis

**Goal:** Identify features unique to each fork (not present in others).

Known divergences to investigate:
- OrcaSlicer: Wall ordering options, calibration tools
- BambuStudio: AMS support, Bambu-specific G-code, LAN mode
- PrusaSlicer: Input shaping, multi-material wipe tower
- CrealityPrint: Creality-specific printer integration

---

## 4. Meta-Prompts for Claude Code

Use these prompts with Claude Code (or paste into a conversation with context) to automate analysis.

### 4.1 Meta-Prompt: Extract All Settings

```
TASK: Analyze the PrintConfig definition files in this C++ slicer project and extract a comprehensive catalog of every configurable setting.

INSTRUCTIONS:
1. Find all files that define print/printer/filament configuration options. Start with:
   - src/slic3r/PrintConfig.cpp
   - src/slic3r/PrintConfig.hpp
   - src/libslic3r/PrintConfig.cpp (some projects use this path)
   
2. For EACH setting found, extract:
   - Setting key/name (the internal identifier)
   - Display name (user-facing label)
   - Description/tooltip text
   - Data type (bool, int, float, enum, string, percent, etc.)
   - Default value
   - Min/max constraints (if any)
   - Enum variants (if applicable)
   - Category (which settings tab it appears in)
   - Which settings it depends on (conditional visibility)
   - Units (mm, mm/s, °C, %, etc.)

3. Output as a structured JSON array, one object per setting.

4. Also note any settings that are:
   - Marked as "expert" or "advanced" mode only
   - Deprecated or hidden
   - Platform/printer-specific

IMPORTANT: Be thorough. The C++ code uses macros and templates to define settings.
Look for patterns like:
  - OPT_DEF, DEF_OPT, def->...
  - ConfigOptionDef declarations
  - add() calls on ConfigDef objects
  - Enum definitions that correspond to setting values

OUTPUT FORMAT:
{
  "settings": [
    {
      "key": "layer_height",
      "display_name": "Layer height",
      "description": "Height of each printed layer",
      "type": "float",
      "default": 0.2,
      "min": 0.01,
      "max": 1.0,
      "category": "layers_and_perimeters",
      "units": "mm",
      "mode": "simple",
      "depends_on": null
    }
  ],
  "total_count": 0,
  "categories": [],
  "source_file": "PrintConfig.cpp"
}
```

### 4.2 Meta-Prompt: Map Slicing Algorithm

```
TASK: Analyze the slicing algorithm implementation in this C++ slicer project and produce a detailed algorithm description.

INSTRUCTIONS:
1. Find the core slicing implementation. Start with:
   - src/slic3r/Slicing.cpp
   - src/slic3r/TriangleMeshSlicer.cpp (or similar)

2. Trace the execution flow from "input mesh" to "output layer contours":
   a. How are layer heights determined?
   b. How does the mesh-plane intersection work?
   c. How are intersection segments chained into contours?
   d. How are contours classified (outer vs. hole)?
   e. How are contours associated (which holes belong to which outer)?
   f. What optimizations are applied (sorting, spatial indexing)?
   g. How is parallelism achieved (if at all)?

3. For each step, document:
   - Input data structures and their types
   - Output data structures and their types
   - Algorithm complexity (Big-O)
   - Any external library calls (Clipper, Eigen, etc.)
   - Known limitations or edge cases handled

4. Identify any adaptive slicing logic:
   - How does variable layer height work?
   - What metric drives the adaptation?
   - What constraints are enforced?

OUTPUT FORMAT: Pseudocode with annotations, followed by a data flow diagram.
```

### 4.3 Meta-Prompt: Infill Pattern Catalog

```
TASK: Catalog every infill pattern implementation in this slicer's Fill/ directory.

INSTRUCTIONS:
1. List every file in the Fill/ (or Infill/) directory.

2. For EACH infill pattern implementation:
   - Pattern name
   - Source file(s)
   - Brief description of the geometric pattern
   - Key parameters it uses (density, angle, line spacing, etc.)
   - Whether it connects between layers (3D pattern like gyroid)
   - Whether it's density-adaptive
   - Performance characteristics (fast vs. slow to generate)
   - When it was added (git blame for first commit if possible)
   - Which forks have this pattern vs. which don't

3. Identify the base class/interface that all infill patterns inherit from.
   Document its virtual methods and what each is responsible for.

4. For the top 5 most complex patterns (gyroid, lightning, adaptive cubic,
   tree supports interface, etc.), provide a detailed algorithm walkthrough.

OUTPUT FORMAT:
| Pattern | File | Params | 3D? | Adaptive? | Complexity |
|---------|------|--------|-----|-----------|------------|
```

### 4.4 Meta-Prompt: G-code Generation Pipeline

```
TASK: Trace the complete G-code generation pipeline from toolpaths to final output.

INSTRUCTIONS:
1. Start at GCode.cpp (or GCode/GCodeGenerator.cpp) and trace the export flow.

2. Map each stage of G-code emission:
   a. Start G-code (initialization sequence)
   b. Per-layer processing order
   c. Per-object processing (for sequential printing)
   d. Feature ordering within a layer (perimeters → infill → etc.)
   e. Travel move generation
   f. Retraction logic
   g. Speed assignment
   h. Fan/cooling control
   i. Temperature changes
   j. Layer change G-code
   k. Tool change G-code (multi-material)
   l. End G-code

3. For each stage, document:
   - Which settings control this behavior
   - What firmware-specific variations exist (Marlin vs. Klipper vs. Bambu)
   - Any post-processing steps applied after initial generation

4. Special attention to:
   - CoolingBuffer: How is per-layer cooling/slowdown calculated?
   - PressureEqualizer: How is pressure advance compensated?
   - SeamPlacer: Full seam placement algorithm
   - AvoidCrossingPerimeters: Travel routing algorithm
   - WipeTower: Multi-material purge tower generation

OUTPUT FORMAT: Flowchart (text-based) with setting references at each stage.
```

### 4.5 Meta-Prompt: Support Generation Deep-Dive

```
TASK: Analyze both traditional and tree support generation algorithms.

INSTRUCTIONS:
1. Traditional supports:
   - File: SupportMaterial.cpp
   - How are overhang regions detected?
   - How is support geometry generated?
   - How are support interfaces created?
   - What are the key parameters and their effects?

2. Tree supports:
   - File: TreeSupport.cpp (OrcaSlicer/BambuStudio may have enhanced versions)
   - What is the tree growth algorithm?
   - How are branches merged?
   - How does collision avoidance work?
   - What data structures are used (SDF, octree, etc.)?
   - Performance: how long does tree support generation take vs. traditional?

3. Compare tree support implementations across forks:
   - PrusaSlicer's version
   - BambuStudio's version  
   - OrcaSlicer's version (likely has enhancements)
   - Identify improvements each fork has made

OUTPUT FORMAT: Algorithm pseudocode + parameter table + cross-fork comparison matrix.
```

### 4.6 Meta-Prompt: Cross-Fork Feature Comparison

```
TASK: Compare features across all four slicer forks to identify unique capabilities.

INSTRUCTIONS:
1. For each of the following categories, identify which features are present
   in which fork:

   Categories:
   - Print settings (unique settings not in other forks)
   - Infill patterns
   - Support algorithms
   - Seam placement strategies
   - G-code post-processing features
   - Printer-specific features
   - Multi-material handling
   - Calibration tools
   - Quality-of-life features
   - Performance optimizations

2. For each feature found in only ONE fork, document:
   - What it does
   - Why it was added (commit message or PR description)
   - How complex the implementation is
   - Whether it would benefit all users

3. Create a feature matrix:
   Feature | PrusaSlicer | BambuStudio | OrcaSlicer | CrealityPrint
   --------|-------------|-------------|------------|---------------

OUTPUT FORMAT: Feature comparison matrix + notes on unique features.
```

### 4.7 Meta-Prompt: Performance Profiling Points

```
TASK: Identify performance-critical code paths and bottlenecks in the C++ LibSlic3r.

INSTRUCTIONS:
1. Find all uses of parallelism:
   - tbb::parallel_for / tbb::parallel_reduce
   - OpenMP pragmas
   - std::thread
   - Any other threading constructs

2. Find all uses of Clipper library:
   - Count call sites
   - Identify which operations use it most heavily
   - Note any performance comments or TODOs

3. Identify memory allocation hotspots:
   - Large vector allocations
   - Frequent small allocations (polygon points)
   - Any use of memory pools or custom allocators

4. Find any explicit performance optimizations:
   - Spatial indices (R-tree, grid, etc.)
   - Caching mechanisms
   - Lazy computation patterns
   - SIMD usage (if any)

5. Note any TODO/FIXME/HACK comments related to performance.

OUTPUT FORMAT: Table of hotspots with file, function, and concern.
```

---

## 5. Analysis Execution Plan

### Step-by-Step Procedure

```
Step 1: Clone repositories (Section 2.1)
        Time: 10 minutes

Step 2: Run Meta-Prompt 4.1 (Settings Extraction) on EACH fork
        Save output as: analysis/settings_{slicer}.json
        Time: ~30 minutes per fork (2 hours total)

Step 3: Run Meta-Prompt 4.6 (Cross-Fork Comparison)
        Using the settings JSONs from Step 2 as input
        Save output as: analysis/feature_matrix.md
        Time: ~1 hour

Step 4: Run Meta-Prompt 4.2 (Slicing Algorithm) on PrusaSlicer
        (PrusaSlicer is the canonical upstream)
        Save output as: analysis/algorithm_slicing.md
        Time: ~1 hour

Step 5: Run Meta-Prompt 4.3 (Infill Catalog) on OrcaSlicer
        (OrcaSlicer has the most infill patterns)
        Save output as: analysis/infill_catalog.md
        Time: ~45 minutes

Step 6: Run Meta-Prompt 4.4 (G-code Pipeline) on PrusaSlicer
        Save output as: analysis/gcode_pipeline.md
        Time: ~1 hour

Step 7: Run Meta-Prompt 4.5 (Support Generation) on OrcaSlicer
        Save output as: analysis/support_algorithms.md
        Time: ~1 hour

Step 8: Run Meta-Prompt 4.7 (Performance) on PrusaSlicer
        Save output as: analysis/performance_hotspots.md
        Time: ~30 minutes

Step 9: Synthesize findings into:
        - Comprehensive settings catalog (union of all forks)
        - Algorithm reference for each pipeline stage
        - Feature parity checklist for LibSlic3r-RS
        Time: ~2 hours

TOTAL ESTIMATED TIME: ~10 hours of Claude Code interaction
```

---

## 6. Output Artifacts

After analysis, you will have:

| Artifact | Description | Used For |
|----------|-------------|----------|
| `analysis/settings_prusa.json` | All PrusaSlicer settings | Config schema baseline |
| `analysis/settings_bambu.json` | All BambuStudio settings | Config schema additions |
| `analysis/settings_orca.json` | All OrcaSlicer settings | Config schema additions |
| `analysis/settings_creality.json` | All CrealityPrint settings | Config schema additions |
| `analysis/settings_unified.json` | Merged superset of all settings | `slicecore-config` schema |
| `analysis/feature_matrix.md` | Cross-fork feature comparison | Prioritization |
| `analysis/algorithm_slicing.md` | Slicing algorithm reference | `slicecore-slicer` impl |
| `analysis/infill_catalog.md` | All infill patterns documented | `slicecore-infill` impl |
| `analysis/gcode_pipeline.md` | G-code generation pipeline | `slicecore-gcode-gen` impl |
| `analysis/support_algorithms.md` | Support generation algorithms | `slicecore-supports` impl |
| `analysis/performance_hotspots.md` | Performance-critical areas | Optimization targets |
| `analysis/feature_parity_checklist.md` | What to implement | Project backlog |

---

## 7. Quick-Start: First Analysis Session

If you want to start immediately with Claude Code, use this combined prompt:

```
I have cloned the PrusaSlicer repository to ~/slicer-analysis/PrusaSlicer.

Please perform the following analysis:

1. Navigate to src/slic3r/ (or src/libslic3r/) and list all .cpp and .hpp files,
   grouped by functional area (mesh, slicing, infill, gcode, support, config, etc.)

2. Count lines of code per functional area to understand relative complexity.

3. Read PrintConfig.cpp/hpp and extract the first 50 settings with their types,
   defaults, and categories. Output as a JSON array.

4. Read the slicing pipeline entry point (likely in Print.cpp or PrintObject.cpp)
   and produce a high-level flowchart showing the order of operations from
   "model loaded" to "G-code written."

5. Identify all #include dependencies between LibSlic3r source files to produce
   a dependency graph. Which files are most depended upon?

Output everything in markdown format for easy review.
```

---

*Next Document: [06-NOVEL-IDEAS.md](./06-NOVEL-IDEAS.md)*