---
created: 2026-03-16T19:32:48.250Z
title: Privacy-preserving defect detection with DP-HD framework
area: ai
files:
  - crates/slicecore-ai/src/types.rs
  - crates/slicecore-ai/src/provider.rs
---

## Problem

AI-based defect detection (spaghetti detection, layer shift detection, surface quality analysis) requires image data from user prints. In fleet/SaaS/cloud scenarios, this raises serious privacy concerns:

- **Part design IP**: Camera images of prints reveal proprietary part geometry
- **Production data**: Print schedules, volumes, and failure rates are competitively sensitive
- **Regulatory compliance**: Some industries (aerospace, defense, medical) prohibit transmitting part images to external servers

Users face a choice: share data for better AI accuracy, or protect privacy and forego AI-assisted quality control. This shouldn't be a tradeoff.

## Solution

### Differential Privacy-Hyperdimensional Computing (DP-HD) Framework

Research demonstrates that the DP-HD framework achieves 94.43% defect detection accuracy while protecting sensitive part design data through a Signal-to-Noise Ratio (SNR) metric.

### How it works

1. **Hyperdimensional Computing (HDC)**: Encodes image features into high-dimensional binary vectors (10,000+ dimensions). The encoding is inherently lossy — geometric details of the part are destroyed while defect signatures are preserved.

2. **Differential Privacy (DP)**: Adds calibrated noise to the HD vectors before they leave the local device. The SNR metric quantifies exactly how much design information leaks.

3. **Federated learning**: Multiple printer sites train a shared defect model by exchanging only noisy HD vectors — never raw images or part geometry.

### Integration with slicecore

| Component | Location | Privacy boundary |
|-----------|----------|-----------------|
| Image capture + HD encoding | Local (on-device) | Raw images never leave |
| DP noise injection | Local (on-device) | SNR-calibrated noise added |
| Noisy HD vectors → cloud | Network | Only privacy-protected vectors transmitted |
| Model training/inference | Cloud or federated | Operates on protected vectors only |
| Defect result | Returned to local | Defect type + confidence only |

### Practical implications for slicecore

- **Local-first detection**: Run basic defect detection entirely on-device (no cloud needed)
- **Fleet learning**: Print farms can improve shared models without exposing proprietary parts
- **SaaS mode**: Cloud-based quality analytics that customers trust with sensitive IP
- **Compliance**: Auditable privacy guarantees via SNR metric (useful for regulated industries)

### Configuration

```toml
[ai.privacy]
mode = "local"              # "local" | "federated" | "cloud"
dp_epsilon = 1.0            # Differential privacy budget (lower = more private)
hd_dimensions = 10000       # Hyperdimensional encoding size
snr_max = 0.1               # Maximum acceptable SNR (design information leakage)
share_defect_stats = true   # Contribute anonymized defect statistics to fleet model
```

## Dependencies

- **AI reliability/XAI** (todo): Detection system that this framework protects
- **ReAct agent** (todo): Agent that consumes defect detection results
- **HDC runtime**: Need Rust implementation of hyperdimensional computing operations
- **Research reference**: DP-HD framework paper for implementation details

## Phased implementation

1. **Phase A**: Local-only defect detection (no privacy concerns — images stay on device)
2. **Phase B**: HD encoding of defect features in Rust
3. **Phase C**: Differential privacy noise injection with configurable epsilon/SNR
4. **Phase D**: Federated learning protocol for fleet defect model improvement
