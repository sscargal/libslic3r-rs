---
created: 2026-03-18T19:57:15.106Z
title: Search and filter profiles by printer and filament compatibility
area: cli
files:
  - crates/slicecore-cli/src/main.rs
  - crates/slicecore-engine/src/config.rs
---

## Problem

Profile libraries grow large quickly — PrusaSlicer ships hundreds of printer and filament presets. Users need to find the right combination for their specific setup but currently must manually browse or know the exact profile name. Key friction points:

- A Bambu Lab X1C with a 0.4mm nozzle should only show compatible filament profiles (correct nozzle diameter, printer family)
- Searching for "PLA" should surface all PLA filament profiles, filtered to the selected printer
- The slicer accepts exactly one printer profile and one or more filament profiles (one per extruder/AMS slot) — the UI should enforce this constraint
- Users switching between printers (e.g., X1C at home, Prusa MK4 at work) need fast profile switching without hunting through incompatible options
- Compatibility rules: nozzle diameter must match, filament temperature range must be within printer capability, some filaments require specific hardware (e.g., abrasive filaments need hardened nozzle)

## Solution

Implement profile search and compatibility filtering:

1. **`slicecore profile search <query>`**: Free-text search across profile names, descriptions, and metadata. Supports filters:
   - `--printer <name>` — filter filament profiles compatible with this printer
   - `--material <type>` — filter by material type (PLA, PETG, ABS, TPU, etc.)
   - `--nozzle <diameter>` — filter by nozzle size
   - `--manufacturer <name>` — filter by brand/vendor

2. **Compatibility engine**: When a printer profile is selected, automatically compute which filament profiles are compatible based on:
   - Nozzle diameter match
   - Temperature range (hotend max, bed max)
   - Hardware requirements (direct drive for flex, enclosure for ABS, hardened nozzle for CF)
   - Vendor compatibility tags (e.g., "@BBL X1C" suffix in profile names)

3. **`slicecore profile list`** enhancement: Add `--compatible-with <printer>` flag to existing list command

4. **Interactive mode**: When no filament is specified, present filtered choices based on selected printer. Show compatibility warnings if user forces an incompatible combination.

5. **Profile sets / favorites**: Let users save named combinations (printer + filaments) for quick recall — e.g., "X1C-PLA-daily" = X1C 0.4mm + Bambu PLA Basic in slots 1-4
