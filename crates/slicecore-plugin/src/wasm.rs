//! WASM plugin loader using wasmtime Component Model.
//!
//! Loads WebAssembly component files (`.wasm`) as infill plugins using
//! the wasmtime runtime with Component Model support. Each plugin is
//! sandboxed with configurable memory and CPU fuel limits.
//!
//! The WIT interface definition (`wit/slicecore-plugin.wit`) mirrors the
//! native plugin API, enabling both native and WASM plugins to be loaded
//! through the same `InfillPluginAdapter` trait.

#[cfg(feature = "wasm-plugins")]
mod wasm_impl {
    use std::path::Path;

    use wasmtime::component::{Component, Linker};
    use wasmtime::{Config, Engine as WasmEngine, Store};
    use wasmtime_wasi::WasiCtxBuilder;

    use crate::error::PluginSystemError;
    use crate::registry::{InfillPluginAdapter, PluginKind};
    use crate::sandbox::SandboxConfig;
    use abi_stable::std_types::RVec;
    use slicecore_plugin_api::types::FfiInfillLine;

    // The bindgen! macro generates types with the same names as the FFI types
    // (InfillRequest, InfillResult), so we use fully qualified paths for the
    // FFI types rather than importing them at module level.
    wasmtime::component::bindgen!({
        world: "infill-plugin",
        path: "wit/slicecore-plugin.wit",
    });

    /// Internal state held in each wasmtime Store.
    struct PluginState {
        wasi_ctx: wasmtime_wasi::WasiCtx,
        table: wasmtime::component::ResourceTable,
    }

