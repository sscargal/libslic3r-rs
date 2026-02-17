# Phase 8: AI Integration - Research

**Researched:** 2026-02-17
**Domain:** LLM provider abstraction, geometry feature extraction, structured output parsing
**Confidence:** HIGH

## Summary

Phase 8 builds the AI integration layer for libslic3r-rs: a new `slicecore-ai` crate that provides provider-agnostic LLM access, mesh geometry feature extraction for LLM consumption, and an end-to-end profile suggestion pipeline. The crate depends on existing infrastructure -- `slicecore-mesh` already provides `MeshStats` (bounding box, volume, surface area, manifold checks), `slicecore-engine` has a comprehensive `PrintConfig` with TOML/serde support, and the support system has overhang detection logic that can inform geometry analysis.

The Rust LLM ecosystem has matured significantly. For a project that values control, minimal dependencies, and WASM-excludability, the recommended approach is a **custom thin abstraction layer built on `reqwest` + `serde_json`** rather than depending on a third-party LLM crate. The OpenAI, Anthropic, and Ollama REST APIs are simple enough (POST JSON, parse JSON response) that the abstraction cost is low, while the benefit is complete control over the provider trait, structured output parsing, and prompt template management. This matches the project's "pure Rust, no heavy dependencies" philosophy and the architecture doc's design for `slicecore-ai` as a Layer 4 crate.

All three target providers (OpenAI, Anthropic, Ollama) now support structured JSON output, which is critical for reliably parsing LLM responses into `PrintConfig` fields. The geometry analysis builds on existing `MeshStats` and extends it with overhang area estimation, thin wall detection, and aspect ratio computation -- all achievable from the existing `TriangleMesh` data.

**Primary recommendation:** Build a custom `slicecore-ai` crate with a `reqwest`-based thin HTTP layer, an `AiProvider` async trait for provider abstraction, and structured JSON output parsing to convert LLM responses directly into `PrintConfig` overrides.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| reqwest | 0.12.x | HTTP client for LLM API calls | De facto Rust HTTP client, 300M+ downloads, WASM-compatible (when needed), async-first |
| tokio | 1.x | Async runtime | Required by reqwest, ecosystem standard since async-std discontinued |
| serde | 1.x (workspace) | Serialization for API request/response types | Already in workspace |
| serde_json | 1.x (workspace) | JSON parsing for LLM API communication | Already in workspace |
| thiserror | 2.x (workspace) | Error types for AI module | Already in workspace |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| secrecy | 0.8 or 0.10 | API key handling without accidental logging | Wrap API keys in `SecretString` to prevent `Debug`/`Display` leakage |
| url | 2.x | URL construction for provider base URLs | Type-safe URL handling for OpenAI/Anthropic/Ollama endpoints |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom reqwest layer | `async-openai` (1.8k stars, v0.32) | Mature OpenAI-only client, but locks to OpenAI API shape; doesn't cover Anthropic/Ollama natively |
| Custom reqwest layer | `llm` crate (v1.3.7, 308 stars) | Multi-provider but young, no formal releases on GitHub, adds large dependency tree |
| Custom reqwest layer | `rig-core` | Framework-level, too opinionated for a library that wants minimal surface area |
| Custom reqwest layer | `ollama-rs` (986 stars, v0.3.4) | Good Ollama-only client, but Ollama also supports OpenAI-compatible endpoint |

**Decision: Custom reqwest layer.** The three target APIs (OpenAI chat completions, Anthropic messages, Ollama chat) are simple REST endpoints. A custom layer gives full control over the provider trait design, keeps dependencies minimal, and avoids coupling to any third-party crate's breaking changes. The total HTTP interaction code for all three providers is roughly 200-400 lines.

**Installation:**
```bash
# Add to slicecore-ai/Cargo.toml
cargo add reqwest --features json,rustls-tls
cargo add tokio --features rt,macros
cargo add secrecy
cargo add url
# serde, serde_json, thiserror are workspace deps
```

## Architecture Patterns

