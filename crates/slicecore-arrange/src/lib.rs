//! Build plate auto-arrangement for the slicecore 3D slicing engine.
//!
//! This crate provides automatic positioning of multiple 3D-printed parts
//! on a build plate to maximize utilization and minimize wasted space.
//! It supports arbitrary bed shapes, convex hull footprint projection,
//! spacing/brim/raft-aware footprint expansion, and collision detection.
//!
//! # Architecture
//!
//! - **Bed parsing** ([`bed`]): Parse bed shape strings and create bed polygons
//! - **Footprint computation** ([`footprint`]): Project 3D meshes to 2D convex
//!   hull footprints, expand for spacing, and detect collisions
//! - **Configuration** ([`config`]): Control arrangement behavior via
//!   [`ArrangeConfig`] and describe parts via [`ArrangePart`]
//! - **Results** ([`result`]): Output structures ([`ArrangementResult`],
//!   [`PlateArrangement`], [`PartPlacement`]) describing the arrangement

#![warn(
    clippy::all,
    clippy::pedantic,
    clippy::cargo,
    missing_docs,
    missing_debug_implementations
)]
#![allow(
    clippy::cargo_common_metadata,
    clippy::multiple_crate_versions,
    clippy::module_name_repetitions
)]

pub mod bed;
pub mod config;
pub mod error;
pub mod footprint;
pub mod result;

pub use config::{ArrangeConfig, ArrangePart, GantryModel, OrientCriterion};
pub use error::ArrangeError;
pub use result::{ArrangementResult, PartPlacement, PlateArrangement};
