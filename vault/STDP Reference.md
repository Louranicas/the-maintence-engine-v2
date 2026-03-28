---
tags: [reference/stdp, progressive-disclosure/L3]
---

# Hebbian STDP Reference

## Constants

```
LTP_RATE            = 0.1    // Long-term potentiation (strengthen)
LTD_RATE            = 0.05   // Long-term depression (weaken)
STDP_WINDOW         = 100ms  // Temporal coincidence window
DECAY_RATE          = 0.1    // Per-tick decay (HRS-001 corrected)
CO_ACTIVATION_DELTA = 0.05   // Per-call increment for co-activated pathways
```

## Learning Rule

```
If pre fires before post (within STDP_WINDOW):
    strength += LTP_RATE * (1.0 - strength)   // potentiate

If post fires before pre (within STDP_WINDOW):
    strength -= LTD_RATE * strength             // depress

Every tick:
    strength *= (1.0 - DECAY_RATE)              // natural decay
```

## Co-Activation (V2)

When two services are called in the same operation:
```
pathway.strength += CO_ACTIVATION_DELTA  // +0.05 per co-activation
```

This enables the system to learn which service combinations are effective (e.g., "when SYNTHEX and SAN-K7 are called together, outcomes improve").

## Pathway Pruning (M28)

Pathways below threshold (typically 0.1) are pruned. Prevents unbounded growth of the pathway graph.

## Cross-Session Persistence

STDP pathways persist in `hebbian_pulse.db`. Pathways from previous sessions are loaded on startup, enabling cross-session learning accumulation.

---

See [[HOME]] | [[L5 — Learning Layer]] | Full spec: `../ai_specs/STDP_SPEC.md`
