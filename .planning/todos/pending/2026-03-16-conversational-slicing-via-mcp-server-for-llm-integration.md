---
created: 2026-03-16T19:32:48.250Z
title: Conversational slicing via MCP server for LLM integration
area: ai
files:
  - crates/slicecore-ai/src/lib.rs
  - crates/slicecore-ai/src/types.rs
  - crates/slicecore-engine/src/engine.rs
---

## Problem

Current AI integration (Phase 8) treats the slicer as an AI *consumer* — it calls out to LLMs for profile suggestions. But the more powerful paradigm is making the slicer an AI *tool provider* via the Model Context Protocol (MCP). This enables LLMs like Claude or GPT-4o to directly invoke slicer operations as tools, creating a "conversational CAD/slicing" workflow.

Users could dictate complex, multi-step commands in natural language:
- "Optimize this part for a 10kg load while minimizing support material"
- "Slice this vase in spiral mode, but use 3 perimeters on the base for strength"
- "Compare PETG vs. ASA for this outdoor bracket — show me the tradeoffs"
- "The last print had stringing — fix it and re-slice"

Without MCP, implementing this requires building a custom NLP layer inside the slicer. With MCP, the slicer exposes structured tools and the LLM handles all natural language understanding natively.

## Solution

### MCP Server for slicecore

Expose slicecore operations as MCP tools that any MCP-compatible LLM client can invoke:

```json
{
  "tools": [
    {
      "name": "slice_model",
      "description": "Slice a 3D model with given configuration",
      "parameters": { "model_path": "string", "profile": "string", "overrides": "object" }
    },
    {
      "name": "analyze_model",
      "description": "Analyze a mesh for printability issues",
      "parameters": { "model_path": "string" }
    },
    {
      "name": "compare_materials",
      "description": "Compare two material profiles for a given model",
      "parameters": { "model_path": "string", "material_a": "string", "material_b": "string" }
    },
    {
      "name": "adjust_profile",
      "description": "Modify specific settings in a print profile",
      "parameters": { "profile": "string", "changes": "object" }
    },
    {
      "name": "estimate_print",
      "description": "Estimate time, material usage, and cost for a slice",
      "parameters": { "model_path": "string", "profile": "string" }
    },
    {
      "name": "diagnose_defect",
      "description": "Analyze a print defect description and suggest fixes",
      "parameters": { "symptoms": "string", "current_profile": "string" }
    }
  ],
  "resources": [
    {
      "name": "available_profiles",
      "description": "List all available printer/material/quality profiles"
    },
    {
      "name": "printer_capabilities",
      "description": "Current printer specs and constraints"
    }
  ]
}
```

### Architecture

```
User ←→ LLM (Claude/GPT) ←→ MCP Protocol ←→ slicecore MCP Server ←→ slicecore engine
```

The MCP server is a thin layer over the existing slicecore library API. It:
1. Exposes engine operations as MCP tools
2. Provides printer/profile state as MCP resources
3. Returns structured results the LLM can reason about
4. Runs as `slicecore mcp-server` (stdio transport) or over SSE for remote use

### Implementation

New crate `slicecore-mcp` or module in `slicecore-cli`:
- Uses the `rmcp` or similar Rust MCP SDK
- Maps MCP tool calls to existing slicecore API functions
- Handles file paths, streaming progress, and result formatting
- Supports both stdio (for Claude Desktop/IDE integration) and SSE transports

### Use cases enabled

1. **IDE integration**: Connect slicecore to Claude Code or Cursor — ask AI to slice while coding parametric models
2. **Voice control**: Any voice assistant with MCP support can drive the slicer
3. **Batch operations**: LLM can orchestrate complex multi-step workflows (analyze → optimize → slice → compare)
4. **Farm management**: LLM manages a fleet of printers via slicecore MCP tools
5. **Education**: Students interact with slicing concepts through natural conversation

## Decision: Should we build this?

### Arguments for

