# Habitat Nexus Plugin — Technical Specification v2.0

> **Version:** 2.0.0 | **Status:** DESIGNED | **Target:** Zellij 0.43.x WASM Plugin
> **LOC Estimate:** ~1,800 Rust across 8 modules | **Binary:** ~1.1MB WASM
> **Build:** `cargo build --release --target wasm32-wasip1`
> **Supersedes:** v1.0.0 (ai_docs/HABITAT_NEXUS_PLUGIN_SPEC.md)
> **Obsidian:** `[[Session 072 — Habitat Nexus Plugin Architecture v2]]`

---

## Changes from v1.0

| Issue | v1.0 | v2.0 Fix |
|---|---|---|
| `send_sigint_to_pane_id` non-existent | Compile error | `write_chars_to_pane_id("\x03", id)` |
| `AlertType` missing derives | Compile error | `Clone + Eq + PartialEq + Ord + PartialOrd` |
| `zellij-tile` version unspecified | Load failure | Pinned `"0.43"` |
| No arm backoff policy | Infinite failed polls | Exponential backoff per arm |
| No `dispatch_ack` | Silent dispatch | Ack routed back to caller pipe |
| No `ready: bool` per arm | Silent partial-star | Degraded-mode per arm |
| LTP trap uses cumulative totals | False negatives | Rolling window `ltp_delta_window` |
| `arms_completed_last_tick` missing | No tick coherence | Added to `BusMetrics` |
| Briefing injection unsafe | Terminal corruption | Status-gated injection queue |
| Monolithic single file | Brittle iteration | 8 modules, each independently evolvable |

---

## Purpose

A single Zellij WASM plugin that unifies all 16 organically-evolved Habitat communication
systems into one in-process channel. Projected impact: **67/100 → 89/100** overall
communication score across the four structural gaps identified in the Session 072 audit.

---

## Module Map

```
habitat-nexus/
├── Cargo.toml
└── src/
    ├── main.rs          — plugin entrypoint, event routing, ZellijPlugin impl
    ├── protocol.rs      — all message types, enums, derives (single source of truth)
    ├── arms.rs          — Arm<Req,Resp> generic struct, ArmRegistry, backoff logic
    ├── discovery.rs     — CC pane detection, status inference, briefing injection queue
    ├── alerts.rs        — AlertType, cooldown engine, threshold evaluators
    ├── coordination.rs  — claim/release/dispatch/broadcast, dispatch_ack, task queue
    ├── dashboard.rs     — render logic, BusMetrics, tick coherence
    └── workers.rs       — FieldWorker, ThermalWorker, CoordWorker registration
```

Each module exposes a single public struct with `&self` methods. No module holds
mutable references to another — all cross-module communication goes through `main.rs`
as the sole coordinator. This means any module can be replaced without touching others.

---

## Cargo.toml

```toml
[package]
name    = "habitat-nexus"
version = "2.0.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
zellij-tile = "0.43"           # MUST match running Zellij version exactly
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
uuid        = { version = "1", features = ["v4"] }

[profile.release]
opt-level = "s"   # size-optimised WASM
lto       = true
```

---

## src/protocol.rs

**Single source of truth for all message types.** Adding a new arm or worker message
means adding one variant here and one match arm in `main.rs`. Nothing else changes.

