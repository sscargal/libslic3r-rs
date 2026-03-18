---
created: 2026-03-16T19:00:00.000Z
title: ReAct agent framework for autonomous print correction
area: ai
files:
  - crates/slicecore-ai/src/lib.rs
  - crates/slicecore-ai/src/provider.rs
  - crates/slicecore-ai/src/types.rs
---

## Problem

Current print monitoring is reactive and binary: detect failure → pause/stop. The user must then manually diagnose and adjust. This wastes prints, time, and material. What if the slicer could autonomously observe, reason about, and correct print issues in real-time — turning the slicer into a self-learning manufacturing agent?

## The ReAct Framework for 3D Print Manufacturing

The ReAct (Reasoning and Acting) pattern from LLM agent research maps perfectly to print monitoring. Instead of hard-coded "if stringing → stop" rules, an LLM agent runs a continuous observe→reason→plan→act cycle:

### The ORPA Cycle

```
┌─────────────────────────────────────────────────────────┐
│                    ORPA Agent Loop                       │
│                                                         │
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌───┐ │
│  │ OBSERVE  │───▶│  REASON  │───▶│   PLAN   │───▶│ACT│ │
│  └──────────┘    └──────────┘    └──────────┘    └───┘ │
│       ▲                                           │     │
│       └───────────────────────────────────────────┘     │
│                   (next observation)                     │
└─────────────────────────────────────────────────────────┘
```

**Observe**: Computer vision analyzes camera frame → identifies defect type, severity, location.
- "Severe stringing detected between objects at layers 45-52"
- "Minor elephant's foot on first 3 layers, left side"
- "No defects detected — print quality nominal"

**Reason**: LLM queries current printer state + its training knowledge to analyze root cause.
- "Stringing typically caused by insufficient retraction or high nozzle temperature. Current: 215°C, retraction 0.5mm. Bowden setup suggests retraction should be 2-6mm. The 0.5mm retraction is likely the primary cause."
- Cross-references: filament type (PLA needs less retraction than PETG), printer type (direct drive vs. bowden), current speed, travel distance between objects.

**Plan**: Formulate specific corrective action with safety bounds.
- "Increase retraction to 0.8mm (conservative step — max safe for this hotend is 2mm). Reduce temperature to 205°C (within PLA safe range 190-220°C). Do NOT change both simultaneously — change retraction first, observe for 5 layers, then adjust temperature if needed."
- Plans include rollback criteria: "If issue worsens within 3 layers, revert retraction to 0.5mm."

**Act**: Execute corrections via printer API.
- Send M207 (set retraction) or modify G-code stream
- Communicate via Klipper macros, OctoPrint API, or Bambu MQTT
- Log every action with timestamp, reason, and expected outcome

### Manufacturing Commentary / Traceability Log

Every ORPA cycle produces a structured log entry:

```json
{
  "cycle": 47,
  "timestamp": "2026-03-16T14:23:45Z",
  "layer": 52,
  "observation": {
    "source": "camera_frame_4712",
    "defects": [
      {
        "type": "stringing",
        "severity": "moderate",
        "location": "between_objects_A_B",
        "confidence": 0.87
      }
    ]
  },
  "reasoning": "Stringing at moderate severity between tall separate objects. Current retraction 0.5mm is below recommended minimum for bowden setup (2mm). Temperature 215°C is within range but on the high side for this PLA.",
  "plan": {
    "actions": [
      {"type": "set_retraction", "from": 0.5, "to": 0.8, "unit": "mm"},
      {"type": "set_retraction_speed", "from": 25, "to": 40, "unit": "mm/s"}
    ],
    "deferred_actions": [
      {"type": "reduce_temperature", "from": 215, "to": 210, "condition": "if stringing persists after 5 layers"}
    ],
    "rollback_trigger": "stringing severity increases to 'severe' within 3 layers",
    "rollback_actions": [
      {"type": "set_retraction", "to": 0.5},
      {"type": "pause_print", "reason": "automated correction failed"}
    ]
  },
  "action_result": {
    "executed": ["set_retraction:0.8mm", "set_retraction_speed:40mm/s"],
    "method": "klipper_macro",
    "success": true
  }
}
```

This log serves multiple purposes:
- **Industrial traceability**: Every print has a complete manufacturing record
- **Certification support**: Auditable evidence of process control for regulated industries
- **Self-learning corpus**: Historical logs train the agent to make better decisions over time
- **User education**: Human-readable commentary explains what happened and why

### Self-Learning System

Over time, the agent builds a knowledge base per printer+material combination:
1. **Successful corrections** reinforce the reasoning path → higher confidence next time
2. **Failed corrections** update priors → avoid repeating ineffective adjustments
3. **Cross-printer patterns** (SaaS/fleet) reveal material-specific or printer-model-specific tendencies
4. **Diminishing interventions**: A well-tuned printer+profile needs fewer corrections → agent learns optimal baseline settings

## Implementation in libslic3r-rs

### Core agent module (new crate or module in slicecore-ai)

```
slicecore-ai/
├── src/
│   ├── agent/
│   │   ├── mod.rs           # ReAct agent loop
│   │   ├── observer.rs      # Defect detection from images
│   │   ├── reasoner.rs      # LLM reasoning with printer context
│   │   ├── planner.rs       # Action planning with safety bounds
│   │   ├── actor.rs         # Printer API action execution
│   │   ├── commentary.rs    # Manufacturing log generation
│   │   └── knowledge.rs     # Self-learning knowledge base
```

### Key design decisions

1. **Safety bounds are hard-coded, not AI-decided**: Max temperature, max retraction, min flow rate — these are non-negotiable limits from the printer profile. The AI reasons within bounds, never outside them.
2. **Conservative by default**: Single-variable changes, small increments, always with rollback criteria.
3. **Human override**: User can always override or disable the agent. Every action is logged before execution with a configurable approval delay.
4. **Offline fallback**: When LLM is unavailable, fall back to rule-based corrections (less intelligent but still useful).
5. **Provider-agnostic**: Uses existing slicecore-ai provider system (Anthropic, OpenAI, Ollama).

### Printer communication layer

| Firmware | Protocol | Capabilities |
|----------|----------|-------------|
| Klipper | Moonraker WebSocket | Full: macros, param changes, pause/resume, camera |
| Marlin | OctoPrint REST + serial | Good: G-code injection, temp changes, camera via plugin |
| Bambu | MQTT + LAN | Limited: can send commands, camera access, but less G-code flexibility |
| RepRap | Direct serial | Basic: raw G-code only, no camera unless OctoPrint |

### Relationship to other todos

- **AI print feedback loop** (todo): v1 (post-print photos) is a stepping stone to this. The ReAct agent is the real-time evolution of that concept.
- **Network printer discovery** (todo): Agent needs to discover and connect to printers.
- **Headless daemon** (todo): Agent runs as a long-lived process alongside the daemon.

## Phased implementation

1. **Phase A**: Observer only — camera feed → defect detection → alert user (no action)
2. **Phase B**: Observer + Reasoner — detect + diagnose + suggest (user approves actions)
3. **Phase C**: Full ORPA — autonomous correction with safety bounds and manufacturing commentary
4. **Phase D**: Self-learning — knowledge base accumulation, cross-printer patterns