### Recommended Crate Structure
```
crates/slicecore-ai/
├── Cargo.toml
└── src/
    ├── lib.rs           # Public API re-exports
    ├── error.rs         # AiError enum
    ├── config.rs        # AiConfig, ProviderType enum
    ├── provider.rs      # AiProvider trait definition
    ├── types.rs         # CompletionRequest, CompletionResponse, Message
    ├── providers/
    │   ├── mod.rs
    │   ├── openai.rs    # OpenAI chat completions provider
    │   ├── anthropic.rs # Anthropic messages API provider
    │   └── ollama.rs    # Ollama chat API provider
    ├── geometry.rs      # GeometryFeatures extraction from TriangleMesh
    ├── prompt.rs        # Prompt template construction
    ├── profile.rs       # LLM response -> PrintConfig parsing
    └── suggest.rs       # End-to-end: mesh -> features -> LLM -> PrintConfig
```

### Pattern 1: Provider Trait with Async Methods
**What:** A trait-based abstraction where each LLM provider implements the same interface.
**When to use:** Always -- this is the core pattern enabling provider-agnostic operation.
**Example:**
```rust
// Provider trait using native async fn in trait (Rust 1.75+)
// Use async_trait only if dyn dispatch is needed
use async_trait::async_trait;

#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Send a completion request and get a response.
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse, AiError>;

    /// Return the capabilities of this provider.
    fn capabilities(&self) -> ProviderCapabilities;

    /// Return a human-readable name for this provider.
    fn name(&self) -> &str;
}

pub struct CompletionRequest {
    pub system_prompt: String,
    pub messages: Vec<Message>,
    pub temperature: f32,
    pub max_tokens: u32,
    pub response_format: Option<ResponseFormat>,
}

pub struct CompletionResponse {
    pub content: String,
    pub model: String,
    pub usage: Usage,
    pub finish_reason: FinishReason,
}

pub struct Message {
    pub role: Role,
    pub content: String,
}

pub enum Role {
    System,
    User,
    Assistant,
}

pub enum ResponseFormat {
    Json,
    JsonSchema(serde_json::Value),
}
```

### Pattern 2: Geometry Feature Extraction
**What:** Extract meaningful 3D printing features from a mesh for LLM consumption.
**When to use:** Before sending any profile suggestion request to the LLM.
**Example:**
```rust
use slicecore_mesh::{TriangleMesh, MeshStats, compute_stats};
use slicecore_math::{BBox3, Vec3};
use serde::{Serialize, Deserialize};

/// Geometry features extracted from a mesh, formatted for LLM consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeometryFeatures {
    // From existing MeshStats
    pub bounding_box: BoundingBoxInfo,
    pub volume_mm3: f64,
    pub surface_area_mm2: f64,
    pub triangle_count: usize,
    pub is_watertight: bool,

    // New analysis
    pub dimensions: Dimensions,
    pub aspect_ratio: f64,            // max_dim / min_dim
    pub overhang_ratio: f64,          // fraction of surface facing downward > 45deg
    pub thin_wall_ratio: f64,         // fraction of model with thin features
    pub max_overhang_angle: f64,      // steepest downward-facing angle
    pub has_bridges: bool,            // horizontal unsupported spans
    pub has_small_features: bool,     // features < 1mm
    pub estimated_print_difficulty: PrintDifficulty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimensions {
    pub width_mm: f64,   // X extent
    pub depth_mm: f64,   // Y extent
    pub height_mm: f64,  // Z extent
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrintDifficulty {
    Easy,      // No overhangs, no thin walls, simple geometry
    Medium,    // Some overhangs or thin features
    Hard,      // Significant overhangs, bridges, thin walls
}
```

### Pattern 3: Prompt Template Construction
**What:** Build structured prompts from geometry features for LLM profile suggestion.
**When to use:** Converting GeometryFeatures into a prompt that yields valid PrintConfig.
**Example:**
```rust
pub fn build_profile_prompt(features: &GeometryFeatures) -> CompletionRequest {
    let system_prompt = r#"You are a 3D printing expert. Given geometry analysis of a 3D model,
suggest optimal FDM print settings. Respond ONLY with a JSON object matching this schema:
{
  "layer_height": <number 0.05-0.3>,
  "wall_count": <integer 1-6>,
  "infill_density": <number 0.0-1.0>,
  "support_enabled": <boolean>,
  "support_overhang_angle": <number 30-80>,
  "perimeter_speed": <number 20-100>,
  "infill_speed": <number 30-150>,
  "nozzle_temp": <number 180-260>,
  "bed_temp": <number 0-110>,
  "brim_width": <number 0-10>,
  "reasoning": "<brief explanation of choices>"
}
Do not include any text outside the JSON object."#;

    let user_content = serde_json::to_string_pretty(features).unwrap();

    CompletionRequest {
        system_prompt: system_prompt.to_string(),
        messages: vec![Message {
            role: Role::User,
            content: format!("Analyze this 3D model and suggest print settings:\n\n{}", user_content),
        }],
        temperature: 0.3,  // Low temp for consistent structured output
        max_tokens: 1024,
        response_format: Some(ResponseFormat::Json),
    }
}
```

