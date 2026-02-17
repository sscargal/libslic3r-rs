//! FFI-safe error types for plugin communication.
//!
//! These error types use [`RString`] instead of `String` to safely cross
//! the FFI boundary between host and plugin.

use abi_stable::std_types::RString;
use abi_stable::StableAbi;
use std::fmt;

/// An FFI-safe error type for plugin operations.
///
/// Uses [`RString`] for the message to ensure safe transmission across
/// the dynamic library boundary.
#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct PluginError {
    /// Human-readable error message.
    pub message: RString,
}

impl PluginError {
    /// Creates a new plugin error with the given message.
    pub fn new(message: impl Into<RString>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PluginError {}

impl From<&str> for PluginError {
    fn from(s: &str) -> Self {
        Self::new(RString::from(s))
    }
}

impl From<String> for PluginError {
    fn from(s: String) -> Self {
        Self::new(RString::from(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_error_display() {
        let err = PluginError::new("something went wrong");
        assert_eq!(format!("{}", err), "something went wrong");
    }

    #[test]
    fn plugin_error_from_str() {
        let err = PluginError::from("test error");
        assert_eq!(err.message.as_str(), "test error");
    }

    #[test]
    fn plugin_error_from_string() {
        let err = PluginError::from(String::from("test error"));
        assert_eq!(err.message.as_str(), "test error");
    }

    #[test]
    fn plugin_error_is_error_trait() {
        let err = PluginError::new("test");
        // Verify it implements std::error::Error
        let _: &dyn std::error::Error = &err;
    }
}
