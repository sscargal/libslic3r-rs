//! Golden file tests for G-code output regression detection.
//!
//! These tests verify G-code structural correctness across multiple model+config
//! combinations. Rather than byte-exact golden file comparison, they use
//! structural comparison that checks:
//!
//! - Layer count matches expected value
//! - G-code starts with correct preamble (G28, M104/M109, etc.)
//! - G-code ends with correct postamble (M104 S0, M140 S0, etc.)
//! - Feature type comments present (;TYPE:)
//! - Total extrusion (final E value) within tolerance of expected
//! - Determinism: two slices produce identical output
//!
//! This approach is more maintainable than byte-exact golden files while still
//! catching regressions in output structure, layer processing, and extrusion
//! calculations.

use std::f64::consts::PI;

use slicecore_engine::{Engine, PrintConfig};
use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

// ---------------------------------------------------------------------------
// Mesh builders
// ---------------------------------------------------------------------------

/// Creates a 20mm calibration cube centered at (100, 100) on a 220x220 bed.
fn build_golden_cube() -> TriangleMesh {
    let ox = 90.0; // center at X=100 (90..110)
    let oy = 90.0; // center at Y=100 (90..110)
    let vertices = vec![
        Point3::new(ox, oy, 0.0),
        Point3::new(ox + 20.0, oy, 0.0),
        Point3::new(ox + 20.0, oy + 20.0, 0.0),
        Point3::new(ox, oy + 20.0, 0.0),
        Point3::new(ox, oy, 20.0),
        Point3::new(ox + 20.0, oy, 20.0),
        Point3::new(ox + 20.0, oy + 20.0, 20.0),
        Point3::new(ox, oy + 20.0, 20.0),
    ];
    let indices = vec![
        // Top face (z=20)
        [4, 5, 6],
        [4, 6, 7],
        // Bottom face (z=0)
        [1, 0, 3],
        [1, 3, 2],
        // Right face (x=ox+20)
        [1, 2, 6],
        [1, 6, 5],
        // Left face (x=ox)
        [0, 4, 7],
        [0, 7, 3],
        // Back face (y=oy+20)
        [3, 7, 6],
        [3, 6, 2],
        // Front face (y=oy)
        [0, 1, 5],
        [0, 5, 4],
    ];
    TriangleMesh::new(vertices, indices).expect("calibration cube should be valid")
}

/// Creates a cylinder mesh with 32 sides, 10mm diameter, 20mm tall,
/// centered at (100, 100).
fn build_golden_cylinder() -> TriangleMesh {
    let cx = 100.0;
    let cy = 100.0;
    let radius = 5.0; // 10mm diameter
    let height = 20.0;
    let sides: u32 = 32;

    let mut vertices = Vec::with_capacity((2 * sides + 2) as usize);
    let mut indices = Vec::new();

    // Bottom center and top center vertices.
    let bot_center = vertices.len() as u32;
    vertices.push(Point3::new(cx, cy, 0.0));
    let top_center = vertices.len() as u32;
    vertices.push(Point3::new(cx, cy, height));

    // Ring vertices: bottom ring then top ring.
    let bot_start = vertices.len() as u32;
    for i in 0..sides {
        let angle = 2.0 * PI * (i as f64) / (sides as f64);
        let x = cx + radius * angle.cos();
        let y = cy + radius * angle.sin();
        vertices.push(Point3::new(x, y, 0.0));
    }
    let top_start = vertices.len() as u32;
    for i in 0..sides {
        let angle = 2.0 * PI * (i as f64) / (sides as f64);
        let x = cx + radius * angle.cos();
        let y = cy + radius * angle.sin();
        vertices.push(Point3::new(x, y, height));
    }

    // Bottom cap (fan from center, CW when viewed from below -> normals point down).
    for i in 0..sides {
        let next = (i + 1) % sides;
        indices.push([bot_center, bot_start + next, bot_start + i]);
    }

    // Top cap (fan from center, CCW when viewed from above -> normals point up).
    for i in 0..sides {
        let next = (i + 1) % sides;
        indices.push([top_center, top_start + i, top_start + next]);
    }

    // Lateral faces: two triangles per quad.
    for i in 0..sides {
        let next = (i + 1) % sides;
        let b0 = bot_start + i;
        let b1 = bot_start + next;
        let t0 = top_start + i;
        let t1 = top_start + next;
        indices.push([b0, b1, t1]);
        indices.push([b0, t1, t0]);
    }

    TriangleMesh::new(vertices, indices).expect("cylinder mesh should be valid")
}

