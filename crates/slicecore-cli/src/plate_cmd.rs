//! Plate management CLI subcommands.
//!
//! Provides the `slicecore plate` command group with subcommands for
//! initializing plate configuration templates, extracting objects from 3MF
//! files, and packaging plate configs into 3MF files.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use clap::Subcommand;

/// Plate management subcommands.
#[derive(Subcommand)]
pub enum PlateCommands {
    /// Initialize a plate config template.
    ///
    /// Generates a commented plate.toml template with optional model files
    /// and profile references pre-populated.
    Init {
        /// Model files to include as objects.
        models: Vec<PathBuf>,
        /// Machine profile name.
        #[arg(short)]
        machine: Option<String>,
        /// Filament profile name.
        #[arg(short)]
        filament: Option<String>,
        /// Process profile name.
        #[arg(short = 'p')]
        process: Option<String>,
        /// Output file path.
        #[arg(short, long, default_value = "plate.toml")]
        output: PathBuf,
        /// Output result as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Extract objects and settings from a 3MF file.
    ///
    /// Parses the 3MF, exports mesh data as STL, and generates a plate.toml
    /// referencing the extracted files.
    From3mf {
        /// Input 3MF file.
        input: PathBuf,
        /// Output directory for extracted files.
        #[arg(short, long, default_value = ".")]
        output: PathBuf,
        /// Output result as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Package a plate config into a 3MF file.
    ///
    /// Reads a plate.toml, loads referenced mesh files, and writes a single
    /// 3MF archive containing all objects.
    To3mf {
        /// Input plate.toml file.
        input: PathBuf,
        /// Output 3MF file.
        #[arg(short, long)]
        output: PathBuf,
        /// Output result as JSON.
        #[arg(long)]
        json: bool,
    },
}

/// Generate a commented plate.toml template string.
fn generate_plate_template(
    models: &[PathBuf],
    machine: Option<&str>,
    filament: Option<&str>,
    process: Option<&str>,
) -> String {
    let mut out = String::new();

    out.push_str("# SliceCore Plate Configuration\n");
    out.push_str("# See: slicecore plate --help\n\n");

    out.push_str("[profiles]\n");
    if let Some(m) = machine {
        out.push_str(&format!("machine = \"{m}\"\n"));
    } else {
        out.push_str("# machine = \"printer-name\"\n");
    }
    if let Some(f) = filament {
        out.push_str(&format!("filament = \"{f}\"\n"));
    } else {
        out.push_str("# filament = \"filament-name\"\n");
    }
    if let Some(p) = process {
        out.push_str(&format!("process = \"{p}\"\n"));
    } else {
        out.push_str("# process = \"quality-preset\"\n");
    }

    out.push_str("\n# [default_overrides]\n");
    out.push_str("# Settings applied to ALL objects (cascade layer 7)\n");
    out.push_str("# infill_density = 0.2\n");

    out.push_str("\n# [override_sets.example]\n");
    out.push_str("# Named override set (reusable across objects)\n");
    out.push_str("# layer_height = 0.1\n");
    out.push_str("# wall_count = 4\n");

    if models.is_empty() {
        out.push_str("\n[[objects]]\n");
        out.push_str("model = \"model.stl\"\n");
        out.push_str("# name = \"Part Name\"\n");
        out.push_str("# override_set = \"example\"\n");
        out.push_str("copies = 1\n");
    } else {
        for model in models {
            let model_str = model.display();
            let name_hint = model.file_stem().and_then(|s| s.to_str()).unwrap_or("Part");
            out.push_str(&format!("\n[[objects]]\n"));
            out.push_str(&format!("model = \"{model_str}\"\n"));
            out.push_str(&format!("name = \"{name_hint}\"\n"));
            out.push_str("# override_set = \"example\"\n");
            out.push_str("copies = 1\n");
        }
    }

    out.push_str("\n# [objects.transform]\n");
    out.push_str("# position = [0.0, 0.0, 0.0]\n");
    out.push_str("# rotation = [0.0, 0.0, 0.0]\n");
    out.push_str("# scale = [1.0, 1.0, 1.0]\n");

    out.push_str("\n# [objects.overrides]\n");
    out.push_str("# Inline overrides (applied after override_set)\n");
    out.push_str("# infill_density = 0.8\n");

    out.push_str("\n# [[objects.modifiers]]\n");
    out.push_str("# shape = \"box\"\n");
    out.push_str("# position = [0.0, 0.0, 0.0]\n");
    out.push_str("# size = [10.0, 10.0, 10.0]\n");
    out.push_str("# [objects.modifiers.overrides]\n");
    out.push_str("# infill_density = 1.0\n");

    out.push_str("\n# [[objects.layer_overrides]]\n");
    out.push_str("# z_range = [0.0, 2.0]\n");
    out.push_str("# [objects.layer_overrides.overrides]\n");
    out.push_str("# speeds.perimeter = 20.0\n");

    out
}

/// Execute a plate subcommand.
///
/// # Errors
///
/// Returns an error if file operations or parsing fails.
pub fn run_plate(cmd: PlateCommands) -> Result<(), anyhow::Error> {
    match cmd {
        PlateCommands::Init {
            models,
            machine,
            filament,
            process,
            output,
            json,
        } => {
            let template = generate_plate_template(
                &models,
                machine.as_deref(),
                filament.as_deref(),
                process.as_deref(),
            );
            fs::write(&output, &template)?;

            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "created": output.display().to_string(),
                        "objects": models.len().max(1),
                        "profiles": {
                            "machine": machine,
                            "filament": filament,
                            "process": process,
                        }
                    }))?
                );
            } else {
                let obj_count = if models.is_empty() { 1 } else { models.len() };
                println!(
                    "Created plate config: {} ({obj_count} object(s))",
                    output.display()
                );
            }
        }

        PlateCommands::From3mf {
            input,
            output,
            json,
        } => {
            if !input.exists() {
                anyhow::bail!("Input file not found: {}", input.display());
            }

            fs::create_dir_all(&output)?;

            let data = fs::read(&input)?;
            let import_result = slicecore_fileio::threemf::parse_with_config(&data)?;

            let stl_name = input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("object");

            // Export each object mesh as a separate STL file
            let mut stl_paths = Vec::new();
            for (idx, mesh) in import_result.meshes.iter().enumerate() {
                let obj_config = import_result.object_configs.get(idx);
                let obj_name = obj_config
                    .and_then(|c| c.name.as_deref())
                    .unwrap_or(stl_name);
                let suffix = if import_result.meshes.len() > 1 {
                    format!("{obj_name}_{}", idx + 1)
                } else {
                    obj_name.to_string()
                };
                let stl_path = output.join(format!("{suffix}.stl"));
                slicecore_fileio::save_mesh(mesh, &stl_path)?;
                stl_paths.push((stl_path, obj_config));
            }

            // Export modifier meshes as separate STL files for each object
            let mut modifier_stl_paths: Vec<Vec<PathBuf>> = Vec::new();
            for (idx, obj_config) in import_result.object_configs.iter().enumerate() {
                let mut mod_paths = Vec::new();
                for (mod_idx, modifier) in obj_config.modifiers.iter().enumerate() {
                    let mod_stl =
                        output.join(format!("{stl_name}_obj{}_mod{}.stl", idx + 1, mod_idx + 1));
                    slicecore_fileio::save_mesh(&modifier.mesh, &mod_stl)?;
                    mod_paths.push(mod_stl);
                }
                modifier_stl_paths.push(mod_paths);
            }

            // Build plate.toml with per-object overrides and modifiers
            let mut plate_toml = String::new();
            plate_toml.push_str("# SliceCore Plate Configuration\n");
            plate_toml.push_str("# Extracted from 3MF file\n\n");
            plate_toml.push_str("[profiles]\n");
            plate_toml.push_str("# machine = \"printer-name\"\n");
            plate_toml.push_str("# filament = \"filament-name\"\n");
            plate_toml.push_str("# process = \"quality-preset\"\n\n");
            plate_toml.push_str("# [default_overrides]\n");
            plate_toml.push_str("# Settings applied to ALL objects (cascade layer 7)\n\n");

            // Build objects array as toml::Value for pretty serialization
            let mut objects_array = Vec::new();
            for (idx, (stl_path, obj_config)) in stl_paths.iter().enumerate() {
                let mut obj_table = toml::map::Map::new();

                // Model path (relative filename only)
                let stl_filename = stl_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("object.stl");
                obj_table.insert(
                    "model".to_string(),
                    toml::Value::String(stl_filename.to_string()),
                );

                if let Some(config) = obj_config {
                    if let Some(name) = &config.name {
                        obj_table.insert("name".to_string(), toml::Value::String(name.clone()));
                    }
                }

                obj_table.insert("copies".to_string(), toml::Value::Integer(1));

                // Per-object overrides
                if let Some(config) = obj_config {
                    if !config.overrides.is_empty() {
                        obj_table.insert(
                            "overrides".to_string(),
                            toml::Value::Table(config.overrides.clone()),
                        );
                    }
                }

                // Modifiers
                let mod_paths = modifier_stl_paths.get(idx).cloned().unwrap_or_default();
                if let Some(config) = obj_config {
                    if !config.modifiers.is_empty() {
                        let mut mods_array = Vec::new();
                        for (mod_idx, modifier) in config.modifiers.iter().enumerate() {
                            let mut mod_table = toml::map::Map::new();
                            if let Some(mod_stl_path) = mod_paths.get(mod_idx) {
                                let mod_filename = mod_stl_path
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("modifier.stl");
                                mod_table.insert(
                                    "model".to_string(),
                                    toml::Value::String(mod_filename.to_string()),
                                );
                            }
                            if !modifier.overrides.is_empty() {
                                mod_table.insert(
                                    "overrides".to_string(),
                                    toml::Value::Table(modifier.overrides.clone()),
                                );
                            }
                            mods_array.push(toml::Value::Table(mod_table));
                        }
                        obj_table.insert("modifiers".to_string(), toml::Value::Array(mods_array));
                    }
                }

                objects_array.push(toml::Value::Table(obj_table));
            }

            // Serialize the objects portion
            let mut wrapper = toml::map::Map::new();
            wrapper.insert("objects".to_string(), toml::Value::Array(objects_array));
            let objects_toml =
                toml::to_string_pretty(&toml::Value::Table(wrapper)).unwrap_or_default();
            plate_toml.push_str(&objects_toml);

            let plate_path = output.join("plate.toml");
            fs::write(&plate_path, &plate_toml)?;

            // Print import summary
            let summary = &import_result.summary;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "input": input.display().to_string(),
                        "output_dir": output.display().to_string(),
                        "objects_extracted": import_result.meshes.len(),
                        "stl_files": stl_paths.iter().map(|(p, _)| p.display().to_string()).collect::<Vec<_>>(),
                        "plate_config": plate_path.display().to_string(),
                        "overrides_imported": summary.overrides_imported,
                        "unmapped_fields": summary.unmapped_fields.len(),
                    }))?
                );
            } else {
                eprintln!("Extracted from: {}", input.display());
                eprintln!("  Objects: {}", import_result.meshes.len());
                for (stl_path, _) in &stl_paths {
                    eprintln!("  Mesh: {}", stl_path.display());
                }
                eprintln!("  Plate config: {}", plate_path.display());
                if !summary.unmapped_fields.is_empty() {
                    eprintln!(
                        "  Unmapped fields: {} (preserved as metadata)",
                        summary.unmapped_fields.len()
                    );
                }
            }
        }

        PlateCommands::To3mf {
            input,
            output,
            json,
        } => {
            if !input.exists() {
                anyhow::bail!("Input plate config not found: {}", input.display());
            }

            let plate_content = fs::read_to_string(&input)?;

            // Parse the plate.toml to find model references
            let plate_table: toml::Value = toml::from_str(&plate_content)?;
            let base_dir = input.parent().unwrap_or_else(|| std::path::Path::new("."));

            // Collect all model paths from [[objects]]
            let objects = plate_table
                .get("objects")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            if objects.is_empty() {
                anyhow::bail!("No objects found in plate config.");
            }

            // Load all model meshes and build per-object configs
            let mut all_meshes = Vec::new();
            let mut obj_configs = Vec::new();
            for obj in &objects {
                let model_path_str = obj
                    .get("model")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Object missing 'model' field"))?;
                let model_path = base_dir.join(model_path_str);
                if !model_path.exists() {
                    anyhow::bail!("Model file not found: {}", model_path.display());
                }
                let data = fs::read(&model_path)?;
                let mesh = slicecore_fileio::load_mesh(&data)?;
                all_meshes.push(mesh);

                let name = obj.get("name").and_then(|v| v.as_str()).map(String::from);

                // Extract inline overrides
                let overrides = obj
                    .get("overrides")
                    .and_then(|v| v.as_table())
                    .cloned()
                    .unwrap_or_default();

                // Extract modifier meshes
                let mut modifiers = Vec::new();
                if let Some(mods) = obj.get("modifiers").and_then(|v| v.as_array()) {
                    for modifier_val in mods {
                        let mod_overrides = modifier_val
                            .get("overrides")
                            .and_then(|v| v.as_table())
                            .cloned()
                            .unwrap_or_default();

                        // For STL-referenced modifiers, load the mesh
                        if let Some(model_path_str) =
                            modifier_val.get("model").and_then(|v| v.as_str())
                        {
                            let mod_path = base_dir.join(model_path_str);
                            let mod_data = fs::read(&mod_path)?;
                            let mod_mesh = slicecore_fileio::load_mesh(&mod_data)?;
                            modifiers.push(slicecore_fileio::ThreeMfModifier {
                                mesh: mod_mesh,
                                overrides: mod_overrides,
                                unmapped: HashMap::new(),
                            });
                        }
                        // For geometric primitives (shape = "box" etc.), skip mesh --
                        // 3MF export only supports mesh-based modifiers
                    }
                }

                obj_configs.push(slicecore_fileio::ThreeMfObjectConfig {
                    name,
                    overrides,
                    unmapped: HashMap::new(),
                    transform: None,
                    modifiers,
                });
            }

            // Export all objects with per-object configs into 3MF
            let mesh_refs: Vec<&slicecore_mesh::TriangleMesh> = all_meshes.iter().collect();
            let output_file = fs::File::create(&output)?;
            let writer = std::io::BufWriter::new(output_file);
            slicecore_fileio::export_plate_to_3mf(&mesh_refs, &obj_configs, writer)?;

            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "input": input.display().to_string(),
                        "output": output.display().to_string(),
                        "objects_packaged": all_meshes.len(),
                    }))?
                );
            } else {
                println!(
                    "Packaged {} object(s) into: {}",
                    all_meshes.len(),
                    output.display()
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plate_init_generates_valid_toml() {
        let template = generate_plate_template(
            &[PathBuf::from("model.stl")],
            Some("my-printer"),
            Some("pla-basic"),
            Some("0.2mm-quality"),
        );

        // Should contain model path
        assert!(template.contains("model.stl"));
        // Should contain profile settings
        assert!(template.contains("my-printer"));
        assert!(template.contains("pla-basic"));
        assert!(template.contains("0.2mm-quality"));
        // Should contain comments
        assert!(template.contains("# SliceCore Plate Configuration"));
        assert!(template.contains("# [default_overrides]"));

        // The non-comment portion should be valid TOML
        let stripped: String = template
            .lines()
            .filter(|line| !line.trim_start().starts_with('#') || line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        // Basic check -- the uncommented portions should be parseable
        assert!(toml::from_str::<toml::Value>(&stripped).is_ok());
    }

    #[test]
    fn plate_init_empty_models_has_placeholder() {
        let template = generate_plate_template(&[], None, None, None);
        assert!(template.contains("model = \"model.stl\""));
        assert!(template.contains("copies = 1"));
    }

    #[test]
    fn plate_init_multiple_models() {
        let models = vec![PathBuf::from("part_a.stl"), PathBuf::from("part_b.stl")];
        let template = generate_plate_template(&models, None, None, None);
        assert!(template.contains("part_a.stl"));
        assert!(template.contains("part_b.stl"));
        assert!(template.contains("name = \"part_a\""));
        assert!(template.contains("name = \"part_b\""));

        // Count [[objects]] entries
        let obj_count = template.matches("[[objects]]").count();
        assert_eq!(obj_count, 2);
    }

    #[test]
    fn plate_init_write_and_read_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let output = tmp.path().join("plate.toml");

        let template =
            generate_plate_template(&[PathBuf::from("cube.stl")], Some("bambu-x1c"), None, None);
        fs::write(&output, &template).unwrap();

        let read_back = fs::read_to_string(&output).unwrap();
        assert!(read_back.contains("cube.stl"));
        assert!(read_back.contains("bambu-x1c"));
    }

    #[test]
    fn plate_from3mf_to3mf_roundtrip() {
        // Create a minimal 3MF file using the threemf test infrastructure
        use slicecore_math::Point3;
        use slicecore_mesh::TriangleMesh;
        use std::collections::HashMap;

        let tmp = tempfile::TempDir::new().unwrap();

        // Create a simple triangle mesh
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
            Point3::new(0.5, 0.5, 1.0),
        ];
        let indices = vec![[0, 1, 2], [0, 1, 3], [1, 2, 3], [0, 2, 3]];
        let mesh = TriangleMesh::new(vertices, indices).unwrap();

        // Save as 3MF
        let input_3mf = tmp.path().join("test.3mf");
        slicecore_fileio::save_mesh(&mesh, &input_3mf).unwrap();

        // Extract with from-3mf using parse_with_config
        let extract_dir = tmp.path().join("extracted");
        fs::create_dir_all(&extract_dir).unwrap();
        let data = fs::read(&input_3mf).unwrap();
        let import_result = slicecore_fileio::threemf::parse_with_config(&data).unwrap();
        assert!(!import_result.meshes.is_empty());

        let stl_path = extract_dir.join("test.stl");
        slicecore_fileio::save_mesh(&import_result.meshes[0], &stl_path).unwrap();
        assert!(stl_path.exists());

        // Generate plate.toml
        let template = generate_plate_template(&[PathBuf::from("test.stl")], None, None, None);
        let plate_path = extract_dir.join("plate.toml");
        fs::write(&plate_path, &template).unwrap();
        assert!(plate_path.exists());

        // Re-package with to-3mf using export_plate_to_3mf
        let output_3mf = tmp.path().join("output.3mf");
        let stl_data = fs::read(&stl_path).unwrap();
        let reloaded = slicecore_fileio::load_mesh(&stl_data).unwrap();
        let mesh_refs: Vec<&TriangleMesh> = vec![&reloaded];
        let obj_config = slicecore_fileio::ThreeMfObjectConfig {
            name: Some("test".to_string()),
            overrides: toml::map::Map::new(),
            unmapped: HashMap::new(),
            transform: None,
            modifiers: Vec::new(),
        };
        let output_file = fs::File::create(&output_3mf).unwrap();
        let writer = std::io::BufWriter::new(output_file);
        slicecore_fileio::export_plate_to_3mf(&mesh_refs, &[obj_config], writer).unwrap();
        assert!(output_3mf.exists());

        // Verify the round-tripped 3MF can be re-parsed
        let final_data = fs::read(&output_3mf).unwrap();
        let final_result = slicecore_fileio::threemf::parse_with_config(&final_data).unwrap();
        assert_eq!(final_result.meshes[0].vertex_count(), 4);
        assert_eq!(final_result.meshes[0].triangle_count(), 4);
    }
}
