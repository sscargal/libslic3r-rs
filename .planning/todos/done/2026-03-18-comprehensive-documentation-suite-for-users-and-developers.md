---
created: 2026-03-18T19:57:15.106Z
title: Comprehensive documentation suite for users and developers
area: docs
files:
  - docs/
  - crates/slicecore-cli/src/main.rs
---

## Problem

libslic3r-rs has no user-facing documentation beyond code comments and the planning directory. As the project approaches usability, users and contributors need structured documentation covering installation, usage, configuration, API reference, and development workflows. Without docs, adoption is blocked — users can't discover features, admins can't deploy, and developers can't contribute.

## Solution

Create a comprehensive documentation suite, likely using mdBook or similar Rust-ecosystem tooling:

1. **Install guide**: Platform-specific instructions (Linux, macOS, Windows), building from source, pre-built binaries, WASM setup
2. **User guide**: CLI usage walkthrough, profile management, slicing a model end-to-end, multi-material setup, plugin system, calibration workflows
3. **Admin guide**: Headless/daemon deployment, print farm configuration, job queue setup, network printer integration
4. **Examples**: Annotated real-world workflows — "slice a benchy", "set up multi-color print", "create custom profile", "use AI suggestions", "write a plugin"
5. **API reference**: Generated from rustdoc for library consumers, with usage examples for each public module
6. **Developer guide**: Architecture overview, crate dependency map, contributing guidelines, testing strategy, adding new G-code flavors, writing plugins
7. **Configuration reference**: Complete listing of all profile settings with descriptions, defaults, valid ranges, and interaction notes
8. **Changelog / migration**: Version history, breaking changes, upgrade paths
