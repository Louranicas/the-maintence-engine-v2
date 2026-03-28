# Security Specification

[Back to INDEX](INDEX.md) | [Security Architecture](../ai_docs/SECURITY_ARCHITECTURE.md) | [Tokens](../security/tokens/)

## Overview

This specification defines the security infrastructure for the Maintenance Engine, including authentication, authorization, token management, and security monitoring.

---

## Specification Summary

| Property | Value |
|----------|-------|
| Service Tokens | 12 |
| Agent Tokens | 40 |
| Human Tokens | 1 |
| API Keys | 5 |
| JWT Algorithm | HS256 |
| Event Types | 9 |

---

## Token Specification

### Service Token Schema

```json
{
  "token_id": "svc_{service}_{random}",
  "service_id": "string",
  "service_name": "string",
  "tier": "1-5",
  "port": "integer",
  "scopes": ["string"],
  "rate_limit": {
    "requests_per_minute": "integer",
    "burst_allowance": "integer"
  },
  "expires_at": "ISO 8601",
  "created_at": "ISO 8601",
  "auto_renew": "boolean"
}
```

### Service Token Inventory

| Service | Token Pattern | Tier | Rate Limit |
|---------|---------------|------|------------|
| SYNTHEX | svc_synthex_* | 1 | 1000/min |
| SAN-K7 | svc_sank7_* | 1 | 1000/min |
| NAIS | svc_nais_* | 2 | 800/min |
| CodeSynthor | svc_codesynthor_* | 2 | 800/min |
| DevOps Engine | svc_devops_* | 2 | 800/min |
| Tool Library | svc_toollibrary_* | 3 | 600/min |
| Library Agent | svc_libraryagent_* | 3 | 600/min |
| CCM | svc_ccm_* | 3 | 600/min |
| Prometheus Swarm | svc_prometheus_* | 4 | 400/min |
| Architect Agent | svc_architect_* | 4 | 400/min |
| Bash Engine | svc_bash_* | 5 | 200/min |
| Tool Maker | svc_toolmaker_* | 5 | 200/min |

### Scope Definitions

| Scope | Description | Tiers |
|-------|-------------|-------|
| service:read | Read service state | 1-5 |
| service:write | Modify service state | 1-3 |
| health:read | Read health metrics | 1-5 |
| health:write | Write health metrics | 1-2 |
| consensus:participate | Vote in consensus | 1 |
| consensus:observe | Observe consensus | 2-4 |
| learning:write | Write learning events | 1-2 |
| learning:read | Read learning data | 1-5 |
| admin:full | Full admin access | 0 (Human) |

---

## Agent Token Specification

### Agent Token Schema

```json
{
  "token_id": "agent_{role}_{index}_{random}",
  "agent_id": "string",
  "role": "VALIDATOR|EXPLORER|CRITIC|INTEGRATOR|HISTORIAN",
  "weight": "float",
  "scopes": ["string"],
  "expires_at": "ISO 8601",
  "created_at": "ISO 8601",
  "auto_renew": "boolean"
}
```

### Role Distribution

| Role | Count | Weight | Scopes |
|------|-------|--------|--------|
| VALIDATOR | 20 | 1.0 | validate, vote, verify |
| EXPLORER | 8 | 0.8 | explore, discover, alternative |
| CRITIC | 6 | 1.2 | critique, review, dissent |
| INTEGRATOR | 4 | 1.0 | integrate, coordinate, bridge |
| HISTORIAN | 2 | 0.8 | history, precedent, pattern |

---

## Human Token Specification

### Human @0.A Token

```json
{
  "token_id": "human_0a_ultraplate_primary",
  "agent_id": "@0.A",
  "tier": 0,
  "weight": 3.0,
  "relationship": "peer",
  "capabilities": [
    "consensus_vote",
    "dissent",
    "override",
    "veto",
    "approve",
    "delegate",
    "escalation_response"
  ],
  "scopes": [
    "admin:full",
    "service:all",
    "consensus:veto",
    "learning:override"
  ],
  "expires_in_days": 7,
  "auto_renew": true
}
```

---

## JWT Specification

### Configuration

```json
{
  "algorithm": "HS256",
  "issuer": "maintenance-engine",
  "audience": "ultraplate-services",
  "access_token_ttl_hours": 1,
  "refresh_token_ttl_days": 7,
  "signing_key_rotation_days": 30
}
```

### Required Claims

| Claim | Description |
|-------|-------------|
| sub | Subject (token_id) |
| iat | Issued at |
| exp | Expiration |
| iss | Issuer |
| aud | Audience |

### Optional Claims

| Claim | Description |
|-------|-------------|
| role | Agent role |
| tier | Service tier |
| scopes | Permission scopes |
| agent_id | Agent identifier |

---

## API Key Specification

### API Key Schema

