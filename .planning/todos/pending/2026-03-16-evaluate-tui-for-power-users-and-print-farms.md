---
created: 2026-03-16T18:00:00.000Z
title: Evaluate TUI for power users and print farms
area: cli
files: []
---

## Problem

The current CLI is command-based (run a command, get output, done). Power users and print farm operators may benefit from a persistent terminal UI (TUI) that provides:

- Real-time slicing progress across multiple jobs
- Queue management for batch/farm slicing
- Live G-code preview or layer-by-layer stats
- Profile selection and parameter tweaking without memorizing flags
- Printer status monitoring (ties into network printer discovery todo)

However, a TUI is a significant investment. It's unclear whether the target audience would actually use it vs. scripting the CLI, using a web UI, or relying on existing farm management tools (OctoPrint, Mainsail, etc.).

## Solution

Discussion points to resolve before committing:

1. **Target users**: Who specifically? Solo power users? Print farm operators? Both?
2. **Competing solutions**: Do OctoPrint/Mainsail/Repetier-Server already cover this? What gap exists?
3. **Scope**: Full interactive TUI (ratatui/crossterm) vs. enhanced CLI with live-updating output (indicatif multi-progress)?
4. **Crate choice**: `ratatui` is the ecosystem standard; `crossterm` for backend
5. **MVP features**: What's the minimum TUI that's actually more useful than plain CLI?
6. **Print farm angle**: Batch queue, multi-printer status, job routing — or is that better served by a daemon + web UI?
7. **Maintenance cost**: TUI code is notoriously hard to test and maintain

Recommend starting with a `/gsd:discuss-phase` conversation to explore before committing to implementation.