// ---------------------------------------------------------------------------
// G-code analysis helpers
// ---------------------------------------------------------------------------

/// Extracts all G/M command lines from G-code (ignoring comments and blanks).
fn extract_command_lines(gcode: &str) -> Vec<&str> {
    gcode
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty()
                && !trimmed.starts_with(';')
                && (trimmed.starts_with('G')
                    || trimmed.starts_with('M')
                    || trimmed.starts_with('T'))
        })
        .collect()
}

/// Counts lines starting with a specific G-code prefix (e.g., "G1", "M106").
fn count_command(gcode: &str, prefix: &str) -> usize {
    gcode
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with(prefix)
                && trimmed[prefix.len()..]
                    .chars()
                    .next()
                    .map_or(true, |c| c == ' ' || c == '\n' || c == '\r')
        })
        .count()
}

/// Extracts the total extrusion (cumulative E value from all G1 E parameters).
/// Assumes relative extrusion mode (M83) where each E is an increment.
fn total_extrusion(gcode: &str) -> f64 {
    let mut total = 0.0;
    for line in gcode.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("G1") {
            continue;
        }
        for param in trimmed.split_whitespace() {
            if param.starts_with('E') {
                if let Ok(val) = param[1..].parse::<f64>() {
                    if val > 0.0 {
                        total += val;
                    }
                }
            }
        }
    }
    total
}

/// Checks whether preamble commands appear in the first N lines.
fn has_preamble(gcode: &str, n: usize) -> bool {
    let first_n: Vec<&str> = gcode.lines().take(n).collect();
    let first_block = first_n.join("\n");

    // Essential preamble commands for FDM printing.
    let required = ["G28", "M83"];
    required.iter().all(|cmd| first_block.contains(cmd))
}

/// Checks whether postamble commands appear in the last N lines.
fn has_postamble(gcode: &str, n: usize) -> bool {
    let lines: Vec<&str> = gcode.lines().collect();
    let start = lines.len().saturating_sub(n);
    let last_block = lines[start..].join("\n");

    // Essential postamble commands.
    let required = ["M107", "M84"];
    required.iter().all(|cmd| last_block.contains(cmd))
}

/// Checks whether feature type comments are present.
/// Comments are formatted as `; TYPE:...` in G-code output.
fn has_feature_type_comments(gcode: &str) -> bool {
    gcode.lines().any(|l| l.contains("TYPE:"))
}

// ---------------------------------------------------------------------------
// Golden test: Calibration cube with default config
// ---------------------------------------------------------------------------

#[test]
fn golden_calibration_cube_default() {
    let config = PrintConfig::default();
    let engine = Engine::new(config);
    let mesh = build_golden_cube();

    let result = engine.slice(&mesh).expect("slice should succeed");
    let gcode = String::from_utf8_lossy(&result.gcode);

    // --- Layer count ---
    // 20mm cube, 0.3mm first layer + 0.2mm rest: ~100 layers
    assert!(
        result.layer_count >= 95 && result.layer_count <= 105,
        "Expected ~100 layers for default cube, got {}",
        result.layer_count
    );

    // --- Preamble ---
    assert!(
        has_preamble(&gcode, 30),
        "G-code should start with preamble (G28, M83) in first 30 lines"
    );

    // --- Postamble ---
    assert!(
        has_postamble(&gcode, 15),
        "G-code should end with postamble (M107, M84) in last 15 lines"
    );

    // --- Feature type comments ---
    assert!(
        has_feature_type_comments(&gcode),
        "G-code should contain ;TYPE: feature comments"
    );

    // --- Command presence ---
    let g1_count = count_command(&gcode, "G1");
    assert!(
        g1_count > 1000,
        "Expected >1000 G1 moves for a 20mm cube, got {}",
        g1_count
    );

    // --- Temperature commands ---
    assert!(
        gcode.contains("M109"),
        "G-code should contain M109 (wait for nozzle temp)"
    );
    assert!(
        gcode.contains("M190"),
        "G-code should contain M190 (wait for bed temp)"
    );

    // --- Total extrusion ---
    let total_e = total_extrusion(&gcode);
    assert!(
        total_e > 100.0,
        "Total extrusion should be substantial for a 20mm cube (got {:.2}mm)",
        total_e
    );

    // --- G-code line count ---
    let total_lines = gcode.lines().count();
    assert!(
        total_lines > 5000,
        "Expected >5000 G-code lines for a 20mm cube, got {}",
        total_lines
    );
}

