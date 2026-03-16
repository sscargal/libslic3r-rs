---
created: 2026-03-16T19:32:48.250Z
title: AI reliability, transparency, and explainable failure detection
area: ai
files:
  - crates/slicecore-ai/src/types.rs
  - crates/slicecore-ai/src/provider.rs
---

## Problem

Existing AI failure detection in 3D printing (e.g., "Spaghetti Detection" on Bambu printers) is widely viewed as unreliable due to:

1. **High false-positive rates**: Cables, purge towers, wipe towers, and normal print artifacts trigger false detections, leading users to disable the feature entirely.
2. **Unhelpful responses**: Detection systems typically only pause the print with no diagnostic feedback — users learn nothing about what was detected or why.
3. **Black box opacity**: AI decisions are opaque. When the system pauses a print or flags an issue, there is no explanation of what visual evidence led to the conclusion, making it impossible for operators to trust or verify the system.

This erodes trust in AI-assisted printing. Users need:
- **Low false-positive rates** (or at least confidence-gated thresholds)
- **Actionable feedback** when issues are detected (not just "paused")
- **Explainable AI (XAI)** so operators understand why a decision was made

## Solution

### Confidence-gated detection thresholds

Instead of binary detect/ignore, use tiered confidence levels with configurable actions:

| Confidence | Default Action | User sees |
|-----------|---------------|-----------|
| > 95%     | Pause print   | "Severe bed adhesion failure detected — filament detached from bed at layer 23" |
| 70-95%    | Alert only    | "Possible stringing detected (82% confidence) — monitoring for 5 more layers" |
| 40-70%    | Log only      | Entry in print report for post-print review |
| < 40%     | Ignore        | Nothing — below noise floor |

Users can adjust thresholds per detection type (e.g., more aggressive for spaghetti, more lenient for stringing).

### Explainable AI (XAI) output

Every detection should produce a structured explanation:

```json
{
  "detection_id": "det_001",
  "type": "adhesion_failure",
  "confidence": 0.92,
  "explanation": {
    "summary": "Object appears detached from build plate on left side",
    "visual_evidence": [
      "Irregular filament pattern at layers 20-23 inconsistent with expected geometry",
      "Shadow analysis indicates gap between object base and build plate",
      "Comparison with expected layer outline shows 15mm deviation"
    ],
    "ruled_out": [
      "Not a purge tower (no purge tower in this print)",
      "Not a cable (static analysis: no cable-like features in printer profile)"
    ],
    "recommended_action": "Pause and inspect — adhesion failure typically worsens",
    "false_positive_hints": [
      "If this is a brim or raft edge curling, consider raising the confidence threshold for this detection type"
    ]
  }
}
```

### Known false-positive suppression

Maintain a registry of common false-positive sources that can be filtered:
- Printer cables/tubes in camera field of view (profile camera geometry)
- Purge/wipe tower operations (cross-reference with G-code progress)
- Normal retraction strings (distinguish from failure-level stringing)
- Ooze shield / draft shield (known geometry from sliced model)

### User feedback loop for accuracy improvement

When a user dismisses a detection as a false positive, capture that feedback:
```bash
slicecore detection dismiss det_001 --reason "cable in frame"
```

Over time, this builds per-printer false-positive profiles that improve accuracy.

## Dependencies

- **AI print feedback loop** (todo): Shares multimodal vision infrastructure
- **ReAct agent** (todo): XAI output feeds into the agent's reasoning chain
- **Network printer discovery** (todo): Camera access for real-time detection

## Phased implementation

1. **Phase A**: XAI output schema and confidence tiers in detection types
2. **Phase B**: False-positive suppression registry (G-code cross-reference, printer profile camera geometry)
3. **Phase C**: User feedback capture and per-printer accuracy tuning
4. **Phase D**: Integration with ReAct agent for automated response with explainable reasoning
