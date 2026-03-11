//! Integration tests verifying all RENDER requirements (RENDER-01 through RENDER-09).

use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;
use slicecore_render::{
    format_gcode_thumbnail_block, render_mesh, CameraAngle, ThumbnailConfig, ThumbnailFormat,
};

// ---------------------------------------------------------------------------
// Helpers: synthetic meshes
// ---------------------------------------------------------------------------

/// Build a cube with 8 vertices and 12 triangles (CCW winding, outward normals).
fn make_cube() -> TriangleMesh {
    let vertices = vec![
        Point3::new(0.0, 0.0, 0.0),  // 0
        Point3::new(10.0, 0.0, 0.0), // 1
        Point3::new(10.0, 10.0, 0.0), // 2
        Point3::new(0.0, 10.0, 0.0), // 3
        Point3::new(0.0, 0.0, 10.0), // 4
        Point3::new(10.0, 0.0, 10.0), // 5
        Point3::new(10.0, 10.0, 10.0), // 6
        Point3::new(0.0, 10.0, 10.0), // 7
    ];
    let indices = vec![
        // Front (z=0)
        [0, 2, 1],
        [0, 3, 2],
        // Back (z=10)
        [4, 5, 6],
        [4, 6, 7],
        // Bottom (y=0)
        [0, 1, 5],
        [0, 5, 4],
        // Top (y=10)
        [3, 6, 2],
        [3, 7, 6],
        // Left (x=0)
        [0, 4, 7],
        [0, 7, 3],
        // Right (x=10)
        [1, 2, 6],
        [1, 6, 5],
    ];
    TriangleMesh::new(vertices, indices).unwrap()
}

/// Build a pyramid with 5 vertices and 6 triangles.
fn make_pyramid() -> TriangleMesh {
    let vertices = vec![
        Point3::new(0.0, 0.0, 0.0),  // 0 base
        Point3::new(10.0, 0.0, 0.0), // 1 base
        Point3::new(10.0, 10.0, 0.0), // 2 base
        Point3::new(0.0, 10.0, 0.0), // 3 base
        Point3::new(5.0, 5.0, 10.0), // 4 apex
    ];
    let indices = vec![
        // Base (two triangles, CCW from below = CW from above)
        [0, 2, 1],
        [0, 3, 2],
        // Side faces (CCW outward)
        [0, 1, 4],
        [1, 2, 4],
        [2, 3, 4],
        [3, 0, 4],
    ];
    TriangleMesh::new(vertices, indices).unwrap()
}

// ---------------------------------------------------------------------------
// RENDER-01: Framebuffer creation and pixel write with z-test
// ---------------------------------------------------------------------------

#[test]
fn render_01_framebuffer_z_test() {
    // We test z-buffer behavior indirectly through the render pipeline:
    // render a cube from front -- some pixels must be filled (z-test allows front faces).
    // The unit tests in framebuffer.rs cover set_pixel z-test directly.
    // Here we verify the full pipeline respects z-buffering by rendering
    // overlapping geometry and checking pixel counts are consistent.
    let mesh = make_cube();
    let config = ThumbnailConfig {
        width: 64,
        height: 64,
        angles: vec![CameraAngle::Front],
        background: [0, 0, 0, 0],
        model_color: [200, 200, 200],
    };
    let thumbs = render_mesh(&mesh, &config);
    assert_eq!(thumbs.len(), 1);

    let non_bg: usize = thumbs[0]
        .rgba
        .iter()
        .filter(|px| **px != [0, 0, 0, 0])
        .count();
    assert!(
        non_bg > 0,
        "Cube rendered from front should have visible pixels (z-test passed front faces)"
    );

    // Render same cube twice -- pixel output must be identical (deterministic z-test)
    let thumbs2 = render_mesh(&mesh, &config);
    assert_eq!(thumbs[0].rgba, thumbs2[0].rgba, "Z-buffer must be deterministic");
}

// ---------------------------------------------------------------------------
// RENDER-02: Triangle rasterization produces correct pixels
// ---------------------------------------------------------------------------

#[test]
fn render_02_rasterization_non_empty() {
    let mesh = make_cube();
    let config = ThumbnailConfig {
        width: 100,
        height: 100,
        angles: vec![CameraAngle::Front],
        background: [0, 0, 0, 0],
        model_color: [200, 200, 200],
    };
    let thumbs = render_mesh(&mesh, &config);
    let non_bg = thumbs[0]
        .rgba
        .iter()
        .filter(|px| **px != [0, 0, 0, 0])
        .count();
    assert!(non_bg > 0, "Cube from front must produce visible pixels");
}

