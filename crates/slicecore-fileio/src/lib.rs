//! File I/O for 3D mesh formats.
//!
//! This crate provides parsers for common 3D model file formats used in
//! 3D printing: STL (binary and ASCII), 3MF, and OBJ. It also provides
//! magic-byte format detection to automatically identify file types.
//!
//! # Supported Formats
//!
//! | Format     | Import | Export | Module          |
//! |------------|--------|--------|-----------------|
//! | Binary STL | Yes    | -      | [`stl_binary`]  |
//! | ASCII STL  | -      | -      | (plan 02-01 T2) |
//! | 3MF        | -      | -      | (plan 02-04)    |
//! | OBJ        | -      | -      | (plan 02-04)    |
//!
//! # Format Detection
//!
//! Use [`detect_format`] to identify the format of a byte buffer before
//! parsing. This handles the well-known "binary STL starting with solid"
//! ambiguity.

pub mod detect;
pub mod error;
pub mod stl_binary;

// Re-export primary types at crate root.
pub use detect::{detect_format, MeshFormat};
pub use error::FileIOError;
