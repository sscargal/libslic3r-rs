---
created: 2026-03-18T19:57:15.106Z
title: G-code upload and print job submission to networked printers
area: general
files:
  - crates/slicecore-cli/src/main.rs
---

## Problem

After slicing, users typically need to get the G-code to their printer. Printers are addressed by IP/hostname and accept jobs via various protocols (Moonraker REST API for Klipper, OctoPrint API, Bambu Lab MQTT/FTP, Prusa Connect, USB serial). Existing slicers (PrusaSlicer, OrcaSlicer, Bambu Studio) include built-in "Send to Printer" functionality.

The key architectural question: should libslic3r-rs implement G-code upload/submission, or is this out of scope for a slicing library/CLI and better left to external tools (e.g., curl to Moonraker, OctoPrint CLI, Bambu Studio)?

Related: the network printer discovery todo (mDNS/SSDP) already concluded that discovery is application-layer, not core library. The sync-from-printer todo assumes some printer communication exists.

## Solution

Evaluate scope and decide:

**Option A — Minimal (CLI convenience, not library)**:
- Add `slicecore send <file.gcode> --printer <address>` as a CLI-only convenience
- Support 2-3 common protocols: Moonraker (Klipper), OctoPrint, Bambu MQTT
- Keep protocol implementations in the CLI crate, not the library — consumers with different needs use their own HTTP/MQTT clients
- Pairs with the sync todo: if we're already talking to printers, sending is a natural extension

**Option B — Library-level abstraction**:
- Create a `slicecore-printer` crate with a `PrinterConnection` trait
- Protocol implementations as feature-gated backends
- Enables downstream apps (GUIs, farm managers) to reuse the same printer communication layer
- More work, but prevents every consumer from reimplementing the same protocols

**Option C — Out of scope**:
- Document recommended tools for each printer type
- Focus libslic3r-rs purely on slicing, let users pipe output to existing tools
- Simplest, but users lose the end-to-end workflow that competing slicers offer

**Recommendation**: Start with Option A (CLI convenience) to validate the workflow. Promote to Option B if multiple consumers need it. The CLI already has `--printer` profile awareness, so `send` is a natural subcommand.