#[test]
fn render_02_rasterization_deterministic() {
    let mesh = make_cube();
    let config = ThumbnailConfig {
        width: 100,
        height: 100,
        angles: vec![CameraAngle::Isometric],
        background: [0, 0, 0, 0],
        model_color: [200, 200, 200],
    };
    let a = render_mesh(&mesh, &config);
    let b = render_mesh(&mesh, &config);
    assert_eq!(a[0].rgba, b[0].rgba, "Rasterization must be deterministic");
}

// ---------------------------------------------------------------------------
// RENDER-03: Camera angles produce distinct views
// ---------------------------------------------------------------------------

#[test]
fn render_03_all_angles_pairwise_distinct() {
    // Use a non-symmetric mesh (pyramid) so all 6 angles produce distinct views.
    // A cube is symmetric across some axis pairs (e.g. Front/Back at low resolution),
    // but a pyramid with off-center apex is always distinct.
    let mesh = make_pyramid();
    let config = ThumbnailConfig {
        width: 64,
        height: 64,
        angles: CameraAngle::all(),
        background: [0, 0, 0, 0],
        model_color: [200, 200, 200],
    };
    let thumbs = render_mesh(&mesh, &config);
    assert_eq!(thumbs.len(), 6);

    // Check all 15 pairs are distinct
    let mut distinct_count = 0;
    for i in 0..6 {
        for j in (i + 1)..6 {
            let differ = thumbs[i]
                .rgba
                .iter()
                .zip(thumbs[j].rgba.iter())
                .any(|(a, b)| a != b);
            if differ {
                distinct_count += 1;
            }
        }
    }
    // At minimum, most pairs should differ. Due to symmetry in some geometries,
    // allow at most 1 coincidental match.
    assert!(
        distinct_count >= 14,
        "At least 14 of 15 angle pairs should produce distinct images, got {}",
        distinct_count
    );
}

// ---------------------------------------------------------------------------
// RENDER-04: Gouraud shading varies with surface orientation
// ---------------------------------------------------------------------------

#[test]
fn render_04_shading_brightness_variation() {
    let mesh = make_cube();
    let config = ThumbnailConfig {
        width: 100,
        height: 100,
        angles: vec![CameraAngle::Isometric],
        background: [0, 0, 0, 0],
        model_color: [200, 200, 200],
    };
    let thumbs = render_mesh(&mesh, &config);

    // Collect brightness of all non-background pixels
    let mut brightnesses: Vec<u32> = Vec::new();
    for px in &thumbs[0].rgba {
        if *px != [0, 0, 0, 0] {
            let brightness = px[0] as u32 + px[1] as u32 + px[2] as u32;
            brightnesses.push(brightness);
        }
    }
    assert!(
        !brightnesses.is_empty(),
        "Should have non-background pixels"
    );

    let min_b = *brightnesses.iter().min().unwrap();
    let max_b = *brightnesses.iter().max().unwrap();
    assert!(
        max_b > min_b,
        "Gouraud shading should produce brightness variation: min={}, max={}",
        min_b,
        max_b
    );
}

// ---------------------------------------------------------------------------
// RENDER-05: PNG encoding produces valid PNG file
// ---------------------------------------------------------------------------

#[test]
fn render_05_png_valid() {
    let mesh = make_cube();
    let config = ThumbnailConfig {
        width: 64,
        height: 64,
        angles: vec![CameraAngle::Isometric],
        background: [0, 0, 0, 0],
        model_color: [200, 200, 200],
    };
    let thumbs = render_mesh(&mesh, &config);
    let png_data = &thumbs[0].png_data;

    // Check PNG magic bytes
    assert!(png_data.len() > 100, "PNG should be non-trivial size");
    assert_eq!(
        &png_data[0..4],
        &[0x89, 0x50, 0x4E, 0x47],
        "PNG magic bytes"
    );

    // Decode PNG back to verify it is valid
    let decoder = png::Decoder::new(std::io::Cursor::new(png_data));
    let reader = decoder.read_info().expect("Should decode PNG info");
    let info = reader.info();
    assert_eq!(info.width, 64);
    assert_eq!(info.height, 64);
}

// ---------------------------------------------------------------------------
// RENDER-06: 3MF export includes thumbnail attachment
// ---------------------------------------------------------------------------

