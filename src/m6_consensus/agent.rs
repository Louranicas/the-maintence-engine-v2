//! # M32: Agent Coordinator
//!
//! Manages the lifecycle and coordination of the 40 CVA-NAM agents plus
//! Human `@0.A`. Handles task assignment, status transitions, heartbeat
//! tracking, fleet health calculations, and role-based agent selection.
//!
//! ## Layer: L6 (Consensus)
//!
//! ## Fleet Composition
//!
//! | Role | Count | Weight | Focus |
//! |------|-------|--------|-------|
//! | Validator | 20 (+1 Human) | 1.0 | Correctness verification |
//! | Explorer | 8 | 0.8 | Alternative detection |
//! | Critic | 6 | 1.2 | Flaw detection |
//! | Integrator | 4 | 1.0 | Cross-system impact |
//! | Historian | 2 | 0.8 | Precedent matching |
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M32_AGENT_COORDINATOR.md)
//! - [Agent Roles](../../nam/NAM_SPEC.md)

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::SystemTime;

use crate::{AgentRole, Error, Result};

use super::{create_human_agent, default_agent_fleet, AgentStatus, ConsensusAgent, PBFT_F, PBFT_N, PBFT_Q};

/// Maximum number of tasks retained in the coordinator.
const MAX_TASKS: usize = 1000;

/// Task status for an assigned agent task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is waiting to be started.
    Pending,
    /// Task is currently being worked on.
    InProgress,
    /// Task completed successfully.
    Completed,
    /// Task failed.
    Failed,
    /// Task was cancelled.
    Cancelled,
}

/// A task assigned to an agent.
#[derive(Clone, Debug)]
pub struct AgentTask {
    /// Unique task identifier.
    pub id: String,
    /// Human-readable description.
    pub description: String,
    /// The agent this task is assigned to.
    pub assigned_agent: String,
    /// The role of the assigned agent.
    pub assigned_role: AgentRole,
    /// Current task status.
    pub status: TaskStatus,
    /// When the task was created.
    pub created_at: SystemTime,
    /// When the task was completed (if applicable).
    pub completed_at: Option<SystemTime>,
}

/// Aggregate fleet health snapshot.
#[derive(Clone, Debug)]
pub struct FleetHealth {
    /// Total agents in the fleet.
    pub total_agents: usize,
    /// Number of agents in `Active` status.
    pub active_agents: usize,
    /// Number of agents in `Idle` status.
    pub idle_agents: usize,
    /// Number of agents in `Busy` status.
    pub busy_agents: usize,
    /// Number of agents in `Failed` status.
    pub failed_agents: usize,
    /// Number of agents in `Offline` status.
    pub offline_agents: usize,
    /// Overall health score (0.0 - 1.0).
    pub health_score: f64,
}

/// Distribution summary for a single agent role.
#[derive(Clone, Debug)]
pub struct RoleDistribution {
    /// The agent role.
    pub role: AgentRole,
    /// Total count of agents with this role.
    pub count: usize,
    /// Number of agents with this role that are `Active` or `Idle`.
    pub active_count: usize,
    /// Average success rate across agents with this role.
    pub avg_success_rate: f64,
}

/// Historical record for an individual agent.
#[derive(Clone, Debug)]
pub struct AgentRecord {
    /// Agent identifier.
    pub agent_id: String,
    /// Number of tasks completed successfully.
    pub tasks_completed: u64,
    /// Number of tasks that failed.
    pub tasks_failed: u64,
    /// Uptime in seconds (cumulative).
    pub uptime_secs: u64,
    /// Last heartbeat timestamp.
    pub last_heartbeat: Option<SystemTime>,
}

/// Agent coordinator for the CVA-NAM fleet.
///
/// Manages 41 agents (40 CVA-NAM + Human `@0.A`), assigns tasks,
/// tracks heartbeats, and computes fleet health. All state is
/// protected by `std::sync::RwLock` for thread-safe access.
pub struct AgentCoordinator {
    /// Agents keyed by agent ID.
    agents: RwLock<HashMap<String, ConsensusAgent>>,
    /// Tasks keyed by task ID (bounded at `MAX_TASKS`).
    tasks: RwLock<HashMap<String, AgentTask>>,
    /// Per-agent historical records keyed by agent ID.
    records: RwLock<HashMap<String, AgentRecord>>,
    /// Monotonically increasing task counter.
    task_counter: RwLock<u64>,
}

