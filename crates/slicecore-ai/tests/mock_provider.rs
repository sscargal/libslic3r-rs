//! Shared test module: SmartMockProvider and synthetic test meshes.
//!
//! The SmartMockProvider inspects the serialized geometry features in the
//! prompt and returns geometry-appropriate profile suggestions. Test meshes
//! represent different print scenarios: simple cube, overhang model, and
//! thin plate.

use slicecore_ai::*;
use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

// ---------------------------------------------------------------------------
// SmartMockProvider
// ---------------------------------------------------------------------------

/// A mock LLM provider that inspects the prompt for geometry signals and
/// returns profile suggestions appropriate to the detected geometry.
pub struct SmartMockProvider;

#[async_trait::async_trait]
impl AiProvider for SmartMockProvider {
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse, AiError> {
        // The user message contains serialized GeometryFeatures JSON.
        let user_msg = &request.messages[0].content;

        // Parse geometry features from the prompt to make intelligent decisions.
        // Extract the JSON portion after "Analyze this 3D model and suggest print settings:\n\n".
        let features: serde_json::Value = {
            let json_start = user_msg.find('{').unwrap_or(0);
            serde_json::from_str(&user_msg[json_start..]).unwrap_or_default()
        };

        let overhang_ratio = features["overhang_ratio"].as_f64().unwrap_or(0.0);
        let has_small_features = features["has_small_features"].as_bool().unwrap_or(false);
        // Use estimated_difficulty to distinguish high-overhang models.
        // The T-shape has difficulty "hard" due to overhang_ratio > 0.15,
        // while a simple 20mm cube on bed plate has ~0.167 (also hard due to
        // bottom face). We use has_bridges AND difficulty together.
        let difficulty = features["estimated_difficulty"].as_str().unwrap_or("easy");
        // Significant overhangs: overhang_ratio > 0.25 (well above the ~0.167
        // that any cube-on-bed has from its bottom face alone).
        let has_significant_overhangs = overhang_ratio > 0.25 && difficulty == "hard";

        let response_json = if has_significant_overhangs && !has_small_features {
            // Overhang model: enable supports, suggest brim.
            serde_json::json!({
                "layer_height": 0.2,
                "wall_count": 3,
                "infill_density": 0.2,
                "infill_pattern": "gyroid",
                "support_enabled": true,
                "support_overhang_angle": 45.0,
                "perimeter_speed": 40.0,
                "infill_speed": 60.0,
                "nozzle_temp": 200.0,
                "bed_temp": 60.0,
                "brim_width": 5.0,
                "reasoning": "Model has significant overhangs requiring support structures."
            })
        } else if has_small_features {
            // Thin/small model: thinner layers, more walls.
            serde_json::json!({
                "layer_height": 0.1,
                "wall_count": 4,
                "infill_density": 0.4,
                "infill_pattern": "rectilinear",
                "support_enabled": false,
                "support_overhang_angle": 45.0,
                "perimeter_speed": 30.0,
                "infill_speed": 50.0,
                "nozzle_temp": 200.0,
                "bed_temp": 60.0,
                "brim_width": 3.0,
                "reasoning": "Model has small features requiring finer resolution."
            })
        } else {
            // Simple model: standard settings.
            serde_json::json!({
                "layer_height": 0.2,
                "wall_count": 2,
                "infill_density": 0.2,
                "infill_pattern": "rectilinear",
                "support_enabled": false,
                "support_overhang_angle": 45.0,
                "perimeter_speed": 45.0,
                "infill_speed": 80.0,
                "nozzle_temp": 200.0,
                "bed_temp": 60.0,
                "brim_width": 0.0,
                "reasoning": "Simple model with no special requirements."
            })
        };

        Ok(CompletionResponse {
            content: serde_json::to_string(&response_json).unwrap(),
            model: "smart-mock".to_string(),
            usage: Usage {
                prompt_tokens: 200,
                completion_tokens: 100,
            },
            finish_reason: FinishReason::Stop,
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_structured_output: true,
            supports_streaming: false,
            max_context_tokens: 32_000,
        }
    }

    fn name(&self) -> &str {
        "smart-mock"
    }
}

// ---------------------------------------------------------------------------
// Test mesh constructors
// ---------------------------------------------------------------------------

/// Creates a simple 20mm cube centered at origin with 12 triangles.
///
/// This mesh has no significant overhangs (only the bottom face, which is
/// standard for any resting object) and represents an easy print.
pub fn simple_cube() -> TriangleMesh {
    let s = 10.0; // half-size = 10mm -> 20mm cube
    let vertices = vec![
        Point3::new(-s, -s, 0.0),     // 0: left-front-bottom
        Point3::new(s, -s, 0.0),      // 1: right-front-bottom
        Point3::new(s, s, 0.0),       // 2: right-back-bottom
        Point3::new(-s, s, 0.0),      // 3: left-back-bottom
        Point3::new(-s, -s, 2.0 * s), // 4: left-front-top
        Point3::new(s, -s, 2.0 * s),  // 5: right-front-top
        Point3::new(s, s, 2.0 * s),   // 6: right-back-top
        Point3::new(-s, s, 2.0 * s),  // 7: left-back-top
    ];

    // Two triangles per face, 6 faces = 12 triangles.
    // Winding order: outward-facing normals (CCW when viewed from outside).
    let indices = vec![
        // Top face (z = 20): normal (0, 0, +1)
        [4, 5, 6],
        [4, 6, 7],
        // Bottom face (z = 0): normal (0, 0, -1)
        [0, 3, 2],
        [0, 2, 1],
        // Front face (y = -10): normal (0, -1, 0)
        [0, 1, 5],
        [0, 5, 4],
        // Back face (y = +10): normal (0, +1, 0)
        [2, 3, 7],
        [2, 7, 6],
        // Right face (x = +10): normal (+1, 0, 0)
        [1, 2, 6],
        [1, 6, 5],
        // Left face (x = -10): normal (-1, 0, 0)
        [3, 0, 4],
        [3, 4, 7],
    ];

    TriangleMesh::new(vertices, indices).expect("simple_cube mesh should be valid")
}

/// Creates a T-shaped model with significant overhangs.
///
/// Geometry: a thin stem (4mm x 4mm x 10mm) topped by a wider cap
/// (20mm x 20mm x 4mm). The underside of the cap extends beyond the
/// stem on all four sides, creating downward-facing horizontal surfaces
/// that are classified as overhangs by the geometry analyzer.
///
/// This triggers `overhang_ratio > 0.05` and `has_bridges = true`.
pub fn overhang_model() -> TriangleMesh {
    // Stem: centered at x=10, y=10, from z=0 to z=10, size 4x4
    // Cap: from x=0 to x=20, y=0 to y=20, z=10 to z=14
    //
    // We model this as two separate boxes sharing the z=10 boundary:
    //   Stem: [8,8,0] to [12,12,10]
    //   Cap:  [0,0,10] to [20,20,14]

    let vertices = vec![
        // Stem bottom (z = 0)
        Point3::new(8.0, 8.0, 0.0),   // 0
        Point3::new(12.0, 8.0, 0.0),  // 1
        Point3::new(12.0, 12.0, 0.0), // 2
        Point3::new(8.0, 12.0, 0.0),  // 3
        // Stem top / Cap inner bottom (z = 10)
        Point3::new(8.0, 8.0, 10.0),   // 4
        Point3::new(12.0, 8.0, 10.0),  // 5
        Point3::new(12.0, 12.0, 10.0), // 6
        Point3::new(8.0, 12.0, 10.0),  // 7
        // Cap bottom corners (z = 10)
        Point3::new(0.0, 0.0, 10.0),   // 8
        Point3::new(20.0, 0.0, 10.0),  // 9
        Point3::new(20.0, 20.0, 10.0), // 10
        Point3::new(0.0, 20.0, 10.0),  // 11
        // Cap top (z = 14)
        Point3::new(0.0, 0.0, 14.0),   // 12
        Point3::new(20.0, 0.0, 14.0),  // 13
        Point3::new(20.0, 20.0, 14.0), // 14
        Point3::new(0.0, 20.0, 14.0),  // 15
    ];

    let indices = vec![
        // === STEM ===
        // Stem bottom face (z = 0): normal (0, 0, -1)
        [0, 2, 1],
        [0, 3, 2],
        // Stem front face (y = 8): normal (0, -1, 0)
        [0, 1, 5],
        [0, 5, 4],
        // Stem back face (y = 12): normal (0, +1, 0)
        [2, 3, 7],
        [2, 7, 6],
        // Stem right face (x = 12): normal (+1, 0, 0)
        [1, 2, 6],
        [1, 6, 5],
        // Stem left face (x = 8): normal (-1, 0, 0)
        [3, 0, 4],
        [3, 4, 7],
        // (No stem top -- it merges with cap bottom inner area)

        // === CAP BOTTOM FACE (z = 10): overhang! normal (0, 0, -1) ===
        // This is the key overhang surface. We cover the full 20x20 area
        // minus the 4x4 stem opening (simplified: full 20x20 quad, since
        // the stem top at same Z doesn't create a visible gap in practice).
        //
        // For simplicity, model the cap bottom as the full 20x20 area.
        // The stem-cap junction is at the same Z level so this is geometrically
        // valid (the stem top and cap bottom share the z=10 plane).
        [8, 10, 9],
        [8, 11, 10],
        // === CAP TOP FACE (z = 14): normal (0, 0, +1) ===
        [12, 13, 14],
        [12, 14, 15],
        // === CAP SIDE FACES ===
        // Front face (y = 0)
        [8, 9, 13],
        [8, 13, 12],
        // Back face (y = 20)
        [10, 11, 15],
        [10, 15, 14],
        // Right face (x = 20)
        [9, 10, 14],
        [9, 14, 13],
        // Left face (x = 0)
        [11, 8, 12],
        [11, 12, 15],
    ];

    TriangleMesh::new(vertices, indices).expect("overhang_model mesh should be valid")
}

/// Creates a very thin flat plate: 50mm x 50mm x 0.8mm.
///
/// The height dimension (0.8mm) is less than 1mm, which triggers
/// `has_small_features = true` in the geometry analysis.
pub fn thin_plate() -> TriangleMesh {
    let w = 25.0; // half-width = 25mm -> 50mm plate
    let h = 0.4; // half-height = 0.4mm -> 0.8mm plate

    let vertices = vec![
        Point3::new(-w, -w, 0.0),     // 0
        Point3::new(w, -w, 0.0),      // 1
        Point3::new(w, w, 0.0),       // 2
        Point3::new(-w, w, 0.0),      // 3
        Point3::new(-w, -w, 2.0 * h), // 4
        Point3::new(w, -w, 2.0 * h),  // 5
        Point3::new(w, w, 2.0 * h),   // 6
        Point3::new(-w, w, 2.0 * h),  // 7
    ];

    let indices = vec![
        // Top face
        [4, 5, 6],
        [4, 6, 7],
        // Bottom face
        [0, 3, 2],
        [0, 2, 1],
        // Front face
        [0, 1, 5],
        [0, 5, 4],
        // Back face
        [2, 3, 7],
        [2, 7, 6],
        // Right face
        [1, 2, 6],
        [1, 6, 5],
        // Left face
        [3, 0, 4],
        [3, 4, 7],
    ];

    TriangleMesh::new(vertices, indices).expect("thin_plate mesh should be valid")
}
