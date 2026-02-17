//! Native plugin loader using `abi_stable`.
//!
//! Loads native dynamic libraries (`.so`, `.dll`, `.dylib`) as infill plugins.
//! The loader resolves library paths from plugin manifests and uses
//! `abi_stable`'s [`RootModule`] infrastructure for ABI-safe loading with
//! automatic type layout verification.

#[cfg(not(target_family = "wasm"))]
mod imp {
    use std::path::{Path, PathBuf};

    use abi_stable::library::RootModule;
    use abi_stable::std_types::RResult::{RErr, ROk};

    use slicecore_plugin_api::{
        InfillPluginMod_Ref, InfillPatternPlugin_TO, InfillRequest, InfillResult, PluginManifest,
    };

    use crate::error::PluginSystemError;
    use crate::registry::InfillPluginAdapter;

    /// A loaded native infill plugin.
    ///
    /// Wraps the FFI-safe trait object from `abi_stable` and provides
    /// the [`InfillPluginAdapter`] interface for the registry.
    pub struct NativeInfillPlugin {
        /// The FFI-safe trait object loaded from the dynamic library.
        inner: InfillPatternPlugin_TO<'static, abi_stable::std_types::RBox<()>>,
        /// Path to the loaded library file (for diagnostics).
        #[allow(dead_code)]
        library_path: PathBuf,
    }

    // SAFETY: The abi_stable trait object is Send + Sync by construction
    // (InfillPatternPlugin requires Send + Sync).
    unsafe impl Send for NativeInfillPlugin {}
    unsafe impl Sync for NativeInfillPlugin {}

    impl InfillPluginAdapter for NativeInfillPlugin {
        fn name(&self) -> String {
            self.inner.name().into()
        }

        fn description(&self) -> String {
            self.inner.description().into()
        }

        fn generate(
            &self,
            request: &InfillRequest,
        ) -> Result<InfillResult, PluginSystemError> {
            match self.inner.generate(request) {
                ROk(result) => Ok(result),
                RErr(err_msg) => Err(PluginSystemError::ExecutionFailed {
                    plugin: self.name(),
                    message: err_msg.into(),
                }),
            }
        }

        fn plugin_type(&self) -> crate::registry::PluginKind {
            crate::registry::PluginKind::Native
        }
    }

    /// Loads a native infill plugin from a directory.
    ///
    /// Resolves the library path from the manifest's `library_filename`, then
    /// loads it via `abi_stable`'s [`RootModule::load_from_directory`].
    ///
    /// # Library Path Resolution
    ///
    /// The manifest's `library_filename` is just a filename (e.g., `libzigzag.so`).
    /// This function searches multiple candidate locations in order:
    /// 1. Direct: `plugin_dir/<filename>` (installed plugins)
    /// 2. Debug build: `plugin_dir/target/debug/<filename>` (development)
    /// 3. Release build: `plugin_dir/target/release/<filename>` (production)
    pub fn load_native_plugin(
        plugin_dir: &Path,
        manifest: &PluginManifest,
    ) -> Result<NativeInfillPlugin, PluginSystemError> {
        let library_filename = &manifest.library_filename;
        let library_path = resolve_library_path(plugin_dir, library_filename)?;

        // abi_stable's RootModule::load_from_directory scans for a library
        // matching the module name in the given directory. We pass the parent
        // directory of the resolved library path.
        let lib_dir = library_path
            .parent()
            .unwrap_or(plugin_dir);
        let module =
            InfillPluginMod_Ref::load_from_directory(lib_dir).map_err(|e| {
                PluginSystemError::LoadFailed {
                    path: library_path.clone(),
                    reason: format!("abi_stable load failed: {}", e),
                }
            })?;

        let plugin_instance = module.new()();
        Ok(NativeInfillPlugin {
            inner: plugin_instance,
            library_path,
        })
    }

    /// Resolves the actual library path from a manifest filename.
    ///
    /// Searches multiple candidate locations and returns the first that exists.
    fn resolve_library_path(
        plugin_dir: &Path,
        library_filename: &str,
    ) -> Result<PathBuf, PluginSystemError> {
        let candidates = [
            // 1. Direct path: plugin_dir/libfoo.so (installed plugins)
            plugin_dir.join(library_filename),
            // 2. Debug build: plugin_dir/target/debug/libfoo.so (development)
            plugin_dir
                .join("target")
                .join("debug")
                .join(library_filename),
            // 3. Release build: plugin_dir/target/release/libfoo.so (production)
            plugin_dir
                .join("target")
                .join("release")
                .join(library_filename),
        ];

        for candidate in &candidates {
            if candidate.exists() {
                return Ok(candidate.clone());
            }
        }

        Err(PluginSystemError::LoadFailed {
            path: plugin_dir.to_path_buf(),
            reason: format!(
                "Library '{}' not found. Searched: {}",
                library_filename,
                candidates
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::fs;
        use tempfile::TempDir;

        #[test]
        fn resolve_library_path_direct() {
            let dir = TempDir::new().unwrap();
            let lib_file = dir.path().join("libtest.so");
            fs::write(&lib_file, b"fake library").unwrap();

            let result = resolve_library_path(dir.path(), "libtest.so");
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), lib_file);
        }

        #[test]
        fn resolve_library_path_debug_build() {
            let dir = TempDir::new().unwrap();
            let debug_dir = dir.path().join("target").join("debug");
            fs::create_dir_all(&debug_dir).unwrap();
            let lib_file = debug_dir.join("libtest.so");
            fs::write(&lib_file, b"fake library").unwrap();

            let result = resolve_library_path(dir.path(), "libtest.so");
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), lib_file);
        }

        #[test]
        fn resolve_library_path_release_build() {
            let dir = TempDir::new().unwrap();
            let release_dir = dir.path().join("target").join("release");
            fs::create_dir_all(&release_dir).unwrap();
            let lib_file = release_dir.join("libtest.so");
            fs::write(&lib_file, b"fake library").unwrap();

            let result = resolve_library_path(dir.path(), "libtest.so");
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), lib_file);
        }

        #[test]
        fn resolve_library_path_direct_takes_priority() {
            let dir = TempDir::new().unwrap();

            // Create both direct and debug versions
            let direct_file = dir.path().join("libtest.so");
            fs::write(&direct_file, b"direct").unwrap();

            let debug_dir = dir.path().join("target").join("debug");
            fs::create_dir_all(&debug_dir).unwrap();
            fs::write(debug_dir.join("libtest.so"), b"debug").unwrap();

            let result = resolve_library_path(dir.path(), "libtest.so");
            assert!(result.is_ok());
            // Direct path should take priority
            assert_eq!(result.unwrap(), direct_file);
        }

        #[test]
        fn resolve_library_path_missing_returns_error() {
            let dir = TempDir::new().unwrap();

            let result = resolve_library_path(dir.path(), "libmissing.so");
            assert!(result.is_err());
            let err = result.unwrap_err();
            match &err {
                PluginSystemError::LoadFailed { reason, .. } => {
                    assert!(reason.contains("libmissing.so"));
                    assert!(reason.contains("not found"));
                    // Should list all searched paths
                    assert!(reason.contains("target/debug"));
                    assert!(reason.contains("target/release"));
                }
                _ => panic!("Expected LoadFailed, got {:?}", err),
            }
        }
    }
}

#[cfg(not(target_family = "wasm"))]
pub use imp::*;
