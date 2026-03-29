use std::collections::BTreeMap;
use zellij_tile::prelude::*;
use crate::protocol::{ArmId, ArmCommand};

// ── WASI monotonic clock for latency measurement ──────────────────────────────

#[cfg(target_arch = "wasm32")]
fn now_us() -> u64 {
    // WASI preview1: clock_time_get(CLOCKID_MONOTONIC=1, precision, *time) → u16
    let mut ts: u64 = 0;
    unsafe {
        wasi_clock_time_get(1, 1_000, &mut ts as *mut u64);
    }
    ts / 1_000 // nanoseconds → microseconds
}

#[cfg(not(target_arch = "wasm32"))]
fn now_us() -> u64 { 0 }

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "wasi_snapshot_preview1")]
extern "C" {
    fn clock_time_get(id: u32, precision: u64, time: *mut u64) -> u16;
}

#[cfg(target_arch = "wasm32")]
unsafe fn wasi_clock_time_get(id: u32, precision: u64, time: *mut u64) -> u16 {
    clock_time_get(id, precision, time)
}

// ── Per-arm state machine ─────────────────────────────────────────────────────

pub struct ArmState {
    pub id:            ArmId,
    pub ready:         bool,
    pub in_flight:     bool,
    pub fail_count:    u8,
    pub backoff_until: u64,
    pub latency_us:    u32,
    pub tx_bytes:      u32,
    pub rx_bytes:      u32,
    pub fired_at_us:   u64,  // wall-clock µs at fire time
    pub completed_at:  u64,  // tick at which last response arrived
    pub fired_tick:    u64,  // tick at which last request was sent
}

impl Default for ArmState {
    fn default() -> Self {
        Self {
            id:            ArmId::Orac,
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
        self.ready        = true;
        if self.fired_at_us > 0 {
            self.latency_us = now.saturating_sub(self.fired_at_us) as u32;
        }
    }

    pub fn on_failure(&mut self, tick: u64) {
        self.in_flight    = false;
        self.fail_count   = self.fail_count.saturating_add(1);
        self.completed_at = tick;
        // Backoff: 2, 4, 8, 16, 16 ticks
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
        // ArmRegistry::new() inserts every ArmId::ALL member — key always present
        self.0.entry(id).or_insert_with(|| ArmState::new(id))
    }

    pub fn ready_to_fire(&self, tick: u64) -> Vec<ArmId> {
        self.0.values()
            .filter(|a| a.should_fire(tick))
            .map(|a| a.id)
            .collect()
    }

    /// Tick coherence — arms that completed in the previous tick
    pub fn completed_last_tick(&self, tick: u64) -> u8 {
        self.0.values()
            .filter(|a| a.completed_at == tick.saturating_sub(1))
            .count() as u8
    }

    /// Outbound write path — serialise and POST command to arm's push endpoint
    pub fn push(&mut self, cmd: &ArmCommand) {
        let arm_id   = cmd.arm();
        let endpoint = match arm_id.push_endpoint() {
            Some(ep) => ep,
            None     => return,
        };
        let body = match serde_json::to_vec(cmd) {
            Ok(b)  => b,
            Err(_) => return,
        };
        let tx_len = body.len();
        let mut headers = BTreeMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        let ctx = BTreeMap::from([
            ("arm".to_string(), arm_id.as_str().to_string()),
            ("dir".to_string(), "push".to_string()),
        ]);
        web_request(endpoint, HttpVerb::Post, headers, body, ctx);
        self.get_mut(arm_id).on_fire(0, tx_len);
    }
}