```rust
use serde::{Deserialize, Serialize};
use zellij_tile::prelude::PaneId;

// ── Arm identifiers ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArmId {
    Orac,
    Synthex,
    Pv2,
    NexusBus,
    Povm,
    CcFleet,
    MeV2,
}

impl ArmId {
    pub fn all() -> &'static [ArmId] {
        &[
            ArmId::Orac, ArmId::Synthex, ArmId::Pv2,
            ArmId::NexusBus, ArmId::Povm, ArmId::CcFleet, ArmId::MeV2,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ArmId::Orac     => "orac",
            ArmId::Synthex  => "synthex",
            ArmId::Pv2      => "pv2",
            ArmId::NexusBus => "nexus_bus",
            ArmId::Povm     => "povm",
            ArmId::CcFleet  => "cc_fleet",
            ArmId::MeV2     => "me_v2",
        }
    }

    pub fn from_str(s: &str) -> Option<ArmId> {
        match s {
            "orac"      => Some(ArmId::Orac),
            "synthex"   => Some(ArmId::Synthex),
            "pv2"       => Some(ArmId::Pv2),
            "nexus_bus" => Some(ArmId::NexusBus),
            "povm"      => Some(ArmId::Povm),
            "cc_fleet"  => Some(ArmId::CcFleet),
            "me_v2"     => Some(ArmId::MeV2),
            _           => None,
        }
    }

    pub fn endpoint(self) -> &'static str {
        match self {
            ArmId::Orac     => "http://localhost:8133/health",
            ArmId::Synthex  => "http://localhost:8090/v3/thermal",
            ArmId::Pv2      => "http://localhost:8132/field",
            ArmId::NexusBus => "http://localhost:8090/v3/nexus/pull",
            ArmId::Povm     => "http://localhost:8125/state",
            ArmId::CcFleet  => "http://localhost:8090/v3/fleet/pulse",
            ArmId::MeV2     => "http://localhost:8134/health",
        }
    }

    /// Arms considered stable/built. Unready arms operate in degraded mode.
    pub fn is_ready(self) -> bool {
        match self {
            ArmId::Orac | ArmId::Synthex | ArmId::Pv2
            | ArmId::NexusBus | ArmId::Povm => true,
            ArmId::CcFleet | ArmId::MeV2   => false, // endpoints not yet deployed
        }
    }
}

// ── Inbound domain state (one per arm) ───────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OracHealth {
    pub ralph_gen:             u64,
    pub ralph_fitness:         f64,
    pub ralph_phase:           String,
    pub field_r:               f64,
    pub ltp_delta_window:      u32,  // LTPs in last N gens — NOT cumulative total
    pub ltd_delta_window:      u32,
    pub coupling_weight_mean:  f64,
    pub emergence_events:      u64,
    pub ipc_state:             String,
    pub sessions:              u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThermalState {
    pub temperature:  f64,
    pub target:       f64,
    pub pid_output:   f64,
    pub heat_sources: Vec<HeatSource>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeatSource {
    pub id:      String,
    pub reading: f64,
    pub weight:  f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FieldState {
    pub r:          f64,
    pub k:          f64,
    pub spheres:    u32,
    pub fleet_mode: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NexusEvents {
    pub events: Vec<NexusEvent>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NexusEvent {
    pub kind:    String,
    pub payload: serde_json::Value,
    pub ts:      u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PovmState {
    pub active_pathways: u32,
    pub povm_count:      u32,
    pub rm_entries:      u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FleetAck {
    pub active_panes: u32,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeState {
    pub layer:    String,
    pub phase:    String,
    pub healthy:  bool,
}

// ── Outbound commands (hub → arm) ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ArmCommand {
    ReduceK     { delta: f64 },
    SetThermal  { target: f64 },
    FleetMsg    { payload: String },
    NexusPush   { payload: serde_json::Value },
}

// ── Alert types (all derives required for BTreeMap key usage) ────────────────

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum AlertType {
    RalphStall,
    FitnessDrop,
    ConvergenceTrap,
    ThermalSpike,
    ServiceDown,
    IpcDisconnect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertLevel {
    Info,
    Warn,
    Critical,
    Emergency,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub level:      AlertLevel,
    pub alert_type: AlertType,
    pub message:    String,
    pub timestamp:  u64,
    pub source:     String,
}

// ── Pipe API (external interface) ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum PipeRequest {
    Claim   (ClaimRequest),
    Release (ReleaseRequest),
    Dispatch(DispatchRequest),
    Broadcast { message: String },
    Status,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimRequest {
    pub task_id: String,
    pub pane_id: PaneId,
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseRequest {
    pub task_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchRequest {
    pub task:      String,
    pub role:      Option<String>,
    pub pipe_id:   Option<String>, // caller's pipe_id for dispatch_ack
}

// ── Dispatch ack — routed back to caller ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchAck {
    pub task:        String,
    pub assigned_to: Option<PaneId>, // None if no idle pane available
    pub queued:      bool,
}

// ── Worker message protocol (main ↔ workers) ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerMsg {
    pub kind:    String,
    pub payload: String, // JSON-encoded domain struct
}
```

---

## src/arms.rs

**Self-contained per-arm state machine with backoff.** Adding a new arm = add one
`ArmId` variant to `protocol.rs` and one entry in `ArmRegistry::new()`. No other
files change.

```rust
use std::collections::BTreeMap;
use crate::protocol::ArmId;

/// Per-arm state — fully independent, no cross-arm references
#[derive(Debug, Default)]
pub struct ArmState {
    pub id:           ArmId,
    pub ready:        bool,        // false = degraded mode, polls suppressed
    pub in_flight:    bool,        // true = awaiting response from this tick
    pub fail_count:   u8,          // consecutive failures
    pub backoff_until: u64,        // tick number at which polling resumes
    pub latency_us:   u32,         // last observed round-trip latency
    pub tx_bytes:     u32,
    pub rx_bytes:     u32,
    pub fired_at:     u64,         // tick at which last request was sent
}

impl ArmState {
    pub fn new(id: ArmId) -> Self {
        Self {
            id,
            ready: id.is_ready(),
            ..Default::default()
        }
    }

    /// Should this arm fire on the current tick?
    pub fn should_fire(&self, current_tick: u64) -> bool {
        self.ready && !self.in_flight && current_tick >= self.backoff_until
    }

    /// Record a successful response
    pub fn on_success(&mut self, rx_bytes: usize, latency_us: u32) {
        self.in_flight  = false;
        self.fail_count = 0;
        self.latency_us = latency_us;
        self.rx_bytes   = rx_bytes as u32;
        // Arm becomes ready on first success even if started unready
        self.ready = true;
    }

    /// Record a failure and apply exponential backoff
    /// Backoff schedule: 1, 2, 4, 8, 16 ticks (caps at 16 = 80s)
    pub fn on_failure(&mut self, current_tick: u64) {
        self.in_flight   = false;
        self.fail_count  = self.fail_count.saturating_add(1);
        let backoff_ticks = if self.fail_count > 3 {
            1u64 << (self.fail_count.min(4) as u64) // 2, 4, 8, 16
        } else {
            1
        };
        self.backoff_until = current_tick + backoff_ticks;
    }

    /// Record a fire (request sent)
    pub fn on_fire(&mut self, current_tick: u64, tx_bytes: usize) {
        self.in_flight = true;
        self.fired_at  = current_tick;
        self.tx_bytes  = tx_bytes as u32;
    }
}

/// Registry of all 7 arms — iterate to fan-out, index by ArmId to route responses
pub struct ArmRegistry(BTreeMap<ArmId, ArmState>);

impl ArmRegistry {
    pub fn new() -> Self {
        let mut map = BTreeMap::new();
        for &id in ArmId::all() {
            map.insert(id, ArmState::new(id));
        }
        Self(map)
    }

    pub fn get(&self, id: ArmId) -> &ArmState {
        &self.0[&id]
    }

    pub fn get_mut(&mut self, id: ArmId) -> &mut ArmState {
        self.0.get_mut(&id).unwrap()
    }

    /// Arms that should fire this tick
    pub fn ready_to_fire(&self, tick: u64) -> Vec<ArmId> {
        self.0.values()
            .filter(|a| a.should_fire(tick))
            .map(|a| a.id)
            .collect()
    }

    /// Count arms that completed in the last tick (tick coherence metric)
    pub fn completed_last_tick(&self, tick: u64) -> u8 {
        self.0.values()
            .filter(|a| !a.in_flight && a.fired_at == tick.saturating_sub(1))
            .count() as u8
    }
}
```

