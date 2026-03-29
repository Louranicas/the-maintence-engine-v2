# Habitat Nexus Plugin — Technical Specification

> **Version:** 1.0.0 | **Status:** DESIGNED | **Target:** Zellij 0.43.x WASM Plugin
> **LOC Estimate:** ~1,400 Rust | **Binary:** ~1MB WASM | **Build:** `cargo build --release --target wasm32-wasip1`
> **Obsidian:** `[[Session 072 — Habitat Nexus Plugin Architecture]]`

## Purpose

A single Zellij WASM plugin that unifies all 16 organically-evolved Habitat communication systems into one in-process channel. Closes 4 structural gaps identified in the Session 072 communication audit: manual discovery (62/100), poll-only signaling (58/100), no mutual exclusion (55/100), and siloed feedback loops (61/100).

Projected impact: **67/100 -> 87/100** overall communication score.

## Plugin Trait Implementation

```rust
use zellij_tile::prelude::*;

#[derive(Default)]
struct HabitatNexus {
    // Discovery
    cc_panes: BTreeMap<PaneId, CcPaneInfo>,

    // Signaling
    alerts: VecDeque<Alert>,
    alert_cooldowns: BTreeMap<AlertType, u64>,

    // Coordination
    claims: BTreeMap<String, TaskClaim>,
    task_queue: Vec<TaskEntry>,

    // Service Bridge
    habitat_state: HabitatState,
    poll_tick: u64,

    // Feedback
    dispatch_outcomes: VecDeque<DispatchOutcome>,

    // UI
    mode: DashboardMode,
    scroll_offset: usize,
}

register_plugin!(HabitatNexus);

impl ZellijPlugin for HabitatNexus {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
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
            EventType::TabUpdate,
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

        // Start polling timer (5-second interval)
        set_timeout(5.0);

        // Load persisted state from /data
        self.load_state();
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PaneUpdate(manifest) => self.handle_pane_update(manifest),
            Event::Timer(elapsed) => self.handle_timer(elapsed),
            Event::WebRequestResult(status, headers, body, ctx) => {
                self.handle_web_response(status, headers, body, ctx)
            }
            Event::RunCommandResult(exit, stdout, stderr, ctx) => {
                self.handle_command_result(exit, stdout, stderr, ctx)
            }
            Event::Key(key) => self.handle_key(key),
            Event::PaneRenderReport(reports) => self.handle_pane_render(reports),
            Event::CommandPaneExited(pane_id, exit_code, ctx) => {
                self.handle_pane_exit(pane_id, exit_code, ctx)
            }
            Event::CustomMessage(name, payload) => {
                self.handle_worker_message(name, payload)
            }
            _ => false,
        }
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        self.handle_pipe(pipe_message)
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.render_dashboard(rows, cols);
    }
}
```

## Data Structures

```rust
struct CcPaneInfo {
    pane_id: PaneId,
    tab_index: usize,
    position: String,          // "alpha-left", "beta-tr", etc.
    status: CcStatus,
    last_seen: u64,
    dispatches_sent: u32,
    dispatches_completed: u32,
    cwd: Option<PathBuf>,
    persona: Option<String>,   // Battern role if assigned
}

enum CcStatus { Idle, Working, Compact, WaitingForPrompt, Unknown }

struct Alert {
    level: AlertLevel,
    alert_type: AlertType,
    message: String,
    timestamp: u64,
    source: String,
}

enum AlertLevel { Info, Warn, Critical, Emergency }

enum AlertType {
    RalphStall,
    FitnessDrop,
    ConvergenceTrap,
    ThermalSpike,
    ServiceDown,
    IpcDisconnect,
}

struct TaskClaim {
    task_id: String,
    claimed_by: PaneId,
    claimed_at: u64,
    timeout_secs: u64,
    status: ClaimStatus,
}

enum ClaimStatus { Active, Completed, TimedOut, Released }

struct TaskEntry {
    id: String,
    description: String,
    priority: Priority,
    claimed_by: Option<PaneId>,
}

enum Priority { High, Medium, Low, Explore }

struct HabitatState {
    orac: Option<OracHealth>,
    synthex: Option<ThermalState>,
    pv2: Option<FieldState>,
    povm_count: u32,
    rm_entries: u32,
    services_up: u8,
    last_poll: u64,
    metabolic: f64,
}

struct OracHealth {
    ralph_gen: u64,
    ralph_fitness: f64,
    ralph_phase: String,
    field_r: f64,
    ltp_total: u64,
    ltd_total: u64,
    coupling_weight_mean: f64,
    emergence_events: u64,
    ipc_state: String,
    sessions: u32,
}

struct ThermalState {
    temperature: f64,
    target: f64,
    pid_output: f64,
    heat_sources: Vec<HeatSource>,
}

struct HeatSource {
    id: String,
    reading: f64,
    weight: f64,
}

struct FieldState {
    r: f64,
    k: f64,
    spheres: u32,
    fleet_mode: String,
}

struct DispatchOutcome {
    pane_id: PaneId,
    task: String,
    dispatched_at: u64,
    completed_at: Option<u64>,
    success: Option<bool>,
}
```

