---
phase: 39-jpeg-thumbnail-export
plan: 01
subsystem: render
tags: [image, jpeg, png, thumbnails, encoding]

# Dependency graph
requires:
  - phase: 08-thumbnail-generation
    provides: "render pipeline with PNG encoding and ThumbnailConfig/Thumbnail structs"
provides:
  - "ImageFormat enum (Png/Jpeg) exported from slicecore-render"
  - "JPEG encoding with alpha compositing via image crate"
  - "encode.rs dispatcher for PNG and JPEG encoding"
  - "Renamed encoded_data field on Thumbnail struct"
  - "Renamed thumbnail_data params in fileio export"
  - "quality and output_format fields on ThumbnailConfig"
affects: [39-02, 39-03, cli-thumbnail, 3mf-export, gcode-embed]

# Tech tracking
tech-stack:
  added: ["image 0.25 (replaces png 0.17)"]
  patterns: ["format-agnostic thumbnail encoding dispatch", "alpha compositing onto white for JPEG"]

key-files:
  created:
    - "crates/slicecore-render/src/encode.rs"
  modified:
    - "crates/slicecore-render/Cargo.toml"
    - "crates/slicecore-render/src/lib.rs"
    - "crates/slicecore-render/src/gcode_embed.rs"
    - "crates/slicecore-gcode-io/src/thumbnail.rs"
    - "crates/slicecore-fileio/src/export.rs"
    - "crates/slicecore-cli/src/main.rs"
    - "crates/slicecore-render/tests/integration.rs"

key-decisions:
  - "Replaced png crate with image crate for unified PNG+JPEG encoding"
  - "Used alpha compositing onto white background for JPEG transparency handling"
  - "Removed unsafe code from encoding (old png_encode.rs had unsafe slice cast)"

patterns-established:
  - "encode::encode() dispatcher pattern for multi-format image encoding"
  - "Format-agnostic field naming (encoded_data, thumbnail_data) across crates"

requirements-completed: [JPEG-01, JPEG-02, JPEG-03]

# Metrics
duration: 7min
completed: 2026-03-19
---

# Phase 39 Plan 01: Image Encoding Foundation Summary

**Migrated render crate from png to image crate with PNG+JPEG dual encoding, ImageFormat enum, alpha compositing, and field renames across 4 crates**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-19T18:33:46Z
- **Completed:** 2026-03-19T18:41:31Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- Replaced `png` crate with `image` crate enabling both PNG and JPEG encoding
- Added `ImageFormat` enum with `Png`/`Jpeg` variants and `extension()` method
- Created `encode.rs` with dispatcher, PNG encoder, and JPEG encoder with alpha compositing
- Renamed `png_data` to `encoded_data` and `thumbnail_png` to `thumbnail_data` across all 4 consuming crates
- Added `output_format` and `quality` fields to `ThumbnailConfig` and `format` field to `Thumbnail`
- Eliminated unsafe code that was present in the old `png_encode.rs`

## Task Commits

Each task was committed atomically:

1. **Task 1: Replace png crate with image crate, create encode.rs** - `e822192` (feat)
2. **Task 2: Rename png_data/thumbnail_png across crates** - `6408c35` (refactor)

## Files Created/Modified
- `crates/slicecore-render/src/encode.rs` - New multi-format encoding module (PNG + JPEG)
- `crates/slicecore-render/src/lib.rs` - ImageFormat enum, updated ThumbnailConfig/Thumbnail structs
- `crates/slicecore-render/Cargo.toml` - Replaced png dep with image crate
- `crates/slicecore-render/src/gcode_embed.rs` - Renamed png_data -> encoded_data
- `crates/slicecore-render/tests/integration.rs` - Updated for new fields and image crate decode
- `crates/slicecore-gcode-io/src/thumbnail.rs` - Renamed png_data param -> encoded_data
- `crates/slicecore-fileio/src/export.rs` - Renamed thumbnail_png -> thumbnail_data
- `crates/slicecore-cli/src/main.rs` - Updated field access and ThumbnailConfig construction

## Decisions Made
- Replaced `png 0.17` with `image 0.25` (default-features=false, png+jpeg features) for unified encoding
- Alpha compositing onto white background for JPEG (transparent pixels become white RGB)
- Used `mul_add` for clippy::pedantic compliance in alpha compositing math
- Removed unsafe slice cast from old encoding (image crate handles flat pixel data safely)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed gcode_embed.rs field references in Task 1**
- **Found during:** Task 1 (encode tests wouldn't compile)
- **Issue:** gcode_embed.rs references `thumbnail.png_data` which prevented the render crate from compiling
- **Fix:** Updated gcode_embed.rs field accesses and test constructors in Task 1 (planned for Task 2)
- **Files modified:** crates/slicecore-render/src/gcode_embed.rs
- **Verification:** Crate compiles, all tests pass
- **Committed in:** e822192 (Task 1 commit)

**2. [Rule 3 - Blocking] Fixed integration tests for new struct fields**
- **Found during:** Task 1 (integration tests wouldn't compile)
- **Issue:** Integration tests constructed ThumbnailConfig without new fields and referenced png_data
- **Fix:** Added output_format/quality fields and renamed png_data in integration tests, replaced png crate decode with image crate
- **Files modified:** crates/slicecore-render/tests/integration.rs
- **Verification:** All 11 integration tests pass
- **Committed in:** e822192 (Task 1 commit)

**3. [Rule 3 - Blocking] Added image and base64 to dev-dependencies**
- **Found during:** Task 1 (integration tests use image crate for PNG decode verification)
- **Issue:** Integration tests need image crate for round-trip PNG verification
- **Fix:** Added image and base64 to dev-dependencies in Cargo.toml
- **Files modified:** crates/slicecore-render/Cargo.toml
- **Committed in:** e822192 (Task 1 commit)

---

**Total deviations:** 3 auto-fixed (3 blocking)
**Impact on plan:** All auto-fixes were necessary for compilation. The gcode_embed rename was planned for Task 2 but required in Task 1 because both modules are in the same crate. No scope creep.

## Issues Encountered
None - all changes compiled and tested correctly on first attempt.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- ImageFormat enum and quality field ready for CLI flag integration (39-02)
- JPEG encoding tested and verified with magic byte checks
- All existing tests updated and passing across entire workspace

---
*Phase: 39-jpeg-thumbnail-export*
*Completed: 2026-03-19*
