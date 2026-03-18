---
created: 2026-03-18T19:57:15.106Z
title: Sync printer and filament settings from connected printer
area: general
files:
  - crates/slicecore-profiles/src/lib.rs
  - crates/slicecore-cli/src/main.rs
---

## Problem

Slicers like PrusaSlicer and OrcaSlicer have a "sync" feature that reads the current printer configuration and loaded filament settings directly from a connected printer. This pulls data like nozzle diameter, build volume, installed filament type/color (especially from AMS/MMU systems), firmware version, and active calibration values. Without this, users must manually configure their slicer to match their printer state, which is error-prone and tedious — especially when switching between printers or after hardware changes.

Key settings typically synced:
- Nozzle diameter and type (hardened steel, brass, etc.)
- Build volume and bed shape
- Currently loaded filament(s) — type, color, temperature range
- AMS/MMU slot mapping — which filament is in which slot
- Firmware-stored calibration values (flow rate, pressure advance, Z offset)
- Printer capabilities (input shaping, direct drive vs bowden)

## Solution

Implement a `slicecore sync` CLI subcommand and corresponding library API:

1. **Discovery**: Detect connected printers via USB serial, network (mDNS/SSDP from existing todo), or user-provided address
2. **Protocol support**: Query printer state via supported protocols — Moonraker API (Klipper), OctoPrint API, Bambu MQTT, Marlin serial (M503/M115)
3. **Settings extraction**: Parse printer response into structured profile data
4. **Profile merge**: Update or create a printer profile with synced values, preserving user overrides for settings the printer doesn't report
5. **Filament mapping**: For multi-material systems (AMS, MMU), sync slot-to-filament assignments and update the active filament profiles
6. **Diff display**: Show what changed before applying — let user accept/reject individual settings
7. **Periodic sync**: Option to auto-sync on connection or before each slice to catch hardware changes
