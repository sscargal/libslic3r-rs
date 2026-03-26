---
quick_id: 260326-3ih
description: Fix cargo clippy warnings in slicecore-slicer
date: 2026-03-26
commit: f655e17
---

# Quick Task 260326-3ih: Fix cargo clippy warnings in slicecore-slicer

## What was done
Fixed 3 clippy errors in slicecore-slicer crate:

1. **Dead code** (`adaptive.rs:173`): `smooth_heights` only used in tests — added `#[cfg(test)]`
2. **Manual range contains** (`vlh/optimizer.rs:282`): Replaced `ratio > MAX || ratio < MIN` with `!range.contains(&ratio)`
3. **Useless vec!** (`vlh/mod.rs:332`): Changed `vec![...]` to array literal `[...]` since the collection is fixed-size

## Verification
`cargo clippy --all-features -- -D warnings` passes clean.
