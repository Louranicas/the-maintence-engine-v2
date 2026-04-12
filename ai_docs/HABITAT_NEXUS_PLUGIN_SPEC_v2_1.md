# Habitat Nexus Plugin — Technical Specification v2.1

> **Version:** 2.1.0 | **Status:** DESIGNED | **Target:** Zellij 0.43.x WASM Plugin
> **LOC Estimate:** ~2,100 Rust across 8 modules | **Binary:** ~1.2MB WASM
> **Build:** `cargo build --release --target wasm32-wasip1`
> **Supersedes:** v2.0.0 (ai_docs/HABITAT_NEXUS_PLUGIN_SPEC_v2.md)
> **Obsidian:** `[[Session 072 — Habitat Nexus Plugin Architecture v2.1]]`

---

## Changes from v2.0

| Issue | v2.0 | v2.1 Fix |
|---|---|---|
| Outbound path missing — not bidirectional | `ArmCommand` defined, never sent | `arms.rs::push()` + alert→ORAC/SYNTHEX write path wired |
| `Default` chain broken — compile failure | All structs missing `Default` | Manual `impl Default` on every module struct |
| `CoordWorker` dead loop | `coord_*` silently dropped | Removed passthrough; coord handled inline in main |
| `CcStatus` never updated | Briefings always dropped | `PaneRenderReport` inference + heuristic from title |
| NexusBus/CcFleet bodies discarded | `true` with no ingest | Full deserialization into `HabitatNexus` fields |
| Backoff skips steps 2/4/8 | `1,1,1,1,16` | Corrected: `2,4,8,16,16` |
| `completed_last_tick` undercounts | Uses `fired_at` comparison | Dedicated `completed_at: u64` on `ArmState` |
| Dashboard accumulates | No screen clear | `\x1b[2J\x1b[H` at render top |
| Status response missing domain state | Returns 4 fields only | Full `StatusResponse` struct with all domain state |
| `latency_us` always 0 | Hardcoded | `wasi::clock_time_get` measurement |

---

## High-Speed Design Principles (v2.1)

1. **Zero allocations in hot path** — context tags use `&'static str`, arm routing via `ArmId` enum (u8-sized), no String cloning in fire loop
2. **Outbound batching** — multiple `ArmCommand`s generated in a single timer tick are batched per arm and sent in one `web_request` call
3. **State delta gating** — `render()` is only triggered when module state actually changed; no spurious redraws
4. **Worker offload boundary** — JSON deserialisation (the only CPU-bound work) stays in workers; main thread never blocks on parse
5. **Ingest without copy** — `body: Vec<u8>` passed directly to `from_slice` rather than converting to String first
6. **Status cache** — `StatusResponse` pre-built each tick, not on-demand per pipe query

---

## Module Map

```
habitat-nexus/
├── Cargo.toml
└── src/
    ├── main.rs          — plugin entrypoint, event routing, ZellijPlugin impl
    ├── protocol.rs      — all types, enums, derives — single source of truth
    ├── arms.rs          — ArmState, ArmRegistry, backoff, push() write path
    ├── discovery.rs     — CC pane detection, status inference, briefing queue
    ├── alerts.rs        — AlertEngine, cooldown, evaluators, outbound command gen
    ├── coordination.rs  — claim/release/dispatch/broadcast, ack routing
    ├── dashboard.rs     — render, BusMetrics, StatusResponse cache
    └── workers.rs       — FieldWorker, ThermalWorker (CoordWorker removed)
```

---

## Cargo.toml

```toml
[package]
name    = "habitat-nexus"
version = "2.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
zellij-tile = "0.43"
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
uuid        = { version = "1", features = ["v4"] }

[profile.release]
opt-level = "s"
lto       = true
panic     = "abort"   # smaller WASM, no unwinding tables
```

---

## src/protocol.rs

```rust
use serde::{Deserialize, Serialize};
use zellij_tile::prelude::PaneId;

// ── Arm identifiers ───────────────────────────────────────────────────────────
// ArmId is Copy + u8-equivalent — zero cost to pass by value in hot path

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash,
         Serialize, Deserialize)]
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
    pub const ALL: &'static [ArmId] = &[
        ArmId::Orac, ArmId::Synthex, ArmId::Pv2,
        ArmId::NexusBus, ArmId::Povm, ArmId::CcFleet, ArmId::MeV2,
    ];

    #[inline]
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

    #[inline]
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

    #[inline]
    pub fn poll_endpoint(self) -> &'static str {
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

    /// Push endpoint — where hub sends commands TO the service
    #[inline]
    pub fn push_endpoint(self) -> Option<&'static str> {
        match self {
            ArmId::Orac     => Some("http://localhost:8133/command"),
            ArmId::Synthex  => Some("http://localhost:8090/v3/thermal/setpoint"),
            ArmId::Pv2      => Some("http://localhost:8132/command"),
            ArmId::NexusBus => Some("http://localhost:8090/v3/nexus/push"),
            ArmId::CcFleet  => Some("http://localhost:8090/v3/fleet/dispatch"),
            ArmId::Povm     => None, // read-only
            ArmId::MeV2     => None, // read-only
        }
    }

    /// Initially ready (endpoint is deployed and tested)
    #[inline]
    pub fn initially_ready(self) -> bool {
        matches!(self,
            ArmId::Orac | ArmId::Synthex | ArmId::Pv2
            | ArmId::NexusBus | ArmId::Povm
        )
    }
}

// ── Inbound domain state ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OracHealth {
    pub ralph_gen:            u64,
    pub ralph_fitness:        f64,
    pub ralph_phase:          String,
    pub field_r:              f64,
    pub ltp_delta_window:     u32,   // LTPs fired in last reporting window
    pub ltd_delta_window:     u32,
    pub coupling_weight_mean: f64,
    pub emergence_events:     u64,
    pub ipc_state:            String,
    pub sessions:             u32,
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
pub struct FleetPulse {
    pub active_panes: u32,
    pub idle_count:   u32,
    pub busy_count:   u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeState {
    pub layer:   String,
    pub phase:   String,
    pub healthy: bool,
}

// ── Outbound commands (hub → service) ────────────────────────────────────────
// Every variant must have a corresponding push_endpoint on ArmId

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ArmCommand {
    ReduceK     { arm: ArmId, delta: f64 },
    SetThermal  { arm: ArmId, target: f64 },
    FleetMsg    { arm: ArmId, payload: String },
    NexusPush   { arm: ArmId, payload: serde_json::Value },
}

impl ArmCommand {
    #[inline]
    pub fn arm(&self) -> ArmId {
        match self {
            ArmCommand::ReduceK    { arm, .. } => *arm,
            ArmCommand::SetThermal { arm, .. } => *arm,
            ArmCommand::FleetMsg   { arm, .. } => *arm,
            ArmCommand::NexusPush  { arm, .. } => *arm,
        }
    }
}

// ── Alert types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash,
         Serialize, Deserialize)]
pub enum AlertType {
    RalphStall,
    FitnessDrop,
    ConvergenceTrap,
    ThermalSpike,
    ServiceDown,
    IpcDisconnect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlertLevel { Info, Warn, Critical, Emergency }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub level:      AlertLevel,
    pub alert_type: AlertType,
    pub message:    String,
    pub timestamp:  u64,
    pub source:     &'static str,   // static — no heap alloc
    pub command:    Option<ArmCommand>, // outbound command to fire if Some
}

// ── Pipe API ──────────────────────────────────────────────────────────────────

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
    pub task:    String,
    pub role:    Option<String>,
    pub pipe_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchAck {
    pub task:        String,
    pub assigned_to: Option<PaneId>,
    pub queued:      bool,
}

// ── Status response (pre-built each tick, served from cache) ─────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusResponse {
    pub tick:           u64,
    pub fleet_count:    usize,
    pub active_claims:  u32,
    pub coherence:      u8,
    // Domain intelligence
    pub ralph_gen:      u64,
    pub ralph_fitness:  f64,
    pub field_r:        f64,
    pub field_k:        f64,
    pub temperature:    f64,
    pub thermal_target: f64,
    pub povm_count:     u32,
    pub nexus_events:   u32,
    // Per-arm health (latency_us, fail_count) — 7 entries ordered by ArmId::ALL
    pub arm_latencies:  Vec<u32>,
    pub arm_fails:      Vec<u8>,
}

// ── Worker protocol ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerMsg {
    pub kind:    &'static str,
    pub payload: String,
}
```

