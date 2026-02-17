//! Profile suggestion parsing and validation from LLM responses.
//!
//! Provides robust parsing of LLM-generated JSON responses into validated
//! [`ProfileSuggestion`] structs with all numeric values clamped to safe
//! 3D printing ranges.

use serde::{Deserialize, Serialize};

use crate::error::AiError;

/// Valid infill pattern names accepted by the profile suggestion system.
const VALID_INFILL_PATTERNS: &[&str] = &[
    "rectilinear",
    "grid",
    "honeycomb",
    "gyroid",
    "cubic",
    "monotonic",
];

/// Suggested print profile settings parsed from an LLM response.
///
/// All numeric fields are validated and clamped to safe ranges after parsing.
/// Missing fields in the LLM response use sensible defaults via `serde(default)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSuggestion {
    /// Layer height in mm (0.05 to 0.3).
    #[serde(default = "default_layer_height")]
    pub layer_height: f64,

    /// Number of perimeter walls (1 to 6).
    #[serde(default = "default_wall_count")]
    pub wall_count: u32,

    /// Infill density as a fraction (0.0 to 1.0).
    #[serde(default = "default_infill_density")]
    pub infill_density: f64,

    /// Infill pattern name.
    #[serde(default = "default_infill_pattern")]
    pub infill_pattern: String,

    /// Whether support structures are recommended.
    #[serde(default)]
    pub support_enabled: bool,

    /// Minimum overhang angle (degrees) before support is generated (30 to 80).
    #[serde(default = "default_support_overhang_angle")]
    pub support_overhang_angle: f64,

    /// Perimeter/wall print speed in mm/s (20 to 100).
    #[serde(default = "default_perimeter_speed")]
    pub perimeter_speed: f64,

    /// Infill print speed in mm/s (30 to 150).
    #[serde(default = "default_infill_speed")]
    pub infill_speed: f64,

    /// Nozzle/hotend temperature in degrees C (180 to 260).
    #[serde(default = "default_nozzle_temp")]
    pub nozzle_temp: f64,

    /// Bed temperature in degrees C (0 to 110).
    #[serde(default = "default_bed_temp")]
    pub bed_temp: f64,

    /// Brim width in mm (0.0 to 10.0).
    #[serde(default)]
    pub brim_width: f64,

    /// Brief explanation of the suggested settings.
    #[serde(default)]
    pub reasoning: String,
}

fn default_layer_height() -> f64 {
    0.2
}
fn default_wall_count() -> u32 {
    2
}
fn default_infill_density() -> f64 {
    0.2
}
fn default_infill_pattern() -> String {
    "rectilinear".to_string()
}
fn default_support_overhang_angle() -> f64 {
    45.0
}
fn default_perimeter_speed() -> f64 {
    45.0
}
fn default_infill_speed() -> f64 {
    80.0
}
fn default_nozzle_temp() -> f64 {
    200.0
}
fn default_bed_temp() -> f64 {
    60.0
}

impl Default for ProfileSuggestion {
    fn default() -> Self {
        Self {
            layer_height: 0.2,
            wall_count: 2,
            infill_density: 0.2,
            infill_pattern: "rectilinear".to_string(),
            support_enabled: false,
            support_overhang_angle: 45.0,
            perimeter_speed: 45.0,
            infill_speed: 80.0,
            nozzle_temp: 200.0,
            bed_temp: 60.0,
            brim_width: 0.0,
            reasoning: String::new(),
        }
    }
}

/// Extracts a JSON value from a raw LLM response string.
///
/// Handles multiple common response formats:
/// 1. Direct JSON (the response is valid JSON)
/// 2. Markdown code fences (```json ... ``` or ``` ... ```)
/// 3. Embedded JSON (text before/after a JSON object)
///
/// # Errors
///
/// Returns [`AiError::InvalidJsonResponse`] if no valid JSON can be extracted.
pub fn extract_json(raw: &str) -> Result<serde_json::Value, AiError> {
    let trimmed = raw.trim();

    // 1. Try direct parse.
    if let Ok(value) = serde_json::from_str(trimmed) {
        return Ok(value);
    }

    // 2. Try stripping markdown code fences.
    let stripped = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|s| s.strip_suffix("```"))
        .map(|s| s.trim());

    if let Some(inner) = stripped {
        if let Ok(value) = serde_json::from_str(inner) {
            return Ok(value);
        }
    }

    // 3. Find first '{' and last '}', extract substring.
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start < end {
            let json_str = &trimmed[start..=end];
            if let Ok(value) = serde_json::from_str(json_str) {
                return Ok(value);
            }
        }
    }

    // 4. All strategies failed.
    Err(AiError::InvalidJsonResponse(raw.to_string()))
}

