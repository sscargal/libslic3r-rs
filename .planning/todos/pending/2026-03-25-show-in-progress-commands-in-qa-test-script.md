---
created: 2026-03-25T00:49:52.214Z
title: Show in-progress commands in QA test script
area: tooling
files:
  - scripts/qa_tests
---

## Problem

When running `scripts/qa_tests`, long-running commands (like `cargo test --all-features --workspace`) produce no visible output indicating what's currently executing. After the build gates pass, the script appears to hang with no indication of which test is running. This makes it hard to distinguish "still working" from "stuck".

## Solution

Add a progress indicator that prints the current command before execution, e.g.:
- Print `  [....] cargo test --all-features --workspace` before running, then overwrite with `[PASS]` or `[FAIL]` on completion
- Or use a simpler approach: print `  [RUN]  command...` before each test, so the user always sees what's in flight
- Consider a `--progress` flag or making it the default behavior in the `run_test` function