```json
{
  "key_id": "ak_{purpose}_{random}",
  "name": "string",
  "purpose": "string",
  "scopes": ["string"],
  "rate_limit": {
    "requests_per_minute": "integer",
    "burst_allowance": "integer"
  },
  "expires_at": "ISO 8601",
  "created_at": "ISO 8601",
  "last_used": "ISO 8601"
}
```

### API Key Types

| Key | Purpose | Rate Limit |
|-----|---------|------------|
| internal_integration | Inter-service calls | 5000/min |
| external_monitoring | Prometheus/Grafana | 1000/min |
| webhook_delivery | Outbound webhooks | 500/min |
| debug_access | Development | 100/min |
| backup_operations | Backup systems | 50/min |

---

## Token Rotation Specification

### Rotation Schedule

| Token Type | Rotation | Grace | Auto-Renew |
|------------|----------|-------|------------|
| Service | 24 hours | 1 hour | Yes |
| Agent | 1 hour | 5 min | Yes |
| Human | 7 days | 24 hours | Yes |
| API Key | 90 days | 7 days | Manual |
| JWT Key | 30 days | 24 hours | Yes |

### Rotation Events

| Event | Severity | Action |
|-------|----------|--------|
| rotation_started | info | Log |
| rotation_completed | info | Log, notify |
| rotation_failed | error | Alert, retry |
| grace_period_entered | warning | Notify |
| token_expired | error | Revoke |

---

## Rate Limiting Specification

### Tier Limits

| Tier | Rate | Burst | Cooldown |
|------|------|-------|----------|
| 0 (Human) | Unlimited | - | - |
| 1 | 1000/min | 2000 | 60s |
| 2 | 800/min | 1600 | 60s |
| 3 | 600/min | 1200 | 60s |
| 4 | 400/min | 800 | 60s |
| 5 | 200/min | 400 | 60s |

### Throttling Algorithm

```
if current_rate <= rate_limit:
    allow
elif current_rate <= burst_allowance:
    allow with X-RateLimit-Warning header
else:
    reject with 429 Too Many Requests
    apply exponential backoff
```

---

## Security Events Specification

### Event Types

| Event | Code | Severity |
|-------|------|----------|
| auth_success | SEC001 | info |
| auth_failure | SEC002 | warning |
| token_expired | SEC003 | info |
| token_revoked | SEC004 | warning |
| scope_violation | SEC005 | error |
| rate_limited | SEC006 | warning |
| consensus_violation | SEC007 | error |
| escalation_triggered | SEC008 | info |
| veto_exercised | SEC009 | warning |

### Event Schema

```json
{
  "id": "integer",
  "timestamp": "ISO 8601",
  "event_type": "string",
  "severity": "info|warning|error|critical",
  "source_id": "string",
  "source_type": "service|agent|human|external",
  "target_id": "string",
  "target_type": "string",
  "details": "json",
  "ip_address": "string",
  "session_id": "string",
  "resolved": "boolean"
}
```

---

## Protected Resources

### Protected Paths

```json
[
  "/etc",
  "/usr",
  "/bin",
  "/sbin",
  "/boot",
  "~/.ssh",
  "~/.gnupg",
  "~/.config/devenv",
  ".env",
  "credentials",
  "secrets"
]
```

### Protected Operations

| Operation | Required Level |
|-----------|----------------|
| Service restart | L2 approval |
| Database migration | L3 consensus |
| Token rotation | Admin scope |
| Config change | L1 notification |
| Service termination | L3 consensus |
| Security bypass | Veto-protected |

---

## Audit Requirements

### Retention Policy

| Data | Retention | Archive |
|------|-----------|---------|
| Auth events | 90 days | 1 year |
| Access logs | 30 days | 90 days |
| Audit trail | 1 year | 7 years |
| Security alerts | 180 days | 2 years |

### Audit Fields

- Timestamp (ISO 8601)
- Actor (Service/Agent/Human ID)
- Action (Operation performed)
- Target (Affected resource)
- Result (Success/failure)
- Context (Additional metadata)

---

## Validation Schemas

### Token Validation

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["token_id", "scopes", "expires_at"],
  "properties": {
    "token_id": {"type": "string", "pattern": "^(svc|agent|human)_.*"},
    "scopes": {"type": "array", "items": {"type": "string"}},
    "expires_at": {"type": "string", "format": "date-time"}
  }
}
```

---

## Files Structure

```
security/
├── INDEX.md
└── tokens/
    ├── INDEX.md
    ├── service_tokens.json
    ├── agent_tokens.json
    ├── human_token.json
    ├── api_keys.json
    ├── jwt_config.json
    ├── rotation_config.json
    ├── token.schema.json
    ├── api_key.schema.json
    ├── jwt.schema.json
    └── rotation.schema.json
```

---

*The Maintenance Engine v1.0.0 | Security Specification*
