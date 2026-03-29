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

    /// Evaluate all thresholds.
    /// Returns (alerts_to_dispatch, outbound_commands_to_queue).
    pub fn evaluate(
        &mut self,
        tick:    u64,
        orac:    &Option<OracHealth>,
        field:   &Option<FieldState>,
        thermal: &Option<ThermalState>,
    ) -> (Vec<Alert>, Vec<ArmCommand>) {
        let mut fired    = Vec::new();
        let mut commands = Vec::new();

        if let Some(o) = orac {
            // Rolling LTP window
            self.ltp_window.push_back(o.ltp_delta_window);
            if self.ltp_window.len() > self.ltp_window_size {
                self.ltp_window.pop_front();
            }
            let ltp_rate: u32 = self.ltp_window.iter().sum();

            if let Some(f) = field {
                // Convergence trap → ReduceK command to ORAC
                if f.r > 0.999
                    && ltp_rate == 0
                    && self.ltp_window.len() == self.ltp_window_size
                {
                    let k_delta = -(f.k * 0.1_f64).max(0.05);
                    let cmd = ArmCommand::ReduceK {
                        arm:   ArmId::Orac,
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
                        source:    "convergence_detector".to_string(),
                        command:   Some(cmd.clone()),
                    }, tick, &mut fired);
                    commands.push(cmd);
                }
            }

            // RALPH stall
            if o.ralph_fitness < 0.01 && o.ralph_gen > 50 {
                self.try_fire(Alert {
                    level:      AlertLevel::Warn,
                    alert_type: AlertType::RalphStall,
                    message:    format!(
                        "RALPH fit={:.4} gen={}. Stall suspected.",
                        o.ralph_fitness, o.ralph_gen
                    ),
                    timestamp: tick,
                    source:    "ralph_monitor".to_string(),
                    command:   None,
                }, tick, &mut fired);
            }
        }

        // Thermal spike → SetThermal command to SYNTHEX
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
                        "temp={:.2} overshoot={:.2}. Setpoint→{:.2} queued.",
                        t.temperature, overshoot, new_target
                    ),
                    timestamp: tick,
                    source:    "thermal_monitor".to_string(),
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

    /// Write critical/emergency alerts to CC panes
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
