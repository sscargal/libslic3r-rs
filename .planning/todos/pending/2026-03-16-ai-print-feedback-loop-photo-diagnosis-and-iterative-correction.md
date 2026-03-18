---
created: 2026-03-16T18:55:00.000Z
title: AI print feedback loop — photo diagnosis and iterative correction
area: ai
files:
  - crates/slicecore-ai/src/lib.rs
  - crates/slicecore-ai/src/types.rs
  - crates/slicecore-ai/src/geometry.rs
  - crates/slicecore-cli/src/main.rs
---

## Problem

When a print fails or has quality issues, users must manually diagnose the problem (often through trial and error or forum posts), figure out which settings to change, re-slice, and try again. This diagnosis→fix cycle can take days of iterative printing. No slicer closes this loop automatically.

The vision: user prints, photographs the result, feeds photos + description to the slicer's AI, which diagnoses the issue, adjusts settings, re-slices, and asks the user to print again. Repeat until quality is acceptable.

## Common Print Defects to Diagnose

| Category | Defects | Root Causes |
|----------|---------|-------------|
| **Adhesion** | Warping, elephant's foot, lifting corners, spaghetti (total bed failure) | Bed temp, Z-offset, first layer speed, bed surface, enclosure |
| **Extrusion** | Under-extrusion, over-extrusion, inconsistent extrusion, blobs/zits | Flow rate, temp, retraction, filament diameter, clogged nozzle |
| **Layer quality** | Layer shifts, Z-banding, ringing/ghosting, visible layer lines | Belt tension, motor current, acceleration, vibration, Z-rod |
| **Structural** | Cracking, delamination, layer splitting, weak infill bonding | Temp too low, cooling too high, layer adhesion, moisture |
| **Surface** | Stringing, oozing, scarring, pillowing, rough top surface | Retraction, travel speed, top layers count, ironing |
| **Material** | Impurities, moisture artifacts (popping/bubbles), color inconsistency | Filament quality, drying, storage |
| **Mechanical** | Motor wear sounds, belt skip marks, extruder gear grinding | Hardware issues — diagnose but can't fix in software |

## Solution — Three Versions

### v1 (MVP): Post-print photo feedback via CLI

**User workflow:**
```bash
# After printing, user takes photos and provides feedback
slicecore feedback \
  --photos ./print-photos/ \
  --description "Stringing between pillars, slight warping on left corner" \
  --gcode model.gcode \
  --config model-config.toml

# AI analyzes and responds interactively
# > I see stringing between the tall features and corner warping.
# > Q1: What filament brand/type are you using?
# > Q2: Is your printer enclosed?
# > Q3: Did you use a brim?
# ...

# After Q&A, AI produces diagnosis and fix
# > Root cause: Retraction distance too short for bowden setup +
# >   bed temp dropping at corners (no enclosure)
# > Changes: retraction 4mm→6mm, retraction speed 25→45mm/s,
# >   bed temp 55→60°C, add brim width 5mm
# > Re-slicing with updated config...
# > Output: model-v2.gcode
# > Please print and provide feedback again.
```

**Implementation:**
1. **New CLI command**: `slicecore feedback` accepting `--photos`, `--description`, `--gcode`, `--config`
2. **Photo analysis**: Send photos to multimodal LLM (Claude, GPT-4V) with structured prompt:
   - "Analyze these 3D print photos. Identify visible defects from this taxonomy: [defect list]"
   - Include the G-code analysis (from Phase 21) and config used
   - Include printer/material info from profile
3. **Diagnostic Q&A**: AI asks targeted follow-up questions based on initial photo analysis
   - Questions are context-aware (don't ask about enclosure if profile says enclosed)
   - Limited to 3-5 questions to avoid frustrating the user
   - Each answer narrows the diagnosis
4. **Root cause engine**: AI maps defects → likely causes → settings changes
   - Prioritize changes by likelihood and impact
   - Never change more than 3-4 settings at once (scientific method)
   - Explain WHY each change is being made
5. **Auto re-slice**: Apply config changes → re-slice → output new G-code with changelog
6. **Session tracking**: Save feedback history per model so AI can see what was already tried
   - `model-feedback-session.json` tracks: iteration #, photos, diagnosis, changes made, user rating

**Photo analysis prompt structure:**
```
You are analyzing photos of a 3D printed object.

Model: [filename, dimensions, estimated print time]
Printer: [make/model from profile]
Filament: [type, brand, temp used]
Key settings: [layer height, speed, retraction, cooling]

Photos: [attached]
User description: [text]

Tasks:
1. Identify all visible defects (use taxonomy)
2. Rate severity of each (minor/moderate/severe)
3. For each defect, list 2-3 most likely root causes
4. Suggest diagnostic questions to narrow down
5. Propose specific setting changes with rationale
```

### v2: Real-time printer monitoring

**Concept**: Connect to printer cameras and sensors during printing to detect failures as they happen.

**Data sources:**
- **Camera feed**: Bambu X1C/P1S have built-in cameras; OctoPrint supports USB cameras
  - Spaghetti detection (model detached from bed)
  - Layer shift detection (visual offset between layers)
  - Blob/string accumulation
- **Sensor data**: Temperature graphs, flow sensor, vibration sensor (if available)
  - Thermal runaway precursor detection
  - Under-extrusion from flow rate drops
  - Resonance detection from accelerometer data
- **G-code progress**: Compare expected vs. actual position/timing
  - Layer time deviation (cooling issues)
  - Unexpected pauses (filament runout, jam)

**Architecture:**
- `slicecore monitor` daemon that connects to printer API (Bambu MQTT, OctoPrint REST, Moonraker WebSocket)
- Periodic frame capture → AI analysis → alert if issue detected
- Alert options: pause print, notify user, log for post-print analysis

**Privacy consideration**: Camera data stays local by default; cloud analysis opt-in only.

### v3: Real-time G-code compensation (moonshot)

**Concept**: When a problem is detected during printing, modify the remaining G-code in real-time to compensate.

**Examples:**
- **Detected: slight warping starting** → Increase bed temp for remaining layers, add extra first-layer adhesion on nearby layers, reduce part cooling fan
- **Detected: under-extrusion on one section** → Increase flow rate for affected region, slow down to give hotend time to recover
- **Detected: layer shift** → If small enough, offset all remaining layers to match the shift (cosmetic fix, maintains structural integrity)
- **Detected: stringing increasing** → Increase retraction distance for remaining travel moves

**How:**
- Requires streaming G-code delivery (not send-whole-file)
- Works with OctoPrint (line-by-line sending) and Klipper (macro injection)
- Bambu printers send the whole file — would need firmware-level integration or a proxy
- `slicecore compensate` intercepts G-code stream, applies real-time modifications
- Each compensation logged for post-print analysis and future profile improvement

**Constraints:**
- Can only modify future layers, not past
- Some compensations risk making things worse — need conservative defaults
- Hardware limitations (can't fix mechanical issues in software)

## Dependencies

- **Phase 8 (AI integration)**: ✓ Foundation for LLM communication
- **Phase 21 (G-code analysis)**: ✓ Needed to understand what was sliced
- **Multimodal LLM support**: Need to extend slicecore-ai to support image inputs (currently text-only)
- **Network printer discovery** (todo): Needed for v2 camera/sensor connection
- **Headless daemon** (todo): Needed for v2 monitoring mode

## Prioritization

1. **v1 MVP** — achievable now, high user value, extends existing AI crate
2. **v2 monitoring** — significant infrastructure, but transformative for farms
3. **v3 compensation** — research project, requires firmware partnerships
