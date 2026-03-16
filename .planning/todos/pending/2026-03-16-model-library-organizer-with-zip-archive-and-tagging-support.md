---
created: 2026-03-16T19:30:00.000Z
title: Model library organizer with ZIP archive and tagging support
area: cli
files:
  - crates/slicecore-fileio/src/lib.rs
  - crates/slicecore-fileio/src/detect.rs
  - crates/slicecore-cli/src/main.rs
---

## Problem

Users accumulate thousands of STL/3MF/OBJ files downloaded from Thingiverse, Printables, MyMiniFactory, and Thangs — typically in nested ZIP archives with inconsistent naming. Finding the right model to print means manually browsing folders, extracting ZIPs, and opening files in a viewer. The "Modelist" project on Reddit highlights how painful this is.

**Pain points:**
- Downloads arrive as ZIPs containing multiple STLs (e.g., "parts_v2_final_FINAL.zip" with 15 files inside)
- No metadata: what printer was this designed for? What material? Is it pre-supported?
- Duplicate models across folders (same thing downloaded twice months apart)
- Can't slice directly from a ZIP — must extract first
- No way to search "all functional brackets I've downloaded"

## Solution

### Feature 1: Direct ZIP/archive slicing

The simplest and most immediately useful feature: allow the slicer to accept ZIP files directly.

```bash
# Slice a model directly from a ZIP
slicecore slice downloaded-parts.zip:bracket_v2.stl -m X1C -f PLA

# List contents of a ZIP
slicecore model list downloaded-parts.zip
# Contents of downloaded-parts.zip:
#   bracket_v2.stl      (12.4 MB, 245k triangles)
#   bracket_v2_lid.stl  (3.1 MB, 62k triangles)
#   readme.txt
#   assembly.pdf

# Slice all STL/3MF files in a ZIP
slicecore slice downloaded-parts.zip --all -m X1C -f PLA
```

**Implementation**: Extend `slicecore-fileio` to accept `archive_path:inner_path` notation. Use the `zip` crate to read ZIP entries as byte streams, pipe into existing mesh loaders. Also support `.tar.gz`, `.rar` (via feature flag).

### Feature 2: Model library index

A local database of all 3D models the user has, with metadata and search.

```bash
# Index a directory of models (recursive, enters ZIPs)
slicecore library scan ~/Downloads/3d-models/
# Scanned: 1,247 models in 342 files (89 ZIP archives)
# New: 823 | Updated: 12 | Duplicates: 412

# Search the library
slicecore library search "bracket"
# ID       Name                  Triangles  Size    Source          Tags
# m-0042   bracket_v2.stl        245k       12.4MB  Printables      structural, functional
# m-0043   bracket_v2_lid.stl    62k        3.1MB   Printables      structural, functional
# m-0187   90deg_bracket.stl     18k        890KB   Thingiverse     hardware

# Show model details
slicecore library info m-0042
# Name: bracket_v2.stl
# Dimensions: 45.2 × 32.1 × 18.7 mm
# Triangles: 245,012
# Volume: 12.3 cm³
# Estimated weight: 15.1g (PLA)
# Source: ~/Downloads/3d-models/brackets/downloaded-parts.zip
# Tags: structural, functional
# Print history: 2 prints (2026-02-15, 2026-03-01)
# Last profile: X1C + CF-PETG

# Tag models
slicecore library tag m-0042 m-0043 --add "project:shelf-mount"

# Slice directly from library ID
slicecore slice library:m-0042 -m X1C -f PLA
```

**Index storage**: SQLite database at `~/.config/slicecore/library.db` containing:
- File hash (SHA256 of mesh data — detects duplicates regardless of filename)
- Bounding box dimensions
- Triangle count / vertex count
- Volume estimate
- Source path (including ZIP internal path)
- User-assigned tags
- Print history (linked to job directories if implemented)
- Auto-generated thumbnail (from Phase 26 render crate)

### Feature 3: Duplicate detection

Using content hashing to identify identical or near-identical models:

```bash
slicecore library duplicates
# Found 412 duplicate groups:
#
# Group 1 (3 copies):
#   ~/Downloads/bracket_v2.stl
#   ~/Downloads/3d-models/brackets/downloaded-parts.zip:bracket_v2.stl
#   ~/Projects/shelf/parts/bracket.stl
#
# Group 2 (2 copies, near-match 98.7% similar):
#   ~/Downloads/benchy.stl (245,012 triangles)
#   ~/Downloads/old/3dbenchy_v2.stl (244,890 triangles)

slicecore library deduplicate --dry-run
# Would remove 412 duplicates, freeing 2.3 GB
```

**Near-duplicate detection**: Compare bounding box dimensions + triangle count + volume. If all match within tolerance, compute mesh distance metric for final confirmation. This catches models that have been re-exported (different triangulation but same shape).

### Feature 4: Auto-tagging from context

```bash
# AI-assisted tagging (uses existing AI crate)
slicecore library auto-tag --ai
# Analyzing 823 untagged models...
# m-0042: bracket_v2.stl → structural, bracket, functional, right-angle
# m-0187: 90deg_bracket.stl → structural, bracket, hardware, L-shaped
# m-0301: dragon_bust.stl → decorative, figurine, organic, display
```

Without AI, use heuristic tagging from:
- Filename keywords ("bracket" → structural, "vase" → decorative)
- Geometry analysis (flat bottom → printable without supports, thin walls → vase mode candidate)
- Source folder names

## Scope for libslic3r-rs

This feature lives primarily in:
- **slicecore-fileio**: ZIP/archive reading, content hashing
- **slicecore-cli**: `library` subcommand, `model list` for ZIP contents
- **slicecore-render**: Thumbnail generation for library entries (already exists)
- **slicecore-ai**: Optional auto-tagging

The library index (SQLite) is a CLI concern, not a library concern. The engine crate stays clean.

## Implementation phases

1. **Phase A**: ZIP slicing (`archive:path` notation in fileio) — immediate QoL win
2. **Phase B**: `model list` command for ZIP contents with mesh metadata
3. **Phase C**: Library index with scan, search, info, tag commands
4. **Phase D**: Duplicate detection (hash-based exact, geometry-based near-match)
5. **Phase E**: Auto-tagging (heuristic + optional AI)
6. **Phase F**: Print history linking (requires job directories todo)