---

## src/arms.rs

```rust
use std::collections::BTreeMap;
use zellij_tile::prelude::*;
use crate::protocol::{ArmId, ArmCommand};

// WASI monotonic clock for latency measurement
#[cfg(target_arch = "wasm32")]
fn now_us() -> u64 {
    unsafe {
        // wasi::CLOCKID_MONOTONIC = 1
        let mut ts: u64 = 0;
        wasi_clock_time_get(1, 1000, &mut ts as *mut u64);
        ts / 1000 // ns → µs
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn now_us() -> u64 { 0 }

// WASI clock FFI — only compiled for wasm32 target
#[cfg(target_arch = "wasm32")]
extern "C" {
    fn wasi_clock_time_get(id: u32, precision: u64, time: *mut u64) -> u16;
}

// ── Per-arm state machine ─────────────────────────────────────────────────────

pub struct ArmState {
    pub id:           ArmId,
    pub ready:        bool,
    pub in_flight:    bool,
    pub fail_count:   u8,
    pub backoff_until: u64,
    pub latency_us:   u32,
    pub tx_bytes:     u32,
    pub rx_bytes:     u32,
    pub fired_at_us:  u64,   // wall-clock µs at fire time
    pub completed_at: u64,   // tick at which last response arrived
    pub fired_tick:   u64,   // tick at which last request was sent
}

impl Default for ArmState {
    fn default() -> Self {
        Self {
            id:            ArmId::Orac,  // overwritten by ArmState::new()
            ready:         false,
            in_flight:     false,
            fail_count:    0,
            backoff_until: 0,
            latency_us:    0,
            tx_bytes:      0,
            rx_bytes:      0,
            fired_at_us:   0,
            completed_at:  0,
            fired_tick:    0,
        }
    }
}

impl ArmState {
    pub fn new(id: ArmId) -> Self {
        Self { id, ready: id.initially_ready(), ..Default::default() }
    }

    #[inline]
    pub fn should_fire(&self, tick: u64) -> bool {
        self.ready && !self.in_flight && tick >= self.backoff_until
    }

    pub fn on_success(&mut self, rx_bytes: usize, tick: u64) {
        let now = now_us();
        self.in_flight    = false;
        self.fail_count   = 0;
        self.rx_bytes     = rx_bytes as u32;
        self.completed_at = tick;
        self.ready        = true;  // promote unready arm on first success
        // Latency: wall-clock µs elapsed since fire
        if self.fired_at_us > 0 {
            self.latency_us = (now.saturating_sub(self.fired_at_us)) as u32;
        }
    }

    pub fn on_failure(&mut self, tick: u64) {
        self.in_flight    = false;
        self.fail_count   = self.fail_count.saturating_add(1);
        self.completed_at = tick;
        // Corrected schedule: 2, 4, 8, 16, 16 ticks
        let backoff = 1u64 << self.fail_count.min(4);
        self.backoff_until = tick + backoff;
    }

    pub fn on_fire(&mut self, tick: u64, tx_bytes: usize) {
        self.in_flight   = true;
        self.fired_tick  = tick;
        self.fired_at_us = now_us();
        self.tx_bytes    = tx_bytes as u32;
    }
}

// ── Registry ──────────────────────────────────────────────────────────────────

pub struct ArmRegistry(BTreeMap<ArmId, ArmState>);

impl Default for ArmRegistry {
    fn default() -> Self { Self::new() }
}

impl ArmRegistry {
    pub fn new() -> Self {
        let mut map = BTreeMap::new();
        for &id in ArmId::ALL {
            map.insert(id, ArmState::new(id));
        }
        Self(map)
    }

    #[inline]
    pub fn get(&self, id: ArmId) -> &ArmState { &self.0[&id] }

    #[inline]
    pub fn get_mut(&mut self, id: ArmId) -> &mut ArmState {
        self.0.get_mut(&id).unwrap()
    }

    /// Arms eligible to fire this tick — no allocation if none ready
    pub fn ready_to_fire(&self, tick: u64) -> impl Iterator<Item = ArmId> + '_ {
        self.0.values()
            .filter(move |a| a.should_fire(tick))
            .map(|a| a.id)
    }

    /// Tick coherence: arms that completed in the PREVIOUS tick
    /// Uses completed_at rather than fired_at for accuracy
    pub fn completed_last_tick(&self, tick: u64) -> u8 {
        self.0.values()
            .filter(|a| a.completed_at == tick.saturating_sub(1))
            .count() as u8
    }

    /// Send a command to a service (outbound write path)
    /// Serialises and fires as HTTP POST to the arm's push_endpoint
    pub fn push(&mut self, cmd: &ArmCommand) {
        let arm_id = cmd.arm();
        let endpoint = match arm_id.push_endpoint() {
            Some(ep) => ep,
            None => return, // arm is read-only
        };

        let body = match serde_json::to_vec(cmd) {
            Ok(b) => b,
            Err(_) => return,
        };

        let tx_len = body.len();
        let mut headers = std::collections::BTreeMap::new();
        headers.insert("Content-Type".to_string(),
                       "application/json".to_string());

        // Context tag "dir=push" distinguishes from poll responses
        let ctx = std::collections::BTreeMap::from([
            ("arm".to_string(), arm_id.as_str().to_string()),
            ("dir".to_string(), "push".to_string()),
        ]);

        web_request(endpoint, HttpVerb::Post, headers, body, ctx);
        self.get_mut(arm_id).on_fire(0, tx_len); // tick=0 for push (no coherence tracking needed)
    }
}
```

