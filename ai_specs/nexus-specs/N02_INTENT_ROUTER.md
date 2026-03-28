# N02: Intent Router Module Specification

> 12D tensor cosine similarity routing to ULTRAPLATE services

---

## Purpose

Route maintenance intents (remediation actions, health queries, learning tasks) to the most appropriate ULTRAPLATE service using 12D tensor cosine similarity matching.

## Interface

```rust
pub trait IntentRouting: Send + Sync {
    fn encode_intent(&self, description: &str) -> Result<[f64; 12]>;
    fn route(&self, intent: &Intent) -> Result<RouteResult>;
    fn service_tensor(&self, service: &ServiceId) -> Result<[f64; 12]>;
    fn update_service_tensor(&self, service: &ServiceId, tensor: [f64; 12]) -> Result<()>;
}
```

## Routing Algorithm

```
similarity = dot(intent, service) / (|intent| * |service|)
```

Select highest similarity above threshold (0.3). If no match, return error.

## Service Tensor Profiles

| Service | Strong Dimensions | Use Cases |
|---------|-------------------|-----------|
| SYNTHEX | D6 (health), D8 (synergy), D10 (error) | Diagnostics, neural, health |
| SAN-K7 | D0 (service), D3 (deps), D5 (protocol) | Orchestration, modules |
| DevOps | D2 (tier), D7 (uptime), D9 (latency) | Pipelines, deployment |
| ME | D6, D10, D11 | Maintenance, monitoring |
| SVF | D8, D11 | Memory, field, tensor |

## Tensor Contribution

Routing quality metric feeds back into system-wide fitness evaluation.

## Target

~1,000 LOC, 50+ tests