// ---------------------------------------------------------------------------
// Golden test: Calibration cube with fine layers (0.1mm)
// ---------------------------------------------------------------------------

#[test]
fn golden_calibration_cube_fine() {
    let config = PrintConfig {
        layer_height: 0.1,
        first_layer_height: 0.1,
        ..PrintConfig::default()
    };
    let engine = Engine::new(config);
    let mesh = build_golden_cube();

    let result = engine.slice(&mesh).expect("slice should succeed");
    let gcode = String::from_utf8_lossy(&result.gcode);

    // --- Layer count ---
    // 20mm cube, 0.1mm layers: ~200 layers
    assert!(
        result.layer_count >= 190 && result.layer_count <= 210,
        "Expected ~200 layers for fine cube, got {}",
        result.layer_count
    );

    // --- Preamble and postamble ---
    assert!(has_preamble(&gcode, 30), "Fine cube should have preamble");
    assert!(has_postamble(&gcode, 15), "Fine cube should have postamble");

    // --- Feature type comments ---
    assert!(
        has_feature_type_comments(&gcode),
        "Fine cube should have feature type comments"
    );

    // --- More G1 moves than default (roughly double layers) ---
    let g1_count = count_command(&gcode, "G1");
    assert!(
        g1_count > 2000,
        "Expected >2000 G1 moves for fine cube, got {}",
        g1_count
    );

    // --- Total extrusion should be similar to default (same volume) ---
    let total_e = total_extrusion(&gcode);
    assert!(
        total_e > 80.0,
        "Fine cube total extrusion should be >80mm (got {:.2}mm)",
        total_e
    );

    // --- G-code line count should be higher than default ---
    let total_lines = gcode.lines().count();
    assert!(
        total_lines > 10000,
        "Expected >10000 G-code lines for fine cube, got {}",
        total_lines
    );
}

// ---------------------------------------------------------------------------
// Golden test: Cylinder with default config
// ---------------------------------------------------------------------------

#[test]
fn golden_cylinder_default() {
    let config = PrintConfig::default();
    let engine = Engine::new(config);
    let mesh = build_golden_cylinder();

    let result = engine.slice(&mesh).expect("slice should succeed");
    let gcode = String::from_utf8_lossy(&result.gcode);

    // --- Layer count ---
    // 20mm cylinder, 0.3mm first layer + 0.2mm rest: ~100 layers
    assert!(
        result.layer_count >= 95 && result.layer_count <= 105,
        "Expected ~100 layers for default cylinder, got {}",
        result.layer_count
    );

    // --- Preamble and postamble ---
    assert!(
        has_preamble(&gcode, 30),
        "Cylinder should have preamble"
    );
    assert!(
        has_postamble(&gcode, 15),
        "Cylinder should have postamble"
    );

    // --- Feature type comments ---
    assert!(
        has_feature_type_comments(&gcode),
        "Cylinder should have feature type comments"
    );

    // --- G1 move count ---
    let g1_count = count_command(&gcode, "G1");
    assert!(
        g1_count > 500,
        "Expected >500 G1 moves for cylinder, got {}",
        g1_count
    );

    // --- Total extrusion ---
    // Cylinder volume is less than cube (pi*5^2*20 ~ 1571 mm^3 vs 20^3 = 8000 mm^3)
    let total_e = total_extrusion(&gcode);
    assert!(
        total_e > 20.0,
        "Cylinder total extrusion should be >20mm (got {:.2}mm)",
        total_e
    );
}

// ---------------------------------------------------------------------------
// Golden test: Determinism verification
// ---------------------------------------------------------------------------

