---
aliases: [index, start, dashboard]
tags: [nav/root, progressive-disclosure/L1]
---

# Maintenance Engine V2 — Vault Home

> **Status:** SCAFFOLDED | **LOC:** 23,907 cloned | **Target:** 65,000+ | **Port:** 8080

---

## Quick Navigation (L1 — always start here)

| Destination | What you'll find |
|-------------|-----------------|
| [[00 — Project Status]] | Current phase, what's done, what's next |
| [[01 — Architecture Overview]] | 8 layers, 48 modules, dependency DAG |
| [[02 — Build & Quality Gate]] | cargo commands, quality gate chain |
| [[03 — Module Map]] | Every module with status, LOC, tests |
| [[04 — Database Inventory]] | 12 databases, schemas, sizes |
| [[05 — Design Constraints]] | 12 inviolable rules (C1-C12) |

## Deep Dive (L2 — per-layer detail)

| Layer | Note | Status |
|-------|------|--------|
| L1 Foundation | [[L1 — Foundation Layer]] | CLONED (16,711 LOC) |
| L2 Services | [[L2 — Services Layer]] | CLONED (7,196 LOC) |
| L3 Core Logic | [[L3 — Core Logic Layer]] | PENDING |
| L4 Integration | [[L4 — Integration Layer]] | PENDING |
| L5 Learning | [[L5 — Learning Layer]] | PENDING |
| L6 Consensus | [[L6 — Consensus Layer]] | PENDING |
| L7 Observer | [[L7 — Observer Layer]] | PENDING |
| L8 Nexus | [[L8 — Nexus Layer]] | PENDING (NEW) |
| V3 Homeostasis | [[V3 — Homeostasis]] | PENDING |

## Context Development (L2 — pre-coding reference)

> Read these BEFORE starting any implementation. They encode everything learned from 4 codebases.

| Note | Contents |
|------|----------|
| [[Gold Standard Patterns]] | 14 mandatory patterns (P1-P14) from M1+M2+ME v1 |
| [[Anti-Patterns]] | 15 things to NEVER do (A1-A15) with severity + fixes |
| [[Rust Exemplars]] | 12 copy-adaptable code blocks (E1-E12) from ME v1 |
| [[Nexus Exemplars]] | VMS/SVF reference implementations for L8 N01-N06 |
| [[Internet Gold Standards]] | 60+ patterns from 100+ web sources (Axum, Tokio, PBFT, STDP) |

## Reference (L3 — full specs on demand)

| Area | Note |
|------|------|
| NAM Framework | [[NAM Reference]] |
| Tensor Encoding | [[Tensor 12D Reference]] |
| PBFT Consensus | [[PBFT Reference]] |
| Hebbian STDP | [[STDP Reference]] |
| Pattern Library | [[Pattern Library]] |
| Schematics | [[Schematics Index]] |
| ULTRAPLATE Context | [[ULTRAPLATE Context]] |

## Claude Code Integration

| Resource | Path |
|----------|------|
| CLAUDE.md | `../CLAUDE.md` |
| CLAUDE.local.md | `../CLAUDE.local.md` |
| Master Plan | `../SCAFFOLDING_MASTER_PLAN.md` |
| ai_specs/ | `../ai_specs/` (14 system specs + 10 layer spec dirs) |
| ai_docs/ | `../ai_docs/` (37 module docs + 7 layer docs + schematics + 4 context docs) |

---

> [!info] Progressive Disclosure
> This vault is structured in 3 tiers:
> - **L1** (~100 tokens) — Index notes. Always read first. Tagged `progressive-disclosure/L1`.
> - **L2** (~500 tokens) — Layer summaries with module tables. Tagged `progressive-disclosure/L2`.
> - **L3** (unbounded) — Full module specs, code patterns, API details. Tagged `progressive-disclosure/L3`.
>
> Claude Code: Read L1 first. Only descend to L2/L3 when working on that specific layer or module.
