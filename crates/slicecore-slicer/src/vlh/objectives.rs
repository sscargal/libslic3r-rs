//! Objective scoring functions for multi-objective VLH optimization.
//!
//! Each objective maps mesh geometry information at a given Z height to a
//! desired layer height. The four objectives (quality, speed, strength,
//! material) are combined via weighted sum to produce a single target height.
