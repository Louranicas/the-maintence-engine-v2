use serde::{Deserialize, Serialize};
use zellij_tile::prelude::*;
use crate::protocol::{OracHealth, FieldState, ThermalState, MeState, PovmState};

// ── Field Worker — ORAC + PV2 ─────────────────────────────────────────────────
// Deserialises JSON off the main thread; posts aggregated state back

#[derive(Default, Serialize, Deserialize)]
pub struct FieldWorker {
    orac:  Option<OracHealth>,
    field: Option<FieldState>,
}

impl ZellijWorker<'_> for FieldWorker {
    fn on_message(&mut self, message: String, payload: String) {
        let updated = match message.as_str() {
            "orac_update" => serde_json::from_str::<OracHealth>(&payload)
                .ok()
                .map(|s| { self.orac = Some(s); })
                .is_some(),
            "pv2_update"  => serde_json::from_str::<FieldState>(&payload)
                .ok()
                .map(|s| { self.field = Some(s); })
                .is_some(),
            _ => false,
        };
        if updated { self.post_update(); }
    }
}

impl FieldWorker {
    fn post_update(&self) {
        if let Ok(payload) = serde_json::to_string(&(&self.orac, &self.field)) {
            post_message_to_plugin(PluginMessage {
                name:        "field_state".to_string(),
                payload:     Some(payload),
                worker_name: None,
            });
        }
    }
}

// ── Thermal Worker — SYNTHEX + ME-v2 + POVM ──────────────────────────────────

#[derive(Default, Serialize, Deserialize)]
pub struct ThermalWorker {
    thermal: Option<ThermalState>,
    me:      Option<MeState>,
    povm:    Option<PovmState>,
}

impl ZellijWorker<'_> for ThermalWorker {
    fn on_message(&mut self, message: String, payload: String) {
        let updated = match message.as_str() {
            "synthex_update" => serde_json::from_str::<ThermalState>(&payload)
                .ok()
                .map(|s| { self.thermal = Some(s); })
                .is_some(),
            "me_update"   => serde_json::from_str::<MeState>(&payload)
                .ok()
                .map(|s| { self.me = Some(s); })
                .is_some(),
            "povm_update" => serde_json::from_str::<PovmState>(&payload)
                .ok()
                .map(|s| { self.povm = Some(s); })
                .is_some(),
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
// CoordWorker intentionally absent — was a dead passthrough, coord handled inline

register_worker!(FieldWorker,   field_worker,   FIELD_WORKER);
register_worker!(ThermalWorker, thermal_worker, THERMAL_WORKER);