### Pattern 4: Provider Factory
**What:** Construct the appropriate provider from configuration.
**When to use:** At initialization when the user specifies their provider choice.
**Example:**
```rust
pub fn create_provider(config: &AiConfig) -> Result<Box<dyn AiProvider>, AiError> {
    match config.provider {
        ProviderType::OpenAi => {
            let api_key = config.api_key.as_ref()
                .ok_or(AiError::MissingApiKey("OpenAI"))?;
            Ok(Box::new(OpenAiProvider::new(
                api_key.clone(),
                config.model.clone(),
                config.base_url.clone(),
                config.timeout,
            )))
        }
        ProviderType::Anthropic => {
            let api_key = config.api_key.as_ref()
                .ok_or(AiError::MissingApiKey("Anthropic"))?;
            Ok(Box::new(AnthropicProvider::new(
                api_key.clone(),
                config.model.clone(),
                config.timeout,
            )))
        }
        ProviderType::Ollama => {
            let base_url = config.base_url.clone()
                .unwrap_or_else(|| Url::parse("http://localhost:11434").unwrap());
            Ok(Box::new(OllamaProvider::new(
                base_url,
                config.model.clone(),
                config.timeout,
            )))
        }
    }
}
```

### Anti-Patterns to Avoid
- **Hard-coding provider logic in the suggestion pipeline:** Always go through the `AiProvider` trait. Never match on provider type in the profile suggestion code.
- **Embedding API keys in code or config files:** Use environment variables or runtime-provided secrets. Never serialize API keys.
- **Trusting LLM output without validation:** Always parse and validate the JSON response against expected ranges. LLMs can produce out-of-range values or malformed JSON despite structured output mode.
- **Blocking the async runtime:** All HTTP calls must be async. Use `tokio::runtime::Runtime::block_on()` only at the CLI entry point, never inside library code.
- **Making the AI crate a hard dependency of slicecore-engine:** Use feature flags (`ai = ["slicecore-ai"]`) so the core slicer builds without any AI/network dependencies.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP client | Custom TCP/TLS | `reqwest` | TLS, connection pooling, retries, WASM compat |
| JSON schema validation | Custom validator | `serde_json` deserialization with `#[serde(default)]` | Compile-time type safety, fallback defaults |
| API key protection | String wrapper | `secrecy::SecretString` | Prevents accidental Debug/Display of secrets |
| Retry with backoff | Custom retry loop | `reqwest` built-in or `tokio::time::sleep` loop | Rate limiting, exponential backoff |
| URL construction | String concatenation | `url::Url` | Proper escaping, path joining, query params |

**Key insight:** The LLM API surface is deceptively simple (POST JSON, get JSON back), but the edge cases around timeouts, rate limiting, malformed responses, and secret handling add up. Use battle-tested crates for these concerns.

## Common Pitfalls

### Pitfall 1: LLM Response Parsing Fragility
**What goes wrong:** LLM returns slightly malformed JSON (trailing comma, markdown code fence, extra text before/after JSON).
**Why it happens:** Even with structured output mode, some models (especially local ones via Ollama) may wrap JSON in markdown backticks or add preamble text.
**How to avoid:** Implement a robust JSON extraction function that: (1) strips markdown code fences, (2) finds the first `{` and last `}`, (3) attempts serde parse, (4) falls back to regex extraction of key fields.
**Warning signs:** Tests pass with GPT-4 but fail with local llama models.

### Pitfall 2: Blocking Async Runtime
**What goes wrong:** Using `reqwest::blocking` or calling `.block_on()` inside async context causes panics.
**Why it happens:** Mixing sync and async code incorrectly. The slicecore-engine is currently synchronous.
**How to avoid:** Keep the AI crate fully async internally. Provide a synchronous wrapper (`suggest_profile_sync`) that creates a `tokio::runtime::Runtime` for callers that don't have an async context. The CLI can use `#[tokio::main]`.
**Warning signs:** "Cannot start a runtime from within a runtime" panic.

