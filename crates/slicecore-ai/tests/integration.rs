//! Integration tests verifying all Phase 8 success criteria.
//!
//! SC1: Geometry analysis extracts meaningful features
//! SC2: Profile suggestion end-to-end pipeline
//! SC3: Provider abstraction (factory, construction)
//! SC4: AI suggestions are reasonable (geometry-appropriate)
//!
//! All tests use the SmartMockProvider (no network access required).

mod mock_provider;

use mock_provider::*;
use slicecore_ai::*;

// ===========================================================================
// SC1: Geometry analysis extracts meaningful features
// ===========================================================================

#[test]
fn sc1_cube_geometry_features() {
    let mesh = simple_cube();
    let features = extract_geometry_features(&mesh);

    // Bounding box dimensions should be ~20mm x 20mm x 20mm.
    assert!(
        (features.dimensions.width_mm - 20.0).abs() < 0.1,
        "width: {}",
        features.dimensions.width_mm
    );
    assert!(
        (features.dimensions.depth_mm - 20.0).abs() < 0.1,
        "depth: {}",
        features.dimensions.depth_mm
    );
    assert!(
        (features.dimensions.height_mm - 20.0).abs() < 0.1,
        "height: {}",
        features.dimensions.height_mm
    );

    // Cube should have aspect ratio ~1.0.
    assert!(
        (features.aspect_ratio - 1.0).abs() < 0.1,
        "aspect_ratio: {}",
        features.aspect_ratio
    );

    // Volume should be ~8000 mm^3.
    assert!(
        (features.volume_mm3 - 8000.0).abs() < 100.0,
        "volume: {}",
        features.volume_mm3
    );

    // Cube should be watertight.
    assert!(features.is_watertight, "cube should be watertight");

    // 12 triangles.
    assert_eq!(features.triangle_count, 12);
}

#[test]
fn sc1_overhang_model_detects_overhangs() {
    let mesh = overhang_model();
    let features = extract_geometry_features(&mesh);

    // T-shape has significant overhangs from the cap underside.
    assert!(
        features.overhang_ratio > 0.05,
        "overhang_ratio {} should be > 0.05",
        features.overhang_ratio
    );
    assert!(
        features.max_overhang_angle_deg > 30.0,
        "max_overhang_angle {} should be > 30 deg",
        features.max_overhang_angle_deg
    );
    assert!(features.has_bridges, "should detect bridges");
}

#[test]
fn sc1_thin_plate_detects_small_features() {
    let mesh = thin_plate();
    let features = extract_geometry_features(&mesh);

    // Height should be < 1mm.
    assert!(
        features.dimensions.height_mm < 1.0,
        "height {} should be < 1.0",
        features.dimensions.height_mm
    );
    assert!(
        features.has_small_features,
        "thin plate should have small features"
    );
}

#[test]
fn sc1_features_are_serializable() {
    let mesh = simple_cube();
    let features = extract_geometry_features(&mesh);
    let json = serde_json::to_string_pretty(&features).unwrap();
    assert!(json.contains("volume_mm3"));
    assert!(json.contains("dimensions"));
    assert!(json.contains("overhang_ratio"));
    // Round-trip deserialization.
    let deserialized: GeometryFeatures = serde_json::from_str(&json).unwrap();
    assert!((deserialized.volume_mm3 - features.volume_mm3).abs() < 1e-6);
}

// ===========================================================================
// SC2: Profile suggestion end-to-end pipeline
// ===========================================================================

#[test]
fn sc2_full_pipeline_cube() {
    let mesh = simple_cube();
    let provider = SmartMockProvider;
    let suggestion = suggest_profile_sync(&provider, &mesh).unwrap();

    // Simple model should get standard settings, no supports.
    assert!(!suggestion.support_enabled, "cube should not need supports");
    assert!(
        (suggestion.layer_height - 0.2).abs() < 0.01,
        "layer_height: {}",
        suggestion.layer_height
    );
    assert!(
        !suggestion.reasoning.is_empty(),
        "reasoning should be populated"
    );
}

#[test]
fn sc2_full_pipeline_overhang_model() {
    let mesh = overhang_model();
    let provider = SmartMockProvider;
    let suggestion = suggest_profile_sync(&provider, &mesh).unwrap();

    // Overhang model should get supports enabled.
    assert!(
        suggestion.support_enabled,
        "Overhang model should suggest supports"
    );
    assert!(
        suggestion.brim_width > 0.0,
        "Overhang model should suggest brim, got {}",
        suggestion.brim_width
    );
}

