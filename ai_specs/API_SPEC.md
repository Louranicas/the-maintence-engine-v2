# API Specification

> Technical API specification for The Maintenance Engine v1.0.0

---

## Endpoints Overview

| Port | Protocol | Purpose |
|------|----------|---------|
| 8080 | REST | Main HTTP API |
| 8081 | gRPC | Binary RPC |
| 8082 | WebSocket | Real-time streaming |

---

## REST API (Port 8080)

### Base URL
```
http://localhost:8080/api/v1
```

### Authentication

| Method | Header | Format |
|--------|--------|--------|
| API Key | X-API-Key | `your-api-key` |
| JWT | Authorization | `Bearer <token>` |

### Health Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /health | No | System health |
| GET | /health/layers/{id} | No | Layer health (L1-L6) |
| GET | /health/ready | No | Readiness probe |
| GET | /health/live | No | Liveness probe |

### Status Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /status | Yes | Full system status |
| GET | /status/consensus | Yes | PBFT cluster status |

### Metrics Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /metrics | Yes | Prometheus format |
| GET | /metrics/json | Yes | JSON format |

### Learning Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /learning/pathways | Yes | List pathways |
| GET | /learning/pathways/{id} | Yes | Pathway details |
| POST | /learning/cycle | Yes | Trigger learning |

### Remediation Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /remediation/pending | Yes | Pending actions |
| GET | /remediation/{id} | Yes | Action details |
| POST | /remediation/{id}/approve | Yes | Approve (L2/L3) |
| POST | /remediation/{id}/reject | Yes | Reject action |
| GET | /remediation/history | Yes | Action history |

### Error Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | /errors/classify | Yes | Classify error → 12D tensor |

### Service Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /services | Yes | List services |
| GET | /services/{id} | Yes | Service details |
| GET | /services/{id}/tensor | Yes | Service tensor |

### Consensus Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | /consensus/pending | Yes | Pending proposals |
| POST | /consensus/vote | Yes | Submit vote |
| GET | /consensus/history | Yes | Vote history |

---

## Response Format

### Success
```json
{
  "success": true,
  "data": { ... },
  "meta": {
    "timestamp": "2026-01-28T12:00:00.000Z",
    "request_id": "req-abc123",
    "version": "1.0.0"
  }
}
```

### Error
```json
{
  "success": false,
  "error": {
    "code": "E2001",
    "message": "Description",
    "details": { ... }
  }
}
```

---

## Rate Limits

| Pattern | Limit | Window |
|---------|-------|--------|
| /health/* | 10000 | 60s |
| /metrics/* | 1000 | 60s |
| /learning/* | 100 | 60s |
| /remediation/* | 100 | 60s |
| Default | 1000 | 60s |

### Headers
```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 998
X-RateLimit-Reset: 1706443200
```

---

## Error Codes

| Code | HTTP | Description |
|------|------|-------------|
| E0001 | 400 | Invalid request |
| E0002 | 401 | Auth required |
| E0003 | 403 | Forbidden |
| E0004 | 404 | Not found |
| E0005 | 429 | Rate limited |
| E0006 | 500 | Server error |
| E0007 | 503 | Unavailable |

---

## WebSocket API (Port 8082)

### Connection
```
ws://localhost:8082/ws
```

### Message Types

| Type | Direction | Description |
|------|-----------|-------------|
| subscribe | Client→Server | Subscribe to topic |
| unsubscribe | Client→Server | Unsubscribe |
| event | Server→Client | Event notification |
| heartbeat | Bidirectional | Keep-alive |

### Topics

| Topic | Description |
|-------|-------------|
| health.* | Health events |
| remediation.* | Remediation events |
| consensus.* | Consensus events |
| learning.* | Learning events |
| errors.* | Error events |

### Subscribe
```json
{"type": "subscribe", "topic": "health.*"}
```

### Event
```json
{
  "type": "event",
  "topic": "health.service",
  "data": { "service_id": "synthex", "health": 0.95 },
  "timestamp": "2026-01-28T12:00:00.000Z"
}
```

---

## gRPC API (Port 8081)

### Service Definitions

```protobuf
service MaintenanceEngine {
  rpc GetHealth(Empty) returns (HealthResponse);
  rpc GetServiceState(ServiceId) returns (ServiceState);
  rpc ClassifyError(ErrorRequest) returns (ErrorVector);
  rpc TriggerRemediation(RemediationRequest) returns (RemediationResponse);
  rpc SubmitVote(VoteRequest) returns (VoteResponse);
}
```

### Message Types

```protobuf
message ErrorVector {
  repeated double dimensions = 1; // 12 dimensions
  string category = 2;
  string severity = 3;
}

message ServiceState {
  string service_id = 1;
  string status = 2;
  double health_score = 3;
  ErrorVector tensor = 4;
}
```

---

*Generated: 2026-01-28 | The Maintenance Engine v1.0.0*