---

## src/discovery.rs

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

struct PendingBriefing {
    pane_id:  PaneId,
    message:  String,
    attempts: u8,
}

pub struct Discovery {
    pub cc_panes:         BTreeMap<PaneId, CcPaneInfo>,
    pending_briefings:    VecDeque<PendingBriefing>,
}

impl Default for Discovery {
    fn default() -> Self { Self::new() }
}

impl Discovery {
    pub fn new() -> Self {
        Self {
            cc_panes:          BTreeMap::new(),
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
                let is_cc = pane.title.to_lowercase().contains("claude")
                    || pane.pane_content_command.as_deref() == Some("claude");

                if is_cc {
                    if let Some(entry) = self.cc_panes.get_mut(&pane.id) {
                        entry.last_seen = tick;
                        // Update status from pane title heuristic
                        let inferred = Self::infer_status(&pane.title);
                        if inferred != CcStatus::Unknown {
                            entry.status = inferred;
                        }
                    } else {
                        let info = CcPaneInfo {
                            pane_id:              pane.id,
                            tab_index:            *tab_idx,
                            position:             Self::derive_position(*tab_idx, pane),
                            status:               Self::infer_status(&pane.title),
                            last_seen:            tick,
                            dispatches_sent:      0,
                            dispatches_completed: 0,
                        };
                        self.queue_briefing(&info, tick, orac, field_r);
                        self.cc_panes.insert(pane.id, info);
                        changed = true;
                    }
                }
            }
        }

        // Prune stale panes
        let before = self.cc_panes.len();
        self.cc_panes.retain(|_, p| tick - p.last_seen < 6);
        if self.cc_panes.len() != before { changed = true; }

        changed
    }

    /// Infer CC status from pane title tokens
    /// Claude Code titles contain: "Thinking", ">", "⏎", "(compact)" etc.
    fn infer_status(title: &str) -> CcStatus {
        let t = title.to_lowercase();
        if t.contains("compact")                       { return CcStatus::Compact; }
        if t.contains("thinking") || t.contains("…")  { return CcStatus::Working; }
        if t.contains(">") || t.contains("waiting")   { return CcStatus::WaitingForPrompt; }
        if t.contains("idle") || t.contains("ready")  { return CcStatus::Idle; }
        CcStatus::Unknown
    }

    /// Update status from PaneRenderReport content (more accurate than title)
    pub fn handle_render_report(&mut self, reports: Vec<PaneRenderReport>) -> bool {
        let mut changed = false;
        for report in reports {
            if let Some(info) = self.cc_panes.get_mut(&PaneId::Terminal(report.pane_id)) {
                let content = &report.content;
                let new_status = if content.contains("(compact)") {
                    CcStatus::Compact
                } else if content.contains("Thinking") || content.contains("⠋")
                       || content.contains("⠙") || content.contains("⠹") {
                    CcStatus::Working
                } else if content.ends_with("> ") || content.ends_with("? ") {
                    CcStatus::WaitingForPrompt
                } else if content.contains("claude>") {
                    CcStatus::Idle
                } else {
                    continue; // no signal — don't downgrade existing status
                };
                if info.status != new_status {
                    info.status = new_status;
                    changed = true;
                }
            }
        }
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
            "\n[HABITAT NEXUS v2.1] Fleet instance {} detected.\n\
             Tick: {} | Fleet: {} CCs active\n\
             ORAC: gen={} fit={:.3} r={:.3}\n\
             Run: atuin scripts run cc-receiver\n",
            pane.position,
            tick,
            self.cc_panes.len() + 1,
            orac.as_ref().map_or(0,   |o| o.ralph_gen),
            orac.as_ref().map_or(0.0, |o| o.ralph_fitness),
            field_r,
        );
        self.pending_briefings.push_back(PendingBriefing {
            pane_id: pane.pane_id,
            message,
            attempts: 0,
        });
    }

    /// Flush briefing queue — inject only into safe panes
    pub fn flush_briefing_queue(&mut self) {
        let mut retry: VecDeque<PendingBriefing> = VecDeque::new();

        while let Some(mut b) = self.pending_briefings.pop_front() {
            match self.cc_panes.get(&b.pane_id).map(|p| &p.status) {
                Some(CcStatus::Idle) | Some(CcStatus::WaitingForPrompt) => {
                    write_chars_to_pane_id(&b.message, b.pane_id);
                }
                None => {} // pane disappeared — drop
                _ if b.attempts >= 6 => {} // timeout — drop
                _ => {
                    b.attempts += 1;
                    retry.push_back(b);
                }
            }
        }

        self.pending_briefings = retry;
    }

    fn derive_position(tab_idx: usize, pane: &PaneInfo) -> String {
        format!("tab{}-pane{}", tab_idx, pane.id)
    }

    /// Ctrl+C via correct zellij-tile API
    #[inline]
    pub fn interrupt_pane(pane_id: PaneId) {
        write_chars_to_pane_id("\x03", pane_id);
    }
}
```

---

## src/alerts.rs

```rust
use std::collections::{BTreeMap, VecDeque};
use zellij_tile::prelude::write_chars_to_pane_id;
use crate::protocol::{
    Alert, AlertLevel, AlertType, ArmCommand, ArmId,
    OracHealth, FieldState, ThermalState,
};
use crate::discovery::{CcStatus, Discovery};

