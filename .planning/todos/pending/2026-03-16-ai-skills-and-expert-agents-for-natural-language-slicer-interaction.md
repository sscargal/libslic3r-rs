---
created: 2026-03-16T19:32:48.250Z
title: AI skills and expert agents for natural language slicer interaction
area: ai
files:
  - crates/slicecore-ai/src/lib.rs
  - crates/slicecore-ai/src/types.rs
  - crates/slicecore-ai/src/provider.rs
  - crates/slicecore-engine/src/engine.rs
---

## Problem

The existing AI integration (Phase 8) provides basic LLM connectivity — send a prompt, get a response. The MCP server (todo) provides a transport for LLMs to invoke slicer tools. But neither provides the **domain expertise layer**: specialized AI skills and agents that deeply understand 3D printing and can guide users through complex workflows via natural language.

Users want to chat with the slicer like talking to an expert:
- "I'm new to 3D printing. Walk me through setting up my Ender 3 and printing my first model."
- "This gear keeps breaking at the teeth. What should I change?"
- "I need to print 200 phone cases by Friday. Help me plan the batch."
- "Why is my Voron printing differently from my Prusa with the same settings?"

This requires more than raw LLM intelligence — it requires **structured domain knowledge, multi-step reasoning workflows, and tool orchestration** packaged as reusable AI skills and agents.

## Concepts

### Skills vs. Agents

| Concept | What it is | Example |
|---------|-----------|---------|
| **Skill** | A focused capability with a specific prompt template, tool access, and knowledge context | "Diagnose print defect from description" |
| **Agent** | An autonomous workflow that orchestrates multiple skills and tools to achieve a goal | "Onboard new user: detect printer → suggest profile → slice test model → guide first print" |

Skills are building blocks. Agents compose skills into workflows.

### Skill catalog

| Skill | Input | Output | Tools used |
|-------|-------|--------|-----------|
| **Profile Advisor** | Model geometry + intent + printer | Recommended settings with rationale | analyze_model, list_profiles |
| **Defect Diagnostician** | Symptoms, photos, current settings | Root cause + specific fixes | diagnose_defect, adjust_profile |
| **Material Selector** | Use case, constraints (strength, heat, cost) | Ranked materials with tradeoffs | compare_materials |
| **G-code Explainer** | G-code file or snippet | Human-readable explanation | gcode_analysis |
| **Cost Optimizer** | Model + constraints (time, material, quality) | Optimized settings + savings estimate | slice_model, estimate_print |
| **Printer Tutor** | User's printer model, experience level | Step-by-step calibration guide | printer_capabilities |
| **Batch Planner** | N models + deadline + printer fleet | Plate layout + schedule + filament needs | arrange, estimate_print |
| **Settings Translator** | Natural language request | Exact config parameters | adjust_profile |
| **Comparison Analyst** | Two configs/materials/orientations | Side-by-side analysis with recommendation | slice_model, estimate_print |
| **Troubleshooter** | "My print failed at layer X" | Diagnosis + fix + prevention | gcode_analysis, diagnose_defect |

### Agent workflows

#### Onboarding Agent
```
User: "I just got a Bambu X1C, what do I do?"

Agent:
1. [Skill: Printer Tutor] → Identifies printer, loads specs
2. [Tool: list_profiles] → Finds X1C profiles
3. [Skill: Profile Advisor] → Suggests starter profile for PLA
4. [Tool: slice_model] → Slices a calibration cube with recommended settings
5. [Skill: Printer Tutor] → Guides user through loading filament, bed leveling
6. Outputs: "Here's your first print file. Let me know how it turns out!"
```

#### Iterative Quality Agent
```
User: "This benchy has stringing and the bow is drooping"

Agent:
1. [Skill: Defect Diagnostician] → Identifies stringing + overhang issues
2. [Skill: Settings Translator] → Maps fixes to config changes
3. [Tool: adjust_profile] → Applies: retraction +0.3mm, bridge speed -10mm/s
4. [Tool: slice_model] → Re-slices with new settings
5. [Tool: estimate_print] → "New print: 47 min (+2 min from changes)"
6. Stores session context for follow-up after next print
```

#### Farm Planning Agent
```
User: "I need 200 phone cases by Friday on 8 printers"

Agent:
1. [Tool: analyze_model] → Assess model, estimate per-part time
2. [Skill: Batch Planner] → Calculate plates, schedule across printers
3. [Skill: Cost Optimizer] → Optimize settings for speed (quality floor met)
4. [Tool: arrange + slice_model] → Generate all plate G-codes
5. Outputs: schedule, filament requirements, risk assessment
```

## Architecture: `slicecore-agent` crate

### Why a separate crate

1. **Optional dependency**: Not everyone needs AI agents — keep core slicer lean
2. **Independent versioning**: Agent skills evolve faster than the core engine
3. **SaaS-ready**: The agent crate is what the SaaS product wraps — it's the product surface
4. **Pluggable**: Users/farms can install only the agents they need

### Crate structure

```
crates/
  slicecore-agent/          # NEW: Optional AI agent crate
    Cargo.toml
    src/
      lib.rs                # Public API: create agents, run skills
      skill/
        mod.rs              # Skill trait and registry
        profile_advisor.rs
        defect_diagnostician.rs
        material_selector.rs
        gcode_explainer.rs
        cost_optimizer.rs
        settings_translator.rs
        batch_planner.rs
        troubleshooter.rs
      agent/
        mod.rs              # Agent trait and orchestration loop
        onboarding.rs
        quality_iteration.rs
        farm_planner.rs
      knowledge/
        mod.rs              # RAG knowledge base interface
        embeddings.rs       # Text embedding for retrieval
        store.rs            # Vector store (SQLite + embeddings)
      chat/
        mod.rs              # Chat session management
        history.rs          # Conversation history + context
        streaming.rs        # Streaming response support
      prompt/
        mod.rs              # Prompt templates and construction
        system.rs           # System prompts per skill/agent
        few_shot.rs         # Few-shot example management
```