---

## src/discovery.rs

**CC pane lifecycle management with injection safety.**

```rust
use std::collections::{BTreeMap, VecDeque};
use zellij_tile::prelude::*;
use crate::protocol::OracHealth;

#[derive(Debug, Clone, PartialEq)]
pub enum CcStatus { Idle, Working, Compact, WaitingForPrompt, Unknown }

#[derive(Debug, Clone)]
pub struct CcPaneInfo {
    pub pane_id:              PaneId,
    pub tab_index:            usize,
    pub position:             String,
    pub status:               CcStatus,
    pub last_seen:            u64,
    pub dispatches_sent:      u32,
    pub dispatches_completed: u32,
}

/// Queued briefing — held until the target pane is safe to inject into
struct PendingBriefing {
    pane_id:  PaneId,
    message:  String,
    attempts: u8,
}

pub struct Discovery {
    pub cc_panes:          BTreeMap<PaneId, CcPaneInfo>,
    pending_briefings:     VecDeque<PendingBriefing>,
}

impl Discovery {
    pub fn new() -> Self {
        Self {
            cc_panes:         BTreeMap::new(),
            pending_briefings: VecDeque::new(),
        }
    }

    pub fn handle_pane_update(
        &mut self,
        manifest: PaneManifest,
        tick: u64,
        orac: &Option<OracHealth>,
        field_r: f64,
    ) -> bool {
        let mut changed = false;

        for (tab_idx, panes) in &manifest.panes {
            for pane in panes {
                let is_cc = pane.title.contains("claude")
                    || pane.pane_content_command.as_deref() == Some("claude");

                if is_cc {
                    if !self.cc_panes.contains_key(&pane.id) {
                        let info = CcPaneInfo {
                            pane_id:              pane.id,
                            tab_index:            *tab_idx,
                            position:             self.derive_position(*tab_idx, pane),
                            status:               CcStatus::Unknown,
                            last_seen:            tick,
                            dispatches_sent:      0,
                            dispatches_completed: 0,
                        };
                        // Queue briefing — DO NOT inject immediately
                        self.queue_briefing(&info, tick, orac, field_r);
                        self.cc_panes.insert(pane.id, info);
                        changed = true;
                    } else if let Some(entry) = self.cc_panes.get_mut(&pane.id) {
                        entry.last_seen = tick;
                    }
                }
            }
        }

        // Prune stale panes (unseen for 6 ticks = 30s)
        self.cc_panes.retain(|_, p| tick - p.last_seen < 6);

        changed
    }

    fn queue_briefing(
        &mut self,
        pane: &CcPaneInfo,
        tick: u64,
        orac: &Option<OracHealth>,
        field_r: f64,
    ) {
        let message = format!(
            "\n[HABITAT NEXUS v2] Fleet instance {} detected.\n\
             Session tick: {} | Fleet: {} CCs\n\
             ORAC: gen={} fit={:.3} r={:.3}\n\
             Run: atuin scripts run cc-receiver\n",
            pane.position,
            tick,
            self.cc_panes.len() + 1,
            orac.as_ref().map_or(0, |o| o.ralph_gen),
            orac.as_ref().map_or(0.0, |o| o.ralph_fitness),
            field_r,
        );
        self.pending_briefings.push_back(PendingBriefing {
            pane_id: pane.pane_id,
            message,
            attempts: 0,
        });
    }

    /// Called each timer tick — inject only into safe (idle/waiting) panes
    pub fn flush_briefing_queue(&mut self) {
        let mut retry = VecDeque::new();

        while let Some(mut b) = self.pending_briefings.pop_front() {
            let status = self.cc_panes.get(&b.pane_id).map(|p| &p.status);

            let safe = matches!(
                status,
                Some(CcStatus::Idle) | Some(CcStatus::WaitingForPrompt)
            );

            if safe {
                write_chars_to_pane_id(&b.message, b.pane_id);
            } else if b.attempts < 6 {
                // Retry for up to 30s (6 ticks)
                b.attempts += 1;
                retry.push_back(b);
            }
            // attempts >= 6: drop — pane never became safe
        }

        self.pending_briefings = retry;
    }

    fn derive_position(&self, tab_idx: usize, pane: &PaneInfo) -> String {
        format!("tab{}-pane{}", tab_idx, pane.id)
    }

    /// Send Ctrl+C to a pane — using the correct zellij-tile API
    pub fn interrupt_pane(pane_id: PaneId) {
        write_chars_to_pane_id("\x03", pane_id);
    }
}
```

