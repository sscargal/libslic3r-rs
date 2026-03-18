---
created: 2026-03-18T19:57:15.106Z
title: Enable-disable printer and filament profiles to narrow search scope
area: cli
files:
  - crates/slicecore-cli/src/main.rs
  - crates/slicecore-engine/src/config.rs
  - crates/slicecore-plugin/src/status.rs
---

## Problem

libslic3r-rs ships thousands of vendor-provided printer and filament profiles, but most users own one or a few printers and use a handful of filaments. Browsing or searching through the full catalog is slow and noisy — a user with a single Bambu Lab X1C doesn't need to see Prusa, Creality, or Voron profiles. Print farms are even more focused, typically running one make and model.

Existing slicers (PrusaSlicer, OrcaSlicer) solve this with an enable/disable system: during initial setup, users select their printer(s), and only compatible/enabled profiles appear in lists. Users can enable additional vendor filaments or add custom ones per printer.

We already implemented a similar enable/disable pattern for plugins (phase 36) using TOML-based `.status` files via the `slicecore-plugin` crate's status module. The question is whether to reuse that filesystem approach for profiles or build something more scalable.

## Solution

Design an enable/disable system for printer and filament profiles:

1. **Storage approach — evaluate tradeoffs**:
   - **Filesystem (like plugins)**: One `.status` TOML per profile or a single `profile-status.toml` manifest. Simple, no dependencies, git-friendly. May be slow with 1000s of profiles.
   - **SQLite index**: Fast queries, supports complex filtering (by vendor, material type, nozzle size). Overkill for most users but scales for power users and farms.
   - **Hybrid**: Filesystem as source of truth, with an optional SQLite cache rebuilt on changes. Best of both worlds.
   - Recommendation: Start with a single `~/.config/slicecore/enabled-profiles.toml` manifest (list of enabled profile IDs) — analogous to the plugin status approach but centralized. Add SQLite cache later if performance demands it.

2. **CLI commands** (mirror plugin pattern):
   - `slicecore profile enable <name>` / `slicecore profile disable <name>`
   - `slicecore profile list --enabled` / `--disabled` / `--all`
   - `slicecore profile setup` — interactive first-run wizard: select printer(s), auto-enable compatible filaments

3. **Default behavior**: All vendor profiles start disabled. On first run or `profile setup`, user picks their printer(s) and the system auto-enables:
   - The selected printer profile(s)
   - All vendor-recommended filament profiles for those printers
   - Generic filament profiles for common materials (PLA, PETG, ABS)

4. **Per-printer filament visibility**: A filament can be enabled globally or per-printer. E.g., enable CF-PETG only for the printer with a hardened nozzle.

5. **Integration with search**: The search/filter todo should default to showing only enabled profiles, with `--all` to search the full catalog.

6. **Custom profiles**: User-created profiles (from the clone todo) are always enabled by default.