## Module Implementations

### Discovery: `handle_pane_update()`

```rust
fn handle_pane_update(&mut self, manifest: PaneManifest) -> bool {
    let mut new_panes = Vec::new();

    for (tab_idx, panes) in &manifest.panes {
        for pane in panes {
            let is_cc = pane.title.contains("claude")
                || pane.pane_content_command.as_deref() == Some("claude");

            if is_cc && !self.cc_panes.contains_key(&pane.id) {
                let info = CcPaneInfo {
                    pane_id: pane.id,
                    tab_index: *tab_idx,
                    position: self.derive_position(*tab_idx, pane),
                    status: CcStatus::Idle,
                    last_seen: self.poll_tick,
                    dispatches_sent: 0,
                    dispatches_completed: 0,
                    cwd: pane.cwd.clone(),
                    persona: None,
                };
                new_panes.push(info.clone());
                self.cc_panes.insert(pane.id, info);
            }
        }
    }

    // Auto-onboard new CC instances
    for pane in &new_panes {
        self.inject_fleet_briefing(pane);
    }

    !new_panes.is_empty() // re-render if fleet changed
}

fn inject_fleet_briefing(&self, pane: &CcPaneInfo) {
    let briefing = format!(
        "\n[HABITAT NEXUS] Fleet instance {} detected.\n\
         Session: 072 | Wave: {} | Fleet: {}/{} CCs\n\
         ORAC: gen={} fit={:.3} r={:.3}\n\
         Run: atuin scripts run cc-receiver\n",
        pane.position,
        self.habitat_state.last_poll, // wave
        self.cc_panes.values().filter(|p| p.status != CcStatus::Unknown).count(),
        self.cc_panes.len(),
        self.habitat_state.orac.as_ref().map_or(0, |o| o.ralph_gen),
        self.habitat_state.orac.as_ref().map_or(0.0, |o| o.ralph_fitness),
        self.habitat_state.pv2.as_ref().map_or(0.0, |f| f.r),
    );
    write_chars_to_pane_id(&briefing, pane.pane_id);
}
```

### Signaling: `handle_timer()`