impl AgentCoordinator {
    /// Create a new coordinator with the default agent fleet (41 agents).
    ///
    /// Initialises agents from `default_agent_fleet()`, creates empty
    /// task and record maps, and sets the task counter to zero.
    #[must_use]
    pub fn new() -> Self {
        let fleet = default_agent_fleet();
        let mut agent_map = HashMap::with_capacity(fleet.len());
        let mut record_map = HashMap::with_capacity(fleet.len());

        for agent in fleet {
            let record = AgentRecord {
                agent_id: agent.id.clone(),
                tasks_completed: 0,
                tasks_failed: 0,
                uptime_secs: 0,
                last_heartbeat: agent.last_heartbeat,
            };
            record_map.insert(agent.id.clone(), record);
            agent_map.insert(agent.id.clone(), agent);
        }

        Self {
            agents: RwLock::new(agent_map),
            tasks: RwLock::new(HashMap::new()),
            records: RwLock::new(record_map),
            task_counter: RwLock::new(0),
        }
    }

    /// Return the number of agents in the fleet.
    #[must_use]
    pub fn agent_count(&self) -> usize {
        let Ok(agents) = self.agents.read() else {
            return 0;
        };
        agents.len()
    }

    /// Retrieve a clone of an agent by ID.
    #[must_use]
    pub fn get_agent(&self, id: &str) -> Option<ConsensusAgent> {
        let Ok(agents) = self.agents.read() else {
            return None;
        };
        agents.get(id).cloned()
    }

    /// Set the status of an agent.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the agent is not found.
    /// Returns `Error::Other` if the lock is poisoned.
    pub fn set_status(&self, id: &str, status: AgentStatus) -> Result<()> {
        let mut agents = self
            .agents
            .write()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;
        let agent = agents
            .get_mut(id)
            .ok_or_else(|| Error::Validation(format!("Agent not found: {id}")))?;
        agent.status = status;
        drop(agents);
        Ok(())
    }

    /// Update the heartbeat timestamp for an agent.
    ///
    /// Also updates the corresponding `AgentRecord`.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the agent is not found.
    /// Returns `Error::Other` if the lock is poisoned.
    pub fn heartbeat(&self, id: &str) -> Result<()> {
        let now = SystemTime::now();

        let mut agents = self
            .agents
            .write()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;
        let agent = agents
            .get_mut(id)
            .ok_or_else(|| Error::Validation(format!("Agent not found: {id}")))?;
        agent.last_heartbeat = Some(now);
        drop(agents);

        let mut records = self
            .records
            .write()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;
        if let Some(record) = records.get_mut(id) {
            record.last_heartbeat = Some(now);
        }
        drop(records);

        Ok(())
    }

    /// Assign a task to an agent.
    ///
    /// Creates a new `AgentTask`, transitions the agent to `Busy` status,
    /// and returns the generated task ID. Respects `MAX_TASKS` capacity.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the agent is not found or the task
    /// store is at capacity. Returns `Error::Other` if the lock is poisoned.
    pub fn assign_task(&self, agent_id: &str, description: &str) -> Result<String> {
        // Validate agent exists and update status
        let mut agents = self
            .agents
            .write()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;
        let agent = agents
            .get_mut(agent_id)
            .ok_or_else(|| Error::Validation(format!("Agent not found: {agent_id}")))?;
        agent.status = AgentStatus::Busy;
        drop(agents);

        // Generate task ID
        let mut counter = self
            .task_counter
            .write()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;
        let task_id = format!("task-{counter:04}");
        *counter += 1;
        drop(counter);

        // Look up agent role
        let agents = self
            .agents
            .read()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;
        let role = agents
            .get(agent_id)
            .map_or(AgentRole::Validator, |a| a.role);
        drop(agents);

        let task = AgentTask {
            id: task_id.clone(),
            description: description.into(),
            assigned_agent: agent_id.into(),
            assigned_role: role,
            status: TaskStatus::InProgress,
            created_at: SystemTime::now(),
            completed_at: None,
        };

        {
            let mut tasks = self
                .tasks
                .write()
                .map_err(|_| Error::Other("Lock poisoned".into()))?;
            if tasks.len() >= MAX_TASKS {
                return Err(Error::Validation("Task store is at capacity".into()));
            }
            tasks.insert(task_id.clone(), task);
        }

        Ok(task_id)
    }

