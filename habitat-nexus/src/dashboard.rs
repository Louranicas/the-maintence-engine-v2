use crate::protocol::{
    ArmId, AlertLevel, StatusResponse,
    OracHealth, FieldState, ThermalState, PovmState,
};
use crate::arms::ArmRegistry;
use crate::alerts::AlertEngine;
use crate::discovery::{CcStatus, Discovery};
use crate::coordination::Coordination;

pub struct BusMetrics {
    pub tick:                u64,
    pub arms_completed_last: u8,
    pub total_dispatches:    u32,
    pub active_claims:       u32,
}

impl Default for BusMetrics {
    fn default() -> Self {
        Self { tick: 0, arms_completed_last: 0, total_dispatches: 0, active_claims: 0 }
    }
}

pub struct Dashboard;

impl Dashboard {
    pub fn render(
        rows:      usize,
        cols:      usize,
        metrics:   &BusMetrics,
        arms:      &ArmRegistry,
        alerts:    &AlertEngine,
        discovery: &Discovery,
        _coord:    &Coordination,
    ) {
        let width = cols.min(88);

        // Clear + home cursor — prevents accumulation
        print!("\x1b[2J\x1b[H");

        // ── Header ────────────────────────────────────────────────────────────
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

        // ── Star bus arm grid ─────────────────────────────────────────────────
        print!(" ARMS  ");
        for &id in ArmId::ALL {
            let arm = arms.get(id);
            let ch  = id.as_str().chars().next().unwrap_or('?').to_ascii_uppercase();
            let cell = if !arm.ready {
                format!("\x1b[2m{}\x1b[0m", ch)
            } else if arm.in_flight {
                format!("\x1b[34m{}\x1b[0m", ch)   // blue  = in-flight
            } else if arm.fail_count > 0 {
                format!("\x1b[33m{}\x1b[0m", ch)   // amber = degraded
            } else {
                format!("\x1b[32m{}\x1b[0m", ch)   // green = healthy
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

        // ── Fleet pane table ──────────────────────────────────────────────────
        if !discovery.cc_panes.is_empty() {
            println!(" FLEET");
            for info in discovery.cc_panes.values() {
                let s = match &info.status {
                    CcStatus::Idle             => "\x1b[32mIDLE\x1b[0m",
                    CcStatus::Working          => "\x1b[34mWORK\x1b[0m",
                    CcStatus::WaitingForPrompt => "\x1b[32mWAIT\x1b[0m",
                    CcStatus::Compact          => "\x1b[2mCMPT\x1b[0m",
                    CcStatus::Unknown          => "\x1b[33m????\x1b[0m",
                };
                println!("   {} {}  sent={} done={}",
                    s, info.position,
                    info.dispatches_sent, info.dispatches_completed,
                );
            }
            println!("{}", "─".repeat(width));
        }

        // ── Recent alerts ─────────────────────────────────────────────────────
        if !alerts.alerts.is_empty() && rows > 14 {
            println!(" ALERTS");
            for alert in alerts.alerts.iter().rev().take(5) {
                let col = match alert.level {
                    AlertLevel::Emergency => "\x1b[1;31m",
                    AlertLevel::Critical  => "\x1b[31m",
                    AlertLevel::Warn      => "\x1b[33m",
                    AlertLevel::Info      => "\x1b[36m",
                };
                let cmd_icon = if alert.command.is_some() { "⚡" } else { " " };
                println!("  {}{:?}\x1b[0m t={} {}{}",
                    col, alert.level, alert.timestamp, cmd_icon, alert.message,
                );
            }
            println!("{}", "─".repeat(width));
        }

        // ── Footer ────────────────────────────────────────────────────────────
        if rows > 18 {
            println!(" \x1b[2mpipe: claim · release · dispatch · broadcast · status · cmd\x1b[0m");
        }
    }

    /// Build full status response — called once per tick, served from cache
    pub fn build_status(
        metrics:  &BusMetrics,
        arms:     &ArmRegistry,
        discovery: &Discovery,
        coord:    &Coordination,
        orac:     &Option<OracHealth>,
        field:    &Option<FieldState>,
        thermal:  &Option<ThermalState>,
        povm:     &Option<PovmState>,
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
