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
            if c.status == ClaimStatus::Active
                && (tick - c.claimed_at) * 5 > c.timeout_secs
            {
                c.status = ClaimStatus::TimedOut;
            }
        }
    }

    pub fn dispatch(
        &mut self,
        req:       DispatchRequest,
        discovery: &Discovery,
        tick:      u64,
    ) -> DispatchAck {
        // Prefer role match, fall back to any idle pane
        let target = discovery.cc_panes.values()
            .find(|p| {
                let idle = matches!(p.status,
                    CcStatus::Idle | CcStatus::WaitingForPrompt);
                let role_ok = req.role.as_deref()
                    .map_or(true, |r| p.position.contains(r));
                idle && role_ok
            })
            .or_else(|| {
                discovery.cc_panes.values().find(|p| {
                    matches!(p.status, CcStatus::Idle | CcStatus::WaitingForPrompt)
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