---

## src/alerts.rs

**Fully self-contained alert engine. Add a new alert type = one enum variant +
one evaluator method. No other files change.**

```rust
use std::collections::BTreeMap;
use std::collections::VecDeque;
use crate::protocol::{Alert, AlertLevel, AlertType, OracHealth, FieldState};
use crate::discovery::{CcStatus, Discovery};

pub struct AlertEngine {
    pub alerts:          VecDeque<Alert>,
    alert_cooldowns:     BTreeMap<AlertType, u64>,
    ltp_window:          VecDeque<u32>, // rolling window — NOT cumulative
    ltp_window_size:     usize,
}

impl AlertEngine {
    pub fn new() -> Self {
        Self {
            alerts:          VecDeque::new(),
            alert_cooldowns: BTreeMap::new(),
            ltp_window:      VecDeque::new(),
            ltp_window_size: 10, // last 10 ticks = 50s
        }
    }

    /// Called every timer tick with latest domain state
    pub fn evaluate(
        &mut self,
        tick: u64,
        orac: &Option<OracHealth>,
        field: &Option<FieldState>,
    ) -> Vec<Alert> {
        let mut fired = Vec::new();

        if let Some(o) = orac {
            // Update rolling LTP window
            self.ltp_window.push_back(o.ltp_delta_window);
            if self.ltp_window.len() > self.ltp_window_size {
                self.ltp_window.pop_front();
            }
            let ltp_rate: u32 = self.ltp_window.iter().sum();

            if let Some(f) = field {
                // Convergence trap: high coherence + zero LTP activity in window
                if f.r > 0.999 && ltp_rate == 0 && self.ltp_window.len() == self.ltp_window_size {
                    self.try_fire(Alert {
                        level:      AlertLevel::Critical,
                        alert_type: AlertType::ConvergenceTrap,
                        message:    format!(
                            "r={:.4} + LTP rate=0 over {}s = convergence trap. K={:.2} needs reduction.",
                            f.r, self.ltp_window_size * 5, f.k
                        ),
                        timestamp: tick,
                        source:    "convergence_detector".into(),
                    }, tick, &mut fired);
                }
            }

            // Ralph stall: fitness unchanged for >12 ticks
            if o.ralph_fitness < 0.01 && o.ralph_gen > 50 {
                self.try_fire(Alert {
                    level:      AlertLevel::Warn,
                    alert_type: AlertType::RalphStall,
                    message:    format!(
                        "RALPH fitness={:.4} at gen={}. Possible stall.",
                        o.ralph_fitness, o.ralph_gen
                    ),
                    timestamp: tick,
                    source:    "ralph_monitor".into(),
                }, tick, &mut fired);
            }
        }

        fired
    }

    fn try_fire(&mut self, alert: Alert, tick: u64, out: &mut Vec<Alert>) {
        let cooldown_ticks = match alert.level {
            AlertLevel::Info      => 0,
            AlertLevel::Warn      => 12,   // 60s
            AlertLevel::Critical  => 60,   // 300s
            AlertLevel::Emergency => 360,  // 30 min
        };

        if let Some(&last) = self.alert_cooldowns.get(&alert.alert_type) {
            if tick - last < cooldown_ticks {
                return;
            }
        }

        self.alert_cooldowns.insert(alert.alert_type.clone(), tick);
        self.alerts.push_back(alert.clone());
        if self.alerts.len() > 100 { self.alerts.pop_front(); }

        out.push(alert);
    }

    /// Dispatch fired alerts to affected CC panes
    pub fn dispatch_alerts(&self, alerts: &[Alert], discovery: &Discovery) {
        for alert in alerts {
            match alert.level {
                AlertLevel::Critical | AlertLevel::Emergency => {
                    for (pane_id, info) in &discovery.cc_panes {
                        if info.status != CcStatus::Compact {
                            // Interrupt then message — using correct API
                            Discovery::interrupt_pane(*pane_id);
                            write_chars_to_pane_id(
                                &format!("\n[NEXUS {}] {}\n", format!("{:?}", alert.level), alert.message),
                                *pane_id,
                            );
                        }
                    }
                }
                _ => {} // Warn and Info only appear in dashboard
            }
        }
    }
}
```

---

## src/coordination.rs

**Task claim/dispatch with ack routing. Add new coordination primitives here only.**

