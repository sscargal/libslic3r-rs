---
created: 2026-03-16T19:32:48.250Z
title: Small language model strategy for local and SaaS deployment
area: ai
files:
  - crates/slicecore-ai/src/provider.rs
  - crates/slicecore-ai/src/providers/ollama.rs
  - crates/slicecore-ai/src/providers/anthropic.rs
  - crates/slicecore-ai/src/providers/openai.rs
  - crates/slicecore-ai/src/config.rs
---

## Problem

The current AI integration (Phase 8) supports Anthropic, OpenAI, and Ollama providers via the `AiProvider` trait. But the strategy for *which models to use* and *how to deploy them* is undefined. Key questions:

1. **Can off-the-shelf SLMs handle 3D printing tasks?** Profile suggestions, defect diagnosis, G-code analysis — do general-purpose small models (Phi-3, Llama 3, Mistral, Gemma) perform adequately, or do we need fine-tuned models?
2. **What's the SaaS cost model?** If every slice calls Claude/GPT-4, the API cost per user per month could be prohibitive. SLMs running on our own infrastructure could reduce per-query cost by 100x.
3. **Local user experience?** Users running Ollama on a laptop need models that fit in 4-8GB VRAM and respond in <5 seconds. Which models meet this bar for our use cases?
4. **Fine-tuning vs. prompting?** Can we get good results with structured prompts + few-shot examples, or do we need domain-specific fine-tuning?

This is critical for monetization — SaaS margins depend on inference cost, and local user experience depends on model quality at small sizes.

## Research areas

### 1. Off-the-shelf SLM evaluation

Test existing small models on slicecore-specific tasks:

| Task | Complexity | Example |
|------|-----------|---------|
| Profile suggestion | Medium | "Given this model geometry and intent, suggest print settings" |
| Defect diagnosis | Medium-High | "Given these symptoms, what settings should change?" |
| Natural language → config | Medium | "Slice at 0.2mm with 20% infill" → TOML config |
| G-code explanation | Low-Medium | "What does this G-code section do?" |
| Material recommendation | Low | "What material for an outdoor bracket?" |
| Printability analysis | Medium | "Will this model print successfully? What issues?" |

**Candidate models to evaluate:**

| Model | Size | VRAM | Local viable? | Notes |
|-------|------|------|--------------|-------|
| Phi-3 Mini (3.8B) | 2.3GB | 4GB | Yes | Microsoft, strong reasoning for size |
| Llama 3.2 (3B) | 2GB | 4GB | Yes | Meta, good general performance |
| Mistral 7B | 4.1GB | 8GB | Marginal | Strong but needs more VRAM |
| Gemma 2 (2B) | 1.5GB | 4GB | Yes | Google, compact |
| Qwen 2.5 (7B) | 4.5GB | 8GB | Marginal | Alibaba, strong coding/reasoning |
| DeepSeek-R1 distills | 1.5-7B | 4-8GB | Yes-Marginal | Reasoning-focused |

**Evaluation methodology:**
1. Create a benchmark dataset of 100+ slicing tasks with ground truth answers
2. Test each model with zero-shot, few-shot, and structured prompting
3. Score on: accuracy, response time, VRAM usage, output format compliance
4. Determine minimum viable model size per task type

### 2. Fine-tuning strategy

If off-the-shelf models fall short, fine-tuning options:

#### Training data sources
- **Print profile databases**: Thousands of printer/material/settings combinations from PrusaSlicer/Cura profiles
- **Community forums**: Reddit r/3Dprinting, Prusa forums — problem descriptions + solutions
- **G-code corpus**: Annotated G-code samples with explanations
- **Defect catalogs**: Defect photos + diagnoses + fixes (from print quality troubleshooting guides)
- **Synthetic data**: Generate training pairs using large models (Claude/GPT-4 as teacher)

#### Fine-tuning approaches

| Approach | Cost | Quality | Maintenance |
|----------|------|---------|-------------|
| **LoRA/QLoRA** | Low ($50-200) | Good for specific tasks | Re-train per model update |
| **Full fine-tune** | High ($500-2000) | Best quality | Expensive to maintain |
| **Distillation** | Medium | Good general + specific | One-time from teacher model |
| **RAG (no fine-tune)** | Lowest | Good if retrieval is strong | Just update knowledge base |

**Recommended approach**: Start with RAG (retrieval-augmented generation) using a knowledge base of print settings, troubleshooting guides, and material data. Only fine-tune if RAG + few-shot prompting proves insufficient.

### 3. Deployment architecture

#### Local users (Ollama/LMStudio/vLLM)

```
User's machine:
  slicecore CLI → Ollama API (localhost:11434) → Phi-3 Mini (4GB VRAM)

  First run:
    slicecore ai setup --local
    # → Downloads recommended model via Ollama
    # → Runs benchmark to verify performance
    # → Configures optimal model for user's hardware
```

