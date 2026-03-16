---
created: 2026-03-16T18:05:00.000Z
title: Headless daemon slicer with job queue for farms and SaaS
area: general
files: []
---

## Problem

The current architecture is single-invocation CLI: one command slices one model, then exits. This works for home users but doesn't serve two important use cases:

1. **Print farms**: Need to queue dozens/hundreds of slice jobs with priority scheduling, so operators submit STLs and profiles and get G-code back without babysitting each invocation.
2. **SaaS / cloud slicing**: A web service needs a long-running backend that accepts slice requests via API, manages concurrency, and returns results — not fork-exec of a CLI per request.

Both need a persistent daemon that:
- Accepts jobs (local socket, HTTP API, or message queue)
- Manages a priority queue (urgent jobs preempt batch jobs)
- Parallelizes slicing across available cores
- Reports progress per job
- Stores results (G-code, thumbnails, cost estimates)
- Handles failures gracefully (retry, notify, skip)

## Solution

Discussion points:

1. **Architecture**: Separate `slicecore-daemon` crate or feature-gated mode in CLI (`slicecore slice --daemon`)?
2. **Transport**: Unix socket for local, HTTP/gRPC for remote, or both?
3. **Queue backend**: In-memory (tokio channels) for simple cases, or pluggable (Redis, SQLite) for persistence across restarts?
4. **Priority model**: Simple numeric priority? Deadline-based? Fair-share per user/printer?
5. **Concurrency**: How many parallel slices? Per-core? Configurable pool size? Memory-bounded?
6. **Job lifecycle**: submit → queued → slicing → postprocessing → complete/failed → cleanup
7. **Integration with existing crates**: `slicecore-engine` is already the core — daemon wraps it with scheduling
8. **SaaS considerations**: Multi-tenant isolation, rate limiting, billing hooks, result storage (S3/local)
9. **Relationship to TUI todo**: Daemon could be the backend that a TUI or web UI connects to
10. **Relationship to network printer discovery todo**: Daemon could route G-code directly to discovered printers

Recommend `/gsd:discuss-phase` to explore before committing. This could be a milestone-level feature (v2.0+).
