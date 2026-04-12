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
        tick:     u64,
        orac:     &Option<OracHealth>,
        field_r:  f64,
    ) -> bool {
        let mut changed = false;

        for (tab_idx, panes) in &manifest.panes {
            for pane in panes {
                let is_cc = pane.title.to_lowercase().contains("claude")
                    || pane.pane_content_command.as_deref() == Some("claude");

                if is_cc {
                    if let Some(entry) = self.cc_panes.get_mut(&pane.id) {
                        entry.last_seen = tick;
                        let inferred = Self::infer_status_from_title(&pane.title);
                        if inferred != CcStatus::Unknown {
                            entry.status = inferred;
                        }
                    } else {
                        let info = CcPaneInfo {
                            pane_id:              pane.id,
                            tab_index:            *tab_idx,
                            position:             Self::derive_position(*tab_idx, pane),
                            status:               Self::infer_status_from_title(&pane.title),
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

        let before = self.cc_panes.len();
        self.cc_panes.retain(|_, p| tick - p.last_seen < 6);
        if self.cc_panes.len() != before { changed = true; }

        changed
    }

    /// Infer CC status from pane title tokens (fast path)
    fn infer_status_from_title(title: &str) -> CcStatus {
        let t = title.to_lowercase();
        if t.contains("compact")                      { return CcStatus::Compact; }
        if t.contains("thinking") || t.contains("…") { return CcStatus::Working; }
        if t.contains('>') || t.contains("waiting")  { return CcStatus::WaitingForPrompt; }
        if t.contains("idle") || t.contains("ready") { return CcStatus::Idle; }
        CcStatus::Unknown
    }

    /// Update status from PaneRenderReport — more accurate than title heuristic
    pub fn handle_render_report(&mut self, reports: Vec<PaneRenderReport>) -> bool {
        let mut changed = false;
        for report in reports {
            if let Some(info) = self.cc_panes.get_mut(&PaneId::Terminal(report.pane_id)) {
                let content = &report.content;
                let new_status = if content.contains("(compact)") {
                    CcStatus::Compact
                } else if content.contains("Thinking")
                    || content.contains('\u{280B}') // ⠋
                    || content.contains('\u{2819}') // ⠙
                    || content.contains('\u{2839}') // ⠹
                {
                    CcStatus::Working
                } else if content.ends_with("> ") || content.ends_with("? ") {
                    CcStatus::WaitingForPrompt
                } else if content.contains("claude>") {
                    CcStatus::Idle
                } else {
                    continue;
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
        pane:    &CcPaneInfo,
        tick:    u64,
        orac:    &Option<OracHealth>,
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

    pub fn flush_briefing_queue(&mut self) {
        let mut retry: VecDeque<PendingBriefing> = VecDeque::new();

        while let Some(mut b) = self.pending_briefings.pop_front() {
            match self.cc_panes.get(&b.pane_id).map(|p| &p.status) {
                Some(CcStatus::Idle) | Some(CcStatus::WaitingForPrompt) => {
                    write_chars_to_pane_id(&b.message, b.pane_id);
                }
                None => {} // pane gone — drop
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

    #[inline]
    pub fn interrupt_pane(pane_id: PaneId) {
        write_chars_to_pane_id("\x03", pane_id);
    }
}
