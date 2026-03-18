---
created: 2026-03-16T19:32:48.250Z
title: TinyML edge monitoring for defect and motor failure detection
area: ai
files:
  - crates/slicecore-ai/src/types.rs
---

## Problem

Cloud-based AI monitoring (sending camera frames to an LLM for analysis) has latency, bandwidth, privacy, and cost limitations. For real-time print monitoring — especially on print farms with dozens of printers — edge inference is essential:

- **Latency**: Cloud round-trip is seconds; a spaghetti failure needs sub-second detection
- **Bandwidth**: Streaming video from 50 printers to cloud is impractical
- **Privacy**: Part geometry visible in camera frames (see DP-HD todo)
- **Cost**: Per-frame LLM inference at 1 fps × 50 printers × 24/7 is expensive
- **Reliability**: Monitoring must work even when internet is down

TinyML models running on standard MCUs or edge devices (Raspberry Pi, ESP32-CAM, Coral TPU) can process camera and sensor data locally with millisecond inference times.

## Solution

### Edge inference capabilities

| Sensor | Model | Detection | Accuracy | Speed |
|--------|-------|-----------|----------|-------|
| **Camera** | YOLOv8-nano | Spaghetti, warping, layer shift, bed adhesion failure | ~91.7% | 72 fps on edge TPU |
| **Accelerometer** | 1D CNN | Ringing/resonance, belt skip, motor stall | ~95% | Real-time on MCU |
| **Microphone** | Audio classifier | Motor bearing wear, extruder grinding, filament snap | ~88% | Real-time on MCU |
| **Thermistor** | Anomaly detector | Thermal runaway precursor, heater failure | ~99% | Trivial on MCU |

### Architecture

```
Printer Hardware Layer:
  Camera → ESP32-CAM / RPi → YOLOv8-nano → defect detection
  Accelerometer → MCU → 1D CNN → vibration anomaly
  Microphone → MCU → audio classifier → mechanical failure
  Thermistors → Klipper/Marlin → anomaly thresholds

Edge Aggregation Layer:
  All sensor results → Edge coordinator (RPi / SBC)
  → Fuse multi-sensor signals → confidence-weighted decision
  → Alert / pause / log

Cloud Layer (optional):
  Edge summaries → slicecore cloud/SaaS
  → Fleet dashboards, trend analysis, model retraining
  → No raw images/audio leave the edge
```

### Multi-sensor fusion

Single sensors have blind spots. Fusing multiple signals dramatically improves reliability:

| Scenario | Camera alone | + Accelerometer | + Audio |
|----------|-------------|-----------------|---------|
| Spaghetti detection | 91% (false positives from cables) | 93% (vibration confirms detachment) | 95% (sound pattern of loose filament) |
| Layer shift | 85% (subtle at small shifts) | 98% (belt skip has distinct vibration signature) | 97% (stepper skip has audible click) |
| Nozzle clog | 60% (under-extrusion visible late) | 70% (extruder motor current changes) | 90% (grinding sound is immediate) |
| Bed adhesion loss | 95% (visually obvious) | 80% (part wobble creates vibration) | 70% (scraping sound) |

### Audio signatures of failure modes

A novel detection approach — printers have distinct audio fingerprints:

| Sound | Meaning | Action |
|-------|---------|--------|
| Regular stepper hum | Normal operation | None |
| Clicking/skipping | Stepper losing steps (belt, motor current) | Pause + alert |
| Grinding | Extruder gear on filament (clog, tangle) | Pause + alert |
| High-pitched whine | Bearing wear, fan failure | Log for maintenance |
| Popping/crackling | Moisture in filament | Alert (reduce temp or dry filament) |
| Silence where expected | Motor/heater failure | Emergency stop |

### Integration with slicecore

slicecore doesn't run on MCUs, but it can:

1. **Generate monitoring profiles**: Based on the sliced G-code, generate expected behavior patterns for the edge monitor:
   ```bash
   slicecore monitor-profile model.gcode --output monitor.json
   # → Expected layer times, retraction count per layer,
   #   travel patterns, temperature schedule
   ```

2. **Consume edge alerts**: Receive defect alerts from edge devices and correlate with G-code position:
   ```bash
   slicecore monitor --printer 192.168.1.50 --edge-device 192.168.1.51
   # → Receives alerts → maps to G-code layer/position → suggests corrective action
   ```

3. **Model training data**: Export annotated print data for training/fine-tuning edge models

4. **Firmware companion**: Provide recommended Klipper macros and sensor configurations for edge monitoring setup

### Hardware targets

| Platform | Cost | Capabilities | Use case |
|----------|------|-------------|----------|
| ESP32-CAM | $5 | Camera + WiFi, minimal ML | Basic spaghetti detection |
| Raspberry Pi + Camera | $50 | Full YOLOv8, multi-sensor | Complete monitoring station |
| Coral USB Accelerator | $25 | TPU for fast inference | Add to existing RPi setup |
| XIAO ESP32S3 Sense | $13 | Camera + mic + IMU + WiFi | All-in-one edge sensor |

## Dependencies

- **AI reliability/XAI** (todo): Edge detections should produce explainable outputs
- **ReAct agent** (todo): Edge alerts feed into the autonomous correction loop
- **Network printer discovery** (todo): Discovering edge monitoring devices on the network
- **Digital twin** (todo): Simulated sensor data for training edge models

## Phased implementation

1. **Phase A**: G-code monitoring profile export (expected behavior patterns for edge devices)
2. **Phase B**: Edge alert consumption and G-code correlation in slicecore
3. **Phase C**: Reference YOLOv8-nano model for camera-based defect detection (separate repo)
4. **Phase D**: Audio/vibration classifier models (separate repo)
5. **Phase E**: Multi-sensor fusion coordinator
