---
tags: [reference/nexus, progressive-disclosure/L2]
aliases: [nexus-exemplars, vms-patterns, l8-reference]
---

# Nexus Layer (L8) — Reference Implementations

> Source code exemplars from Vortex Memory System + Sphere Vortex Framework
> Proven algorithms for N01-N06 modules

## Module → Source Mapping

| Module | Purpose | VMS/SVF Source | Key Algorithm |
|--------|---------|---------------|---------------|
| N01 Field Bridge | r-tracking | `vortex/kuramoto.rs` + `coherence_tracker.rs` | Kuramoto order parameter + EMA |
| N02 Intent Router | Service routing | `intent_encoder.rs` + `intent_router.rs` | 12D cosine similarity |
| N03 Regime Manager | K-regime | `swarm/coordinator.rs` | Swarm/Fleet/Armada detection |
| N04 STDP Bridge | Tool learning | `learning/stdp.rs` | LTP/LTD with geometric grounding |
| N05 Evolution Gate | Mutation testing | `evolution/chamber.rs` | RALPH transient Kuramoto eval |
| N06 Morphogenic Adapter | Homeostatic feedback | `learning/homeostatic.rs` | Setpoint control (r_target=0.70) |

## Key Algorithms (Summary)

**N01 — Order Parameter:**
`r = |⟨e^(iφⱼ)⟩| = √(cos²_mean + sin²_mean)` — bounded [0,1] by construction.
EMA smoothing: `r_ema = 0.10·r + 0.90·r_ema`

**N02 — Cosine Routing:**
Each service has a fixed 12D tensor signature. Route intent to service with max cosine similarity ≥ 0.3.

**N03 — K-Regime:**
`K < 1.0` → Swarm (parallel, exploratory)
`1.0 ≤ K < 2.0` → Fleet (coordinated, default)
`K ≥ 2.0` → Armada (synchronized, critical)

**N04 — STDP:**
LTP_RATE=0.05, LTD_RATE=0.02, FLOOR=0.10, CEILING=0.95, MAX_PATHWAYS=500.
Auto-Hebbian: potentiate(A→B) also depresses(B→A).

**N05 — RALPH Gate:**
Create transient Kuramoto (K=1.0, N=5, 500 steps). Compare r_baseline vs r_perturbed. Pass if r_after ≥ r_baseline.

**N06 — Homeostatic:**
r_target=0.70, tolerance=0.05. Below target → IncreaseK. Above target → DecreaseK.

---

**Full reference:** `ai_docs/NEXUS_EXEMPLARS.md` (with complete Rust code from VMS/SVF)

See [[L8 — Nexus Layer]] | [[Rust Exemplars]] | [[Gold Standard Patterns]]
