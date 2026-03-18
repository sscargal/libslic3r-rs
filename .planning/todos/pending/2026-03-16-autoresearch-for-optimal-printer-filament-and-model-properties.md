---
created: 2026-03-16T19:45:00.000Z
title: Autoresearch for optimal printer, filament, and model properties
area: ai
files:
  - crates/slicecore-ai/src/lib.rs
  - crates/slicecore-engine/src/engine.rs
  - crates/slicecore-engine/src/config.rs
  - crates/slicecore-engine/src/cost_model.rs
---

## Problem

Finding optimal print settings is a high-dimensional search problem. A typical print profile has 50+ parameters (temperature, speed, retraction, cooling, infill, etc.) that interact in complex ways. Users currently tune these through:
- Trial and error (slow, wasteful)
- Community forum advice (generic, not printer-specific)
- Manufacturer defaults (conservative, not optimal)

What if an AI agent could autonomously explore the parameter space and converge on optimal settings — the way Karpathy's `autoresearch` autonomously optimizes neural network architectures?

## Karpathy's Autoresearch Pattern

**Core loop** (from [github.com/karpathy/autoresearch](https://github.com/karpathy/autoresearch)):

1. AI agent modifies a single file (parameters/code)
2. Run fixed-duration experiment (5 minutes)
3. Evaluate result with a quantitative metric (val_bpb)
4. Decide: keep changes or revert
5. Repeat — ~100 iterations overnight

**Key design principles**:
- Single file modification scope (keeps diffs reviewable)
- Fixed time budget (fair comparison across experiments)
- Quantitative evaluation metric (no subjective judgment)
- Agent guided by `program.md` instructions (human sets strategy, AI executes)

## Adapting Autoresearch for 3D Printing

### The challenge: what's the "validation metric"?

In ML, val_bpb is computed in seconds. In 3D printing, the true quality metric requires physically printing and inspecting the result — which takes hours, not minutes.

**Solution: proxy metrics that don't require physical printing.**

### v1/MVP: Slicer-Computable Proxy Metrics (No Simulation)

The slicer itself can compute quality proxies without a full physics simulation:

```
┌──────────────────────────────────────────────────┐
│               Autoresearch Loop (v1)              │
│                                                   │
│  1. Agent modifies print_config.toml              │
│  2. Slicer runs: slicecore slice model.stl        │
│  3. Evaluate proxy metrics from G-code analysis   │
│  4. Score = weighted combination of metrics       │
│  5. Keep or revert → repeat                       │
│                                                   │
│  Time per iteration: ~10-30 seconds (slicing)     │
│  Iterations per hour: ~120-360                    │
│  Overnight: ~1,000-4,000 experiments              │
└──────────────────────────────────────────────────┘
```

**Proxy metrics computable from G-code (no printing required):**

| Metric | What it measures | How to compute | Lower is better? |
|--------|-----------------|----------------|-------------------|
| Print time | Speed efficiency | G-code time estimate | ✓ |
| Filament used | Material efficiency | G-code extrusion total | ✓ |
| Retraction count | Stringing risk | Count G1 E-negative moves | ✓ |
| Travel distance | Oozing/stringing risk | Sum non-extrusion moves | ✓ |
| Min layer time | Cooling adequacy | Min time for any layer | Higher = better |
| Support volume | Support waste | Calculate support extrusion | ✓ |
| Max overhang unsupported | Print risk | Analyze geometry vs. support | ✓ |
| Speed variance | Consistency | Std dev of print speeds | ✓ |
| Extrusion width consistency | Quality uniformity | Analyze flow rate variations | ✓ |
| Estimated cost | Cost efficiency | Cost model (Phase 31) | ✓ |

**Composite score** (configurable weights based on user priority):

```
score = w_time × normalized_time
      + w_material × normalized_filament
      + w_quality × (retraction_penalty + travel_penalty + layer_time_bonus)
      + w_risk × overhang_penalty
      + w_cost × normalized_cost
```

**User-selectable optimization targets**:
```bash
slicecore autoresearch model.stl --printer X1C --filament PLA \
  --optimize speed         # Minimize time, accept quality tradeoffs
  --optimize quality       # Minimize retractions/travel, accept slower
  --optimize cost          # Minimize material + energy
  --optimize balanced      # Default balanced weights
  --optimize "speed:0.5,quality:0.3,cost:0.2"  # Custom weights
```

### Parameter search space

Not all 50+ parameters should be searched. Define a focused search space:

**Tier 1: High impact, safe to vary** (always search)
- Print speed (20-300 mm/s per feature type)
- Temperature (material range ±15°C)
- Layer height (0.08-0.32mm)
- Infill density (5-50%)
- Infill pattern (from available set)
- Retraction distance (0.2-6mm depending on setup)
- Retraction speed (20-80 mm/s)
- Fan speed per feature (0-100%)

**Tier 2: Medium impact** (search if user opts in)
- Perimeter count (1-5)
- Top/bottom layers (2-8)
- Seam position strategy
- Support density and pattern
- First layer settings

**Tier 3: Rarely varies** (don't search — from printer profile)
- Nozzle diameter, bed size, firmware type

### Search strategies

**Strategy 1: Random search with pruning**
- Sample random configs from the parameter space
- Evaluate proxy score
- Keep top 10%, perturb to generate next generation
- Simple, embarrassingly parallel

**Strategy 2: Bayesian optimization (Gaussian Process)**
- Build a surrogate model of score vs. parameters
- Use acquisition function (Expected Improvement) to pick next experiment
- Converges faster than random search on smooth landscapes
- Use `argmin` Rust crate for optimization

**Strategy 3: LLM-guided search (true autoresearch pattern)**
- AI agent reasons about results: "Temperature increase improved layer adhesion proxy but increased stringing proxy → try higher retraction to compensate"
- More sample-efficient than blind search
- Can incorporate domain knowledge (e.g., "PETG typically needs less cooling than PLA")
- Produces human-readable research log

### v2: Physical Validation Loop

After v1 converges on promising candidates via proxy metrics, validate the top 3-5 configs by actually printing:

```
v1 automated search (1000+ iterations, proxy metrics)
  → Top 5 candidates
    → User prints each (calibration cube or target model)
      → User rates quality (1-5) or provides photos
        → AI refines understanding of proxy-to-reality mapping
          → Better proxy weights for next search
```

This creates a feedback loop where physical printing data improves the proxy metrics over time.

### v3: Simulated Validation (No Physical Printing)

Replace physical printing with simulation-based validation:

1. **Thermal field prediction** (ties to U-Net todo): Predict warping risk from thermal analysis
2. **Mechanical simulation**: Predict part strength from infill + perimeter analysis
3. **Visual quality prediction**: Train ML model on print photos to predict quality from G-code features
4. **Extrusion simulation** (ties to PFEM todo): Predict actual line width and layer bonding

With simulation, the entire loop runs autonomously:
```
Agent modifies config → Slice → Simulate → Score → Repeat
100% automated, no human in the loop
```

## Implementation Architecture

```
slicecore-autoresearch/
├── src/
│   ├── agent.rs          # LLM agent that modifies configs
│   ├── evaluator.rs      # Proxy metric computation from G-code
│   ├── search.rs         # Search strategies (random, Bayesian, LLM-guided)
│   ├── program.rs        # Load program.md instructions (Karpathy pattern)
│   ├── journal.rs        # Experiment log (every iteration recorded)
│   └── report.rs         # Human-readable research report generation
```

**program.md** (user-configurable research instructions):
```markdown
# Autoresearch Program: Optimize PLA on X1C

## Objective
Find the fastest print settings for PLA on Bambu X1C that maintain
acceptable quality (retraction count < 50, no unsupported overhangs > 60°).

## Constraints
- Temperature: 195-225°C (PLA safe range)
- Speed: 30-300 mm/s (X1C capable range)
- Layer height: 0.12-0.28mm (0.4mm nozzle)
- Must use Arachne perimeters (non-negotiable)

## Strategy
Start with manufacturer defaults. Prioritize speed increases.
Only increase temperature if layer adhesion proxy degrades.
```

## Dependencies

- **Phase 8 (AI)**: ✓ LLM agent for reasoning about results
- **Phase 19 (Statistics)**: ✓ G-code analysis for proxy metrics
- **Phase 21 (G-code analysis)**: ✓ Detailed G-code metric extraction
- **Phase 31 (Cost model)**: ✓ Cost estimation as an optimization metric
- **Thermal prediction** (todo): Enables v3 simulated validation
- **PFEM squish** (todo): Enables v3 extrusion quality prediction

## Research references

- Karpathy, "autoresearch" (2025) — autonomous ML research framework
- Shahriari et al., "Taking the Human Out of the Loop: Bayesian Optimization" (2016) — Bayesian optimization survey
- Snoek et al., "Practical Bayesian Optimization of ML Algorithms" (2012) — Gaussian Process BO
