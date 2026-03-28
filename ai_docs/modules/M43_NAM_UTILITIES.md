# M43: NAM Utilities — Module Specification

**Module ID:** M43
**Layer:** L1 (Foundation)
**File:** `src/m1_foundation/nam.rs`
**Status:** DEPLOYED (previously unnumbered infrastructure)

---

## Purpose

Core vocabulary types for NAM (Non-Anthropocentric Model) compliance. Provides foundation primitives used across all layers for agent identity, confidence scoring, learning signals, dissent tracking, and outcome recording.

## Key Types

| Type | Kind | Purpose |
|------|------|---------|
| `AgentOrigin` | Enum | Agent identity: Human(@0.A), System, Autonomous |
| `Confidence` | Newtype(f64) | Bounded [0.0, 1.0] confidence score |
| `Dissent` | Struct | Structured dissent record with reason and severity |
| `LearningSignal` | Enum | LTP/LTD/Neutral signal for Hebbian routing |
| `Outcome` | Enum | Success/Failure/Partial/Timeout |
| `HUMAN_AGENT_TAG` | Const | "@0.A" — human agent identifier |
| `LAYER_ID` | Const | Layer identifier for L1 |
| `MODULE_COUNT` | Const | Total module count in L1 |

## Dependencies

- None (foundational — no imports from other modules)

## Dependents

- All layers consume NAM primitives via `crate::m1_foundation::{AgentOrigin, Confidence, ...}`

## Related Documentation

- [NAM Spec](../../ai_specs/NAM_SPEC.md)
- [L1 Foundation](../layers/L01_FOUNDATION.md)