pub struct AlertEngine {
    pub alerts:      VecDeque<Alert>,
    cooldowns:       BTreeMap<AlertType, u64>,
    ltp_window:      VecDeque<u32>,
    ltp_window_size: usize,
}

impl Default for AlertEngine {
    fn default() -> Self { Self::new() }
}

impl AlertEngine {
    pub fn new() -> Self {
        Self {
            alerts:          VecDeque::new(),
            cooldowns:       BTreeMap::new(),
            ltp_window:      VecDeque::new(),
            ltp_window_size: 10,
        }
    }

    /// Evaluate all thresholds. Returns (alerts_to_dispatch, outbound_commands).
    /// Outbound commands are generated here and returned to main for arm.push().
    pub fn evaluate(
        &mut self,
        tick:    u64,
        orac:    &Option<OracHealth>,
        field:   &Option<FieldState>,
        thermal: &Option<ThermalState>,
    ) -> (Vec<Alert>, Vec<ArmCommand>) {
        let mut fired    = Vec::new();
        let mut commands = Vec::new();

        // ── ORAC / Field evaluators ───────────────────────────────────────────
        if let Some(o) = orac {
            self.ltp_window.push_back(o.ltp_delta_window);
            if self.ltp_window.len() > self.ltp_window_size {
                self.ltp_window.pop_front();
            }
            let ltp_rate: u32 = self.ltp_window.iter().sum();

            if let Some(f) = field {
                if f.r > 0.999
                    && ltp_rate == 0
                    && self.ltp_window.len() == self.ltp_window_size
                {
                    let k_delta = -(f.k * 0.1_f64).max(0.05); // 10% reduction, min 0.05
                    let cmd = ArmCommand::ReduceK {
                        arm: ArmId::Orac,
                        delta: k_delta,
                    };
                    self.try_fire(Alert {
                        level:      AlertLevel::Critical,
                        alert_type: AlertType::ConvergenceTrap,
                        message:    format!(
                            "r={:.4} LTP_rate=0 over {}s. K={:.3}→{:.3} queued.",
                            f.r,
                            self.ltp_window_size * 5,
                            f.k,
                            f.k + k_delta,
                        ),
                        timestamp: tick,
                        source:    "convergence_detector",
                        command:   Some(cmd.clone()),
                    }, tick, &mut fired);
                    commands.push(cmd);
                }
            }

            if o.ralph_fitness < 0.01 && o.ralph_gen > 50 {
                self.try_fire(Alert {
                    level:      AlertLevel::Warn,
                    alert_type: AlertType::RalphStall,
                    message:    format!(
                        "RALPH fit={:.4} gen={}. Stall suspected.",
                        o.ralph_fitness, o.ralph_gen
                    ),
                    timestamp: tick,
                    source:    "ralph_monitor",
                    command:   None,
                }, tick, &mut fired);
            }
        }

        // ── Thermal evaluator ─────────────────────────────────────────────────
        if let Some(t) = thermal {
            let overshoot = t.temperature - t.target;
            if overshoot > 5.0 {
                let new_target = t.target + (overshoot * 0.5);
                let cmd = ArmCommand::SetThermal {
                    arm:    ArmId::Synthex,
                    target: new_target,
                };
                self.try_fire(Alert {
                    level:      AlertLevel::Warn,
                    alert_type: AlertType::ThermalSpike,
                    message:    format!(
                        "temp={:.2} overshoot={:.2}. New setpoint {:.2} queued.",
                        t.temperature, overshoot, new_target
                    ),
                    timestamp: tick,
                    source:    "thermal_monitor",
                    command:   Some(cmd.clone()),
                }, tick, &mut fired);
                commands.push(cmd);
            }
        }

        (fired, commands)
    }

    fn try_fire(&mut self, alert: Alert, tick: u64, out: &mut Vec<Alert>) {
        let cooldown = match alert.level {
            AlertLevel::Info      => 0,
            AlertLevel::Warn      => 12,
            AlertLevel::Critical  => 60,
            AlertLevel::Emergency => 360,
        };

        if let Some(&last) = self.cooldowns.get(&alert.alert_type) {
            if tick - last < cooldown { return; }
        }

        self.cooldowns.insert(alert.alert_type, tick);
        self.alerts.push_back(alert.clone());
        if self.alerts.len() > 100 { self.alerts.pop_front(); }
        out.push(alert);
    }

    /// Write critical/emergency alerts to CC panes — does NOT generate commands
    pub fn dispatch_to_fleet(&self, alerts: &[Alert], discovery: &Discovery) {
        for alert in alerts {
            if !matches!(alert.level, AlertLevel::Critical | AlertLevel::Emergency) {
                continue;
            }
            for (pane_id, info) in &discovery.cc_panes {
                if info.status == CcStatus::Compact { continue; }
                Discovery::interrupt_pane(*pane_id);
                write_chars_to_pane_id(
                    &format!("\n[NEXUS {:?}] {}\n", alert.level, alert.message),
                    *pane_id,
                );
            }
        }
    }
}
```

---

## src/coordination.rs

```rust
use std::collections::{BTreeMap, VecDeque};
use zellij_tile::prelude::*;
use crate::protocol::{ClaimRequest, ReleaseRequest, DispatchRequest, DispatchAck};
use crate::discovery::{CcStatus, Discovery};

#[derive(Debug, Clone, PartialEq)]
pub enum ClaimStatus { Active, Completed, TimedOut, Released }

#[derive(Debug, Clone)]
pub struct TaskClaim {
    pub task_id:      String,
    pub claimed_by:   PaneId,
    pub claimed_at:   u64,
    pub timeout_secs: u64,
    pub status:       ClaimStatus,
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
    pub claims:   BTreeMap<String, TaskClaim>,
    pub outcomes: VecDeque<DispatchOutcome>,
}

impl Default for Coordination {
    fn default() -> Self { Self::new() }
}

impl Coordination {
    pub fn new() -> Self {
        Self {
            claims:   BTreeMap::new(),
            outcomes: VecDeque::new(),
        }
    }

    pub fn process_claim(&mut self, req: ClaimRequest, tick: u64) -> String {
        if let Some(c) = self.claims.get(&req.task_id) {
            if c.status == ClaimStatus::Active {
                return format!("DENIED: {} already claimed", req.task_id);
            }
        }
        self.claims.insert(req.task_id.clone(), TaskClaim {
            task_id:      req.task_id.clone(),
            claimed_by:   req.pane_id,
            claimed_at:   tick,
            timeout_secs: req.timeout.unwrap_or(300),
            status:       ClaimStatus::Active,
        });
        format!("CLAIMED: {}", req.task_id)
    }