#[test]
fn sc2_full_pipeline_thin_plate() {
    let mesh = thin_plate();
    let provider = SmartMockProvider;
    let suggestion = suggest_profile_sync(&provider, &mesh).unwrap();

    // Thin model should get finer layers.
    assert!(
        suggestion.layer_height <= 0.15,
        "Thin model should suggest fine layers, got {}",
        suggestion.layer_height
    );
}

#[test]
fn sc2_suggestion_values_in_valid_range() {
    let mesh = simple_cube();
    let provider = SmartMockProvider;
    let s = suggest_profile_sync(&provider, &mesh).unwrap();

    assert!(s.layer_height >= 0.05 && s.layer_height <= 0.3);
    assert!(s.wall_count >= 1 && s.wall_count <= 6);
    assert!(s.infill_density >= 0.0 && s.infill_density <= 1.0);
    assert!(s.nozzle_temp >= 180.0 && s.nozzle_temp <= 260.0);
    assert!(s.bed_temp >= 0.0 && s.bed_temp <= 110.0);
}

// ===========================================================================
// SC3: Provider abstraction
// ===========================================================================

#[test]
fn sc3_provider_factory_openai_requires_key() {
    let config = AiConfig {
        provider: ProviderType::OpenAi,
        model: "gpt-4o".to_string(),
        api_key: None,
        ..Default::default()
    };
    assert!(create_provider(&config).is_err());
}

#[test]
fn sc3_provider_factory_anthropic_requires_key() {
    let config = AiConfig {
        provider: ProviderType::Anthropic,
        model: "claude-sonnet-4-20250514".to_string(),
        api_key: None,
        ..Default::default()
    };
    assert!(create_provider(&config).is_err());
}

#[test]
fn sc3_provider_factory_ollama_no_key_needed() {
    let config = AiConfig {
        provider: ProviderType::Ollama,
        model: "llama3.2".to_string(),
        api_key: None,
        ..Default::default()
    };
    assert!(create_provider(&config).is_ok());
}

#[test]
fn sc3_all_providers_construct() {
    use secrecy::SecretString;

    // OpenAI
    let openai_config = AiConfig {
        provider: ProviderType::OpenAi,
        model: "gpt-4o".to_string(),
        api_key: Some(SecretString::from("test-key-123".to_string())),
        ..Default::default()
    };
    let openai = create_provider(&openai_config);
    assert!(openai.is_ok());
    assert_eq!(openai.unwrap().name(), "openai");

    // Anthropic
    let anthropic_config = AiConfig {
        provider: ProviderType::Anthropic,
        model: "claude-sonnet-4-20250514".to_string(),
        api_key: Some(SecretString::from("test-key-456".to_string())),
        ..Default::default()
    };
    let anthropic = create_provider(&anthropic_config);
    assert!(anthropic.is_ok());
    assert_eq!(anthropic.unwrap().name(), "anthropic");

    // Ollama (no key needed)
    let ollama_config = AiConfig {
        provider: ProviderType::Ollama,
        ..Default::default()
    };
    let ollama = create_provider(&ollama_config);
    assert!(ollama.is_ok());
    assert_eq!(ollama.unwrap().name(), "ollama");
}

// ===========================================================================
// SC4: AI suggestions are reasonable (geometry-appropriate)
// ===========================================================================

#[test]
fn sc4_overhang_model_gets_supports() {
    // Key SC4 test: overhang model must get support_enabled = true.
    let mesh = overhang_model();
    let provider = SmartMockProvider;
    let suggestion = suggest_profile_sync(&provider, &mesh).unwrap();

    assert!(
        suggestion.support_enabled,
        "SC4: Model with overhangs must get support_enabled=true"
    );
}

#[test]
fn sc4_simple_model_no_supports() {
    // Key SC4 test: simple cube should NOT need supports.
    let mesh = simple_cube();
    let provider = SmartMockProvider;
    let suggestion = suggest_profile_sync(&provider, &mesh).unwrap();

    assert!(
        !suggestion.support_enabled,
        "SC4: Simple cube should not need supports"
    );
}