#[test]
fn golden_determinism_cube() {
    let config = PrintConfig::default();
    let mesh = build_golden_cube();

    let engine1 = Engine::new(config.clone());
    let result1 = engine1.slice(&mesh).expect("first slice should succeed");

    let engine2 = Engine::new(config);
    let result2 = engine2.slice(&mesh).expect("second slice should succeed");

    // Bit-for-bit identical output.
    assert_eq!(
        result1.gcode, result2.gcode,
        "Determinism: identical input must produce bit-for-bit identical G-code. \
         Size: {} vs {} bytes",
        result1.gcode.len(),
        result2.gcode.len()
    );

    assert_eq!(
        result1.layer_count, result2.layer_count,
        "Determinism: layer counts must match ({} vs {})",
        result1.layer_count,
        result2.layer_count
    );
}

#[test]
fn golden_determinism_cylinder() {
    let config = PrintConfig::default();
    let mesh = build_golden_cylinder();

    let engine1 = Engine::new(config.clone());
    let result1 = engine1.slice(&mesh).expect("first slice should succeed");

    let engine2 = Engine::new(config);
    let result2 = engine2.slice(&mesh).expect("second slice should succeed");

    assert_eq!(
        result1.gcode, result2.gcode,
        "Determinism: cylinder slices must produce identical output. \
         Size: {} vs {} bytes",
        result1.gcode.len(),
        result2.gcode.len()
    );
}

// ---------------------------------------------------------------------------
// Golden test: Structural consistency across configs
// ---------------------------------------------------------------------------

#[test]
fn golden_cube_extrusion_consistency() {
    // Verify that total extrusion is consistent across two slices
    // and within expected range for the geometry.
    let config = PrintConfig::default();
    let mesh = build_golden_cube();

    let engine = Engine::new(config);
    let result = engine.slice(&mesh).expect("slice should succeed");
    let gcode = String::from_utf8_lossy(&result.gcode);

    let total_e = total_extrusion(&gcode);

    // Sanity check: A 20mm cube with 20% infill and 2 walls should use
    // a reasonable amount of filament. The exact amount depends on many
    // parameters but should be in a broad reasonable range.
    assert!(
        total_e > 50.0 && total_e < 5000.0,
        "Total extrusion {:.2}mm is outside reasonable range [50, 5000] for a 20mm cube",
        total_e
    );

    // Verify extrusion is proportional between default and fine layers.
    let fine_config = PrintConfig {
        layer_height: 0.1,
        first_layer_height: 0.1,
        ..PrintConfig::default()
    };
    let fine_engine = Engine::new(fine_config);
    let fine_result = fine_engine.slice(&mesh).expect("fine slice should succeed");
    let fine_gcode = String::from_utf8_lossy(&fine_result.gcode);
    let fine_total_e = total_extrusion(&fine_gcode);

    // Total extrusion for the same model should be in the same ballpark
    // regardless of layer height (same volume of plastic, just different
    // per-layer distribution). Allow wide tolerance (factor of 3).
    let ratio = total_e / fine_total_e;
    assert!(
        ratio > 0.3 && ratio < 3.0,
        "Extrusion ratio default/fine = {:.2} (default={:.2}, fine={:.2}) -- \
         should be within factor of 3",
        ratio,
        total_e,
        fine_total_e
    );
}

#[test]
fn golden_cube_gcode_command_variety() {
    // Verify that a variety of G-code commands are present,
    // indicating all pipeline stages ran.
    let config = PrintConfig::default();
    let engine = Engine::new(config);
    let mesh = build_golden_cube();

    let result = engine.slice(&mesh).expect("slice should succeed");
    let gcode = String::from_utf8_lossy(&result.gcode);

    let commands = extract_command_lines(&gcode);
    assert!(
        commands.len() > 1000,
        "Expected >1000 command lines, got {}",
        commands.len()
    );

    // Verify essential command types present.
    let essential = [
        ("G0", "rapid travel"),
        ("G1", "linear move"),
        ("G28", "homing"),
        ("M83", "relative extrusion"),
        ("M104", "set nozzle temp"),
        ("M109", "wait nozzle temp"),
        ("M140", "set bed temp"),
        ("M190", "wait bed temp"),
    ];

    for (cmd, desc) in essential {
        assert!(
            commands.iter().any(|l| l.starts_with(cmd)),
            "Missing essential command {} ({}) in G-code output",
            cmd,
            desc
        );
    }
}