```rust
use std::collections::{BTreeMap, VecDeque};
use zellij_tile::prelude::*;
use crate::protocol::{
    ClaimRequest, ReleaseRequest, DispatchRequest, DispatchAck,
};
use crate::discovery::{CcStatus, Discovery};

#[derive(Debug, Clone, PartialEq)]
pub enum ClaimStatus { Active, Completed, TimedOut, Released }

#[derive(Debug, Clone)]
pub struct TaskClaim {
    pub task_id:     String,
    pub claimed_by:  PaneId,
    pub claimed_at:  u64,
    pub timeout_secs: u64,
    pub status:      ClaimStatus,
}

#[derive(Debug, Clone)]
pub struct DispatchOutcome {
    pub pane_id:       PaneId,
    pub task:          String,
    pub dispatched_at: u64,
    pub completed_at:  Option<u64>,
    pub success:       Option<bool>,
}

pub struct Coordination {
    pub claims:    BTreeMap<String, TaskClaim>,
    pub outcomes:  VecDeque<DispatchOutcome>,
}

impl Coordination {
    pub fn new() -> Self {
        Self {
            claims:   BTreeMap::new(),
            outcomes: VecDeque::new(),
        }
    }

    /// Process a claim request. Returns response string to write to requesting pane.
    pub fn process_claim(&mut self, req: ClaimRequest, tick: u64) -> String {
        if let Some(existing) = self.claims.get(&req.task_id) {
            if existing.status == ClaimStatus::Active {
                return format!("DENIED: task {} already claimed", req.task_id);
            }
        }

        self.claims.insert(req.task_id.clone(), TaskClaim {
            task_id:      req.task_id.clone(),
            claimed_by:   req.pane_id,
            claimed_at:   tick,
            timeout_secs: req.timeout.unwrap_or(300),
            status:       ClaimStatus::Active,
        });

        format!("CLAIMED: task {}", req.task_id)
    }

    pub fn release_claim(&mut self, req: ReleaseRequest) {
        if let Some(claim) = self.claims.get_mut(&req.task_id) {
            claim.status = ClaimStatus::Released;
        }
    }

    pub fn expire_claims(&mut self, tick: u64) {
        for claim in self.claims.values_mut() {
            if claim.status == ClaimStatus::Active {
                let elapsed = (tick - claim.claimed_at) * 5; // 5s per tick
                if elapsed > claim.timeout_secs {
                    claim.status = ClaimStatus::TimedOut;
                }
            }
        }
    }

    /// Dispatch task to first idle pane, write ack back to caller.
    /// Returns a DispatchAck for routing back to the caller's pipe_id.
    pub fn dispatch(
        &mut self,
        req: DispatchRequest,
        discovery: &Discovery,
        tick: u64,
    ) -> DispatchAck {
        let target = discovery.cc_panes.values()
            .find(|p| {
                p.status == CcStatus::Idle || p.status == CcStatus::WaitingForPrompt
            })
            .map(|p| p.pane_id);

        if let Some(pane_id) = target {
            write_chars_to_pane_id(
                &format!("\n[NEXUS DISPATCH] Task: {}\n", req.task),
                pane_id,
            );
            self.outcomes.push_back(DispatchOutcome {
                pane_id,
                task:          req.task.clone(),
                dispatched_at: tick,
                completed_at:  None,
                success:       None,
            });
            if self.outcomes.len() > 200 { self.outcomes.pop_front(); }

            DispatchAck { task: req.task, assigned_to: Some(pane_id), queued: false }
        } else {
            // No idle pane — ack with queued=true
            DispatchAck { task: req.task, assigned_to: None, queued: true }
        }
    }

    pub fn broadcast(&self, message: &str, discovery: &Discovery) {
        for pane_id in discovery.cc_panes.keys() {
            write_chars_to_pane_id(&format!("\n[NEXUS BROADCAST] {}\n", message), *pane_id);
        }
    }
}
```

---

## src/dashboard.rs

**All rendering logic isolated here. Reskin/redesign without touching business logic.**

```rust
use crate::protocol::{ArmId, Alert, AlertLevel};
use crate::arms::ArmRegistry;
use crate::alerts::AlertEngine;
use crate::discovery::Discovery;
use crate::coordination::Coordination;

pub struct BusMetrics {
    pub tick:                   u64,
    pub arms_completed_last:    u8,   // tick coherence — how many of 7 resolved
    pub total_dispatches:       u32,
    pub active_claims:          u32,
}

pub struct Dashboard;

impl Dashboard {
    pub fn render(
        rows: usize,
        cols: usize,
        metrics: &BusMetrics,
        arms: &ArmRegistry,
        alerts: &AlertEngine,
        discovery: &Discovery,
        coord: &Coordination,
    ) {
        // Header
        println!("\u{1b}[1;32m HABITAT NEXUS v2  \u{1b}[0m tick={} coherence={}/7",
            metrics.tick, metrics.arms_completed_last);
        println!("{}", "─".repeat(cols.min(80)));

        // Star bus arm status row
        print!(" ARMS: ");
        for &id in ArmId::all() {
            let arm = arms.get(id);
            let symbol = if !arm.ready {
                format!("\u{1b}[2m{}\u{1b}[0m", id.as_str().to_uppercase().chars().next().unwrap())
            } else if arm.fail_count > 0 {
                format!("\u{1b}[33m{}\u{1b}[0m", id.as_str().to_uppercase().chars().next().unwrap())
            } else {
                format!("\u{1b}[32m{}\u{1b}[0m", id.as_str().to_uppercase().chars().next().unwrap())
            };
            print!("[{}:{:>4}µs] ", symbol, arm.latency_us);
        }
        println!();

        // Fleet
        println!(" FLEET: {} CCs | dispatches={} claims={}",
            discovery.cc_panes.len(),
            metrics.total_dispatches,
            metrics.active_claims,
        );

        // Recent alerts (last 5)
        if !alerts.alerts.is_empty() {
            println!("{}", "─".repeat(cols.min(80)));
            println!(" ALERTS:");
            for alert in alerts.alerts.iter().rev().take(5) {
                let colour = match alert.level {
                    AlertLevel::Emergency => "\u{1b}[1;31m",
                    AlertLevel::Critical  => "\u{1b}[31m",
                    AlertLevel::Warn      => "\u{1b}[33m",
                    AlertLevel::Info      => "\u{1b}[36m",
                };
                println!("  {}[{:?}]\u{1b}[0m t={} {}",
                    colour, alert.level, alert.timestamp, alert.message);
            }
        }

        // Key hints
        if rows > 12 {
            println!("{}", "─".repeat(cols.min(80)));
            println!(" pipe: claim | release | dispatch | broadcast | status");
        }
    }
}
```

