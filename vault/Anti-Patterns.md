---
tags: [reference/anti-patterns, progressive-disclosure/L2]
aliases: [anti-patterns, what-not-to-do]
---

# Anti-Patterns — What ME v2 Must Never Do

> 15 anti-patterns compiled from ME v1 bugs, database forensics, and cross-codebase analysis

## Severity Matrix

| ID | Anti-Pattern | Severity | Detection |
|----|-------------|----------|-----------|
| A1 | SystemTime/chrono usage | HIGH | `rg 'SystemTime\|chrono' --type rs` |
| A2 | Reference through RwLock | CRITICAL | Code review |
| A3 | Signal under lock | CRITICAL | Code review |
| A4 | Unbounded collection | HIGH | `rg 'unbounded' --type rs` |
| A5 | `&mut self` trait method | CRITICAL | Compile error |
| A6 | unwrap/expect/panic | HIGH | `clippy::unwrap_used` |
| A7 | Naive float math | MEDIUM | Code review |
| A8 | Float equality `==` | MEDIUM | `clippy::float_cmp` |
| A9 | Missing TensorContributor | HIGH | Compile check |
| A10 | Upward import (layer DAG) | CRITICAL | Compile error |
| A11 | Clippy suppression | MEDIUM | `rg '#\[allow' --type rs` |
| A12 | No constructor validation | MEDIUM | Code review |
| A13 | Stagnant evolution data | HIGH | DB query |
| A14 | Missing field capture (C11) | HIGH | Code review |
| A15 | Missing STDP recording (C12) | MEDIUM | Code review |

## Key Findings from Database Forensics

**A13 — Stagnant Evolution:** ME v1's `evolution_tracking.db` has 19,809 fitness records but **0 correlations** and only 6 emergence events. The system measured everything but learned nothing from measurements.

**Fix:** M39 must populate `correlation_log` every RALPH cycle. M38 must auto-detect emergence from fitness deltas.

## V2-Specific Anti-Patterns (A14, A15)

**A14:** Every L4+ operation must capture `r_before` and `r_after` from Nexus field. Trigger morphogenic adaptation if `|r_delta| > 0.05`.

**A15:** Every cross-service call must record STDP co-activation (`+0.05` per interaction).

---

**Full reference:** `ai_docs/ANTI_PATTERNS.md` (with bad code → fix code for each)

See [[Gold Standard Patterns]] | [[05 — Design Constraints]]