/// Parses and validates a profile suggestion from a raw LLM response.
///
/// This function:
/// 1. Extracts JSON from the raw response (handling code fences, embedded text)
/// 2. Deserializes into [`ProfileSuggestion`] (missing fields get defaults)
/// 3. Clamps all numeric values to safe printing ranges
///
/// # Errors
///
/// Returns [`AiError::InvalidJsonResponse`] if no JSON can be extracted, or
/// [`AiError::ParseError`] if the JSON does not match the expected structure.
pub fn parse_profile_suggestion(raw: &str) -> Result<ProfileSuggestion, AiError> {
    let value = extract_json(raw)?;
    let mut suggestion: ProfileSuggestion =
        serde_json::from_value(value).map_err(|e| AiError::ParseError(e.to_string()))?;
    validate_and_clamp(&mut suggestion);
    Ok(suggestion)
}

/// Clamps all numeric fields in a profile suggestion to safe printing ranges.
fn validate_and_clamp(suggestion: &mut ProfileSuggestion) {
    suggestion.layer_height = suggestion.layer_height.clamp(0.05, 0.3);
    suggestion.wall_count = suggestion.wall_count.clamp(1, 6);
    suggestion.infill_density = suggestion.infill_density.clamp(0.0, 1.0);
    suggestion.support_overhang_angle = suggestion.support_overhang_angle.clamp(30.0, 80.0);
    suggestion.perimeter_speed = suggestion.perimeter_speed.clamp(20.0, 100.0);
    suggestion.infill_speed = suggestion.infill_speed.clamp(30.0, 150.0);
    suggestion.nozzle_temp = suggestion.nozzle_temp.clamp(180.0, 260.0);
    suggestion.bed_temp = suggestion.bed_temp.clamp(0.0, 110.0);
    suggestion.brim_width = suggestion.brim_width.clamp(0.0, 10.0);

    if !VALID_INFILL_PATTERNS.contains(&suggestion.infill_pattern.as_str()) {
        suggestion.infill_pattern = "rectilinear".to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_clean_json() {
        let input = r#"{"layer_height": 0.2, "reasoning": "test"}"#;
        let result = extract_json(input);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["layer_height"], 0.2);
    }

    #[test]
    fn extract_json_with_markdown_json_fence() {
        let input = r#"```json
{"layer_height": 0.15, "reasoning": "fine detail"}
```"#;
        let result = extract_json(input);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["layer_height"], 0.15);
    }

    #[test]
    fn extract_json_with_plain_markdown_fence() {
        let input = r#"```
{"layer_height": 0.1}
```"#;
        let result = extract_json(input);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["layer_height"], 0.1);
    }

    #[test]
    fn extract_json_with_surrounding_text() {
        let input = r#"Here are the suggested settings:

{"layer_height": 0.2, "wall_count": 3}

I hope these settings work well for your model."#;
        let result = extract_json(input);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["wall_count"], 3);
    }

    #[test]
    fn extract_json_invalid_content() {
        let input = "This is not JSON at all. No curly braces here.";
        let result = extract_json(input);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AiError::InvalidJsonResponse(_)));
    }

    #[test]
    fn extract_json_empty_string() {
        let result = extract_json("");
        assert!(result.is_err());
    }

    #[test]
    fn parse_profile_suggestion_valid_json() {
        let input = r#"{
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
            "reasoning": "Moderate complexity model"
        }"#;
        let result = parse_profile_suggestion(input);
        assert!(result.is_ok());
        let suggestion = result.unwrap();
        assert!((suggestion.layer_height - 0.15).abs() < 1e-6);
        assert_eq!(suggestion.wall_count, 3);
        assert!((suggestion.infill_density - 0.3).abs() < 1e-6);
        assert_eq!(suggestion.infill_pattern, "gyroid");
        assert!(suggestion.support_enabled);
        assert!((suggestion.support_overhang_angle - 50.0).abs() < 1e-6);
        assert_eq!(suggestion.reasoning, "Moderate complexity model");
    }

    #[test]
    fn parse_profile_suggestion_out_of_range_values_clamped() {
        let input = r#"{
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
            "reasoning": "extreme values"
        }"#;
        let result = parse_profile_suggestion(input);
        assert!(result.is_ok());
        let s = result.unwrap();

        assert!((s.layer_height - 0.05).abs() < 1e-6, "layer_height should be clamped to 0.05");
        assert_eq!(s.wall_count, 6, "wall_count should be clamped to 6");
        assert!((s.infill_density - 1.0).abs() < 1e-6, "infill_density should be clamped to 1.0");
        assert!((s.support_overhang_angle - 30.0).abs() < 1e-6, "support_overhang_angle should be clamped to 30.0");
        assert!((s.perimeter_speed - 20.0).abs() < 1e-6, "perimeter_speed should be clamped to 20.0");
        assert!((s.infill_speed - 150.0).abs() < 1e-6, "infill_speed should be clamped to 150.0");
        assert!((s.nozzle_temp - 260.0).abs() < 1e-6, "nozzle_temp should be clamped to 260.0");
        assert!((s.bed_temp - 0.0).abs() < 1e-6, "bed_temp should be clamped to 0.0");
        assert!((s.brim_width - 10.0).abs() < 1e-6, "brim_width should be clamped to 10.0");
    }

    #[test]
    fn parse_profile_suggestion_missing_fields_get_defaults() {
        let input = r#"{"reasoning": "minimal response"}"#;
        let result = parse_profile_suggestion(input);
        assert!(result.is_ok());
        let s = result.unwrap();

        assert!((s.layer_height - 0.2).abs() < 1e-6, "default layer_height should be 0.2");
        assert_eq!(s.wall_count, 2, "default wall_count should be 2");
        assert!((s.infill_density - 0.2).abs() < 1e-6, "default infill_density should be 0.2");
        assert_eq!(s.infill_pattern, "rectilinear", "default infill_pattern should be rectilinear");
        assert!(!s.support_enabled, "default support_enabled should be false");
        assert!((s.support_overhang_angle - 45.0).abs() < 1e-6);
        assert!((s.perimeter_speed - 45.0).abs() < 1e-6);
        assert!((s.infill_speed - 80.0).abs() < 1e-6);
        assert!((s.nozzle_temp - 200.0).abs() < 1e-6);
        assert!((s.bed_temp - 60.0).abs() < 1e-6);
        assert!((s.brim_width - 0.0).abs() < 1e-6);
        assert_eq!(s.reasoning, "minimal response");
    }

    #[test]
    fn parse_profile_suggestion_invalid_infill_pattern_defaults() {
        let input = r#"{"infill_pattern": "zigzag_custom"}"#;
        let result = parse_profile_suggestion(input);
        assert!(result.is_ok());
        let s = result.unwrap();
        assert_eq!(
            s.infill_pattern, "rectilinear",
            "Invalid infill pattern should default to rectilinear"
        );
    }

    #[test]
    fn parse_profile_suggestion_from_markdown_response() {
        let input = r#"Here are the suggested settings:

```json
{
  "layer_height": 0.2,
  "wall_count": 2,
  "infill_density": 0.2,
  "infill_pattern": "rectilinear",
  "support_enabled": false,
  "reasoning": "Simple cube"
}
```"#;
        let result = parse_profile_suggestion(input);
        assert!(result.is_ok());
        let s = result.unwrap();
        assert!((s.layer_height - 0.2).abs() < 1e-6);
        assert_eq!(s.reasoning, "Simple cube");
    }

    #[test]
    fn validate_and_clamp_each_field() {
        let mut s = ProfileSuggestion {
            layer_height: 0.5,
            wall_count: 0,
            infill_density: -0.5,
            infill_pattern: "unknown_pattern".to_string(),
            support_enabled: true,
            support_overhang_angle: 100.0,
            perimeter_speed: 0.0,
            infill_speed: 0.0,
            nozzle_temp: 100.0,
            bed_temp: 200.0,
            brim_width: -5.0,
            reasoning: "test".to_string(),
        };
        validate_and_clamp(&mut s);

        assert!((s.layer_height - 0.3).abs() < 1e-6);
        assert_eq!(s.wall_count, 1);
        assert!((s.infill_density - 0.0).abs() < 1e-6);
        assert_eq!(s.infill_pattern, "rectilinear");
        assert!((s.support_overhang_angle - 80.0).abs() < 1e-6);
        assert!((s.perimeter_speed - 20.0).abs() < 1e-6);
        assert!((s.infill_speed - 30.0).abs() < 1e-6);
        assert!((s.nozzle_temp - 180.0).abs() < 1e-6);
        assert!((s.bed_temp - 110.0).abs() < 1e-6);
        assert!((s.brim_width - 0.0).abs() < 1e-6);
    }

    #[test]
    fn validate_and_clamp_valid_patterns_preserved() {
        for &pattern in VALID_INFILL_PATTERNS {
            let mut s = ProfileSuggestion {
                infill_pattern: pattern.to_string(),
                ..Default::default()
            };
            validate_and_clamp(&mut s);
            assert_eq!(
                s.infill_pattern, pattern,
                "Valid pattern '{}' should be preserved",
                pattern
            );
        }
    }

    #[test]
    fn default_profile_suggestion_has_sensible_values() {
        let s = ProfileSuggestion::default();
        assert!((s.layer_height - 0.2).abs() < 1e-6);
        assert_eq!(s.wall_count, 2);
        assert!((s.infill_density - 0.2).abs() < 1e-6);
        assert_eq!(s.infill_pattern, "rectilinear");
        assert!(!s.support_enabled);
        assert!(s.reasoning.is_empty());
    }

    #[test]
    fn parse_profile_suggestion_completely_invalid() {
        let result = parse_profile_suggestion("not json at all");
        assert!(result.is_err());
    }
}