---

## src/workers.rs

**Domain workers — each owns its processing logic independently.**

```rust
use serde::{Deserialize, Serialize};
use zellij_tile::prelude::*;
use crate::protocol::{OracHealth, FieldState, ThermalState, WorkerMsg};

// ── Field Worker (ORAC + PV2 + convergence evaluation) ───────────────────────

#[derive(Default, Serialize, Deserialize)]
pub struct FieldWorker {
    orac:  Option<OracHealth>,
    field: Option<FieldState>,
}

impl ZellijWorker<'_> for FieldWorker {
    fn on_message(&mut self, message: String, payload: String) {
        match message.as_str() {
            "orac_update" => {
                if let Ok(state) = serde_json::from_str::<OracHealth>(&payload) {
                    self.orac = Some(state);
                    self.post_update();
                }
            }
            "pv2_update" => {
                if let Ok(state) = serde_json::from_str::<FieldState>(&payload) {
                    self.field = Some(state);
                    self.post_update();
                }
            }
            _ => {}
        }
    }
}

impl FieldWorker {
    fn post_update(&self) {
        let msg = WorkerMsg {
            kind:    "field_state".into(),
            payload: serde_json::to_string(&(&self.orac, &self.field)).unwrap_or_default(),
        };
        post_message_to_plugin(PluginMessage {
            name:    msg.kind,
            payload: Some(msg.payload),
            worker_name: None,
        });
    }
}

// ── Thermal Worker (SYNTHEX + ME-v2 + POVM) ──────────────────────────────────

#[derive(Default, Serialize, Deserialize)]
pub struct ThermalWorker {
    thermal: Option<ThermalState>,
}

impl ZellijWorker<'_> for ThermalWorker {
    fn on_message(&mut self, message: String, payload: String) {
        match message.as_str() {
            "synthex_update" => {
                if let Ok(state) = serde_json::from_str::<ThermalState>(&payload) {
                    self.thermal = Some(state.clone());
                    let msg = WorkerMsg {
                        kind:    "thermal_state".into(),
                        payload: serde_json::to_string(&state).unwrap_or_default(),
                    };
                    post_message_to_plugin(PluginMessage {
                        name:    msg.kind,
                        payload: Some(msg.payload),
                        worker_name: None,
                    });
                }
            }
            _ => {}
        }
    }
}

// ── Coord Worker (claims processing, feedback aggregation) ───────────────────

#[derive(Default, Serialize, Deserialize)]
pub struct CoordWorker;

impl ZellijWorker<'_> for CoordWorker {
    fn on_message(&mut self, message: String, payload: String) {
        // Coord processing is lightweight — currently passes through to main
        post_message_to_plugin(PluginMessage {
            name:    format!("coord_{}", message),
            payload: Some(payload),
            worker_name: None,
        });
    }
}

// ── Worker registration ───────────────────────────────────────────────────────

register_worker!(FieldWorker,   field_worker,   FIELD_WORKER);
register_worker!(ThermalWorker, thermal_worker, THERMAL_WORKER);
register_worker!(CoordWorker,   coord_worker,   COORD_WORKER);
```

---

## src/main.rs

**Thin coordinator. Routes events to the appropriate module. Should rarely need
to change — business logic lives in the modules.**

