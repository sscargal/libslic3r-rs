//! Profile management verification test

use slicecore_engine::PrintConfig;
use std::fs;

fn main() {
    println!("=== Profile Management Verification ===\n");

    // Test 1: Serialize to TOML
    println!("1. Testing TOML serialization...");
    let default_config = PrintConfig::default();
    match toml::to_string_pretty(&default_config) {
        Ok(toml_string) => {
            println!("   ✓ Config can be serialized to TOML");
            println!("   Sample (first 300 chars):");
            println!("{}", &toml_string[..300.min(toml_string.len())]);

            // Save to temp file
            fs::write("/tmp/test_profile.toml", &toml_string).ok();
        }
        Err(e) => {
            println!("   ✗ Serialization failed: {}", e);
            return;
        }
    }

    // Test 2: Load from TOML
    println!("\n2. Testing TOML deserialization...");
    let partial_toml = r#"
layer_height = 0.1
wall_count = 4
infill_density = 0.25
nozzle_diameter = 0.6
"#;
    match PrintConfig::from_toml(partial_toml) {
        Ok(config) => {
            println!("   ✓ Config loaded from TOML");
            println!("   - Layer height: {} mm", config.layer_height);
            println!("   - Wall count: {}", config.wall_count);
            println!("   - Infill density: {}", config.infill_density);
            println!("   - Nozzle: {} mm", config.nozzle_diameter);
        }
        Err(e) => {
            println!("   ✗ Load failed: {}", e);
            return;
        }
    }

    // Test 3: Complex nested structures
    println!("\n3. Testing complex nested structures...");
    let complex_toml = r#"
layer_height = 0.15

[support]
enabled = true
type = "tree"

[multi_material]
enabled = true
tool_count = 2
"#;
    match PrintConfig::from_toml(complex_toml) {
        Ok(config) => {
            println!("   ✓ Complex config loaded");
            println!("   - Support enabled: {}", config.support.enabled);
            println!("   - Multi-material tool count: {}", config.multi_material.tool_count);
        }
        Err(e) => {
            println!("   ✗ Complex load failed: {}", e);
            return;
        }
    }

    println!("\n=== Profile Management: VERIFIED ===");
}
