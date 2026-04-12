use serde::{Deserialize, Serialize};
use zellij_tile::prelude::PaneId;

// ── Arm identifiers ───────────────────────────────────────────────────────────

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

    #[inline]
    pub fn push_endpoint(self) -> Option<&'static str> {
        match self {
            ArmId::Orac     => Some("http://localhost:8133/command"),
            ArmId::Synthex  => Some("http://localhost:8090/v3/thermal/setpoint"),
            ArmId::Pv2      => Some("http://localhost:8132/command"),
            ArmId::NexusBus => Some("http://localhost:8090/v3/nexus/push"),
            ArmId::CcFleet  => Some("http://localhost:8090/v3/fleet/dispatch"),
            ArmId::Povm     => None,
            ArmId::MeV2     => None,
        }
    }

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
    pub ltp_delta_window:     u32,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ArmCommand {
    ReduceK    { arm: ArmId, delta: f64 },
    SetThermal { arm: ArmId, target: f64 },
    FleetMsg   { arm: ArmId, payload: String },
    NexusPush  { arm: ArmId, payload: serde_json::Value },
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
    pub source:     String,
    pub command:    Option<ArmCommand>,
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

// ── Status response cache ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusResponse {
    pub tick:           u64,
    pub fleet_count:    usize,
    pub active_claims:  u32,
    pub coherence:      u8,
    pub ralph_gen:      u64,
    pub ralph_fitness:  f64,
    pub field_r:        f64,
    pub field_k:        f64,
    pub temperature:    f64,
    pub thermal_target: f64,
    pub povm_count:     u32,
    pub nexus_events:   u32,
    pub arm_latencies:  Vec<u32>,
    pub arm_fails:      Vec<u8>,
}
