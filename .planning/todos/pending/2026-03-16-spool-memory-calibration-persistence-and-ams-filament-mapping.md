---
created: 2026-03-16T19:15:00.000Z
title: Spool memory — calibration persistence and AMS filament mapping
area: engine
files:
  - crates/slicecore-engine/src/config.rs
  - crates/slicecore-cli/src/main.rs
---

## Problem

Two related pain points for users:

### 1. Spool Memory (Calibration Persistence)

When a user calibrates a specific spool of filament (flow rate, pressure advance, temperature, retraction), those tuned settings are lost when the spool is swapped out. Weeks later when the spool is reloaded, they must re-calibrate or manually re-apply settings.

This is especially frustrating because:
- Each spool is unique — even same brand/color batches can vary (moisture, diameter tolerance, pigment load)
- Calibration takes 30-60 minutes of print tests
- Users with 20+ spools can't remember which settings work for each
- Print farms with hundreds of spools need automated spool→profile mapping

### 2. AMS Filament Name Mapping

Bambu AMS (and similar multi-material systems) reports filament names from RFID tags or user assignment (e.g., "Bambu PLA Basic"). Users want to map these printer-reported names to their own finely-tuned local presets (e.g., "My Ultra-Smooth PLA" with custom PA, flow, and temp).

Currently this mapping is manual every time a multi-material project is prepared.

## Solution

### Spool Identity System

A spool needs a unique identifier. Multiple identification methods:

| Method | How it works | Pros | Cons |
|--------|-------------|------|------|
| **RFID tag** | Bambu/some spools have RFID. Read tag ID via printer API. | Automatic, no user effort | Only Bambu + RFID-tagged spools |
| **QR code on spool** | User prints QR label from slicer, sticks on spool. Scan with phone/camera. | Works with any spool | Manual labeling step |
| **Manual spool ID** | User assigns a name/number when registering a spool | Universal, simple | Manual, error-prone |
| **Weight-based** | Weigh spool on scale, combined with color → likely match | Semi-automatic | Ambiguous as spool depletes |
| **NFC tag** | Write-once NFC sticker on spool, read via phone | Cheap, reusable | Requires NFC reader |

**Recommended MVP**: Manual spool ID + optional RFID integration (for Bambu users).

### Spool Database Schema

```toml
# ~/.config/slicecore/spools/spool-abc123.toml
[spool]
id = "abc123"                    # Unique identifier
name = "eSun PLA+ White #3"     # User-friendly name
rfid_tag = "BAMBU:PLA:001234"   # Optional: RFID tag data
qr_code = "SC:abc123"           # Optional: QR code data

[material]
type = "PLA"
brand = "eSun"
color = "White"
diameter_mm = 1.75
weight_g = 1000                  # Initial spool weight
remaining_g = 743                # Tracked remaining weight

[calibration]
temperature_c = 213              # Calibrated (not default 215)
flow_multiplier = 0.97           # Calibrated extrusion multiplier
pressure_advance = 0.042         # Calibrated PA value
retraction_mm = 0.8              # Calibrated retraction distance
retraction_speed_mm_s = 45       # Calibrated retraction speed
max_volumetric_flow_mm3_s = 15.2 # Calibrated max flow rate
bed_temperature_c = 58           # Calibrated bed temp

[calibration_meta]
calibrated_on = "2026-03-10"
calibrated_with_printer = "X1C-001"  # Which printer was used
calibrated_nozzle_mm = 0.4
notes = "Tuned after drying 4h at 55°C. Stringing-free at 213°C."

[history]
# Track usage for remaining weight estimation
prints = [
  { date = "2026-03-10", grams_used = 23.4, model = "calibration_tests" },
  { date = "2026-03-12", grams_used = 47.2, model = "bracket_v3.stl" },
]
```

### CLI Commands

```bash
# Register a new spool
slicecore spool add --name "eSun PLA+ White #3" --material PLA --brand eSun

# List registered spools
slicecore spool list
# ID       Name                    Material  Remaining  Last Used
# abc123   eSun PLA+ White #3      PLA       743g       2026-03-12
# def456   Polymaker PETG Blue     PETG      520g       2026-03-08

# Update calibration from test results
slicecore spool calibrate abc123 --temp 213 --flow 0.97 --pa 0.042

# Use spool for slicing (applies calibrated settings as overrides)
slicecore slice model.stl --spool abc123 --printer X1C

# Check if spool has enough filament for a print
slicecore spool check abc123 --gcode model.gcode
# ⚠ Estimated usage: 127g, remaining: 103g — may run short

# Import calibration from Bambu AMS RFID data
slicecore spool import-ams --printer 192.168.1.50
```

### AMS Filament Name Mapping

```toml
# ~/.config/slicecore/ams-mapping.toml

# Map printer-reported names to local spool IDs or profile names
[mappings]
"Bambu PLA Basic" = { spool = "abc123" }              # Map to specific spool
"Bambu PLA Basic @White" = { spool = "abc123" }       # Color-specific mapping
"Bambu PETG Basic" = { profile = "my-petg-tuned" }    # Map to profile name
"Generic PLA" = { profile = "pla-conservative" }       # Fallback for unknowns

[auto_rules]
# Regex-based auto-mapping
"Bambu.*PLA.*" = { profile = "bambu-pla-tuned" }
"eSun.*PETG.*" = { profile = "esun-petg" }
```

**Workflow**: When preparing a multi-material slice, the slicer reads AMS slot assignments from the printer (via Bambu MQTT), maps each to a local spool or profile, and auto-applies the calibrated settings. No manual re-assignment needed.

### Integration with calibration workflow (Phase 31)

The calibration commands from Phase 31 should save results directly to spool records:

```bash
# Run temp tower for this spool → result auto-saved
slicecore calibrate temp-tower --spool abc123 --printer X1C

# User determines 213°C is optimal, records it
slicecore spool calibrate abc123 --temp 213

# Future: AI analyzes calibration print photo and auto-records
slicecore feedback --spool abc123 --photos ./temp-tower-photos/
```

## Dependencies

- **Phase 31 (Calibration)**: ✓ Calibration commands generate the data that spool memory stores
- **Network printer discovery** (todo): Needed for AMS RFID reading and printer communication
- **AI feedback loop** (todo): AI can suggest calibration values from photo analysis
- **Material database** (todo): Default values for new spools before calibration

## Phased implementation

1. **Phase A**: Spool database + CLI commands (add, list, calibrate, check) — pure local storage
2. **Phase B**: `--spool` flag on `slice` command — applies calibrated overrides
3. **Phase C**: AMS name mapping — TOML config + auto-apply during multi-material prep
4. **Phase D**: RFID integration — read Bambu AMS tags, auto-identify spools
5. **Phase E**: Usage tracking + remaining weight estimation
