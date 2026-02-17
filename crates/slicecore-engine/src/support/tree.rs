//! Tree support generation with bottom-up growth, branching, and merging.
//!
//! This module implements tree-style support structures that grow from the
//! build plate upward toward overhang contact points. Tree supports use
//! less material than traditional supports and leave smaller contact marks.
//!
//! # Algorithm
//!
//! 1. Extract contact points from overhang regions.
//! 2. Grow tree from build plate upward to each contact point.
//! 3. Merge nearby branches for material efficiency.
//! 4. Apply taper and branch style.
//! 5. Slice tree into per-layer support polygons.
