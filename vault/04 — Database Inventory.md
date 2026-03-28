---
tags: [nav/databases, progressive-disclosure/L1]
---

# Database Inventory (12 Databases, 5.9MB)

## Overview

All databases in `data/databases/`. SQL migrations in `migrations/` (11 files). Schema in `config/database.toml`.

| Database | Size | Rows | Status | Migration |
|----------|------|------|--------|-----------|
| evolution_tracking.db | 3.6MB | 19,803 fitness | DATA | 011 |
| workflow_tracking.db | 280KB | 25 records | DATA | 010 |
| service_tracking.db | 260KB | 13 services | DATA | 001 |
| security_events.db | 256KB | — | SCHEMA | 009 |
| consensus_tracking.db | 248KB | 82 votes | DATA | 004 |
| hebbian_pulse.db | 240KB | 2 pulses | DATA | 003 |
| flow_state.db | 224KB | 5 states | DATA | 008 |
| system_synergy.db | 212KB | 40 pairs | DATA | 002 |
| performance_metrics.db | 204KB | 7 samples | DATA | 007 |
| episodic_memory.db | 192KB | 25 episodes | DATA | 005 |
| tensor_memory.db | 164KB | 6 snapshots | DATA | 006 |
| remediation_log.db | 0B | — | EMPTY | — |

## Quick Access

```bash
# Integrity check all
for db in data/databases/*.db; do
  echo "$(basename $db): $(sqlite3 $db 'PRAGMA integrity_check;')"
done

# Schema inspection (ALWAYS do this before writing SQL)
sqlite3 data/databases/service_tracking.db ".schema"

# Service status
sqlite3 -header -column data/databases/service_tracking.db \
  "SELECT name, status, health_status FROM services;"

# Synergy scores
sqlite3 -header -column data/databases/system_synergy.db \
  "SELECT system_1 || ' <-> ' || system_2, ROUND(synergy_score,1)||'%' FROM system_synergy ORDER BY synergy_score DESC;"
```

---

See [[HOME]] | Schema details: `../config/database.toml` | Migrations: `../migrations/`