```rust
fn handle_timer(&mut self, _elapsed: f64) -> bool {
    self.poll_tick += 1;

    // Poll ORAC every tick (5s)
    web_request(
        "http://localhost:8133/health",
        HttpVerb::Get,
        BTreeMap::new(),
        vec![],
        BTreeMap::from([("source".into(), "orac_poll".into())]),
    );

    // Poll SYNTHEX thermal every tick
    web_request(
        "http://localhost:8090/v3/thermal",
        HttpVerb::Get,
        BTreeMap::new(),
        vec![],
        BTreeMap::from([("source".into(), "synthex_poll".into())]),
    );

    // Poll Nexus Bus every tick
    web_request(
        "http://localhost:8090/v3/nexus/pull",
        HttpVerb::Get,
        BTreeMap::new(),
        vec![],
        BTreeMap::from([("source".into(), "nexus_pull".into())]),
    );

    // Check alert conditions
    self.check_convergence_trap();
    self.check_ralph_stall();
    self.check_thermal_spike();

    // Release timed-out claims
    self.expire_claims();

    // Persist state every 12 ticks (60s)
    if self.poll_tick % 12 == 0 {
        self.save_state();
    }

    // Re-arm timer
    set_timeout(5.0);

    true // always re-render after poll
}

fn check_convergence_trap(&mut self) {
    if let Some(orac) = &self.habitat_state.orac {
        if let Some(pv2) = &self.habitat_state.pv2 {
            if pv2.r > 0.999 && orac.ltp_total == 0 && orac.ralph_gen > 100 {
                self.fire_alert(Alert {
                    level: AlertLevel::Critical,
                    alert_type: AlertType::ConvergenceTrap,
                    message: format!(
                        "r={:.4} + LTP=0 = convergence trap. K={:.2} needs reduction.",
                        pv2.r, pv2.k
                    ),
                    timestamp: self.poll_tick,
                    source: "convergence_detector".into(),
                });
            }
        }
    }
}

fn fire_alert(&mut self, alert: Alert) {
    // Check cooldown
    if let Some(&last) = self.alert_cooldowns.get(&alert.alert_type) {
        let cooldown = match alert.level {
            AlertLevel::Warn => 12,      // 60s
            AlertLevel::Critical => 60,   // 300s
            AlertLevel::Emergency => 360, // 30 min
            _ => 0,
        };
        if self.poll_tick - last < cooldown { return; }
    }

    self.alert_cooldowns.insert(alert.alert_type.clone(), self.poll_tick);

    // Push to fleet panes
    match alert.level {
        AlertLevel::Critical | AlertLevel::Emergency => {
            for (pane_id, info) in &self.cc_panes {
                if info.status != CcStatus::Compact {
                    // Interrupt + alert
                    send_sigint_to_pane_id(*pane_id);
                    write_chars_to_pane_id(
                        &format!("\n[NEXUS {}] {}\n", alert.level, alert.message),
                        *pane_id,
                    );
                }
            }
        }
        AlertLevel::Warn => {
            // Only write to dashboard, don't interrupt CCs
        }
        _ => {}
    }

    self.alerts.push_back(alert);
    if self.alerts.len() > 100 { self.alerts.pop_front(); }
}
```

### Coordination: `handle_pipe()`

```rust
fn handle_pipe(&mut self, msg: PipeMessage) -> bool {
    let payload = msg.payload.unwrap_or_default();
    let action = msg.name.as_deref().unwrap_or("");

    match action {
        "claim" => {
            if let Ok(req) = serde_json::from_str::<ClaimRequest>(&payload) {
                self.process_claim(req, msg.source);
            }
        }
        "release" => {
            if let Ok(req) = serde_json::from_str::<ReleaseRequest>(&payload) {
                self.release_claim(&req.task_id);
            }
        }
        "dispatch" => {
            if let Ok(req) = serde_json::from_str::<DispatchRequest>(&payload) {
                self.dispatch_to_idle(&req.task, &req.role);
            }
        }
        "broadcast" => {
            self.broadcast_to_fleet(&payload);
        }
        "status" => {
            // Return fleet state via CLI pipe output
            if let PipeSource::Cli(pipe_id) = msg.source {
                let status = serde_json::to_string(&self.get_fleet_status()).unwrap_or_default();
                cli_pipe_output(&pipe_id, &status);
                unblock_cli_pipe_input(&pipe_id);
            }
        }
        _ => {}
    }

    true
}

fn process_claim(&mut self, req: ClaimRequest, source: PipeSource) {
    let response = if self.claims.contains_key(&req.task_id) {
        let existing = &self.claims[&req.task_id];
        format!("DENIED: task {} already claimed by {:?}", req.task_id, existing.claimed_by)
    } else {
        self.claims.insert(req.task_id.clone(), TaskClaim {
            task_id: req.task_id.clone(),
            claimed_by: req.pane_id,
            claimed_at: self.poll_tick,
            timeout_secs: req.timeout.unwrap_or(300),
            status: ClaimStatus::Active,
        });
        format!("CLAIMED: task {} by {:?}", req.task_id, req.pane_id)
    };

    // Respond to the requesting pane
    write_chars_to_pane_id(&format!("\n[NEXUS] {}\n", response), req.pane_id);
}
```

