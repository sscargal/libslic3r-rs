//! Overhang detection using hybrid layer-diff and raycast validation.
//!
//! Implements the core overhang detection algorithm:
//! 1. **Layer comparison**: Compare adjacent layers to find regions extending
//!    beyond the configurable overhang angle threshold.
//! 2. **Raycast validation**: Cast downward rays to filter false positives
//!    from internally-supported geometry.
//! 3. **Area filtering**: Remove unprintable tiny regions using a two-tier
//!    area threshold.