**Auto-detection of hardware capabilities:**
```rust
fn recommend_local_model(available_vram_gb: f64) -> &str {
    match available_vram_gb {
        v if v >= 16.0 => "llama3.2:70b-q4",   // Best quality
        v if v >= 8.0  => "mistral:7b-q4",      // Good balance
        v if v >= 4.0  => "phi3:mini-q4",        // Minimum viable
        v if v >= 2.0  => "gemma2:2b-q4",        // Ultra-compact
        _ => "none"                               // CPU-only or cloud
    }
}
```

#### SaaS deployment

```
slicecore SaaS:
  Tier 1 (Free): Rate-limited cloud SLM (Phi-3/Llama on our GPUs)
  Tier 2 (Pro):  Faster SLM + priority queue
  Tier 3 (Enterprise): Claude/GPT-4 for complex tasks, SLM for simple ones

  Routing:
    Simple tasks (config translation, material lookup) → SLM ($0.0001/query)
    Complex tasks (defect diagnosis, optimization) → Large model ($0.01/query)
```

**Cost comparison:**

| Provider | Model | Cost per 1K tokens | 100 queries/user/month |
|----------|-------|-------------------|----------------------|
| Anthropic | Claude Sonnet | $3/$15 (in/out) | ~$2.00/user/month |
| OpenAI | GPT-4o | $2.50/$10 | ~$1.50/user/month |
| Self-hosted | Llama 3.2 7B | ~$0.10/$0.10 | ~$0.02/user/month |
| Self-hosted | Phi-3 Mini | ~$0.05/$0.05 | ~$0.01/user/month |

**75-100x cost reduction** with self-hosted SLMs — critical for SaaS margins.

#### Hybrid routing (smart model selection)

```rust
/// Route AI requests to the most cost-effective model
fn route_request(request: &CompletionRequest) -> ProviderConfig {
    match request.task_complexity() {
        Complexity::Simple => local_slm(),           // Config translation
        Complexity::Medium => cloud_slm(),           // Profile suggestion
        Complexity::Complex => cloud_large_model(),  // Defect diagnosis with photos
    }
}
```

### 4. RAG knowledge base

Instead of (or in addition to) fine-tuning, build a retrieval-augmented generation system:

```
Knowledge base (embedded + indexed):
  ├── print_profiles/       # 5000+ printer/material/settings combos
  ├── troubleshooting/      # 500+ defect→fix mappings
  ├── materials/            # Material properties database
  ├── printer_specs/        # Printer capabilities and constraints
  ├── gcode_reference/      # G-code command documentation
  └── community_wisdom/     # Curated tips from forums/guides
```

**RAG pipeline:**
1. User query → embed with small embedding model (all-MiniLM, nomic-embed)
2. Retrieve top-K relevant documents from knowledge base
3. Inject retrieved context into SLM prompt
4. SLM generates answer grounded in retrieved facts

This gives even a 3B model access to expert-level 3D printing knowledge without fine-tuning.

### 5. Evaluation framework

Build an automated eval suite to compare models and track regression:

```bash
# Run evaluation suite
slicecore ai eval --models phi3,llama3,mistral --tasks all
# → Profile suggestion: phi3=72%, llama3=81%, mistral=85%
# → Config translation: phi3=95%, llama3=96%, mistral=97%
# → Defect diagnosis:  phi3=45%, llama3=62%, mistral=71%
# → Conclusion: Mistral 7B recommended for local, Phi-3 viable for simple tasks
```

## Existing infrastructure

The `AiProvider` trait in `provider.rs` already supports multiple backends:
- `anthropic.rs` — Cloud large models
- `openai.rs` — Cloud large/small models
- `ollama.rs` — Local models via Ollama

Adding SLM support requires no architectural changes — just model selection logic and RAG infrastructure.

## Dependencies

- **Phase 8 (AI integration)**: ✓ Provider system already built
- **Ollama provider**: ✓ Already implemented
- **Embedding model**: Need for RAG vector search (can use Ollama embedding models)
- **Vector store**: Need lightweight vector DB (SQLite + embedding, or purpose-built like qdrant)
- **Evaluation dataset**: Need to create benchmark tasks with ground truth

## Phased implementation

1. **Phase A**: Benchmark off-the-shelf SLMs on slicecore tasks — determine baseline quality
2. **Phase B**: Build RAG knowledge base with print profiles + troubleshooting data
3. **Phase C**: Implement smart model routing (complexity-based provider selection)
4. **Phase D**: Auto-detect hardware and recommend local model in `slicecore ai setup`
5. **Phase E**: Fine-tune (LoRA) if RAG proves insufficient for specific tasks
6. **Phase F**: SaaS deployment with tiered model access