    /// Complete a task, marking it as `Completed` or `Failed`.
    ///
    /// Transitions the assigned agent back to `Active` and updates
    /// the agent's record.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the task is not found.
    /// Returns `Error::Other` if the lock is poisoned.
    pub fn complete_task(&self, task_id: &str, success: bool) -> Result<()> {
        let mut tasks = self
            .tasks
            .write()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| Error::Validation(format!("Task not found: {task_id}")))?;
        task.status = if success {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        };
        task.completed_at = Some(SystemTime::now());
        let agent_id = task.assigned_agent.clone();
        drop(tasks);

        // Transition agent back to Active
        let mut agents = self
            .agents
            .write()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;
        if let Some(agent) = agents.get_mut(&agent_id) {
            agent.status = AgentStatus::Active;
        }
        drop(agents);

        // Update record
        let mut records = self
            .records
            .write()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;
        if let Some(record) = records.get_mut(&agent_id) {
            if success {
                record.tasks_completed += 1;
            } else {
                record.tasks_failed += 1;
            }
        }
        drop(records);

        Ok(())
    }

    /// Cancel a task.
    ///
    /// Marks the task as `Cancelled` and transitions the assigned agent
    /// back to `Active`.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the task is not found.
    /// Returns `Error::Other` if the lock is poisoned.
    pub fn cancel_task(&self, task_id: &str) -> Result<()> {
        let mut tasks = self
            .tasks
            .write()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| Error::Validation(format!("Task not found: {task_id}")))?;
        task.status = TaskStatus::Cancelled;
        task.completed_at = Some(SystemTime::now());
        let agent_id = task.assigned_agent.clone();
        drop(tasks);

        let mut agents = self
            .agents
            .write()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;
        if let Some(agent) = agents.get_mut(&agent_id) {
            agent.status = AgentStatus::Active;
        }
        drop(agents);

        Ok(())
    }

    /// Compute a snapshot of fleet health.
    ///
    /// The `health_score` is the ratio of operational agents (`Active`,
    /// `Idle`, or `Busy`) to the total fleet size.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn fleet_health(&self) -> FleetHealth {
        let Ok(agents) = self.agents.read() else {
            return FleetHealth {
                total_agents: 0,
                active_agents: 0,
                idle_agents: 0,
                busy_agents: 0,
                failed_agents: 0,
                offline_agents: 0,
                health_score: 0.0,
            };
        };

        let total_agents = agents.len();
        let mut active_agents = 0usize;
        let mut idle_agents = 0usize;
        let mut busy_agents = 0usize;
        let mut failed_agents = 0usize;
        let mut offline_agents = 0usize;

        for agent in agents.values() {
            match agent.status {
                AgentStatus::Active => active_agents += 1,
                AgentStatus::Idle => idle_agents += 1,
                AgentStatus::Busy => busy_agents += 1,
                AgentStatus::Failed => failed_agents += 1,
                AgentStatus::Offline => offline_agents += 1,
            }
        }

        let operational = active_agents + idle_agents + busy_agents;
        let health_score = if total_agents == 0 {
            0.0
        } else {
            operational as f64 / total_agents as f64
        };

        FleetHealth {
            total_agents,
            active_agents,
            idle_agents,
            busy_agents,
            failed_agents,
            offline_agents,
            health_score,
        }
    }

    /// Compute the distribution of roles across the fleet.
    ///
    /// Returns one `RoleDistribution` entry per role present in the fleet,
    /// with counts, active counts, and average success rates. Sorted by
    /// count descending for deterministic output.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn role_distribution(&self) -> Vec<RoleDistribution> {
        let Ok(agents) = self.agents.read() else {
            return Vec::new();
        };

        let roles = [
            AgentRole::Validator,
            AgentRole::Explorer,
            AgentRole::Critic,
            AgentRole::Integrator,
            AgentRole::Historian,
        ];

        let mut result: Vec<RoleDistribution> = roles
            .iter()
            .filter_map(|&role| {
                let mut count = 0usize;
                let mut active_count = 0usize;
                let mut total_rate = 0.0_f64;

                for agent in agents.values() {
                    if agent.role == role {
                        count += 1;
                        if agent.status == AgentStatus::Active
                            || agent.status == AgentStatus::Idle
                        {
                            active_count += 1;
                        }
                        total_rate += agent.success_rate;
                    }
                }

                if count == 0 {
                    return None;
                }

                let avg_success_rate = total_rate / count as f64;

                Some(RoleDistribution {
                    role,
                    count,
                    active_count,
                    avg_success_rate,
                })
            })
            .collect();

        // Sort by count descending for deterministic output
        result.sort_by(|a, b| b.count.cmp(&a.count));
        result
    }

    /// Return all agents with the given role.
    #[must_use]
    pub fn agents_by_role(&self, role: AgentRole) -> Vec<ConsensusAgent> {
        let Ok(agents) = self.agents.read() else {
            return Vec::new();
        };
        agents
            .values()
            .filter(|a| a.role == role)
            .cloned()
            .collect()
    }

    /// Return all available agents (status `Active` or `Idle`).
    #[must_use]
    pub fn available_agents(&self) -> Vec<ConsensusAgent> {
        let Ok(agents) = self.agents.read() else {
            return Vec::new();
        };
        agents
            .values()
            .filter(|a| a.status == AgentStatus::Active || a.status == AgentStatus::Idle)
            .cloned()
            .collect()
    }

    /// Return all agents in `Failed` status.
    #[must_use]
    pub fn failed_agents(&self) -> Vec<ConsensusAgent> {
        let Ok(agents) = self.agents.read() else {
            return Vec::new();
        };
        agents
            .values()
            .filter(|a| a.status == AgentStatus::Failed)
            .cloned()
            .collect()
    }

    /// Return the number of tasks tracked.
    #[must_use]
    pub fn task_count(&self) -> usize {
        let Ok(tasks) = self.tasks.read() else {
            return 0;
        };
        tasks.len()
    }

    /// Return all tasks in `Pending` or `InProgress` status.
    #[must_use]
    pub fn pending_tasks(&self) -> Vec<AgentTask> {
        let Ok(tasks) = self.tasks.read() else {
            return Vec::new();
        };
        tasks
            .values()
            .filter(|t| t.status == TaskStatus::Pending || t.status == TaskStatus::InProgress)
            .cloned()
            .collect()
    }

    /// Update the success rate of an agent.
    ///
    /// The rate is clamped to the `[0.0, 1.0]` range.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the agent is not found.
    /// Returns `Error::Other` if the lock is poisoned.
    pub fn update_success_rate(&self, id: &str, rate: f64) -> Result<()> {
        let mut agents = self
            .agents
            .write()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;
        let agent = agents
            .get_mut(id)
            .ok_or_else(|| Error::Validation(format!("Agent not found: {id}")))?;
        agent.success_rate = rate.clamp(0.0, 1.0);
        drop(agents);
        Ok(())
    }

    /// Select the best available agent for the given role.
    ///
    /// Picks the agent with the highest success rate among those that
    /// are `Active` or `Idle` and match the requested role.
    #[must_use]
    pub fn select_agent_for_role(&self, role: AgentRole) -> Option<ConsensusAgent> {
        let Ok(agents) = self.agents.read() else {
            return None;
        };
        agents
            .values()
            .filter(|a| {
                a.role == role
                    && (a.status == AgentStatus::Active || a.status == AgentStatus::Idle)
            })
            .max_by(|a, b| a.success_rate.partial_cmp(&b.success_rate).unwrap_or(std::cmp::Ordering::Equal))
            .cloned()
    }

    /// Retrieve the Human `@0.A` agent, if present.
    #[must_use]
    pub fn human_agent(&self) -> Option<ConsensusAgent> {
        self.get_agent("@0.A")
    }

    /// Reset an agent to `Idle` status and clear any in-progress tasks.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the agent is not found.
    /// Returns `Error::Other` if the lock is poisoned.
    pub fn reset_agent(&self, id: &str) -> Result<()> {
        let mut agents = self
            .agents
            .write()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;
        let agent = agents
            .get_mut(id)
            .ok_or_else(|| Error::Validation(format!("Agent not found: {id}")))?;
        agent.status = AgentStatus::Idle;
        drop(agents);

        // Cancel any in-progress tasks assigned to this agent
        let mut tasks = self
            .tasks
            .write()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;
        for task in tasks.values_mut() {
            if task.assigned_agent == id && task.status == TaskStatus::InProgress {
                task.status = TaskStatus::Cancelled;
                task.completed_at = Some(SystemTime::now());
            }
        }
        drop(tasks);

        Ok(())
    }
}