1. **Differentiation**: No other slicer has an MCP server. This positions slicecore as the AI-native slicer.
2. **Ecosystem leverage**: MCP adoption is exploding — Claude Desktop, Cursor, Windsurf, VS Code, and more all support MCP. We get integrations with all of them for free.
3. **Low marginal cost**: The MCP server is a thin wrapper over our existing library API. Most of the work is already done — we just need to expose it.
4. **SaaS enabler**: An MCP server over SSE is essentially a hosted API. This directly feeds the monetization strategy.
5. **Community growth**: Developers building AI workflows can integrate slicecore via MCP without learning our API — they just describe what they want in natural language.

### Arguments against

1. **Maintenance burden**: MCP protocol is still evolving. Early adoption means tracking spec changes.
2. **Security surface**: Exposing slicer operations to arbitrary LLM clients requires careful sandboxing (file paths, resource limits).
3. **Dependency risk**: If MCP doesn't win the protocol war, this work is wasted. (Counter: MCP has strong momentum from Anthropic, Google, OpenAI adoption.)
4. **Scope creep**: Could distract from core slicing quality work.

### Verdict

**Build it.** The cost is low (thin wrapper), the upside is high (ecosystem integration, SaaS foundation), and the risk is manageable (optional crate, can be deprecated if MCP fades).

## Packaging: Optional Rust crate

### Crate structure

```
crates/
  slicecore-mcp/          # NEW: Optional MCP server crate
    Cargo.toml
    src/
      lib.rs              # MCP server setup and configuration
      tools.rs            # MCP tool definitions → slicecore API calls
      resources.rs        # MCP resource providers (profiles, printers)
      transport.rs        # stdio + SSE transport handlers
      auth.rs             # API key / token auth for remote access
      sandbox.rs          # File path sandboxing, resource limits
```

### Cargo feature flag approach

```toml
# In slicecore-cli/Cargo.toml
[features]
default = []
mcp = ["dep:slicecore-mcp"]

# Users opt-in:
# cargo install slicecore-cli --features mcp
```

This keeps the MCP server fully optional:
- **Without `mcp` feature**: No MCP dependency, no binary size increase, `slicecore mcp-server` command not available
- **With `mcp` feature**: Adds ~2-5MB to binary, enables `slicecore mcp-server` command

### Rust MCP SDK options

| Crate | Maturity | Notes |
|-------|----------|-------|
| `rmcp` | Early | Rust-native, async, both client + server |
| `mcp-server` | Early | Server-focused, simpler API |
| Custom impl | N/A | MCP is JSON-RPC over stdio/SSE — could hand-roll with `serde_json` + `tokio` |

**Recommendation**: Start with `rmcp` if it's stable enough; fall back to a thin custom implementation over JSON-RPC if needed. The protocol is simple enough that a custom impl is viable.

### Security considerations for remote access (SSE)

When the MCP server runs over SSE (for SaaS or remote use):
- **Auth**: Require API key or OAuth token for every connection
- **File sandboxing**: Restrict file access to a designated workspace directory
- **Rate limiting**: Prevent abuse (max requests/min, max concurrent slices)
- **Resource limits**: Cap memory, CPU time, and output size per request
- **Audit logging**: Log every tool invocation for security review

```toml
# slicecore-mcp config
[mcp.server]
transport = "stdio"          # "stdio" | "sse"
bind = "127.0.0.1:3000"     # For SSE only
auth_required = true         # Require API key for SSE
workspace = "/tmp/slicecore" # Sandboxed file access root
max_concurrent = 4           # Max parallel slice operations
```

## Dependencies

- **Phase 8 (AI integration)**: ✓ Provides the AI provider infrastructure
- **Phase 30 (CLI workflow)**: ✓ CLI profile composition that MCP tools can invoke
- **MCP Rust SDK**: `rmcp` or custom JSON-RPC implementation

## Phased implementation

1. **Phase A**: Core MCP server with slice, analyze, and estimate tools (stdio transport, optional crate)
2. **Phase B**: Profile management and comparison tools
3. **Phase C**: Defect diagnosis and feedback tools
4. **Phase D**: SSE transport with auth for remote/SaaS use
5. **Phase E**: Streaming progress and real-time status resources
6. **Phase F**: Security hardening — sandboxing, rate limits, audit logging
