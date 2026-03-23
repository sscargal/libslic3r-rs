---
type: quick-fix
files_modified:
  - scripts/qa_tests
autonomous: true
---

<objective>
Fix QA test failures caused by the Phase 44-03 rename of `profile set` (config setter) to `profile setting`.

The QA script at `scripts/qa_tests` line 900-901 still calls `profile set <name> <key> <value>`, which now routes to the new ProfileSetCommand (profile set management) instead of the config value setter. Update to use `profile setting`.
</objective>

<context>
@scripts/qa_tests (lines 895-905)
</context>

<tasks>

<task type="auto">
  <name>Task 1: Update QA test to use `profile setting` command</name>
  <files>scripts/qa_tests</files>
  <action>
In `scripts/qa_tests`, update lines 900-901:

Change:
```
run_test "profile set" \
    "$SLICECORE" profile set my-printer machine.nozzle_diameters 0.6
```

To:
```
run_test "profile setting" \
    "$SLICECORE" profile setting my-printer machine.nozzle_diameters 0.6
```

Both the test label (first arg to run_test) and the actual command need updating. The `profile get` command on lines 904-905 is unchanged -- `get` was not renamed.
  </action>
  <verify>
    <automated>grep -n "profile setting my-printer" scripts/qa_tests && ! grep -n 'profile set my-printer' scripts/qa_tests</automated>
  </verify>
  <done>QA test calls `profile setting` instead of `profile set` for the config value setter. The `profile get` test on the following line should now pass because the set actually succeeds.</done>
</task>

</tasks>

<verification>
- `grep "profile setting my-printer" scripts/qa_tests` returns the updated line
- `grep "profile set my-printer" scripts/qa_tests` returns nothing (no old usage remains)
- Optionally: run the full QA test suite to confirm both "profile setting" and "profile get" tests pass
</verification>

<success_criteria>
- The QA test script uses `profile setting` for config value setting
- No remaining references to the old `profile set <name> <key> <value>` pattern
</success_criteria>