impl Default for AgentCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

// Ensure PBFT constants and `create_human_agent` are reachable from this module
// for use by downstream integration code. The const assertions suppress unused
// import warnings while validating the invariant.
const _: u32 = PBFT_N;
const _: u32 = PBFT_F;
const _: u32 = PBFT_Q;

/// Create a standalone Human `@0.A` agent for use outside the coordinator.
///
/// Delegates to `super::create_human_agent()`.
#[must_use]
pub fn standalone_human_agent() -> ConsensusAgent {
    create_human_agent()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // Fleet initialisation
    // ---------------------------------------------------------------

    #[test]
    fn test_default_fleet_has_41_agents() {
        let coord = AgentCoordinator::new();
        assert_eq!(coord.agent_count(), 41);
    }

    #[test]
    fn test_default_fleet_contains_human() {
        let coord = AgentCoordinator::new();
        let human = coord.get_agent("@0.A");
        assert!(human.is_some());
    }

    #[test]
    fn test_default_fleet_human_properties() {
        let coord = AgentCoordinator::new();
        let human = coord.human_agent().unwrap_or_else(|| unreachable!());
        assert_eq!(human.id, "@0.A");
        assert_eq!(human.tier, 0);
        assert_eq!(human.role, AgentRole::Validator);
        assert_eq!(human.status, AgentStatus::Active);
        assert!((human.weight - 1.0).abs() < f64::EPSILON);
        assert!((human.success_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_default_fleet_validator_count() {
        let coord = AgentCoordinator::new();
        let validators = coord.agents_by_role(AgentRole::Validator);
        // 20 CVA-NAM validators + 1 Human @0.A
        assert_eq!(validators.len(), 21);
    }

    #[test]
    fn test_default_fleet_explorer_count() {
        let coord = AgentCoordinator::new();
        let explorers = coord.agents_by_role(AgentRole::Explorer);
        assert_eq!(explorers.len(), 8);
    }

    #[test]
    fn test_default_fleet_critic_count() {
        let coord = AgentCoordinator::new();
        let critics = coord.agents_by_role(AgentRole::Critic);
        assert_eq!(critics.len(), 6);
    }

    #[test]
    fn test_default_fleet_integrator_count() {
        let coord = AgentCoordinator::new();
        let integrators = coord.agents_by_role(AgentRole::Integrator);
        assert_eq!(integrators.len(), 4);
    }

    #[test]
    fn test_default_fleet_historian_count() {
        let coord = AgentCoordinator::new();
        let historians = coord.agents_by_role(AgentRole::Historian);
        assert_eq!(historians.len(), 2);
    }

    // ---------------------------------------------------------------
    // Agent status transitions
    // ---------------------------------------------------------------

    #[test]
    fn test_set_status_active() {
        let coord = AgentCoordinator::new();
        let result = coord.set_status("agent-01", AgentStatus::Active);
        assert!(result.is_ok());
        let agent = coord.get_agent("agent-01").unwrap_or_else(|| unreachable!());
        assert_eq!(agent.status, AgentStatus::Active);
    }

    #[test]
    fn test_set_status_failed() {
        let coord = AgentCoordinator::new();
        let result = coord.set_status("agent-02", AgentStatus::Failed);
        assert!(result.is_ok());
        let agent = coord.get_agent("agent-02").unwrap_or_else(|| unreachable!());
        assert_eq!(agent.status, AgentStatus::Failed);
    }

    #[test]
    fn test_set_status_offline() {
        let coord = AgentCoordinator::new();
        let result = coord.set_status("agent-03", AgentStatus::Offline);
        assert!(result.is_ok());
        let agent = coord.get_agent("agent-03").unwrap_or_else(|| unreachable!());
        assert_eq!(agent.status, AgentStatus::Offline);
    }

    #[test]
    fn test_set_status_nonexistent_agent_fails() {
        let coord = AgentCoordinator::new();
        let result = coord.set_status("nonexistent", AgentStatus::Active);
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // Heartbeat
    // ---------------------------------------------------------------

    #[test]
    fn test_heartbeat_updates_agent() {
        let coord = AgentCoordinator::new();
        // agent-01 starts with no heartbeat
        let before = coord.get_agent("agent-01").unwrap_or_else(|| unreachable!());
        assert!(before.last_heartbeat.is_none());

        let result = coord.heartbeat("agent-01");
        assert!(result.is_ok());

        let after = coord.get_agent("agent-01").unwrap_or_else(|| unreachable!());
        assert!(after.last_heartbeat.is_some());
    }

    #[test]
    fn test_heartbeat_updates_record() {
        let coord = AgentCoordinator::new();
        let result = coord.heartbeat("agent-05");
        assert!(result.is_ok());

        let records = coord.records.read().unwrap_or_else(|_| unreachable!());
        let record = records.get("agent-05").unwrap_or_else(|| unreachable!());
        assert!(record.last_heartbeat.is_some());
    }

    #[test]
    fn test_heartbeat_nonexistent_agent_fails() {
        let coord = AgentCoordinator::new();
        let result = coord.heartbeat("nonexistent");
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // Task assignment and completion
    // ---------------------------------------------------------------

    #[test]
    fn test_assign_task() {
        let coord = AgentCoordinator::new();
        let task_id = coord
            .assign_task("agent-01", "Run health check")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(task_id, "task-0000");
        assert_eq!(coord.task_count(), 1);
    }

    #[test]
    fn test_assign_task_sets_agent_busy() {
        let coord = AgentCoordinator::new();
        let _ = coord.assign_task("agent-01", "Run health check");
        let agent = coord.get_agent("agent-01").unwrap_or_else(|| unreachable!());
        assert_eq!(agent.status, AgentStatus::Busy);
    }

    #[test]
    fn test_assign_task_nonexistent_agent_fails() {
        let coord = AgentCoordinator::new();
        let result = coord.assign_task("nonexistent", "Some task");
        assert!(result.is_err());
    }

    #[test]
    fn test_assign_task_increments_counter() {
        let coord = AgentCoordinator::new();
        let t0 = coord
            .assign_task("agent-01", "Task A")
            .unwrap_or_else(|_| unreachable!());
        let t1 = coord
            .assign_task("agent-02", "Task B")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(t0, "task-0000");
        assert_eq!(t1, "task-0001");
    }

    #[test]
    fn test_complete_task_success() {
        let coord = AgentCoordinator::new();
        let task_id = coord
            .assign_task("agent-01", "Run health check")
            .unwrap_or_else(|_| unreachable!());

        let result = coord.complete_task(&task_id, true);
        assert!(result.is_ok());

        let agent = coord.get_agent("agent-01").unwrap_or_else(|| unreachable!());
        assert_eq!(agent.status, AgentStatus::Active);

        let records = coord.records.read().unwrap_or_else(|_| unreachable!());
        let record = records.get("agent-01").unwrap_or_else(|| unreachable!());
        assert_eq!(record.tasks_completed, 1);
        assert_eq!(record.tasks_failed, 0);
    }

    #[test]
    fn test_complete_task_failure() {
        let coord = AgentCoordinator::new();
        let task_id = coord
            .assign_task("agent-02", "Risky operation")
            .unwrap_or_else(|_| unreachable!());

        let result = coord.complete_task(&task_id, false);
        assert!(result.is_ok());

        let records = coord.records.read().unwrap_or_else(|_| unreachable!());
        let record = records.get("agent-02").unwrap_or_else(|| unreachable!());
        assert_eq!(record.tasks_completed, 0);
        assert_eq!(record.tasks_failed, 1);
    }

    #[test]
    fn test_complete_task_nonexistent_fails() {
        let coord = AgentCoordinator::new();
        let result = coord.complete_task("nonexistent-task", true);
        assert!(result.is_err());
    }

    #[test]
    fn test_complete_task_sets_completed_at() {
        let coord = AgentCoordinator::new();
        let task_id = coord
            .assign_task("agent-01", "Check logs")
            .unwrap_or_else(|_| unreachable!());
        let _ = coord.complete_task(&task_id, true);

        let tasks = coord.tasks.read().unwrap_or_else(|_| unreachable!());
        let task = tasks.get(&task_id).unwrap_or_else(|| unreachable!());
        assert!(task.completed_at.is_some());
        assert_eq!(task.status, TaskStatus::Completed);
    }

    // ---------------------------------------------------------------
    // Task cancellation
    // ---------------------------------------------------------------

    #[test]
    fn test_cancel_task() {
        let coord = AgentCoordinator::new();
        let task_id = coord
            .assign_task("agent-03", "Deploy update")
            .unwrap_or_else(|_| unreachable!());

        let result = coord.cancel_task(&task_id);
        assert!(result.is_ok());

        let tasks = coord.tasks.read().unwrap_or_else(|_| unreachable!());
        let task = tasks.get(&task_id).unwrap_or_else(|| unreachable!());
        assert_eq!(task.status, TaskStatus::Cancelled);
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn test_cancel_task_resets_agent_to_active() {
        let coord = AgentCoordinator::new();
        let task_id = coord
            .assign_task("agent-04", "Deploy update")
            .unwrap_or_else(|_| unreachable!());
        let _ = coord.cancel_task(&task_id);

        let agent = coord.get_agent("agent-04").unwrap_or_else(|| unreachable!());
        assert_eq!(agent.status, AgentStatus::Active);
    }

    #[test]
    fn test_cancel_nonexistent_task_fails() {
        let coord = AgentCoordinator::new();
        let result = coord.cancel_task("nonexistent");
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // Fleet health
    // ---------------------------------------------------------------

    #[test]
    fn test_fleet_health_default() {
        let coord = AgentCoordinator::new();
        let health = coord.fleet_health();
        assert_eq!(health.total_agents, 41);
        // Human is Active, 40 others are Idle
        assert_eq!(health.active_agents, 1);
        assert_eq!(health.idle_agents, 40);
        assert_eq!(health.busy_agents, 0);
        assert_eq!(health.failed_agents, 0);
        assert_eq!(health.offline_agents, 0);
        assert!((health.health_score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fleet_health_with_failures() {
        let coord = AgentCoordinator::new();
        let _ = coord.set_status("agent-01", AgentStatus::Failed);
        let _ = coord.set_status("agent-02", AgentStatus::Failed);
        let _ = coord.set_status("agent-03", AgentStatus::Offline);

        let health = coord.fleet_health();
        assert_eq!(health.failed_agents, 2);
        assert_eq!(health.offline_agents, 1);
        // 41 - 3 = 38 operational
        assert!((health.health_score - 38.0 / 41.0).abs() < f64::EPSILON);
    }

    // ---------------------------------------------------------------
    // Role distribution
    // ---------------------------------------------------------------

    #[test]
    fn test_role_distribution_count() {
        let coord = AgentCoordinator::new();
        let dist = coord.role_distribution();
        // 5 distinct roles
        assert_eq!(dist.len(), 5);
    }

    #[test]
    fn test_role_distribution_validator_entry() {
        let coord = AgentCoordinator::new();
        let dist = coord.role_distribution();
        let validator = dist
            .iter()
            .find(|d| d.role == AgentRole::Validator)
            .unwrap_or_else(|| unreachable!());
        // 20 CVA-NAM + 1 Human
        assert_eq!(validator.count, 21);
        // Human is Active (1), 20 are Idle -> all 21 are active_count
        assert_eq!(validator.active_count, 21);
    }

    #[test]
    fn test_role_distribution_avg_success_rate() {
        let coord = AgentCoordinator::new();
        let _ = coord.update_success_rate("agent-29", 0.9);
        let _ = coord.update_success_rate("agent-30", 0.7);
        let dist = coord.role_distribution();
        let critic = dist
            .iter()
            .find(|d| d.role == AgentRole::Critic)
            .unwrap_or_else(|| unreachable!());
        // 4 critics at 0.5 + agent-29 at 0.9 + agent-30 at 0.7 = (0.5*4 + 0.9 + 0.7) / 6
        let expected = (0.5 * 4.0 + 0.9 + 0.7) / 6.0;
        assert!((critic.avg_success_rate - expected).abs() < f64::EPSILON);
    }

    // ---------------------------------------------------------------
    // Available and failed agents
    // ---------------------------------------------------------------

    #[test]
    fn test_available_agents_default() {
        let coord = AgentCoordinator::new();
        let available = coord.available_agents();
        // Human is Active, 40 others are Idle -> all 41 are available
        assert_eq!(available.len(), 41);
    }

    #[test]
    fn test_available_agents_excludes_busy() {
        let coord = AgentCoordinator::new();
        let _ = coord.assign_task("agent-01", "Busy work");
        let available = coord.available_agents();
        assert_eq!(available.len(), 40);
    }

    #[test]
    fn test_available_agents_excludes_failed() {
        let coord = AgentCoordinator::new();
        let _ = coord.set_status("agent-05", AgentStatus::Failed);
        let _ = coord.set_status("agent-06", AgentStatus::Offline);
        let available = coord.available_agents();
        assert_eq!(available.len(), 39);
    }

    #[test]
    fn test_failed_agents_returns_failed_only() {
        let coord = AgentCoordinator::new();
        let _ = coord.set_status("agent-10", AgentStatus::Failed);
        let _ = coord.set_status("agent-11", AgentStatus::Failed);
        let _ = coord.set_status("agent-12", AgentStatus::Offline);
        let failed = coord.failed_agents();
        assert_eq!(failed.len(), 2);
    }

    // ---------------------------------------------------------------
    // Agent selection
    // ---------------------------------------------------------------

    #[test]
    fn test_select_agent_for_role_validator() {
        let coord = AgentCoordinator::new();
        // Human @0.A has success_rate 1.0, highest among validators
        let agent = coord.select_agent_for_role(AgentRole::Validator);
        assert!(agent.is_some());
        let agent = agent.unwrap_or_else(|| unreachable!());
        assert_eq!(agent.role, AgentRole::Validator);
        // Should pick @0.A with success_rate 1.0
        assert!((agent.success_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_select_agent_for_role_critic() {
        let coord = AgentCoordinator::new();
        let _ = coord.update_success_rate("agent-29", 0.95);
        let agent = coord
            .select_agent_for_role(AgentRole::Critic)
            .unwrap_or_else(|| unreachable!());
        assert_eq!(agent.role, AgentRole::Critic);
        assert!((agent.success_rate - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn test_select_agent_excludes_offline() {
        let coord = AgentCoordinator::new();
        // Set all historians to Offline
        let _ = coord.set_status("agent-39", AgentStatus::Offline);
        let _ = coord.set_status("agent-40", AgentStatus::Offline);
        let agent = coord.select_agent_for_role(AgentRole::Historian);
        assert!(agent.is_none());
    }

    // ---------------------------------------------------------------
    // Pending tasks
    // ---------------------------------------------------------------

    #[test]
    fn test_pending_tasks_includes_in_progress() {
        let coord = AgentCoordinator::new();
        let _ = coord.assign_task("agent-01", "Task A");
        let _ = coord.assign_task("agent-02", "Task B");
        let pending = coord.pending_tasks();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn test_pending_tasks_excludes_completed() {
        let coord = AgentCoordinator::new();
        let t0 = coord
            .assign_task("agent-01", "Task A")
            .unwrap_or_else(|_| unreachable!());
        let _ = coord.assign_task("agent-02", "Task B");
        let _ = coord.complete_task(&t0, true);
        let pending = coord.pending_tasks();
        assert_eq!(pending.len(), 1);
    }

    // ---------------------------------------------------------------
    // Success rate
    // ---------------------------------------------------------------

    #[test]
    fn test_update_success_rate() {
        let coord = AgentCoordinator::new();
        let _ = coord.update_success_rate("agent-01", 0.85);
        let agent = coord.get_agent("agent-01").unwrap_or_else(|| unreachable!());
        assert!((agent.success_rate - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_update_success_rate_clamps_high() {
        let coord = AgentCoordinator::new();
        let _ = coord.update_success_rate("agent-01", 1.5);
        let agent = coord.get_agent("agent-01").unwrap_or_else(|| unreachable!());
        assert!((agent.success_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_update_success_rate_clamps_low() {
        let coord = AgentCoordinator::new();
        let _ = coord.update_success_rate("agent-01", -0.5);
        let agent = coord.get_agent("agent-01").unwrap_or_else(|| unreachable!());
        assert!((agent.success_rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_update_success_rate_nonexistent_fails() {
        let coord = AgentCoordinator::new();
        let result = coord.update_success_rate("nonexistent", 0.5);
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // Reset agent
    // ---------------------------------------------------------------

    #[test]
    fn test_reset_agent() {
        let coord = AgentCoordinator::new();
        let _ = coord.set_status("agent-01", AgentStatus::Busy);
        let result = coord.reset_agent("agent-01");
        assert!(result.is_ok());
        let agent = coord.get_agent("agent-01").unwrap_or_else(|| unreachable!());
        assert_eq!(agent.status, AgentStatus::Idle);
    }

    #[test]
    fn test_reset_agent_cancels_tasks() {
        let coord = AgentCoordinator::new();
        let task_id = coord
            .assign_task("agent-01", "Running job")
            .unwrap_or_else(|_| unreachable!());
        let _ = coord.reset_agent("agent-01");

        let tasks = coord.tasks.read().unwrap_or_else(|_| unreachable!());
        let task = tasks.get(&task_id).unwrap_or_else(|| unreachable!());
        assert_eq!(task.status, TaskStatus::Cancelled);
    }

    #[test]
    fn test_reset_nonexistent_agent_fails() {
        let coord = AgentCoordinator::new();
        let result = coord.reset_agent("nonexistent");
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // Edge cases
    // ---------------------------------------------------------------

    #[test]
    fn test_get_nonexistent_agent_returns_none() {
        let coord = AgentCoordinator::new();
        assert!(coord.get_agent("nonexistent").is_none());
    }

    #[test]
    fn test_human_agent_heartbeat_initially_set() {
        let coord = AgentCoordinator::new();
        let human = coord.human_agent().unwrap_or_else(|| unreachable!());
        // Human @0.A has a heartbeat set during fleet creation
        assert!(human.last_heartbeat.is_some());
    }

    #[test]
    fn test_default_impl() {
        let coord = AgentCoordinator::default();
        assert_eq!(coord.agent_count(), 41);
    }
}
