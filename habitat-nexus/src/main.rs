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
    // Domain state — populated by worker→main messages and inline ingest
    orac:         Option<OracHealth>,
    field:        Option<FieldState>,
    thermal:      Option<ThermalState>,
    me:           Option<MeState>,
    povm:         Option<PovmState>,
    nexus_events: Option<NexusEvents>,
    fleet_pulse:  Option<FleetPulse>,
    // Status cache — rebuilt once per tick, served zero-cost on pipe query
    status_cache: StatusResponse,
    // Outbound commands batched for next-tick dispatch
    pending_cmds: Vec<ArmCommand>,
}

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
            EventType::PaneRenderReport,
            EventType::Timer,
            EventType::WebRequestResult,
            EventType::RunCommandResult,
            EventType::CustomMessage,
            EventType::CommandPaneExited,
            EventType::Key,
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
                self.discovery.handle_render_report(reports)
            }
            Event::Timer(elapsed) => self.handle_timer(elapsed),
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
            &self.metrics, &self.arms,
            &self.alert_engine, &self.discovery, &self.coord,
        );
    }
}

// ── Hot path ──────────────────────────────────────────────────────────────────

impl HabitatNexus {
    fn handle_timer(&mut self, _: f64) -> bool {
        self.tick += 1;

        // 1. Fan-out — all ready arms fire in parallel (inbound polls)
        for arm_id in self.arms.ready_to_fire(self.tick) {
            let url    = arm_id.poll_endpoint();
            let ctx    = Self::build_ctx(arm_id, "poll");
            let tx_len = url.len();
            web_request(url, HttpVerb::Get, BTreeMap::new(), vec![], ctx);
            self.arms.get_mut(arm_id).on_fire(self.tick, tx_len);
        }

        // 2. Dispatch pending outbound commands (write path)
        let cmds = std::mem::take(&mut self.pending_cmds);
        for cmd in &cmds {
            self.arms.push(cmd);
        }

        // 3. Flush briefing queue to safe CC panes
        self.discovery.flush_briefing_queue();

        // 4. Evaluate thresholds — may generate new outbound commands
        let (fired, new_cmds) = self.alert_engine.evaluate(
            self.tick, &self.orac, &self.field, &self.thermal,
        );
        if !fired.is_empty() {
            self.alert_engine.dispatch_to_fleet(&fired, &self.discovery);
        }
        // Queue commands for next tick — avoids double-firing in same cycle
        self.pending_cmds.extend(new_cmds);

        // 5. Expire stale claims
        self.coord.expire_claims(self.tick);

        // 6. Update metrics
        self.metrics.tick                = self.tick;
        self.metrics.arms_completed_last = self.arms.completed_last_tick(self.tick);
        self.metrics.active_claims       = self.coord.active_claim_count();

        // 7. Rebuild status cache once per tick
        let nexus_ev = self.nexus_events.as_ref()
            .map_or(0, |n| n.events.len() as u32);
        self.status_cache = Dashboard::build_status(
            &self.metrics, &self.arms, &self.discovery, &self.coord,
            &self.orac, &self.field, &self.thermal, &self.povm, nexus_ev,
        );

        // 8. Persist every 60s
        if self.tick % 12 == 0 { self.save_state(); }

        set_timeout(5.0);
        true
    }

    // ── Inbound + outbound web response router ────────────────────────────────

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
        let is_push = ctx.get("dir").map(|d| d == "push").unwrap_or(false);

        // Push acknowledgement — update arm metrics only, no re-render
        if is_push {
            if status == 200 || status == 204 {
                self.arms.get_mut(arm_id).on_success(body.len(), self.tick);
            } else {
                self.arms.get_mut(arm_id).on_failure(self.tick);
            }
            return false;
        }

        // Poll response
        if status != 200 {
            self.arms.get_mut(arm_id).on_failure(self.tick);
            return true;
        }
        self.arms.get_mut(arm_id).on_success(body.len(), self.tick);

        // Route: heavy arms → worker (off main thread); light arms → inline
        match arm_id {
            ArmId::Orac => {
                post_message_to("field", "orac_update",
                    &String::from_utf8_lossy(&body));
                false
            }
            ArmId::Pv2 => {
                post_message_to("field", "pv2_update",
                    &String::from_utf8_lossy(&body));
                false
            }
            ArmId::Synthex => {
                post_message_to("thermal", "synthex_update",
                    &String::from_utf8_lossy(&body));
                false
            }
            ArmId::MeV2 => {
                post_message_to("thermal", "me_update",
                    &String::from_utf8_lossy(&body));
                false
            }
            ArmId::Povm => {
                post_message_to("thermal", "povm_update",
                    &String::from_utf8_lossy(&body));
                false
            }
            ArmId::NexusBus => {
                if let Ok(events) = serde_json::from_slice::<NexusEvents>(&body) {
                    self.nexus_events = Some(events);
                }
                true
            }
            ArmId::CcFleet => {
                if let Ok(pulse) = serde_json::from_slice::<FleetPulse>(&body) {
                    self.fleet_pulse = Some(pulse);
                }
                true
            }
        }
    }

    // ── Worker → main state updates ───────────────────────────────────────────

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
                        if let Ok(json) = serde_json::to_string(&ack) {
                            cli_pipe_output(&pid, &json);
                            unblock_cli_pipe_input(&pid);
                        }
                    }
                }
            }
            "broadcast" => {
                self.coord.broadcast(&payload, &self.discovery);
            }
            "status" => {
                // Zero-compute — serve from pre-built cache
                if let PipeSource::Cli(pid) = msg.source {
                    if let Ok(json) = serde_json::to_string(&self.status_cache) {
                        cli_pipe_output(&pid, &json);
                        unblock_cli_pipe_input(&pid);
                    }
                }
            }
            // CC panes can inject commands directly → queued for next tick
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

    #[inline]
    fn build_ctx(id: ArmId, dir: &str) -> BTreeMap<String, String> {
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
