# Architecture Decision Records

## Index

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| [ADR-001](ADR-001-ai-mode.md) | AI Mode - LLM and GPU/CPU Optimization | Proposed | 2024-01-24 |
| [ADR-002](ADR-002-resource-bridge.md) | Unified Resource Bridge | Proposed | 2024-01-24 |

## Overview

RuVector is evolving from a memory optimizer to a **comprehensive system optimizer** with deep AI workload awareness.

```
┌─────────────────────────────────────────────────────────────────┐
│                    RuVector Evolution                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  v0.1 - v0.2: Memory Optimizer                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  • RAM optimization                                      │   │
│  │  • Process trimming                                      │   │
│  │  • Neural decision engine                                │   │
│  │  • PageRank/MinCut algorithms                           │   │
│  └─────────────────────────────────────────────────────────┘   │
│                            │                                     │
│                            ▼                                     │
│  v0.3+: AI-Aware System Optimizer                               │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  • AI Mode (LLM/SD/Whisper optimization)                │   │
│  │  • GPU/VRAM management                                   │   │
│  │  • CPU-GPU-NPU resource bridge                          │   │
│  │  • KV cache optimization                                 │   │
│  │  • Thermal-aware scheduling                              │   │
│  │  • Runtime integrations (Ollama, vLLM, etc)             │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Vision

**RuVector becomes the essential companion for local AI:**

1. **Detect** - Automatically identify AI workloads
2. **Optimize** - Manage GPU/CPU/NPU resources intelligently
3. **Bridge** - Orchestrate memory across devices
4. **Predict** - Prevent OOM before it happens
5. **Learn** - Improve optimization through usage patterns

## Key Differentiators

| Feature | Generic Optimizers | RuVector AI Mode |
|---------|-------------------|------------------|
| LLM awareness | No | Yes |
| VRAM management | No | Yes |
| KV cache optimization | No | Yes |
| Layer offloading | No | Yes |
| Ollama integration | No | Yes |
| Thermal prediction | Basic | Advanced |
| NPU support | No | Yes |

## Target Users

1. **Local LLM Users** - Running Ollama, llama.cpp, LM Studio
2. **AI Developers** - Testing models locally
3. **Content Creators** - Using Stable Diffusion, ComfyUI
4. **Gamers** - Optimizing GPU for gaming + AI
5. **Power Users** - Maximizing hardware utilization

## Roadmap

```
v0.2.x (Current)
  └── Memory optimization + tray icon

v0.3.0 (Next)
  ├── GPU detection (NVIDIA/AMD/Intel)
  ├── VRAM monitoring
  ├── AI process detection
  └── Basic AI Mode CLI

v0.3.x
  ├── Ollama integration
  ├── KV cache tracking
  └── Model management

v0.4.0
  ├── Resource Bridge
  ├── Workload classification
  ├── Placement engine
  └── Memory tier management

v0.5.0
  ├── vLLM/TGI integration
  ├── Stable Diffusion optimization
  ├── Multi-GPU support
  └── NPU integration

v1.0.0
  ├── Full AI Mode suite
  ├── Production stability
  └── Plugin system
```