### Pitfall 3: Overhang Analysis Performance
**What goes wrong:** Iterating all triangles for overhang angle computation is O(n) per mesh and slow for high-poly models.
**Why it happens:** Naive implementation checks every face normal.
**How to avoid:** The normal vectors are already precomputed in `TriangleMesh`. Overhang analysis is a simple dot product of each face normal against the Z-up vector -- this is fast even for 1M+ triangles. Use `rayon::par_iter` if available.
**Warning signs:** Geometry analysis taking >1s for large meshes.

### Pitfall 4: API Key Exposure in Logs/Errors
**What goes wrong:** API keys appear in debug output, error messages, or serialized config.
**Why it happens:** Using `String` for API keys and deriving `Debug` on config structs.
**How to avoid:** Use `secrecy::SecretString` for all API keys. Implement custom `Debug` on `AiConfig` that redacts the key. Never include the API key in error messages.
**Warning signs:** API key visible in `cargo test` output or log files.

### Pitfall 5: Timeout and Rate Limit Handling
**What goes wrong:** Requests hang forever or fail without retry on rate limits.
**Why it happens:** Not configuring timeouts on the reqwest client, not handling HTTP 429.
**How to avoid:** Set a default timeout (30s) on all requests. Implement exponential backoff for HTTP 429 responses with a maximum of 3 retries. Make timeout configurable via `AiConfig`.
**Warning signs:** Tests hanging in CI, intermittent failures in cloud provider tests.

### Pitfall 6: Profile Suggestion Values Out of Range
**What goes wrong:** LLM suggests `layer_height: 5.0` or `nozzle_temp: 500`.
**Why it happens:** LLMs can hallucinate numeric values, especially with local models.
**How to avoid:** Implement validation/clamping on every field of the parsed response. Define `PrintConfigSuggestion` with bounded ranges that are checked before conversion to `PrintConfig`. Log warnings when clamping occurs.
**Warning signs:** Slicing with AI-suggested profile produces invalid G-code.

## Code Examples

### OpenAI Provider Implementation
```rust
// Source: OpenAI Chat Completions API (https://platform.openai.com/docs/api-reference/chat)
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

pub struct OpenAiProvider {
    client: Client,
    api_key: SecretString,
    model: String,
    base_url: String,
}

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    temperature: f32,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<OpenAiResponseFormat>,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OpenAiResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    model: String,
    usage: OpenAiUsage,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiChoiceMessage,
    finish_reason: String,
}

#[derive(Deserialize)]
struct OpenAiChoiceMessage {
    content: String,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

impl OpenAiProvider {
    pub fn new(api_key: SecretString, model: String, base_url: Option<String>, timeout: std::time::Duration) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .expect("Failed to build HTTP client");
        Self {
            client,
            api_key,
            model,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com".to_string()),
        }
    }
}

#[async_trait::async_trait]
impl AiProvider for OpenAiProvider {
    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse, AiError> {
        let mut messages = vec![OpenAiMessage {
            role: "system".to_string(),
            content: request.system_prompt.clone(),
        }];
        messages.extend(request.messages.iter().map(|m| OpenAiMessage {
            role: match m.role { Role::User => "user", Role::Assistant => "assistant", Role::System => "system" }.to_string(),
            content: m.content.clone(),
        }));

        let response_format = request.response_format.as_ref().map(|_| OpenAiResponseFormat {
            format_type: "json_object".to_string(),
        });

        let body = OpenAiRequest {
            model: self.model.clone(),
            messages,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            response_format,
        };

        let resp = self.client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(self.api_key.expose_secret())
            .json(&body)
            .send()
            .await
            .map_err(AiError::HttpError)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AiError::ProviderError { status: status.as_u16(), body });
        }

        let api_resp: OpenAiResponse = resp.json().await.map_err(AiError::ParseError)?;
        let choice = api_resp.choices.into_iter().next()
            .ok_or(AiError::EmptyResponse)?;

        Ok(CompletionResponse {
            content: choice.message.content,
            model: api_resp.model,
            usage: Usage {
                prompt_tokens: api_resp.usage.prompt_tokens,
                completion_tokens: api_resp.usage.completion_tokens,
            },
            finish_reason: match choice.finish_reason.as_str() {
                "stop" => FinishReason::Stop,
                "length" => FinishReason::Length,
                _ => FinishReason::Other(choice.finish_reason),
            },
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_structured_output: true,
            supports_streaming: true,
            max_context_tokens: 128_000,  // GPT-4o default
        }
    }

    fn name(&self) -> &str { "openai" }
}
```