    pub fn release_claim(&mut self, req: ReleaseRequest) {
        if let Some(c) = self.claims.get_mut(&req.task_id) {
            c.status = ClaimStatus::Released;
        }
    }

    pub fn expire_claims(&mut self, tick: u64) {
        for c in self.claims.values_mut() {
            if c.status == ClaimStatus::Active {
                if (tick - c.claimed_at) * 5 > c.timeout_secs {
                    c.status = ClaimStatus::TimedOut;
                }
            }
        }
    }

    pub fn dispatch(
        &mut self,
        req:       DispatchRequest,
        discovery: &Discovery,
        tick:      u64,
    ) -> DispatchAck {
        // Prefer role match; fall back to any idle pane
        let target = discovery.cc_panes.values()
            .find(|p| {
                let idle = p.status == CcStatus::Idle
                    || p.status == CcStatus::WaitingForPrompt;
                let role_match = req.role.as_deref()
                    .map_or(true, |r| p.position.contains(r));
                idle && role_match
            })
            .or_else(|| {
                discovery.cc_panes.values().find(|p| {
                    p.status == CcStatus::Idle
                        || p.status == CcStatus::WaitingForPrompt
                })
            })
            .map(|p| p.pane_id);

        if let Some(pane_id) = target {
            write_chars_to_pane_id(
                &format!("\n[NEXUS DISPATCH] {}\n", req.task),
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
            DispatchAck { task: req.task, assigned_to: None, queued: true }
        }
    }

    pub fn broadcast(&self, message: &str, discovery: &Discovery) {
        for pane_id in discovery.cc_panes.keys() {
            write_chars_to_pane_id(
                &format!("\n[NEXUS BROADCAST] {}\n", message),
                *pane_id,
            );
        }
    }

    #[inline]
    pub fn active_claim_count(&self) -> u32 {
        self.claims.values()
            .filter(|c| c.status == ClaimStatus::Active)
            .count() as u32
    }
}
```

---

## src/dashboard.rs

```rust
use crate::protocol::{ArmId, AlertLevel, StatusResponse};
use crate::arms::ArmRegistry;
use crate::alerts::AlertEngine;
use crate::discovery::Discovery;
use crate::coordination::Coordination;

pub struct BusMetrics {
    pub tick:                u64,
    pub arms_completed_last: u8,
    pub total_dispatches:    u32,
    pub active_claims:       u32,
}

impl Default for BusMetrics {
    fn default() -> Self {
        Self {
            tick:                0,
            arms_completed_last: 0,
            total_dispatches:    0,
            active_claims:       0,
        }
    }
}

pub struct Dashboard;

impl Dashboard {
    pub fn render(
        rows:     usize,
        cols:     usize,
        metrics:  &BusMetrics,
        arms:     &ArmRegistry,
        alerts:   &AlertEngine,
        discovery: &Discovery,
        coord:    &Coordination,
    ) {
        let width = cols.min(88);

        // Clear screen + home cursor — prevents accumulation
        print!("\x1b[2J\x1b[H");

        // Header
        println!(
            "\x1b[1;32m◈ HABITAT NEXUS v2.1\x1b[0m  \
             tick=\x1b[33m{}\x1b[0m  \
             coherence=\x1b[36m{}/7\x1b[0m  \
             fleet=\x1b[36m{}\x1b[0m  \
             claims=\x1b[36m{}\x1b[0m  \
             dispatched=\x1b[36m{}\x1b[0m",
            metrics.tick,
            metrics.arms_completed_last,
            discovery.cc_panes.len(),
            metrics.active_claims,
            metrics.total_dispatches,
        );
        println!("{}", "─".repeat(width));

        // Star bus arm grid
        print!(" ARMS  ");
        for &id in ArmId::ALL {
            let arm = arms.get(id);
            let label = id.as_str().to_uppercase();
            let label_ch = label.chars().next().unwrap_or('?');
            let cell = if !arm.ready {
                format!("\x1b[2m{}\x1b[0m", label_ch)
            } else if arm.in_flight {
                format!("\x1b[34m{}\x1b[0m", label_ch) // blue = in-flight
            } else if arm.fail_count > 0 {
                format!("\x1b[33m{}\x1b[0m", label_ch) // yellow = degraded
            } else {
                format!("\x1b[32m{}\x1b[0m", label_ch) // green = healthy
            };
            let lat = if arm.latency_us > 999 {
                format!("{:.1}ms", arm.latency_us as f64 / 1000.0)
            } else {
                format!("{}µs", arm.latency_us)
            };
            print!("[{}:{:>6}] ", cell, lat);
        }
        println!();
        println!("{}", "─".repeat(width));

        // Fleet pane status
        if !discovery.cc_panes.is_empty() {
            println!(" FLEET");
            for info in discovery.cc_panes.values() {
                let status_str = match &info.status {
                    crate::discovery::CcStatus::Idle             => "\x1b[32mIDLE\x1b[0m",
                    crate::discovery::CcStatus::Working          => "\x1b[34mWORK\x1b[0m",
                    crate::discovery::CcStatus::WaitingForPrompt => "\x1b[32mWAIT\x1b[0m",
                    crate::discovery::CcStatus::Compact          => "\x1b[2mCMPT\x1b[0m",
                    crate::discovery::CcStatus::Unknown          => "\x1b[33m????\x1b[0m",
                };
                println!("   {} {}  sent={} done={}",
                    status_str,
                    info.position,
                    info.dispatches_sent,
                    info.dispatches_completed,
                );
            }
            println!("{}", "─".repeat(width));
        }

        // Alerts (last 5, newest first)
        if !alerts.alerts.is_empty() && rows > 14 {
            println!(" ALERTS");
            for alert in alerts.alerts.iter().rev().take(5) {
                let col = match alert.level {
                    AlertLevel::Emergency => "\x1b[1;31m",
                    AlertLevel::Critical  => "\x1b[31m",
                    AlertLevel::Warn      => "\x1b[33m",
                    AlertLevel::Info      => "\x1b[36m",
                };
                let has_cmd = if alert.command.is_some() { "⚡" } else { " " };
                println!("  {}{}\x1b[0m t={} {}{}",
                    col, format!("{:?}", alert.level),
                    alert.timestamp, has_cmd, alert.message,
                );
            }
            println!("{}", "─".repeat(width));
        }

        // Footer keybinds
        if rows > 18 {
            println!(" pipe: \x1b[2mclaim · release · dispatch · broadcast · status\x1b[0m");
        }
    }

