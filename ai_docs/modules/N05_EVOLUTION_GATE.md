# N05: Evolution Gate — Module Specification

**Module ID:** N05
**Layer:** L8 (Nexus)
**File:** `src/nexus/evolution_gate.rs`
**Status:** STUB

---

## Purpose

Guards deployments by running mutation testing through the Evolution Chamber before accepting changes. Snapshots field state, creates parameter/code mutations, simulates in a controlled environment (K=1.0, 500 steps, 5 spheres), and accepts only if r_after >= r_baseline, preventing field coherence regressions.

## Key Types

| Type | Kind | Purpose |
|------|------|---------|
| `EvolutionGate` | Trait | Core trait for mutation testing before deployment acceptance |
| `GateDecision` | Enum | Accept / Reject with r_delta justification |
| `MutationTrial` | Struct | Mutation parameters, sphere count, step count, r_baseline |
| `TrialResult` | Struct | r_before, r_after, r_delta, acceptance boolean |
| `EVOLUTION_GATE_MODULE_ID` | Const | "N05" — module identifier |

## Dependencies

- M39 (EvolutionChamber) — runs RALPH mutation trials in isolated evolution environment
- N01 (FieldBridge) — captures r_baseline before mutation and r_after for comparison

## Dependents

- Engine orchestrator — all deployment actions pass through the evolution gate
- M17 (OutcomeRecorder) — records gate accept/reject outcomes for episodic learning

## Related Documentation

- [L8 Nexus Layer](../layers/L08_NEXUS.md)
- [Nexus Specs](../../ai_specs/nexus-specs/)
- [Evolution Gate Protocol](../../CLAUDE.md#evolution-gate)
