//! Prompt construction for LLM-based print profile suggestion.
//!
//! Builds a [`CompletionRequest`] from [`GeometryFeatures`] that instructs
//! an LLM to analyze the geometry and respond with optimal print settings
//! in a structured JSON format.

use crate::geometry::GeometryFeatures;
use crate::types::{CompletionRequest, Message, ResponseFormat, Role};

/// System prompt instructing the LLM to act as a 3D printing expert.
const SYSTEM_PROMPT: &str = r#"You are a 3D printing expert. Given geometry analysis of a 3D model, suggest optimal FDM print settings.
Respond ONLY with a JSON object matching this exact schema:
{
  "layer_height": <number 0.05-0.3>,
  "wall_count": <integer 1-6>,
  "infill_density": <number 0.0-1.0>,
  "infill_pattern": <string: "rectilinear" | "grid" | "honeycomb" | "gyroid">,
  "support_enabled": <boolean>,
  "support_overhang_angle": <number 30-80>,
  "perimeter_speed": <number 20-100>,
  "infill_speed": <number 30-150>,
  "nozzle_temp": <number 180-260>,
  "bed_temp": <number 0-110>,
  "brim_width": <number 0.0-10.0>,
  "reasoning": "<brief explanation of choices>"
}
Do not include any text outside the JSON object."#;

/// Builds a [`CompletionRequest`] for profile suggestion from geometry features.
///
/// The request contains:
/// - A system prompt instructing the LLM to respond with structured JSON
/// - A user message containing the serialized geometry features
/// - Low temperature (0.3) for consistent structured output
/// - JSON response format constraint
///
/// # Example
///
/// ```rust,ignore
/// use slicecore_ai::geometry::extract_geometry_features;
/// use slicecore_ai::prompt::build_profile_prompt;
///
/// let features = extract_geometry_features(&mesh);
/// let request = build_profile_prompt(&features);
/// // Send request to an AiProvider...
/// ```
pub fn build_profile_prompt(features: &GeometryFeatures) -> CompletionRequest {
    let features_json =
        serde_json::to_string_pretty(features).expect("GeometryFeatures should be serializable");

    let user_message = format!(
        "Analyze this 3D model and suggest print settings:\n\n{}",
        features_json
    );

    CompletionRequest {
        system_prompt: SYSTEM_PROMPT.to_string(),
        messages: vec![Message {
            role: Role::User,
            content: user_message,
        }],
        temperature: 0.3,
        max_tokens: 1024,
        response_format: Some(ResponseFormat::Json),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Dimensions, PrintDifficulty};

    fn sample_features() -> GeometryFeatures {
        GeometryFeatures {
            bounding_box_min: [0.0, 0.0, 0.0],
            bounding_box_max: [20.0, 20.0, 20.0],
            volume_mm3: 8000.0,
            surface_area_mm2: 2400.0,
            triangle_count: 12,
            is_watertight: true,
            dimensions: Dimensions {
                width_mm: 20.0,
                depth_mm: 20.0,
                height_mm: 20.0,
            },
            aspect_ratio: 1.0,
            overhang_ratio: 0.0,
            max_overhang_angle_deg: 0.0,
            thin_wall_ratio: 0.0,
            has_bridges: false,
            has_small_features: false,
            estimated_difficulty: PrintDifficulty::Easy,
        }
    }

    #[test]
    fn build_profile_prompt_contains_system_prompt() {
        let features = sample_features();
        let request = build_profile_prompt(&features);

        assert!(request.system_prompt.contains("3D printing expert"));
        assert!(request.system_prompt.contains("layer_height"));
        assert!(request.system_prompt.contains("JSON object"));
    }

    #[test]
    fn build_profile_prompt_contains_features_json() {
        let features = sample_features();
        let request = build_profile_prompt(&features);

        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].role, Role::User);
        assert!(request.messages[0].content.contains("volume_mm3"));
        assert!(request.messages[0].content.contains("8000"));
        assert!(request.messages[0]
            .content
            .contains("Analyze this 3D model"));
    }

    #[test]
    fn build_profile_prompt_has_low_temperature() {
        let features = sample_features();
        let request = build_profile_prompt(&features);

        assert!(
            (request.temperature - 0.3).abs() < 0.01,
            "Expected temperature ~0.3, got {}",
            request.temperature
        );
    }

    #[test]
    fn build_profile_prompt_has_max_tokens() {
        let features = sample_features();
        let request = build_profile_prompt(&features);

        assert_eq!(request.max_tokens, 1024);
    }

    #[test]
    fn build_profile_prompt_requests_json_format() {
        let features = sample_features();
        let request = build_profile_prompt(&features);

        assert!(
            matches!(request.response_format, Some(ResponseFormat::Json)),
            "Expected Json response format"
        );
    }

    #[test]
    fn build_profile_prompt_system_prompt_has_all_fields() {
        let features = sample_features();
        let request = build_profile_prompt(&features);

        let expected_fields = [
            "layer_height",
            "wall_count",
            "infill_density",
            "infill_pattern",
            "support_enabled",
            "support_overhang_angle",
            "perimeter_speed",
            "infill_speed",
            "nozzle_temp",
            "bed_temp",
            "brim_width",
            "reasoning",
        ];

        for field in &expected_fields {
            assert!(
                request.system_prompt.contains(field),
                "System prompt missing field: {}",
                field
            );
        }
    }
}