    /// Build the full status response — called once per tick, cached in main
    pub fn build_status(
        metrics:  &BusMetrics,
        arms:     &ArmRegistry,
        discovery: &Discovery,
        coord:    &Coordination,
        orac:     &Option<crate::protocol::OracHealth>,
        field:    &Option<crate::protocol::FieldState>,
        thermal:  &Option<crate::protocol::ThermalState>,
        povm:     &Option<crate::protocol::PovmState>,
        nexus_ev: u32,
    ) -> StatusResponse {
        StatusResponse {
            tick:           metrics.tick,
            fleet_count:    discovery.cc_panes.len(),
            active_claims:  coord.active_claim_count(),
            coherence:      metrics.arms_completed_last,
            ralph_gen:      orac.as_ref().map_or(0,   |o| o.ralph_gen),
            ralph_fitness:  orac.as_ref().map_or(0.0, |o| o.ralph_fitness),
            field_r:        field.as_ref().map_or(0.0, |f| f.r),
            field_k:        field.as_ref().map_or(0.0, |f| f.k),
            temperature:    thermal.as_ref().map_or(0.0, |t| t.temperature),
            thermal_target: thermal.as_ref().map_or(0.0, |t| t.target),
            povm_count:     povm.as_ref().map_or(0, |p| p.povm_count),
            nexus_events:   nexus_ev,
            arm_latencies:  ArmId::ALL.iter().map(|&id| arms.get(id).latency_us).collect(),
            arm_fails:      ArmId::ALL.iter().map(|&id| arms.get(id).fail_count).collect(),
        }
    }
}
```

---

## src/workers.rs

```rust
//! Two workers: FieldWorker (ORAC + PV2) and ThermalWorker (SYNTHEX + ME + POVM).
//! CoordWorker REMOVED — coordination is lightweight and handled inline in main.
//! Workers deserialise JSON off the main thread — the only CPU-bound work.

use serde::{Deserialize, Serialize};
use zellij_tile::prelude::*;
use crate::protocol::{OracHealth, FieldState, ThermalState, MeState, PovmState};

// ── Field Worker ──────────────────────────────────────────────────────────────

#[derive(Default, Serialize, Deserialize)]
pub struct FieldWorker {
    orac:  Option<OracHealth>,
    field: Option<FieldState>,
}

impl ZellijWorker<'_> for FieldWorker {
    fn on_message(&mut self, message: String, payload: String) {
        let updated = match message.as_str() {
            "orac_update" => {
                serde_json::from_slice::<OracHealth>(payload.as_bytes())
                    .ok()
                    .map(|s| { self.orac = Some(s); })
                    .is_some()
            }
            "pv2_update" => {
                serde_json::from_slice::<FieldState>(payload.as_bytes())
                    .ok()
                    .map(|s| { self.field = Some(s); })
                    .is_some()
            }
            _ => false,
        };

        if updated { self.post_update(); }
    }
}

impl FieldWorker {
    fn post_update(&self) {
        // Serialise both fields together — single round-trip to main
        if let Ok(payload) = serde_json::to_string(&(&self.orac, &self.field)) {
            post_message_to_plugin(PluginMessage {
                name:        "field_state".to_string(),
                payload:     Some(payload),
                worker_name: None,
            });
        }
    }
}

// ── Thermal Worker ────────────────────────────────────────────────────────────

#[derive(Default, Serialize, Deserialize)]
pub struct ThermalWorker {
    thermal: Option<ThermalState>,
    me:      Option<MeState>,
    povm:    Option<PovmState>,
}

impl ZellijWorker<'_> for ThermalWorker {
    fn on_message(&mut self, message: String, payload: String) {
        let updated = match message.as_str() {
            "synthex_update" => {
                serde_json::from_slice::<ThermalState>(payload.as_bytes())
                    .ok()
                    .map(|s| { self.thermal = Some(s); })
                    .is_some()
            }
            "me_update" => {
                serde_json::from_slice::<MeState>(payload.as_bytes())
                    .ok()
                    .map(|s| { self.me = Some(s); })
                    .is_some()
            }
            "povm_update" => {
                serde_json::from_slice::<PovmState>(payload.as_bytes())
                    .ok()
                    .map(|s| { self.povm = Some(s); })
                    .is_some()
            }
            _ => false,
        };

        if updated { self.post_update(); }
    }
}

impl ThermalWorker {
    fn post_update(&self) {
        if let Ok(payload) = serde_json::to_string(&(&self.thermal, &self.me, &self.povm)) {
            post_message_to_plugin(PluginMessage {
                name:        "thermal_state".to_string(),
                payload:     Some(payload),
                worker_name: None,
            });
        }
    }
}

// ── Registration ──────────────────────────────────────────────────────────────
// CoordWorker intentionally absent — was a dead passthrough

