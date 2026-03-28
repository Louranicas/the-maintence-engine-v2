---
tags: [reference/ultraplate, progressive-disclosure/L3]
---

# ULTRAPLATE Context

> ME V2 operates within the 14-service ULTRAPLATE ecosystem.

## Service Map

| Service | Port | Relationship to ME V2 |
|---------|------|----------------------|
| SYNTHEX | 8090 | Brain — receives health data, sends patterns |
| DevOps Engine | 8081 | Hebbian pulse source, pipeline orchestration |
| SAN-K7 | 8100 | Orchestrator — spawns child services |
| CodeSynthor V7 | 8110 | Code synthesis engine |
| NAIS | 8101 | Neural intelligence layer |
| Bash Engine | 8102 | Command execution |
| Tool Maker | 8103 | Dynamic tool creation |
| CCM | 8104 | Context management |
| Tool Library | 8105 | Tool registry (ME registers tools here) |
| SVF | 8120 | Cognitive substrate |
| VMS | 8120 | Vortex memory (Nexus source patterns) |

## DevEnv Commands

```bash
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml start
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml status
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml stop
```

## Health Endpoints

```bash
curl http://localhost:8080/api/health   # ME
curl http://localhost:8081/health       # DevOps
curl http://localhost:8090/api/health   # SYNTHEX
curl http://localhost:8100/health       # SAN-K7
curl http://localhost:8105/health       # Tool Library
curl http://localhost:8120/health       # SVF/VMS
```

## Reference Codebases

| Need | Path |
|------|------|
| ME v1 | `../the_maintenance_engine/` |
| DevOps v2 | `../devops_engine_v2/` |
| VMS | `../vortex-memory-system/` |
| SVF | `../sphere_vortex_framework/` |

---

See [[HOME]]
