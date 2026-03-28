---
tags: [reference/pbft, progressive-disclosure/L3]
---

# PBFT Consensus Reference

## Constants

```
N = 40   (total agents)
F = 13   (Byzantine tolerance)
Q = 27   (quorum = 2F + 1)
```

## Protocol Flow

```
1. Client sends request to primary
2. Primary broadcasts PRE-PREPARE to all replicas
3. Each replica broadcasts PREPARE
4. On receiving Q PREPARE: broadcast COMMIT
5. On receiving Q COMMIT: execute and reply
```

## When Consensus Is Required

Only at L3 escalation tier:
- `confidence < 0.7` OR `severity = CRITICAL`
- All L3 actions require 27/40 agreement before execution

## View Change

If the primary fails, M34 handles leader election:
1. Replicas timeout waiting for primary
2. Broadcast VIEW-CHANGE
3. New primary selected (round-robin)
4. New primary broadcasts NEW-VIEW

## Dissent (NAM R3)

M35 captures dissenting votes. Even when quorum is reached, minority opinions are recorded in `consensus_tracking.db` for analysis. This prevents the system from silently suppressing valid concerns.

---

See [[HOME]] | [[L6 — Consensus Layer]] | Full spec: `../ai_specs/PBFT_SPEC.md`
