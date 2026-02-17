# Phase 5: Support Structures - Context

**Gathered:** 2026-02-17
**Status:** Ready for planning

<domain>
## Phase Boundary

Automatic and manual support generation for overhangs and bridges — enabling printable geometry where the model extends beyond the buildable angle. Includes automatic overhang detection, tree supports, bridge handling, manual override system (enforcers/blockers), and interface layers for clean part surfaces. Support removal and surface quality are balanced through configurable parameters.

Out of scope: GUI-based painted region support (deferred to future UI phase), support structure preview/visualization (API/engine focus).

</domain>

<decisions>
## Implementation Decisions

### Support generation strategy
- **Detection approach:** Hybrid — layer-by-layer analysis for speed, downward raycast validation for accuracy on complex geometry
- **Support type selection:** Automatic based on geometry (small contact areas → tree, large areas → traditional) with user override via config to force specific type
- **Overhang angle threshold:** Claude's discretion — research common slicer patterns for sensible defaults
- **Tiny support regions:** Claude's discretion — determine appropriate minimum area filtering or single-point pillar conversion based on printability research

### Tree support architecture
- **Growth direction:** Bottom-up from build plate — start at bed, branch upward toward overhangs for stability
- **Branch merging:** Claude's discretion — research tree support best practices for optimal merging rules (distance threshold with stability checking)
- **Branch structure:** Support both organic (curved, natural-looking) and geometric (angular, easier to slice) tree structures
  - Automatic selection based on model geometry (may need both types in one model)
  - User override to force organic or geometric style
- **Diameter progression:** Implement all taper methods (linear, exponential, load-based)
  - Automatic selection based on model geometry or default to single best option
  - User override to select specific taper method

### Bridge detection and overrides
- **Bridge detection criteria:** Combined approach — angle threshold (<10° from horizontal) + endpoint-based (both ends supported) + minimum span length (>5mm) for accurate classification
- **Bridge-specific settings:** All settings configurable — speed, fan, flow, acceleration, line width
  - Allows full control for advanced users
  - Research common bridge parameter adjustments for smart defaults
- **Manual override system:** Support all non-GUI options in Phase 5
  - **Mesh-based enforcers/blockers:** Import separate STL meshes marking enforce/block regions (PrusaSlicer approach)
  - **Volume modifier approach:** Define 3D volumes (boxes, cylinders, spheres) with enforce/block properties (programmatic and GUI-compatible)
  - **Painted regions:** Deferred to future GUI phase (not supported in CLI/API)
- **Override priority rules:**
  - **Default:** "Warn on conflicts" — manual overrides win, but log warnings when blocking removes critical supports
  - **Optional:** "Smart merge" can be set as default — combine auto + manual intelligently
  - **Interactive:** When conflicts detected, show smart merge results and allow user to accept/decline

### Interface layers and surface quality
- **Interface thickness:** Configurable with smart default (recommend 2 layers = ~0.4mm as default)
- **Interface density:** Configurable ratio between interface and body density
  - Research common ratios (e.g., 100% interface / 15% body vs 50% interface / 15% body)
  - Allow user to specify custom ratio
- **Separation gap:** Configurable with material-specific defaults
  - Different materials have different adhesion properties (PLA vs PETG vs ABS)
  - Default values per material (e.g., PLA = 0.2mm, PETG = 0.25mm)
  - User can override for specific models
- **Quality vs removability balance:** User-selectable quality preset system
  - Low/Medium/High quality presets that adjust multiple interface parameters together
  - Low = easier removal, rougher surface (larger gaps, fewer interface layers)
  - High = best surface quality, harder removal (tighter gaps, more interface layers)
  - Presets provide simple tuning while advanced users can adjust individual parameters

### Claude's Discretion
- Exact overhang angle threshold defaults (research 40-50° range for various materials)
- Minimum support area filtering approach (area threshold vs single-point pillar conversion)
- Tree branch merging rules and distance thresholds
- Interface layer pattern (solid vs sparse vs specific pattern types)
- Default bridge parameter adjustments (speed/fan/flow values)
- Default interface:body density ratios
- Material-specific separation gap distances

</decisions>

<specifics>
## Specific Ideas

- Tree supports should handle models that need both organic and geometric branches in different regions — flexibility is key
- Manual override system must work through API/CLI (no GUI dependency in Phase 5) — programmatic control for automation
- Smart merge conflict resolution provides an interactive way to handle complex override scenarios
- Quality presets simplify the interface layer tuning for users who don't want to tweak individual parameters
- Material-specific defaults acknowledge that PLA, PETG, ABS, etc. have different support removal characteristics

</specifics>

<deferred>
## Deferred Ideas

- **GUI-based painted support regions:** Paint enforce/block areas directly on 3D model surface — requires GUI, deferred to future UI phase
- **Support structure preview/visualization:** 3D rendering of generated supports — useful but GUI-focused, deferred to visualization phase
- **Automatic support material switching:** Multi-material support with different materials for interface vs body — Phase 6 multi-material work
- **Support raft generation:** Raft beneath supports for bed adhesion — could be combined with build plate adhesion features in Phase 6

</deferred>

---

*Phase: 05-support-structures*
*Context gathered: 2026-02-17*