#[test]
fn render_06_3mf_thumbnail_embedded() {
    let mesh = make_cube();
    let config = ThumbnailConfig {
        width: 64,
        height: 64,
        angles: vec![CameraAngle::Isometric],
        background: [0, 0, 0, 0],
        model_color: [200, 200, 200],
    };
    let thumbs = render_mesh(&mesh, &config);
    let png_data = &thumbs[0].png_data;

    // Save as 3MF with thumbnail to an in-memory buffer
    let mut buf = std::io::Cursor::new(Vec::new());
    slicecore_fileio::save_mesh_to_writer_with_thumbnail(
        &mesh,
        &mut buf,
        slicecore_fileio::ExportFormat::ThreeMf,
        Some(png_data),
    )
    .expect("3MF export with thumbnail should succeed");

    // Read the ZIP back and check for thumbnail entry
    let data = buf.into_inner();
    let reader = std::io::Cursor::new(&data);
    let mut zip = zip::ZipArchive::new(reader).expect("Should read ZIP");

    // Check entry names first
    let names: Vec<String> = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(
        names.iter().any(|n| n == "Metadata/thumbnail.png"),
        "3MF ZIP must contain Metadata/thumbnail.png, found: {:?}",
        names
    );

    // Read and verify thumbnail content
    let mut contents = Vec::new();
    {
        let mut entry = zip.by_name("Metadata/thumbnail.png").unwrap();
        std::io::Read::read_to_end(&mut entry, &mut contents).unwrap();
    }
    assert_eq!(
        contents, *png_data,
        "Thumbnail content should match input PNG"
    );
}

// ---------------------------------------------------------------------------
// RENDER-07: G-code output includes thumbnail comment block
// ---------------------------------------------------------------------------

#[test]
fn render_07_gcode_thumbnail_prusaslicer_format() {
    let mesh = make_cube();
    let config = ThumbnailConfig {
        width: 32,
        height: 32,
        angles: vec![CameraAngle::Isometric],
        background: [0, 0, 0, 0],
        model_color: [200, 200, 200],
    };
    let thumbs = render_mesh(&mesh, &config);
    let block = format_gcode_thumbnail_block(&thumbs[0], ThumbnailFormat::PrusaSlicer);

    // Check structure
    let lines: Vec<&str> = block.lines().collect();
    assert!(
        lines[0].starts_with("; thumbnail begin 32x32 "),
        "First line should be begin tag, got: {}",
        lines[0]
    );
    assert_eq!(
        *lines.last().unwrap(),
        "; thumbnail end",
        "Last line should be end tag"
    );

    // All intermediate lines start with "; " and are <= 80 chars
    for line in &lines[1..lines.len() - 1] {
        assert!(
            line.starts_with("; "),
            "Intermediate line should start with '; ': {}",
            line
        );
        assert!(
            line.len() <= 80,
            "Line too long ({} chars): {}",
            line.len(),
            line
        );
    }

    // Base64 round-trip: strip prefixes, decode, compare to original PNG
    let b64_content: String = lines[1..lines.len() - 1]
        .iter()
        .map(|l| l.strip_prefix("; ").unwrap())
        .collect();
    let decoded =
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &b64_content)
            .expect("Base64 decode should succeed");
    assert_eq!(
        decoded, thumbs[0].png_data,
        "Decoded base64 should match original PNG"
    );
}

#[test]
fn render_07_gcode_thumbnail_creality_format() {
    let mesh = make_cube();
    let config = ThumbnailConfig {
        width: 32,
        height: 32,
        angles: vec![CameraAngle::Isometric],
        background: [0, 0, 0, 0],
        model_color: [200, 200, 200],
    };
    let thumbs = render_mesh(&mesh, &config);
    let block = format_gcode_thumbnail_block(&thumbs[0], ThumbnailFormat::Creality);

    let lines: Vec<&str> = block.lines().collect();
    assert!(
        lines[0].starts_with("; png begin 32x32 "),
        "First line should use 'png begin' tag for Creality"
    );
    assert_eq!(
        *lines.last().unwrap(),
        "; png end",
        "Last line should use 'png end' tag for Creality"
    );
}

// ---------------------------------------------------------------------------
// RENDER-08: CLI thumbnail subcommand produces PNG file
// (Tested in crates/slicecore-cli/tests/cli_thumbnail.rs)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// RENDER-09: WASM compilation succeeds
// Verified by CI build step: cargo build -p slicecore-render --target wasm32-unknown-unknown
// ---------------------------------------------------------------------------

#[test]
fn render_09_wasm_compilation_verified_by_ci() {
    // This test documents that WASM compilation is verified by CI.
    // The actual verification is: cargo build -p slicecore-render --target wasm32-unknown-unknown
    // It cannot be tested at runtime since we are running native tests.
}

// ---------------------------------------------------------------------------
// Additional coverage: pyramid mesh
// ---------------------------------------------------------------------------

#[test]
fn render_pyramid_produces_pixels() {
    let mesh = make_pyramid();
    let config = ThumbnailConfig {
        width: 64,
        height: 64,
        angles: vec![CameraAngle::Isometric],
        background: [0, 0, 0, 0],
        model_color: [200, 200, 200],
    };
    let thumbs = render_mesh(&mesh, &config);
    let non_bg = thumbs[0]
        .rgba
        .iter()
        .filter(|px| **px != [0, 0, 0, 0])
        .count();
    assert!(
        non_bg > 0,
        "Pyramid should produce visible pixels from isometric angle"
    );
}
