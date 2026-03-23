---
type: quick-fix
id: 260323-wwu
completed: 2026-03-23T23:43:55Z
duration: 24s
tasks_completed: 1
tasks_total: 1
key-files:
  modified:
    - scripts/qa_tests
decisions: []
---

# Quick Fix 260323-wwu: Fix profile set/get QA test failures

Updated QA test script to use renamed `profile setting` command after Phase 44-03 renamed the config value setter from `profile set` to `profile setting`.

## Tasks Completed

| Task | Name | Commit | Files |
| ---- | ---- | ------ | ----- |
| 1 | Update QA test to use `profile setting` command | 95ba52a | scripts/qa_tests |

## Changes Made

Changed lines 900-901 in `scripts/qa_tests`:
- Test label: `"profile set"` -> `"profile setting"`
- Command: `profile set my-printer` -> `profile setting my-printer`

The `profile get` test on lines 904-905 was unchanged as `get` was not renamed.

## Deviations from Plan

None - plan executed exactly as written.

## Verification

- `grep "profile setting my-printer" scripts/qa_tests` returns the updated line (line 901)
- `grep -c "profile set my-printer" scripts/qa_tests` returns 0 (no old usage remains)