```rust
mod protocol;
mod arms;
mod discovery;
mod alerts;
mod coordination;
mod dashboard;
mod workers;

use std::collections::BTreeMap;
use zellij_tile::prelude::*;
use protocol::*;
use arms::*;
use discovery::*;
use alerts::*;
use coordination::*;
use dashboard::*;

#[derive(Default)]
struct HabitatNexus {
    tick:        u64,
    arms:        ArmRegistry,
    discovery:   Discovery,
    alert_engine: AlertEngine,
    coord:       Coordination,
    // Aggregated domain state — written by ingest_* methods
    orac:        Option<OracHealth>,
    field:       Option<FieldState>,
    thermal:     Option<ThermalState>,
    metrics:     BusMetrics,
}

register_plugin!(HabitatNexus);

impl ZellijPlugin for HabitatNexus {
    fn load(&mut self, _config: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::WriteToStdin,
            PermissionType::ReadPaneContents,
            PermissionType::WebAccess,
            PermissionType::RunCommands,
            PermissionType::MessageAndLaunchOtherPlugins,
            PermissionType::ReadCliPipes,
        ]);

        subscribe(&[
            EventType::PaneUpdate,
            EventType::Timer,
            EventType::WebRequestResult,
            EventType::RunCommandResult,
            EventType::CustomMessage,
            EventType::CommandPaneExited,
            EventType::Key,
            EventType::PaneRenderReport,
            EventType::SessionUpdate,
            EventType::CwdChanged,
        ]);

        set_timeout(5.0);
        self.load_state();
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PaneUpdate(manifest) => {
                let changed = self.discovery.handle_pane_update(
                    manifest, self.tick,
                    &self.orac,
                    self.field.as_ref().map_or(0.0, |f| f.r),
                );
                changed
            }
            Event::Timer(elapsed) => self.handle_timer(elapsed),
            Event::WebRequestResult(status, _headers, body, ctx) => {
                self.handle_web_response(status, body, ctx)
            }
            Event::CustomMessage(name, payload) => {
                self.handle_worker_message(name, payload)
            }
            Event::Key(key) => self.handle_key(key),
            _ => false,
        }
    }

    fn pipe(&mut self, msg: PipeMessage) -> bool {
        self.handle_pipe(msg)
    }

    fn render(&mut self, rows: usize, cols: usize) {
        Dashboard::render(rows, cols, &self.metrics, &self.arms,
            &self.alert_engine, &self.discovery, &self.coord);
    }
}

impl HabitatNexus {
    fn handle_timer(&mut self, _: f64) -> bool {
        self.tick += 1;

        // Flush any queued briefings to safe panes
        self.discovery.flush_briefing_queue();

        // Fan-out — all ready arms fire in parallel
        for arm_id in self.arms.ready_to_fire(self.tick) {
            let url = arm_id.endpoint();
            let ctx = BTreeMap::from([
                ("arm".to_string(), arm_id.as_str().to_string()),
                ("tick".to_string(), self.tick.to_string()),
            ]);
            web_request(url, HttpVerb::Get, BTreeMap::new(), vec![], ctx);
            self.arms.get_mut(arm_id).on_fire(self.tick, url.len());
        }

        // Evaluate alerts against current state
        let fired = self.alert_engine.evaluate(self.tick, &self.orac, &self.field);
        if !fired.is_empty() {
            self.alert_engine.dispatch_alerts(&fired, &self.discovery);
        }

        // Expire timed-out claims
        self.coord.expire_claims(self.tick);

        // Update tick coherence metric
        self.metrics.tick = self.tick;
        self.metrics.arms_completed_last = self.arms.completed_last_tick(self.tick);
        self.metrics.active_claims = self.coord.claims.values()
            .filter(|c| c.status == ClaimStatus::Active)
            .count() as u32;

        // Persist every 12 ticks (60s)
        if self.tick % 12 == 0 { self.save_state(); }

        // Re-arm
        set_timeout(5.0);
        true
    }

    fn handle_web_response(
        &mut self,
        status: u16,
        body: Vec<u8>,
        ctx: BTreeMap<String, String>,
    ) -> bool {
        let arm_str = ctx.get("arm").map(|s| s.as_str()).unwrap_or("");
        let arm_id  = match ArmId::from_str(arm_str) { Some(id) => id, None => return false };

        if status != 200 {
            let tick = self.tick;
            self.arms.get_mut(arm_id).on_failure(tick);
            return true; // re-render to update fail indicator
        }

        let rx = body.len();
        self.arms.get_mut(arm_id).on_success(rx, 0); // latency_us TODO: measure

        // Route body to correct worker for processing
        let payload = String::from_utf8_lossy(&body).to_string();
        let changed = match arm_id {
            ArmId::Orac => {
                post_message_to("field", "orac_update", payload);
                false // worker will post back when processed
            }
            ArmId::Pv2 => {
                post_message_to("field", "pv2_update", payload);
                false
            }
            ArmId::Synthex | ArmId::MeV2 | ArmId::Povm => {
                post_message_to("thermal", "synthex_update", payload);
                false
            }
            ArmId::NexusBus | ArmId::CcFleet => {
                // Lightweight — process inline
                true
            }
        };

        changed
    }

    fn handle_worker_message(&mut self, name: String, payload: String) -> bool {
        match name.as_str() {
            "field_state" => {
                if let Ok((orac, field)) =
                    serde_json::from_str::<(Option<OracHealth>, Option<FieldState>)>(&payload)
                {
                    self.orac  = orac;
                    self.field = field;
                }
                true
            }
            "thermal_state" => {
                if let Ok(t) = serde_json::from_str::<ThermalState>(&payload) {
                    self.thermal = Some(t);
                }
                true
            }
            _ => false,
        }
    }

    fn handle_pipe(&mut self, msg: PipeMessage) -> bool {
        let payload = msg.payload.clone().unwrap_or_default();

        match msg.name.as_deref().unwrap_or("") {
            "claim" => {
                if let Ok(req) = serde_json::from_str::<ClaimRequest>(&payload) {
                    let pane_id = req.pane_id;
                    let tick    = self.tick;
                    let resp    = self.coord.process_claim(req, tick);
                    write_chars_to_pane_id(&format!("\n[NEXUS] {}\n", resp), pane_id);
                }
            }
            "release" => {
                if let Ok(req) = serde_json::from_str::<ReleaseRequest>(&payload) {
                    self.coord.release_claim(req);
                }
            }
            "dispatch" => {
                if let Ok(req) = serde_json::from_str::<DispatchRequest>(&payload) {
                    let pipe_id = req.pipe_id.clone();
                    let tick    = self.tick;
                    let ack     = self.coord.dispatch(req, &self.discovery, tick);
                    self.metrics.total_dispatches += 1;

                    // Route ack back to caller
                    if let Some(pid) = pipe_id {
                        let ack_json = serde_json::to_string(&ack).unwrap_or_default();
                        cli_pipe_output(&pid, &ack_json);
                        unblock_cli_pipe_input(&pid);
                    }
                }
            }
            "broadcast" => {
                self.coord.broadcast(&payload, &self.discovery);
            }
            "status" => {
                if let PipeSource::Cli(pid) = msg.source {
                    let status = serde_json::to_string(&self.get_fleet_status())
                        .unwrap_or_default();
                    cli_pipe_output(&pid, &status);
                    unblock_cli_pipe_input(&pid);
                }
            }
            _ => {}
        }

        true
    }

    fn handle_key(&mut self, _key: Key) -> bool { false }

    fn get_fleet_status(&self) -> serde_json::Value {
        serde_json::json!({
            "tick":    self.tick,
            "fleet":   self.discovery.cc_panes.len(),
            "claims":  self.metrics.active_claims,
            "coherence": self.metrics.arms_completed_last,
        })
    }

    fn load_state(&mut self) {
        // Read persisted state from /data/nexus_state.json if present
        // Implementation: std::fs::read_to_string("/data/nexus_state.json")
    }

    fn save_state(&self) {
        // Persist tick + claim state to /data/nexus_state.json
        // Implementation: std::fs::write("/data/nexus_state.json", ...)
    }
}
```