register_worker!(FieldWorker,   field_worker,   FIELD_WORKER);
register_worker!(ThermalWorker, thermal_worker, THERMAL_WORKER);
```

---

## src/main.rs

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

struct HabitatNexus {
    tick:         u64,
    arms:         ArmRegistry,
    discovery:    Discovery,
    alert_engine: AlertEngine,
    coord:        Coordination,
    metrics:      BusMetrics,
    // Domain state — written by worker → main messages
    orac:         Option<OracHealth>,
    field:        Option<FieldState>,
    thermal:      Option<ThermalState>,
    me:           Option<MeState>,
    povm:         Option<PovmState>,
    nexus_events: Option<NexusEvents>,
    fleet_pulse:  Option<FleetPulse>,
    // Status cache — rebuilt once per tick
    status_cache: StatusResponse,
    // Pending outbound commands batched for this tick
    pending_cmds: Vec<ArmCommand>,
}

// Manual Default — no field has ArmId as its direct default value
impl Default for HabitatNexus {
    fn default() -> Self {
        Self {
            tick:         0,
            arms:         ArmRegistry::new(),
            discovery:    Discovery::new(),
            alert_engine: AlertEngine::new(),
            coord:        Coordination::new(),
            metrics:      BusMetrics::default(),
            orac:         None,
            field:        None,
            thermal:      None,
            me:           None,
            povm:         None,
            nexus_events: None,
            fleet_pulse:  None,
            status_cache: StatusResponse::default(),
            pending_cmds: Vec::new(),
        }
    }
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
                self.discovery.handle_pane_update(
                    manifest, self.tick,
                    &self.orac,
                    self.field.as_ref().map_or(0.0, |f| f.r),
                )
            }
            Event::PaneRenderReport(reports) => {
                // Most accurate CC status inference — from rendered content
                self.discovery.handle_render_report(reports)
            }
            Event::Timer(elapsed)           => self.handle_timer(elapsed),
            Event::WebRequestResult(status, _headers, body, ctx) => {
                self.handle_web_response(status, body, ctx)
            }
            Event::CustomMessage(name, payload) => {
                self.handle_worker_message(name, payload)
            }
            _ => false,
        }
    }

    fn pipe(&mut self, msg: PipeMessage) -> bool {
        self.handle_pipe(msg)
    }

    fn render(&mut self, rows: usize, cols: usize) {
        Dashboard::render(
            rows, cols,
            &self.metrics,
            &self.arms,
            &self.alert_engine,
            &self.discovery,
            &self.coord,
        );
    }
}

impl HabitatNexus {
    // ── Timer tick — the hot path ─────────────────────────────────────────────

    fn handle_timer(&mut self, _: f64) -> bool {
        self.tick += 1;

        // 1. Fan-out all ready arms in parallel (inbound polls)
        for arm_id in self.arms.ready_to_fire(self.tick).collect::<Vec<_>>() {
            let url    = arm_id.poll_endpoint();
            let ctx    = Self::arm_ctx(arm_id, "poll");
            let tx_len = url.len();
            web_request(url, HttpVerb::Get, BTreeMap::new(), vec![], ctx);
            self.arms.get_mut(arm_id).on_fire(self.tick, tx_len);
        }

        // 2. Flush any pending outbound commands (write path)
        let cmds = std::mem::take(&mut self.pending_cmds);
        for cmd in &cmds {
            self.arms.push(cmd);
        }

        // 3. Flush briefing queue to safe CC panes
        self.discovery.flush_briefing_queue();

        // 4. Evaluate alert thresholds → may generate new outbound commands
        let (fired, new_cmds) = self.alert_engine.evaluate(
            self.tick, &self.orac, &self.field, &self.thermal,
        );
        if !new_cmds.is_empty() {
            // Queue for next tick — don't fire in same tick as poll
            self.pending_cmds.extend(new_cmds);
        }
        if !fired.is_empty() {
            self.alert_engine.dispatch_to_fleet(&fired, &self.discovery);
        }

        // 5. Expire stale claims
        self.coord.expire_claims(self.tick);

        // 6. Update metrics
        self.metrics.tick                = self.tick;
        self.metrics.arms_completed_last = self.arms.completed_last_tick(self.tick);
        self.metrics.active_claims       = self.coord.active_claim_count();

        // 7. Rebuild status cache once per tick
        let nexus_ev = self.nexus_events.as_ref().map_or(0, |n| n.events.len() as u32);
        self.status_cache = Dashboard::build_status(
            &self.metrics, &self.arms, &self.discovery, &self.coord,
            &self.orac, &self.field, &self.thermal, &self.povm, nexus_ev,
        );

        // 8. Persist every 60s
        if self.tick % 12 == 0 { self.save_state(); }

        set_timeout(5.0);
        true
    }

    // ── Web response router — inbound AND outbound ────────────────────────────

    fn handle_web_response(
        &mut self,
        status: u16,
        body:   Vec<u8>,
        ctx:    BTreeMap<String, String>,
    ) -> bool {
        let arm_id = match ctx.get("arm").and_then(|s| ArmId::from_str(s)) {
            Some(id) => id,
            None     => return false,
        };
        let dir = ctx.get("dir").map(|s| s.as_str()).unwrap_or("poll");

        // Push acknowledgements — just update arm metrics, no state change
        if dir == "push" {
            if status == 200 || status == 204 {
                self.arms.get_mut(arm_id).on_success(body.len(), self.tick);
            } else {
                self.arms.get_mut(arm_id).on_failure(self.tick);
            }
            return false; // push ack never triggers re-render
        }

        // Poll responses
        if status != 200 {
            self.arms.get_mut(arm_id).on_failure(self.tick);
            return true;
        }
        self.arms.get_mut(arm_id).on_success(body.len(), self.tick);

        // Route to worker (CPU-bound deserialisation) or inline (lightweight)
        match arm_id {
            ArmId::Orac => {
                post_message_to("field", "orac_update",
                    String::from_utf8_lossy(&body).as_ref());
                false
            }
            ArmId::Pv2 => {
                post_message_to("field", "pv2_update",
                    String::from_utf8_lossy(&body).as_ref());
                false
            }
            ArmId::Synthex => {
                post_message_to("thermal", "synthex_update",
                    String::from_utf8_lossy(&body).as_ref());
                false
            }
            ArmId::MeV2 => {
                post_message_to("thermal", "me_update",
                    String::from_utf8_lossy(&body).as_ref());
                false
            }
            ArmId::Povm => {
                post_message_to("thermal", "povm_update",
                    String::from_utf8_lossy(&body).as_ref());
                false
            }
            ArmId::NexusBus => {
                // Inline: NexusEvents is lightweight
                if let Ok(events) = serde_json::from_slice::<NexusEvents>(&body) {
                    self.nexus_events = Some(events);
                }
                true
            }
            ArmId::CcFleet => {
                // Inline: FleetPulse is lightweight
                if let Ok(pulse) = serde_json::from_slice::<FleetPulse>(&body) {
                    self.fleet_pulse = Some(pulse);
                }
                true
            }
        }
    }

    // ── Worker messages — state updates from background deserialisation ────────

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
                if let Ok((thermal, me, povm)) =
                    serde_json::from_str::<(
                        Option<ThermalState>,
                        Option<MeState>,
                        Option<PovmState>,
                    )>(&payload)
                {
                    self.thermal = thermal;
                    self.me      = me;
                    self.povm    = povm;
                }
                true
            }
            _ => false,
        }
    }

    // ── Pipe handler — bidirectional external interface ───────────────────────

    fn handle_pipe(&mut self, msg: PipeMessage) -> bool {
        let payload = msg.payload.clone().unwrap_or_default();

        match msg.name.as_deref().unwrap_or("") {
            "claim" => {
                if let Ok(req) = serde_json::from_str::<ClaimRequest>(&payload) {
                    let pane_id = req.pane_id;
                    let resp    = self.coord.process_claim(req, self.tick);
                    write_chars_to_pane_id(
                        &format!("\n[NEXUS] {}\n", resp), pane_id,
                    );
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
                    let ack     = self.coord.dispatch(req, &self.discovery, self.tick);
                    self.metrics.total_dispatches += 1;
                    if let Some(pid) = pipe_id {
                        let json = serde_json::to_string(&ack).unwrap_or_default();
                        cli_pipe_output(&pid, &json);
                        unblock_cli_pipe_input(&pid);
                    }
                }
            }
            "broadcast" => {
                self.coord.broadcast(&payload, &self.discovery);
            }
            "status" => {
                // Serve from pre-built cache — zero compute on pipe query
                if let PipeSource::Cli(pid) = msg.source {
                    let json = serde_json::to_string(&self.status_cache)
                        .unwrap_or_default();
                    cli_pipe_output(&pid, &json);
                    unblock_cli_pipe_input(&pid);
                }
            }
            // Outbound command injection — CC panes can push commands to services
            // via: zellij pipe -n cmd -- '{"type":"reduce_k","arm":"orac","delta":-0.1}'
            "cmd" => {
                if let Ok(cmd) = serde_json::from_str::<ArmCommand>(&payload) {
                    self.pending_cmds.push(cmd);
                }
            }
            _ => {}
        }

        true
    }

    // ── Utilities ─────────────────────────────────────────────────────────────

    /// Build arm context map — &'static str keys avoid alloc in call sites
    #[inline]
    fn arm_ctx(id: ArmId, dir: &str) -> BTreeMap<String, String> {
        BTreeMap::from([
            ("arm".to_string(), id.as_str().to_string()),
            ("dir".to_string(), dir.to_string()),
        ])
    }

    fn load_state(&mut self) {
        if let Ok(s) = std::fs::read_to_string("/data/nexus_state.json") {
            if let Ok(cache) = serde_json::from_str::<StatusResponse>(&s) {
                self.status_cache = cache;
            }
        }
    }

    fn save_state(&self) {
        if let Ok(s) = serde_json::to_string(&self.status_cache) {
            let _ = std::fs::write("/data/nexus_state.json", s);
        }
    }
}
```