### Anthropic Provider Request/Response Shape
```rust
// Source: Anthropic Messages API (https://docs.anthropic.com/en/api/messages)
#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,  // "user" or "assistant"
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    id: String,
    content: Vec<AnthropicContent>,
    model: String,
    stop_reason: String,
    usage: AnthropicUsage,
}

#[derive(Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

// Key differences from OpenAI:
// - Auth header: "x-api-key" (not Bearer)
// - Version header: "anthropic-version: 2023-06-01"
// - System prompt: top-level "system" field (not a message)
// - Response: content is an array of blocks, not choices
// - Endpoint: POST https://api.anthropic.com/v1/messages
```

### Ollama Provider Request/Response Shape
```rust
// Source: Ollama API docs (https://docs.ollama.com/api/chat)
#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<serde_json::Value>,  // JSON schema for structured output
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Serialize)]
struct OllamaMessage {
    role: String,  // "system", "user", "assistant"
    content: String,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: u32,  // max tokens equivalent
}

#[derive(Deserialize)]
struct OllamaResponse {
    message: OllamaResponseMessage,
    model: String,
    done: bool,
    total_duration: Option<u64>,
    eval_count: Option<u32>,
    prompt_eval_count: Option<u32>,
}

#[derive(Deserialize)]
struct OllamaResponseMessage {
    role: String,
    content: String,
}

// Key differences:
// - No auth required (local service)
// - Endpoint: POST http://localhost:11434/api/chat
// - stream: false for non-streaming
// - format: JSON schema object (not a type string)
// - Also supports OpenAI-compatible endpoint at /v1/chat/completions
```

### Robust JSON Extraction from LLM Response
```rust
/// Extract JSON from an LLM response that may contain extra text or markdown.
pub fn extract_json(raw: &str) -> Result<serde_json::Value, AiError> {
    // 1. Try direct parse first
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
        return Ok(value);
    }

    // 2. Strip markdown code fences
    let stripped = raw
        .trim()
        .strip_prefix("```json").or_else(|| raw.trim().strip_prefix("```"))
        .and_then(|s| s.strip_suffix("```"))
        .unwrap_or(raw)
        .trim();

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(stripped) {
        return Ok(value);
    }

    // 3. Find first { and last } to extract embedded JSON
    if let (Some(start), Some(end)) = (raw.find('{'), raw.rfind('}')) {
        if start < end {
            let json_str = &raw[start..=end];
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
                return Ok(value);
            }
        }
    }

    Err(AiError::InvalidJsonResponse(raw.to_string()))
}
```

### Geometry Feature Extraction
```rust
use slicecore_math::Vec3;
use slicecore_mesh::{compute_stats, TriangleMesh};

pub fn extract_geometry_features(mesh: &TriangleMesh) -> GeometryFeatures {
    let stats = compute_stats(mesh);
    let aabb = stats.aabb;

    let width = aabb.max.x - aabb.min.x;
    let depth = aabb.max.y - aabb.min.y;
    let height = aabb.max.z - aabb.min.z;
    let dims = [width, depth, height];
    let max_dim = dims.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_dim = dims.iter().cloned().fold(f64::INFINITY, f64::min).max(0.001);

    // Overhang analysis: count faces with downward-facing normals
    let z_up = Vec3::new(0.0, 0.0, 1.0);
    let overhang_threshold = 45.0_f64.to_radians().cos(); // cos(45deg) = 0.707
    let normals = mesh.normals();

    let mut overhang_area = 0.0;
    let mut max_overhang_angle: f64 = 0.0;
    let mut thin_feature_count = 0_usize;

    for (i, normal) in normals.iter().enumerate() {
        let dot = normal.dot(z_up);
        if dot < 0.0 {
            // Downward-facing: overhang angle = acos(-dot)
            let angle = (-dot).acos();
            if angle > 45.0_f64.to_radians() {
                let [v0, v1, v2] = mesh.triangle_vertices(i);
                let edge1 = Vec3::from_points(v0, v1);
                let edge2 = Vec3::from_points(v0, v2);
                let tri_area = edge1.cross(edge2).length() * 0.5;
                overhang_area += tri_area;
                max_overhang_angle = max_overhang_angle.max(angle.to_degrees());
            }
        }
    }

    let overhang_ratio = if stats.surface_area > 0.0 {
        overhang_area / stats.surface_area
    } else {
        0.0
    };

    GeometryFeatures {
        bounding_box: BoundingBoxInfo { min: aabb.min, max: aabb.max },
        volume_mm3: stats.volume.abs(),
        surface_area_mm2: stats.surface_area,
        triangle_count: stats.triangle_count,
        is_watertight: stats.is_watertight,
        dimensions: Dimensions { width_mm: width, depth_mm: depth, height_mm: height },
        aspect_ratio: max_dim / min_dim,
        overhang_ratio,
        thin_wall_ratio: 0.0, // TODO: implement thin wall detection
        max_overhang_angle,
        has_bridges: overhang_ratio > 0.05, // heuristic
        has_small_features: min_dim < 1.0,
        estimated_print_difficulty: classify_difficulty(overhang_ratio, min_dim, height),
    }
}

