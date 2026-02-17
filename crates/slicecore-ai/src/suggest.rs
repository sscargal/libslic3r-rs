//! End-to-end profile suggestion pipeline.
//!
//! Chains geometry feature extraction, prompt construction, LLM completion,
//! and response parsing into a single async function. Also provides a
//! synchronous wrapper for non-async callers.
//!
//! # Pipeline Stages
//!
//! 1. [`extract_geometry_features`] -- mesh analysis
//! 2. [`build_profile_prompt`] -- prompt construction from features
//! 3. [`AiProvider::complete`] -- LLM request/response
//! 4. [`parse_profile_suggestion`] -- response parsing and validation
//!
//! # Example
//!
//! ```rust,ignore
//! use slicecore_ai::{suggest_profile_sync, AiConfig, create_provider};
//! use slicecore_mesh::TriangleMesh;
//!
//! let config = AiConfig::default();
//! let provider = create_provider(&config).unwrap();
//! let suggestion = suggest_profile_sync(provider.as_ref(), &mesh).unwrap();
//! println!("Suggested layer height: {}", suggestion.layer_height);
//! ```

use crate::error::AiError;
use crate::geometry::{extract_geometry_features, GeometryFeatures};
use crate::profile::{parse_profile_suggestion, ProfileSuggestion};
use crate::prompt::build_profile_prompt;
use crate::provider::AiProvider;
use slicecore_mesh::TriangleMesh;

/// Suggests optimal print settings for a mesh using an LLM provider.
///
/// This is the primary async entry point for the AI suggestion pipeline.
/// It performs the full chain: geometry extraction -> prompt building ->
/// LLM completion -> response parsing and validation.
///
/// # Errors
///
/// Returns [`AiError`] if the LLM provider fails, the response cannot be
/// parsed, or any pipeline stage encounters an error.
pub async fn suggest_profile(
    provider: &dyn AiProvider,
    mesh: &TriangleMesh,
) -> Result<ProfileSuggestion, AiError> {
    // Step 1: Extract geometry features from the mesh.
    let features = extract_geometry_features(mesh);

    // Steps 2-4: Delegate to the features-based variant.
    suggest_profile_from_features(provider, &features).await
}

/// Suggests optimal print settings from pre-extracted geometry features.
///
/// This variant is useful when the caller has already computed geometry
/// features (e.g., for display purposes) and wants to avoid redundant
/// extraction.
///
/// # Errors
///
/// Returns [`AiError`] if the LLM provider fails or the response cannot
/// be parsed.
pub async fn suggest_profile_from_features(
    provider: &dyn AiProvider,
    features: &GeometryFeatures,
) -> Result<ProfileSuggestion, AiError> {
    // Step 2: Build prompt from features.
    let request = build_profile_prompt(features);

    // Step 3: Call LLM provider.
    let response = provider.complete(&request).await?;

    // Step 4: Parse and validate response.
    let suggestion = parse_profile_suggestion(&response.content)?;

    Ok(suggestion)
}

