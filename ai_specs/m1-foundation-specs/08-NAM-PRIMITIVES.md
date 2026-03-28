# NAM Foundation Primitives — nam.rs

> **File:** `src/m1_foundation/nam.rs` | **LOC:** ~707 | **Tests:** 35
> **Role:** Non-Anthropocentric Model vocabulary types — agent identity, confidence, outcomes, learning, dissent

---

## Constants

```rust
pub const HUMAN_AGENT_TAG: &str = "@0.A";    // NAM R5 — human agent identifier
pub const LAYER_ID: &str = "L1";
pub const MODULE_COUNT: u8 = 9;
```

---

## AgentOrigin

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub enum AgentOrigin {
    Human { tag: String },
    Service { service_id: String },
    Agent { agent_id: String, role: AgentRole },
    #[default] System,
}
```

| Factory | Result |
|---------|--------|
| `human()` | `Human { tag: "@0.A" }` |
| `service(id)` | `Service { service_id }` |
| `agent(id, role)` | `Agent { agent_id, role }` |

Traits: `Display` ("Human(@0.A)", "Service(synthex)", "Agent(a1, Critic)", "System")

Conversion: `From<&AgentOrigin> for AgentId` — maps to prefixed format

---

## Confidence

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Confidence {
    pub value: f64,    // [0.0, 1.0]
    pub lower: f64,    // [0.0, 1.0]
    pub upper: f64,    // [0.0, 1.0], lower <= upper
}
```

| Factory | Values |
|---------|--------|
| `certain()` | value=1.0, lower=1.0, upper=1.0 (const fn) |
| `uncertain()` | value=0.5, lower=0.0, upper=1.0 (const fn) |
| `new(v, lo, hi)` | All clamped [0,1]; lo/hi swapped if inverted |

`is_valid()` → true iff all in [0,1] and lower <= upper

Traits: `Display` ("0.700 [0.500, 0.900]"), `Default` (manual → `certain()`)

---

## Outcome

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome { Success, Failure, Partial }
```

Traits: `Display` ("Success", "Failure", "Partial")

Used by L5 Learning for Hebbian LTP/LTD routing:
- `Success` → LTP (potentiation)
- `Failure` → LTD (depression)
- `Partial` → magnitude-scaled signal

---

## LearningSignal

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct LearningSignal {
    pub source: String,
    pub outcome: Outcome,
    pub magnitude: f64,          // [0.0, 1.0]
    pub pathway_id: Option<String>,
}
```

| Factory | Default Magnitude |
|---------|-------------------|
| `success(source)` | 1.0 |
| `failure(source)` | 1.0 |
| `partial(source, magnitude)` | clamped [0,1] |

Builder: `.with_pathway(id)` sets `pathway_id: Some(...)`. All `#[must_use]`.

---

## Dissent (NAM R3)

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Dissent {
    pub agent: AgentOrigin,
    pub target: String,
    pub reasoning: String,
    pub confidence: f64,           // [0.0, 1.0]
    pub alternative: Option<String>,
}
```

```rust
Dissent::new(AgentOrigin::human(), "decision-001", "insufficient data")
    .with_confidence(0.7)          // const fn — manual clamp (not f64::clamp, not const-stable)
    .with_alternative("safer approach")
```

`is_valid()` → confidence in [0.0, 1.0]

Traits: `Display` ("Dissent(Human(@0.A) on 'decision-001': insufficient data [conf=0.70])")

---

## Error Strategy

**nam.rs raises zero errors.** All constructors are infallible with clamping. Validation surfaces are `bool`-returning (`is_valid()`), not `Result`-returning. These are vocabulary types that other modules use when forming error contexts.

---

## NAM Compliance Map

| NAM Req | Type | Mechanism |
|---------|------|-----------|
| R1 SelfQuery | Confidence | `is_valid()` self-inspection |
| R2 HebbianRouting | LearningSignal, Outcome | Success→LTP, Failure→LTD, Partial→magnitude-scaled |
| R3 DissentCapture | Dissent | Agent + reasoning + confidence + alternative |
| R5 HumanAsAgent | AgentOrigin::Human | `HUMAN_AGENT_TAG="@0.A"`, `From<&AgentOrigin> for AgentId` |

---

*NAM Foundation Primitives Spec v1.0 | 2026-03-01*