fn classify_difficulty(overhang_ratio: f64, min_dim: f64, height: f64) -> PrintDifficulty {
    if overhang_ratio > 0.15 || min_dim < 0.5 || height > 150.0 {
        PrintDifficulty::Hard
    } else if overhang_ratio > 0.05 || min_dim < 2.0 || height > 80.0 {
        PrintDifficulty::Medium
    } else {
        PrintDifficulty::Easy
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No structured output from LLMs | OpenAI, Anthropic, Ollama all support JSON schema output | 2024-2025 | Reliable parsing of LLM responses into typed data |
| `async_trait` macro required | Native `async fn` in traits (Rust 1.75+) | Dec 2023 | Simpler code, but still need `async_trait` for `dyn Trait` |
| async-std as alternative runtime | Tokio is sole mainstream runtime (async-std discontinued Mar 2025) | Mar 2025 | Use tokio, no choice to make |
| Anthropic had no structured output | Anthropic structured outputs beta (Nov 2025) | Nov 2025 | Can use same structured output pattern across all 3 providers |
| Ollama required separate API format | Ollama supports OpenAI-compatible endpoint | 2024 | Could use OpenAI provider for Ollama too, but native API gives more control |

**Deprecated/outdated:**
- `async-std`: Discontinued March 2025. Do not use.
- `llm-chain`: Last release was 0.1.1-rc.1, appears unmaintained.
- `rustformers/llm`: Explicitly marked unmaintained on GitHub README.

## Integration with Existing Codebase

### Dependencies on Existing Crates
- **slicecore-mesh**: `TriangleMesh`, `compute_stats`, `MeshStats` for geometry analysis
- **slicecore-math**: `Vec3`, `BBox3`, `Point3` for vector operations in overhang analysis
- **slicecore-engine**: `PrintConfig` as the target output type for profile suggestions

### Feature Flag Design
```toml
# In slicecore-engine/Cargo.toml
[features]
ai = ["dep:slicecore-ai"]

[dependencies]
slicecore-ai = { path = "../slicecore-ai", optional = true }
```

This ensures:
1. Core slicing pipeline builds without AI/network dependencies
2. WASM builds exclude AI (as specified in architecture doc)
3. AI is opt-in per the design docs' feature flag strategy

### Sync/Async Bridge
The existing `Engine` is synchronous. The AI crate needs to provide both:
- `async fn suggest_profile(...)` for async contexts
- `fn suggest_profile_sync(...)` that creates a tokio runtime internally

```rust
/// Synchronous wrapper for non-async callers.
pub fn suggest_profile_sync(
    provider: &dyn AiProvider,
    features: &GeometryFeatures,
) -> Result<ProfileSuggestion, AiError> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| AiError::RuntimeError(e.to_string()))?;
    rt.block_on(suggest_profile(provider, features))
}
```

## Open Questions

1. **Thin wall detection algorithm**
   - What we know: Need to identify regions where wall thickness < 2x extrusion width. The mesh has normals and BVH for raycasting.
   - What's unclear: Optimal sampling strategy. Raycast-based thickness measurement (cast inward from each face, measure distance) vs. simpler heuristics (detect narrow bounding box regions).
   - Recommendation: Start with a heuristic approach (check if bounding box has any dimension < 2mm), then add raycast-based thin wall detection as a follow-up. The heuristic is sufficient for v1 AI suggestions.

2. **Prompt template versioning**
   - What we know: Prompts will evolve as we discover what works best with different models.
   - What's unclear: Should prompts be embedded in code or loaded from config files?
   - Recommendation: Embed default prompts in code (const strings). Provide an optional `prompt_template` field in `AiConfig` for user override. This keeps the simple case simple while allowing customization.

3. **Response validation strictness**
   - What we know: LLMs can return out-of-range values. We need validation.
   - What's unclear: Should we reject invalid suggestions entirely, or clamp to valid ranges?
   - Recommendation: Clamp to valid ranges with warnings, not rejection. The user should see a "best effort" suggestion rather than a failure. Log clamped fields for debugging.

4. **Ollama model detection**
   - What we know: Ollama can list available models via `GET /api/tags`.
   - What's unclear: Should we auto-detect available models or require user to specify?
   - Recommendation: Require model name in config. Optionally provide a `list_models()` method for CLI tooling, but don't make model selection automatic.

5. **Caching strategy**
   - What we know: Architecture doc mentions a cache layer.
   - What's unclear: Cache key strategy (hash geometry features? full mesh hash?).
   - Recommendation: Defer caching to a future phase. The v1 implementation should be stateless. Profile suggestion is not a hot path -- it runs once per model upload.

## Sources

### Primary (HIGH confidence)
- Existing codebase: `slicecore-mesh/src/stats.rs` -- MeshStats, compute_stats (bounding box, volume, surface area, manifold checks)
- Existing codebase: `slicecore-engine/src/config.rs` -- PrintConfig with all slicer settings (Serialize/Deserialize)
- Existing codebase: `slicecore-engine/src/support/detect.rs` -- Overhang detection algorithm (layer-diff + raycast)
- Existing codebase: `slicecore-mesh/src/triangle_mesh.rs` -- TriangleMesh with precomputed normals and BVH
- Architecture doc: `designDocs/02-ARCHITECTURE.md` Section 7 -- AI Integration Architecture (AiProvider trait, AiConfig, use cases)
- API Design doc: `designDocs/03-API-DESIGN.md` -- Feature flags, WASM exclusions, slicecore-ai crate design

### Secondary (MEDIUM confidence)
- [async-openai GitHub](https://github.com/64bit/async-openai) -- v0.32.4, 1.8k stars, 90 releases, actively maintained
- [ollama-rs GitHub](https://github.com/pepperoni21/ollama-rs) -- v0.3.4, 986 stars, comprehensive Ollama API coverage
- [OpenAI Chat Completions API](https://platform.openai.com/docs/api-reference/chat) -- Request/response format
- [Anthropic Messages API](https://docs.anthropic.com/en/api/messages) -- Request/response format, structured outputs beta
- [Ollama API docs](https://docs.ollama.com/api/chat) -- Chat endpoint, structured output support
- [Anthropic Structured Outputs](https://platform.claude.com/docs/en/build-with-claude/structured-outputs) -- Beta as of Nov 2025, JSON schema compliance
- [reqwest crate](https://docs.rs/reqwest) -- v0.12.x, 300M+ downloads, async+WASM-compatible

### Tertiary (LOW confidence)
- [llm crate](https://github.com/graniet/llm) -- v1.3.7, 308 stars, no formal GitHub releases
- [Rust async traits discussion](https://blog.rust-lang.org/2023/12/21/async-fn-rpit-in-traits/) -- Stabilized in 1.75, dyn dispatch still needs async_trait

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - reqwest and tokio are unambiguously the standard choices; the APIs are simple REST endpoints
- Architecture: HIGH - matches existing codebase patterns (trait-based, serde, modular crates) and architecture doc Section 7
- Pitfalls: HIGH - well-known issues from working with LLM APIs (JSON parsing, rate limits, timeouts, hallucinated values)
- Geometry analysis: MEDIUM - overhang analysis is well-understood, but thin wall detection algorithm needs runtime validation

**Research date:** 2026-02-17
**Valid until:** 2026-04-17 (90 days -- LLM APIs evolve but the abstraction layer design is stable)
