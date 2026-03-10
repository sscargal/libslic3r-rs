//! Error types for file I/O operations.

use thiserror::Error;

/// Errors that can occur during mesh file parsing and format detection.
#[derive(Debug, Error)]
pub enum FileIOError {
    /// File is too small to identify its format.
    #[error("file too small to identify format ({0} bytes)")]
    FileTooSmall(usize),

    /// Cannot detect a recognized mesh format from the file content.
    #[error("unrecognized mesh file format")]
    UnrecognizedFormat,

    /// File is truncated before the expected end.
    #[error("unexpected end of file: {0}")]
    UnexpectedEof(String),

    /// ASCII STL file contains non-UTF-8 bytes.
    #[error("invalid UTF-8 in ASCII STL file")]
    InvalidUtf8,

    /// A parse error with a descriptive message.
    #[error("parse error: {0}")]
    ParseError(String),

    /// 3MF-specific error (placeholder for plan 02-04).
    #[error("3MF error: {0}")]
    ThreeMfError(String),

    /// OBJ-specific error (placeholder for plan 02-04).
    #[error("OBJ error: {0}")]
    ObjError(String),

    /// File contains no mesh data.
    #[error("file contains no mesh data")]
    EmptyModel,

    /// Upstream mesh construction error.
    #[error(transparent)]
    MeshError(#[from] slicecore_mesh::MeshError),

    /// Error during mesh export/write operations.
    #[error("write error: {0}")]
    WriteError(String),

    /// Unsupported export format (file extension not recognized for export).
    #[error("unsupported export format: {0}")]
    UnsupportedExportFormat(String),

    /// I/O error from the standard library.
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}