    impl wasmtime_wasi::WasiView for PluginState {
        fn ctx(&mut self) -> wasmtime_wasi::WasiCtxView<'_> {
            wasmtime_wasi::WasiCtxView {
                ctx: &mut self.wasi_ctx,
                table: &mut self.table,
            }
        }
    }

    /// A loaded WASM infill plugin.
    ///
    /// Uses wasmtime Component Model with sandboxed execution. Each `generate()`
    /// call creates a fresh [`Store`] with the configured fuel limits, so a
    /// misbehaving plugin cannot accumulate state or exceed resource bounds
    /// across calls.
    pub struct WasmInfillPlugin {
        engine: WasmEngine,
        component: Component,
        sandbox_config: SandboxConfig,
        cached_name: String,
        cached_description: String,
    }

    impl WasmInfillPlugin {
        /// Loads a WASM component from the given path with the specified sandbox
        /// configuration.
        ///
        /// This instantiates the component once to query its `name()` and
        /// `description()` exports, caching the results for subsequent calls.
        pub fn load(
            wasm_path: &Path,
            sandbox_config: SandboxConfig,
        ) -> Result<Self, PluginSystemError> {
            let mut config = Config::new();
            config.wasm_component_model(true);
            config.consume_fuel(true);

            let engine = WasmEngine::new(&config).map_err(|e| PluginSystemError::LoadFailed {
                path: wasm_path.to_path_buf(),
                reason: format!("Failed to create WASM engine: {}", e),
            })?;

            let component =
                Component::from_file(&engine, wasm_path).map_err(|e| {
                    PluginSystemError::LoadFailed {
                        path: wasm_path.to_path_buf(),
                        reason: format!("Failed to load WASM component: {}", e),
                    }
                })?;

            // Instantiate once to get name/description (cached)
            let (name, description) = Self::query_metadata(&engine, &component, wasm_path)?;

            Ok(Self {
                engine,
                component,
                sandbox_config,
                cached_name: name,
                cached_description: description,
            })
        }

        /// Queries the plugin's name and description by instantiating it once.
        ///
        /// Uses generous fuel (10M instructions) since metadata queries are
        /// lightweight and should never exhaust fuel.
        fn query_metadata(
            engine: &WasmEngine,
            component: &Component,
            wasm_path: &Path,
        ) -> Result<(String, String), PluginSystemError> {
            let wasi_ctx = WasiCtxBuilder::new().build();
            let state = PluginState {
                wasi_ctx,
                table: wasmtime::component::ResourceTable::new(),
            };
            let mut store = Store::new(engine, state);
            // Use generous fuel for metadata queries (not the sandbox limit,
            // which may be intentionally low for testing crash isolation).
            store
                .set_fuel(10_000_000)
                .map_err(|e| PluginSystemError::LoadFailed {
                    path: wasm_path.to_path_buf(),
                    reason: format!("Failed to set fuel: {}", e),
                })?;

            let mut linker = Linker::new(engine);
            wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|e| {
                PluginSystemError::LoadFailed {
                    path: wasm_path.to_path_buf(),
                    reason: format!("Failed to link WASI: {}", e),
                }
            })?;

            let bindings =
                InfillPlugin::instantiate(&mut store, component, &linker).map_err(|e| {
                    PluginSystemError::LoadFailed {
                        path: wasm_path.to_path_buf(),
                        reason: format!("Failed to instantiate WASM component: {}", e),
                    }
                })?;

            let name =
                bindings
                    .call_name(&mut store)
                    .map_err(|e| PluginSystemError::LoadFailed {
                        path: wasm_path.to_path_buf(),
                        reason: format!("Failed to call name(): {}", e),
                    })?;

            let description =
                bindings
                    .call_description(&mut store)
                    .map_err(|e| PluginSystemError::LoadFailed {
                        path: wasm_path.to_path_buf(),
                        reason: format!("Failed to call description(): {}", e),
                    })?;

            Ok((name, description))
        }

        /// Creates a fresh Store with the configured sandbox fuel limits.
        fn create_store(&self) -> Result<Store<PluginState>, PluginSystemError> {
            let wasi_ctx = WasiCtxBuilder::new().build();
            let state = PluginState {
                wasi_ctx,
                table: wasmtime::component::ResourceTable::new(),
            };
            let mut store = Store::new(&self.engine, state);
            store
                .set_fuel(self.sandbox_config.max_cpu_fuel)
                .map_err(|e| PluginSystemError::ExecutionFailed {
                    plugin: self.cached_name.clone(),
                    message: format!("Failed to set fuel: {}", e),
                })?;
            Ok(store)
        }

        /// Creates a fresh Linker and instantiates the component.
        fn instantiate(
            &self,
            store: &mut Store<PluginState>,
        ) -> Result<InfillPlugin, PluginSystemError> {
            let mut linker = Linker::new(&self.engine);
            wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|e| {
                PluginSystemError::ExecutionFailed {
                    plugin: self.cached_name.clone(),
                    message: format!("Failed to link WASI: {}", e),
                }
            })?;

            let bindings =
                InfillPlugin::instantiate(store, &self.component, &linker).map_err(|e| {
                    PluginSystemError::ExecutionFailed {
                        plugin: self.cached_name.clone(),
                        message: format!("Failed to instantiate WASM component: {}", e),
                    }
                })?;

            Ok(bindings)
        }

        /// Converts an FFI InfillRequest to the WIT-generated request type.
        fn convert_request(
            request: &slicecore_plugin_api::InfillRequest,
        ) -> slicecore::plugin::types::InfillRequest {
            // Decode boundary_points from flattened [x0,y0,x1,y1,...] to list<point2>
            let boundary_points: Vec<slicecore::plugin::types::Point2> = request
                .boundary_points
                .chunks(2)
                .map(|chunk| slicecore::plugin::types::Point2 {
                    x: chunk[0],
                    y: chunk[1],
                })
                .collect();

            let boundary_lengths: Vec<u32> =
                request.boundary_lengths.iter().copied().collect();

            slicecore::plugin::types::InfillRequest {
                boundary_points,
                boundary_lengths,
                density: request.density,
                layer_index: request.layer_index,
                layer_z: request.layer_z,
                line_width: request.line_width,
            }
        }

        /// Converts the WIT-generated result type back to FFI InfillResult.
        fn convert_result(
            result: slicecore::plugin::types::InfillResult,
        ) -> slicecore_plugin_api::InfillResult {
            let lines: Vec<FfiInfillLine> = result
                .lines
                .into_iter()
                .map(|line| FfiInfillLine {
                    start_x: line.start.x,
                    start_y: line.start.y,
                    end_x: line.end.x,
                    end_y: line.end.y,
                })
                .collect();

            slicecore_plugin_api::InfillResult {
                lines: RVec::from(lines),
            }
        }
    }

    impl InfillPluginAdapter for WasmInfillPlugin {
        fn name(&self) -> String {
            self.cached_name.clone()
        }

        fn description(&self) -> String {
            self.cached_description.clone()
        }

        fn generate(
            &self,
            request: &slicecore_plugin_api::InfillRequest,
        ) -> Result<slicecore_plugin_api::InfillResult, PluginSystemError> {
            let mut store = self.create_store()?;
            let bindings = self.instantiate(&mut store)?;

            let wit_request = Self::convert_request(request);

            // Call the WASM plugin's generate function.
            // If the plugin traps (OOM, fuel exhaustion, unreachable, etc.),
            // wasmtime returns an Err -- it does NOT crash the host process.
            let wit_result = bindings
                .call_generate(&mut store, &wit_request)
                .map_err(|e| PluginSystemError::ExecutionFailed {
                    plugin: self.cached_name.clone(),
                    message: format!("WASM plugin trapped: {}", e),
                })?;

            // The WIT interface returns Result<infill-result, string>,
            // so we need to handle the inner Result as well.
            match wit_result {
                Ok(result) => Ok(Self::convert_result(result)),
                Err(error_msg) => Err(PluginSystemError::ExecutionFailed {
                    plugin: self.cached_name.clone(),
                    message: error_msg,
                }),
            }
        }

        fn plugin_type(&self) -> PluginKind {
            PluginKind::Wasm
        }
    }
}

#[cfg(feature = "wasm-plugins")]
pub use wasm_impl::*;
