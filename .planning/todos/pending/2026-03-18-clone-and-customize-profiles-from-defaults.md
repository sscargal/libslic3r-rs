---
created: 2026-03-18T19:57:15.106Z
title: Clone and customize profiles from defaults
area: cli
files:
  - crates/slicecore-cli/src/main.rs
  - crates/slicecore-engine/src/config.rs
---

## Problem

Users need to create custom printer and filament profiles tailored to their specific setup, but writing a profile from scratch is impractical — profiles have dozens of interdependent settings. The standard workflow in existing slicers is to clone a vendor-provided default profile, tweak the settings that differ, and save under a new name. libslic3r-rs currently has no CLI workflow for this clone-modify-save cycle.

Without this, users must either manually copy and edit TOML files (error-prone, no validation) or use default profiles as-is without customization.

## Solution

Implement `slicecore profile clone <source> <new-name>`:

1. **Clone**: Copy source profile (built-in or user-created) to user profile directory with the new name
   - Preserve the `inherits` field pointing to the original for future diff/update tracking
   - Set `is_custom: true` and `based_on: <source>` metadata
   - Validate new name is unique across all profile directories

2. **Edit workflow**: After cloning, user modifies the TOML file directly or via:
   - `slicecore profile set <name> <key> <value>` — set individual fields with validation
   - `slicecore profile edit <name>` — open in $EDITOR

3. **Save/validate**: On next use, validate the modified profile against the schema (temperature ranges, nozzle constraints, etc.) and warn on invalid combinations

4. **Profile locations**: User profiles stored in `~/.config/slicecore/profiles/` (or XDG equivalent), separate from built-in vendor profiles. User profiles take precedence when names conflict.

5. **Delete/rename**: `slicecore profile delete <name>` (user profiles only, refuse to delete built-ins) and `slicecore profile rename <old> <new>`