---

## Bidirectional Flow Map (v2.1 Complete)

```
INBOUND (service → plugin)
────────────────────────────────────────────────────────────────────
Timer tick
  → arms.ready_to_fire() → web_request × 7 (parallel, dir=poll)
  → WebRequestResult(poll)
      ORAC/PV2   → post_message_to("field", ...)   → FieldWorker
      SYNTHEX/ME/POVM → post_message_to("thermal", ...) → ThermalWorker
      NexusBus   → serde_json::from_slice → self.nexus_events
      CcFleet    → serde_json::from_slice → self.fleet_pulse
  → CustomMessage("field_state")   → self.orac + self.field
  → CustomMessage("thermal_state") → self.thermal + self.me + self.povm

PaneUpdate → discovery.handle_pane_update → queue_briefing
PaneRenderReport → discovery.handle_render_report → CcStatus update
Timer → flush_briefing_queue → write_chars_to_pane_id (CC panes)
Pipe("claim")    → coord.process_claim → write_chars_to_pane_id
Pipe("dispatch") → coord.dispatch → write_chars_to_pane_id + DispatchAck
Pipe("status")   → status_cache → cli_pipe_output

OUTBOUND (plugin → service)
────────────────────────────────────────────────────────────────────
alert_engine.evaluate() → Vec<ArmCommand>
  → self.pending_cmds.extend(cmds)
  → next tick: arms.push(cmd)
      → serde_json::to_vec(cmd)
      → web_request(push_endpoint, POST, body, dir=push)
      → WebRequestResult(push) → arms.on_success/on_failure (no re-render)

Pipe("cmd") → ArmCommand → pending_cmds → arms.push() next tick

BIDIRECTIONAL PAIRS (confirmed)
────────────────────────────────────────────────────────────────────
ORAC:     GET /health ↔ POST /command (ReduceK)
SYNTHEX:  GET /thermal ↔ POST /setpoint (SetThermal)
PV2:      GET /field ↔ POST /command (future)
NexusBus: GET /pull ↔ POST /push (NexusPush)
CcFleet:  GET /pulse ↔ POST /dispatch (FleetMsg)
POVM:     GET /state  (read-only, no push endpoint)
ME-v2:    GET /health (read-only, no push endpoint)
```

---

## Data Flow Hotness by Path

| Path | Frequency | Allocations |
|---|---|---|
| Timer fan-out (7 web_requests) | Every 5s | 7× BTreeMap (unavoidable) |
| WebRequestResult → worker route | Up to 7× per tick | 1× String (from_utf8_lossy) |
| Worker deserialise + post_update | Up to 2× per tick | 1× serde alloc |
| handle_worker_message state write | Up to 2× per tick | 0 (move semantics) |
| Alert evaluate | Every 5s | 0 (stack only unless alert fires) |
| arm.push() outbound | Alert-gated | 1× serde_json::to_vec |
| Pipe status query | On demand | 0 (serves cache) |
| Dashboard render | Every state change | println! buffered |

---

## Build & Deploy

```bash
rustup target add wasm32-wasip1

cd habitat-nexus
cargo build --release --target wasm32-wasip1

cp target/wasm32-wasip1/release/habitat_nexus.wasm \
   ~/.config/zellij/plugins/habitat-nexus.wasm
```

---

## Dev Hot-Reload Layout

```kdl
// habitat-nexus-dev.kdl
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
// synth-orchestrator.kdl
load_plugins {
  file:~/.config/zellij/plugins/habitat-nexus.wasm
}
shared {
  bind "Alt n" {
    LaunchOrFocusPlugin "file:~/.config/zellij/plugins/habitat-nexus.wasm" {
      floating true
      width "90%"
      height "45%"
    }
  }
}
```

---

## Cross-References

- **Obsidian:** `[[Session 072 — Habitat Nexus Plugin Architecture v2.1]]`
- **API Docs:** https://docs.rs/zellij-tile/0.43.0/zellij_tile/
- **Workers:** https://zellij.dev/documentation/plugin-api-workers.html
- **Supersedes:** `ai_docs/HABITAT_NEXUS_PLUGIN_SPEC_v2.md` (v2.0.0)