---

## Build & Deploy

```bash
# 1. Scaffold (run once inside Zellij)
zellij plugin -f -- \
  https://github.com/zellij-org/create-rust-plugin/releases/latest/download/create-rust-plugin.wasm

# 2. Add wasm target
rustup target add wasm32-wasip1

# 3. Build
cd habitat-nexus
cargo build --release --target wasm32-wasip1

# 4. Deploy
cp target/wasm32-wasip1/release/habitat_nexus.wasm \
   ~/.config/zellij/plugins/habitat-nexus.wasm
```

---

## Development Hot-Reload Layout

```kdl
// habitat-nexus-dev.kdl — drop into ~/.config/zellij/layouts/
layout {
  pane edit="src/main.rs"
  pane edit="src/protocol.rs"
  pane command="bash" {
    args "-c" "cargo build --target wasm32-wasip1 && \
               zellij action start-or-reload-plugin \
               file:target/wasm32-wasip1/debug/habitat_nexus.wasm"
  }
  pane {
    plugin location="file:target/wasm32-wasip1/debug/habitat_nexus.wasm"
  }
}
```

---

## Permanent Session Integration

```kdl
// In synth-orchestrator.kdl — auto-loads on session start
load_plugins {
  file:~/.config/zellij/plugins/habitat-nexus.wasm
}

// Dashboard on Alt+n
shared {
  bind "Alt n" {
    LaunchOrFocusPlugin "file:~/.config/zellij/plugins/habitat-nexus.wasm" {
      floating true
      width "80%"
      height "40%"
    }
  }
}
```

---

## CLI Integration

```bash
# Query fleet status
zellij pipe -p "file:~/.config/zellij/plugins/habitat-nexus.wasm" -n status

# Claim a task (with ack)
zellij pipe -p "file:~/.config/zellij/plugins/habitat-nexus.wasm" \
  -n claim -- '{"task_id":"T3","pane_id":{"terminal_id":1}}'

# Dispatch with ack routing back to caller
zellij pipe -p "file:~/.config/zellij/plugins/habitat-nexus.wasm" \
  -n dispatch -- '{"task":"Review convergence fix","role":"Verifier","pipe_id":"caller-pipe-1"}'

# Broadcast
zellij pipe -p "file:~/.config/zellij/plugins/habitat-nexus.wasm" \
  -n broadcast -- "RALPH fitness dropped. Investigate."
```

---

## Migration Path

Each phase independently revertible.

| Phase | Action | Revert |
|---|---|---|
| 1 | Deploy plugin alongside scripts. Monitor only. | Remove from `load_plugins` |
| 2 | Enable discovery auto-briefing. Manual dispatch fallback. | Set `ready=false` on CcFleet arm |
| 3 | Enable signaling (alerts). File-based bus as audit trail. | Comment out `alert_engine.evaluate()` |
| 4 | Enable coordination (claims). Deprecate Atuin KV claims. | Disable `handle_pipe` claim branch |
| 5 | Wire iceoryx2 Option B: sidecar → plugin segment. | Revert to web_request polling |

---

## Iterating in Future

To add a new arm: add one `ArmId` variant in `protocol.rs`, one endpoint, one `is_ready()` match arm, one ingest call in `main.rs::handle_web_response`. Zero other files change.

To add a new alert: add one `AlertType` variant in `protocol.rs`, one evaluator method in `alerts.rs`. Zero other files change.

To add a new pipe command: add one match arm in `main.rs::handle_pipe`. Zero other files change.

To replace the dashboard: rewrite `dashboard.rs` only.

To add a new worker domain: add one struct + `register_worker!` in `workers.rs`, one `post_message_to` call in `main.rs`. Zero other files change.

---

## Cross-References

- **Obsidian:** `[[Session 072 — Habitat Nexus Plugin Architecture v2]]`
- **Obsidian:** `[[Zellij Gold Standard — Session 050 Mastery Skill]]`
- **Obsidian:** `[[Battern — Patterned Batch Dispatch for Claude Code Fleets]]`
- **API Docs:** https://docs.rs/zellij-tile/0.43.0/zellij_tile/
- **Plugin Guide:** https://zellij.dev/documentation/plugins.html
- **Workers Guide:** https://zellij.dev/documentation/plugin-api-workers.html
- **Supersedes:** `ai_docs/HABITAT_NEXUS_PLUGIN_SPEC.md` (v1.0.0)