### Core traits

```rust
/// A focused AI capability with specific domain knowledge
#[async_trait]
pub trait Skill: Send + Sync {
    /// Unique identifier for this skill
    fn id(&self) -> &str;

    /// Human-readable description
    fn description(&self) -> &str;

    /// What tools this skill needs access to
    fn required_tools(&self) -> &[ToolId];

    /// Execute the skill with given context
    async fn execute(
        &self,
        input: &SkillInput,
        tools: &ToolContext,
        provider: &dyn AiProvider,
    ) -> Result<SkillOutput, AgentError>;
}

/// An autonomous workflow that orchestrates skills and tools
#[async_trait]
pub trait Agent: Send + Sync {
    /// Run the agent with a user message and conversation context
    async fn run(
        &self,
        message: &str,
        context: &mut ConversationContext,
        skills: &SkillRegistry,
        tools: &ToolContext,
        provider: &dyn AiProvider,
    ) -> Result<AgentResponse, AgentError>;
}

/// Chat session that routes to appropriate agents
pub struct ChatSession {
    history: ConversationHistory,
    skills: SkillRegistry,
    agents: AgentRegistry,
    provider: Box<dyn AiProvider>,
    tools: ToolContext,
}

impl ChatSession {
    /// Process a user message and return a response
    pub async fn send(&mut self, message: &str) -> Result<ChatResponse, AgentError> {
        // 1. Classify intent → select appropriate agent/skill
        // 2. Inject relevant knowledge from RAG
        // 3. Execute agent/skill with tool access
        // 4. Stream response back to user
        // 5. Update conversation history
    }
}
```

### Chat interface integration

```bash
# Interactive chat mode
slicecore chat
# > Welcome! I'm your slicing assistant. What are you working on?
# User: I need to print a replacement gear for my dishwasher
# > Great, a functional mechanical part! A few questions:
# > 1. What printer and nozzle size are you using?
# > 2. Do you have the STL already, or do you need help finding one?
# > 3. What filament do you have available?

# Single-shot query
slicecore ask "What's the best infill for a load-bearing bracket?"
# > For load-bearing brackets, I'd recommend...

# Chat with context from a file
slicecore chat --model bracket.stl --profile my-petg.toml
# > I see you're working with bracket.stl (82x45x30mm, 12.3cm³).
# > Current profile: PETG at 0.2mm layers. What would you like to adjust?
```

### Knowledge base (RAG)

The agent crate includes a built-in knowledge base:

```
slicecore-agent/knowledge/
  ├── profiles.json        # Printer/material/settings knowledge
  ├── troubleshooting.json # Defect→cause→fix mappings
  ├── materials.json       # Material properties and recommendations
  ├── printers.json        # Printer capabilities and quirks
  ├── gcode-ref.json       # G-code command reference
  └── best-practices.json  # Community wisdom, tips, gotchas
```

Embedded at build time or downloaded on first use. Updated independently of the crate version.

### Deployment targets

| Target | How agents run | Provider |
|--------|---------------|----------|
| **Local CLI** | `slicecore chat` via terminal | Ollama (local SLM) or cloud API key |
| **Desktop app** | Embedded chat panel | Local or cloud, user's choice |
| **SaaS API** | REST/WebSocket endpoint | Cloud LLM, our API keys, usage-metered |
| **Print farm** | Headless agent daemon | Cloud or self-hosted vLLM |
| **IDE** | Via MCP server → agent crate | User's configured LLM |

### SaaS monetization angle

The agent crate is the key monetization surface:
- **Free tier**: Basic skills (profile advisor, settings translator) via local SLM
- **Pro tier**: Advanced agents (iterative quality, batch planner) + cloud LLM
- **Enterprise tier**: Custom skills, fleet agents, dedicated model instances
- **Per-query pricing**: Each agent invocation costs tokens — metered billing

The core slicer is free/open. The AI agents are the premium product.

## Off-the-shelf vs. fine-tuned models

(See also: SLM strategy todo)

| Approach | For skills | For agents |
|----------|-----------|-----------|
| **Off-the-shelf + RAG** | Works for most skills with good prompt engineering + knowledge base | May struggle with complex multi-step orchestration |
| **Fine-tuned SLM** | Overkill for simple skills | Could improve agent decision-making for slicer-specific workflows |
| **Hybrid** | RAG for knowledge retrieval, fine-tuned for output formatting | Fine-tuned orchestrator + RAG skills |

**Recommendation**: Start with off-the-shelf + RAG. Fine-tune only if specific skills consistently underperform. The knowledge base does most of the heavy lifting.

## Dependencies

- **Phase 8 (AI integration)**: ✓ Provider trait for LLM communication
- **MCP server** (todo): Transport layer that agents can be accessed through
- **SLM strategy** (todo): Model selection for local/cloud deployment
- **slicecore-engine**: Agent tools wrap engine operations

## Phased implementation

1. **Phase A**: `slicecore-agent` crate scaffold — Skill/Agent traits, SkillRegistry, basic chat session
2. **Phase B**: First 3 skills — Profile Advisor, Settings Translator, Defect Diagnostician
3. **Phase C**: RAG knowledge base — build and embed 3D printing knowledge
4. **Phase D**: First agent — Onboarding Agent (multi-step guided workflow)
5. **Phase E**: `slicecore chat` CLI command and streaming responses
6. **Phase F**: Advanced agents — Iterative Quality, Farm Planner
7. **Phase G**: SaaS API wrapper — REST/WebSocket endpoints, usage metering
