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
    async fn complete(
        &self,
        request: &CompletionRequest,
    ) -> Result<CompletionResponse, AiError> {
        // The user message contains serialized GeometryFeatures JSON.
        let user_msg = &request.messages[0].content;

        // Detect overhang model by checking for non-zero overhang_ratio.
        let has_overhangs = user_msg.contains("\"overhang_ratio\":")
            && !user_msg.contains("\"overhang_ratio\": 0.0");
        let has_small_features = user_msg.contains("\"has_small_features\": true");

        let response_json = if has_overhangs && !has_small_features {
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
        Point3::new(-s, -s, 0.0), // 0: left-front-bottom
        Point3::new(s, -s, 0.0),  // 1: right-front-bottom
        Point3::new(s, s, 0.0),   // 2: right-back-bottom
        Point3::new(-s, s, 0.0),  // 3: left-back-bottom
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

/// Creates a model with significant overhangs.
///
/// This is a wedge/ramp shape: a box with a tilted top face angled at
/// approximately 60 degrees from horizontal, producing overhangs that
/// should trigger `overhang_ratio > 0.05` and `has_bridges = true`.
///
/// The shape is a pentahedron (triangulated prism) with vertices:
/// - Base: 20mm x 20mm rectangle at z = 0
/// - Top: angled edge at z = 20mm on one side, z = 0mm on the other
pub fn overhang_model() -> TriangleMesh {
    // A wedge: base rectangle at z=0, top edge at z=20 on the left side,
    // sloping down to z=0 on the right side.
    let vertices = vec![
        // Bottom face vertices
        Point3::new(0.0, 0.0, 0.0),   // 0: bottom-left-front
        Point3::new(20.0, 0.0, 0.0),  // 1: bottom-right-front
        Point3::new(20.0, 20.0, 0.0), // 2: bottom-right-back
        Point3::new(0.0, 20.0, 0.0),  // 3: bottom-left-back
        // Top edge vertices (left side elevated)
        Point3::new(0.0, 0.0, 20.0),  // 4: top-left-front
        Point3::new(0.0, 20.0, 20.0), // 5: top-left-back
    ];

    let indices = vec![
        // Bottom face (z = 0): normal pointing down
        [0, 2, 1],
        [0, 3, 2],
        // Left face (x = 0): vertical wall, normal pointing left
        [0, 4, 5],
        [0, 5, 3],
        // Front face (y = 0): triangle
        [0, 1, 4],
        // Back face (y = 20): triangle
        [3, 5, 2],
        // Sloped top face: from top-left edge (z=20) down to bottom-right edge (z=0)
        // This face has a large overhang angle.
        // Vertices: 4 (0,0,20), 5 (0,20,20), 2 (20,20,0), 1 (20,0,0)
        [4, 1, 2],
        [4, 2, 5],
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
        Point3::new(-w, -w, 0.0), // 0
        Point3::new(w, -w, 0.0),  // 1
        Point3::new(w, w, 0.0),   // 2
        Point3::new(-w, w, 0.0),  // 3
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