## CLI Integration

The plugin responds to `zellij pipe` commands:

```bash
# Query fleet status
zellij pipe -p "file:~/.config/zellij/plugins/habitat-nexus.wasm" -n status

# Claim a task
zellij pipe -p "file:~/.config/zellij/plugins/habitat-nexus.wasm" \
  -n claim -- '{"task_id":"T3","pane_id":"alpha-left"}'

# Broadcast message
zellij pipe -p "file:~/.config/zellij/plugins/habitat-nexus.wasm" \
  -n broadcast -- "RALPH fitness dropped 10%. Investigate."

# Dispatch to idle pane
zellij pipe -p "file:~/.config/zellij/plugins/habitat-nexus.wasm" \
  -n dispatch -- '{"task":"Review convergence trap fix","role":"Verifier"}'
```

## Build and Deploy

```bash
# Prerequisites
rustup target add wasm32-wasip1

# Build
cd habitat-nexus
cargo build --release --target wasm32-wasip1

# Deploy
cp target/wasm32-wasip1/release/habitat_nexus.wasm \
   ~/.config/zellij/plugins/habitat-nexus.wasm

# Launch (floating)
zellij action launch-or-focus-plugin --floating \
  "file:~/.config/zellij/plugins/habitat-nexus.wasm"

# Or add to config.kdl for permanent keybind:
# bind "Alt n" { LaunchOrFocusPlugin "file:~/.config/zellij/plugins/habitat-nexus.wasm" { floating true } }
```

## Layout Integration

```kdl
// In synth-orchestrator.kdl, add to Tab 1:
pane size=1 borderless=true {
    plugin location="file:~/.config/zellij/plugins/habitat-nexus.wasm" {
        poll_interval "5"
        alert_cooldown "60"
        max_fleet_panes "9"
    }
}
```

## Testing Strategy

1. **Unit tests** — Claim logic, alert thresholds, state serialization (in-process, no WASM)
2. **Integration tests** — Build WASM, load in Zellij, verify PaneUpdate fires, verify write_chars reaches panes
3. **Live verification** — Deploy, open 3 fleet CCs, verify auto-discovery + briefing injection
4. **Stress test** — 9 simultaneous fleet panes, rapid dispatch, verify no race conditions in claims

## Migration Path

The plugin does NOT replace existing systems — it wraps them:

1. **Phase 1:** Deploy plugin alongside existing scripts. Plugin monitors, doesn't dispatch.
2. **Phase 2:** Enable discovery auto-briefing. Keep manual dispatch as fallback.
3. **Phase 3:** Enable signaling (alerts). Keep file-based message bus as audit trail.
4. **Phase 4:** Enable coordination (claims). Deprecate Atuin KV claim pattern.
5. **Phase 5:** Enable feedback loops. Wire STDP cross-feeding. Full unified communication.

Each phase is independently revertible by disabling the plugin.

## Cross-References

- **Obsidian:** `[[Session 072 — Habitat Nexus Plugin Architecture]]` — full design rationale
- **Obsidian:** `[[Session 072 — ORAC-SYNTHEX Nexus Bus Design]]` — Nexus Bus (Session 072 deliverable)
- **Obsidian:** `[[Session 071 — Reflections and Learnings]]` — "Communication creates intelligence"
- **Obsidian:** `[[Session 066 — Novel Discoveries]]` — Composition algebra, semantic cluster fusion
- **Obsidian:** `[[Zellij Gold Standard — Session 050 Mastery Skill]]` — Plugin ecosystem
- **Obsidian:** `[[Battern — Patterned Batch Dispatch for Claude Code Fleets]]` — Dispatch protocol
- **API Docs:** https://docs.rs/zellij-tile — Rust plugin SDK
- **Plugin Guide:** https://zellij.dev/documentation/plugins.html — Official documentation