/// Synchronous wrapper around [`suggest_profile`].
///
/// Creates a single-threaded Tokio runtime and blocks on the async pipeline.
/// Use this when calling from synchronous code (e.g., the Engine API).
///
/// # Errors
///
/// Returns [`AiError::RuntimeError`] if the Tokio runtime cannot be created,
/// or any error from [`suggest_profile`].
pub fn suggest_profile_sync(
    provider: &dyn AiProvider,
    mesh: &TriangleMesh,
) -> Result<ProfileSuggestion, AiError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| AiError::RuntimeError(e.to_string()))?;
    rt.block_on(suggest_profile(provider, mesh))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        CompletionRequest, CompletionResponse, FinishReason, ProviderCapabilities, Usage,
    };
    use slicecore_math::Point3;

    /// A mock LLM provider that returns a preconfigured response.
    struct MockProvider {
        response: String,
    }

    #[async_trait::async_trait]
    impl AiProvider for MockProvider {
        async fn complete(
            &self,
            _request: &CompletionRequest,
        ) -> Result<CompletionResponse, AiError> {
            Ok(CompletionResponse {
                content: self.response.clone(),
                model: "mock".to_string(),
                usage: Usage {
                    prompt_tokens: 10,
                    completion_tokens: 50,
                },
                finish_reason: FinishReason::Stop,
            })
        }

        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                supports_structured_output: true,
                supports_streaming: false,
                max_context_tokens: 4096,
            }
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    /// Creates a unit cube mesh for testing.
    fn unit_cube() -> TriangleMesh {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 1.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(1.0, 1.0, 1.0),
            Point3::new(0.0, 1.0, 1.0),
        ];
        let indices = vec![
            [4, 5, 6],
            [4, 6, 7],
            [1, 0, 3],
            [1, 3, 2],
            [1, 2, 6],
            [1, 6, 5],
            [0, 4, 7],
            [0, 7, 3],
            [3, 7, 6],
            [3, 6, 2],
            [0, 1, 5],
            [0, 5, 4],
        ];
        TriangleMesh::new(vertices, indices).expect("unit cube should be valid")
    }

    /// Valid JSON response matching the expected ProfileSuggestion schema.
    fn valid_mock_response() -> String {
        r#"{
            "layer_height": 0.15,
            "wall_count": 3,
            "infill_density": 0.3,
            "infill_pattern": "gyroid",
            "support_enabled": true,
            "support_overhang_angle": 50.0,
            "perimeter_speed": 40.0,
            "infill_speed": 80.0,
            "nozzle_temp": 210.0,
            "bed_temp": 60.0,
            "brim_width": 5.0,
            "reasoning": "Small cube with overhangs needs support and moderate infill"
        }"#
        .to_string()
    }

    #[test]
    fn suggest_profile_sync_valid_response() {
        let provider = MockProvider {
            response: valid_mock_response(),
        };
        let mesh = unit_cube();

        let result = suggest_profile_sync(&provider, &mesh);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());

        let suggestion = result.unwrap();
        assert!(
            (suggestion.layer_height - 0.15).abs() < 1e-6,
            "Expected layer_height ~0.15, got {}",
            suggestion.layer_height
        );
        assert_eq!(suggestion.wall_count, 3);
        assert!(
            (suggestion.infill_density - 0.3).abs() < 1e-6,
            "Expected infill_density ~0.3, got {}",
            suggestion.infill_density
        );
        assert_eq!(suggestion.infill_pattern, "gyroid");
        assert!(suggestion.support_enabled);
        assert!(
            (suggestion.support_overhang_angle - 50.0).abs() < 1e-6,
            "Expected support_overhang_angle ~50.0, got {}",
            suggestion.support_overhang_angle
        );
        assert!(
            !suggestion.reasoning.is_empty(),
            "Reasoning should be populated"
        );
    }

    #[test]
    fn suggest_profile_sync_malformed_json_returns_error() {
        let provider = MockProvider {
            response: "This is not JSON at all, just plain text without braces".to_string(),
        };
        let mesh = unit_cube();

        let result = suggest_profile_sync(&provider, &mesh);
        assert!(result.is_err(), "Expected error for malformed response");
        match result.unwrap_err() {
            AiError::InvalidJsonResponse(_) => {} // expected
            other => panic!("Expected InvalidJsonResponse, got: {:?}", other),
        }
    }

    #[test]
    fn suggest_profile_sync_out_of_range_values_clamped() {
        let response = r#"{
            "layer_height": 0.01,
            "wall_count": 10,
            "infill_density": 2.0,
            "infill_pattern": "gyroid",
            "support_overhang_angle": 10.0,
            "perimeter_speed": 5.0,
            "infill_speed": 200.0,
            "nozzle_temp": 300.0,
            "bed_temp": -10.0,
            "brim_width": 20.0,
            "reasoning": "extreme values test"
        }"#
        .to_string();

        let provider = MockProvider { response };
        let mesh = unit_cube();

        let result = suggest_profile_sync(&provider, &mesh);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());

        let s = result.unwrap();
        assert!(
            (s.layer_height - 0.05).abs() < 1e-6,
            "layer_height should be clamped to 0.05, got {}",
            s.layer_height
        );
        assert_eq!(s.wall_count, 6, "wall_count should be clamped to 6");
        assert!(
            (s.infill_density - 1.0).abs() < 1e-6,
            "infill_density should be clamped to 1.0"
        );
        assert!(
            (s.support_overhang_angle - 30.0).abs() < 1e-6,
            "support_overhang_angle should be clamped to 30.0"
        );
        assert!(
            (s.perimeter_speed - 20.0).abs() < 1e-6,
            "perimeter_speed should be clamped to 20.0"
        );
        assert!(
            (s.infill_speed - 150.0).abs() < 1e-6,
            "infill_speed should be clamped to 150.0"
        );
        assert!(
            (s.nozzle_temp - 260.0).abs() < 1e-6,
            "nozzle_temp should be clamped to 260.0"
        );
        assert!(
            (s.bed_temp - 0.0).abs() < 1e-6,
            "bed_temp should be clamped to 0.0"
        );
        assert!(
            (s.brim_width - 10.0).abs() < 1e-6,
            "brim_width should be clamped to 10.0"
        );
    }

    #[tokio::test]
    async fn suggest_profile_async_valid_response() {
        let provider = MockProvider {
            response: valid_mock_response(),
        };
        let mesh = unit_cube();

        let result = suggest_profile(&provider, &mesh).await;
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());

        let suggestion = result.unwrap();
        assert!(
            (suggestion.layer_height - 0.15).abs() < 1e-6,
            "Expected layer_height ~0.15"
        );
        assert_eq!(suggestion.infill_pattern, "gyroid");
    }

    #[tokio::test]
    async fn suggest_profile_from_features_valid_response() {
        let provider = MockProvider {
            response: valid_mock_response(),
        };
        let mesh = unit_cube();
        let features = extract_geometry_features(&mesh);

        let result = suggest_profile_from_features(&provider, &features).await;
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());

        let suggestion = result.unwrap();
        assert!(
            (suggestion.layer_height - 0.15).abs() < 1e-6,
            "Expected layer_height ~0.15"
        );
    }
}
