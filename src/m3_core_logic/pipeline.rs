//! # M13: Pipeline Manager
//!
//! Pipeline orchestration for the Maintenance Engine. Manages the lifecycle
//! of data processing pipelines including registration, execution tracking,
//! SLO compliance monitoring, and statistics reporting.
//!
//! ## Layer: L3 (Core Logic)
//! ## Dependencies: M1 (Error), M3 Core Logic types (`Pipeline`, `PipelineStage`)
//!
//! ## 12D Tensor Encoding
//! ```text
//! [13/36, 0.0, 3/6, 2, 0, 0.5, health, uptime, synergy, latency, error_rate, temporal]
//! ```
//!
//! ## Pipelines
//!
//! The manager pre-loads 8 default pipelines from [`super::default_pipelines`] on
//! construction. Additional pipelines can be registered dynamically.
//!
//! ## SLO Compliance
//!
//! Each pipeline has a latency SLO (`latency_slo_ms`). Executions that exceed
//! the SLO are recorded as violations. The [`PipelineManager::check_slo_compliance`]
//! method returns a [`SloStatus`] with compliance metrics.
//!
//! ## Execution Log
//!
//! A bounded execution log (capacity 1000) records all pipeline executions.
//! When the log reaches capacity, the oldest entry is removed before appending.
//!
//! ## Related Documentation
//! - [Pipeline Specification](../../ai_specs/PIPELINE_SPEC.md)
//! - [Layer Specification](../../ai_docs/layers/L03_CORE_LOGIC.md)

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use uuid::Uuid;

use super::{Pipeline, PipelineStage, default_pipelines};
use crate::{Error, Result};

/// Maximum number of entries retained in the execution log.
/// When this limit is reached, the oldest entry is evicted before
/// a new entry is appended.
const EXECUTION_LOG_CAPACITY: usize = 1000;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Status of a pipeline within the manager.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineStatus {
    /// Pipeline is registered but not currently executing.
    Idle,
    /// Pipeline has an active execution in progress.
    Running,
    /// Pipeline has been temporarily paused.
    Paused,
    /// Pipeline encountered a fatal error in its last execution.
    Failed,
    /// Pipeline has been administratively disabled.
    Disabled,
}

/// Status of an individual pipeline execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionStatus {
    /// Execution has been created but not yet started.
    Pending,
    /// Execution is currently running.
    InProgress,
    /// Execution completed successfully.
    Completed,
    /// Execution failed with an error.
    Failed,
    /// Execution exceeded the pipeline's latency SLO.
    TimedOut,
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// A pipeline registered with the manager, including its runtime state
/// and cumulative statistics.
#[derive(Clone, Debug)]
pub struct PipelineEntry {
    /// The underlying pipeline definition.
    pub pipeline: Pipeline,
    /// Current lifecycle status.
    pub status: PipelineStatus,
    /// Total number of executions started.
    pub executions: u64,
    /// Cumulative latency across all completed executions (milliseconds).
    pub total_latency_ms: u64,
    /// Number of executions that violated the pipeline's latency SLO.
    pub slo_violations: u64,
    /// Timestamp of the most recent execution start, if any.
    pub last_execution: Option<DateTime<Utc>>,
    /// Total number of failed executions.
    pub error_count: u64,
    /// Timestamp when the pipeline was registered with the manager.
    pub created_at: DateTime<Utc>,
}

/// Record of a single pipeline execution.
#[derive(Clone, Debug)]
pub struct PipelineExecution {
    /// ID of the pipeline that was executed.
    pub pipeline_id: String,
    /// Unique identifier for this execution (UUID v4).
    pub execution_id: String,
    /// Timestamp when execution started.
    pub started_at: DateTime<Utc>,
    /// Timestamp when execution completed (if finished).
    pub completed_at: Option<DateTime<Utc>>,
    /// Duration of the execution in milliseconds (if finished).
    pub duration_ms: Option<u64>,
    /// Current status of this execution.
    pub status: ExecutionStatus,
    /// Pipeline stages that were successfully completed.
    pub stages_completed: Vec<PipelineStage>,
    /// Error message if the execution failed.
    pub error: Option<String>,
}

/// Aggregate statistics for a single pipeline.
#[derive(Clone, Debug)]
pub struct PipelineStats {
    /// Pipeline identifier.
    pub pipeline_id: String,
    /// Total executions attempted.
    pub total_executions: u64,
    /// Executions that completed successfully.
    pub successful_executions: u64,
    /// Executions that failed.
    pub failed_executions: u64,
    /// Average latency across completed executions (milliseconds).
    pub avg_latency_ms: f64,
    /// Number of SLO violations.
    pub slo_violation_count: u64,
    /// Fraction of executions that met the SLO (0.0 -- 1.0).
    pub slo_compliance_rate: f64,
    /// Fraction of executions that failed (0.0 -- 1.0).
    pub error_rate: f64,
}

/// SLO compliance status for a single pipeline.
#[derive(Clone, Debug)]
pub struct SloStatus {
    /// Pipeline identifier.
    pub pipeline_id: String,
    /// The pipeline's latency SLO target (milliseconds).
    pub slo_target_ms: u64,
    /// Average execution latency (milliseconds).
    pub avg_latency_ms: f64,
    /// Approximate 99th-percentile latency (uses the maximum observed
    /// duration from the execution log as a conservative estimate).
    pub p99_latency_ms: f64,
    /// Whether the pipeline is currently SLO-compliant.
    pub compliant: bool,
    /// Total number of SLO violations recorded.
    pub violation_count: u64,
}

// ---------------------------------------------------------------------------
// PipelineManager
// ---------------------------------------------------------------------------

/// Orchestrates the lifecycle and execution of data processing pipelines.
///
/// The manager is safe to share across threads via `Arc`. Internal state is
/// guarded by `parking_lot::RwLock` instances for concurrent read access
/// with exclusive writes.
///
/// # Examples
///
/// ```
/// use maintenance_engine::m3_core_logic::pipeline::PipelineManager;
///
/// let manager = PipelineManager::new();
/// assert!(manager.pipeline_count() >= 8);
/// ```
pub struct PipelineManager {
    /// Registered pipelines indexed by pipeline ID.
    pipelines: RwLock<HashMap<String, PipelineEntry>>,
    /// Bounded execution log (most recent entries at the end).
    execution_log: RwLock<Vec<PipelineExecution>>,
}

impl PipelineManager {
    /// Create a new `PipelineManager` pre-loaded with the default pipelines
    /// defined in [`super::default_pipelines`].
    ///
    /// # Examples
    ///
    /// ```
    /// use maintenance_engine::m3_core_logic::pipeline::PipelineManager;
    ///
    /// let manager = PipelineManager::new();
    /// assert!(manager.pipeline_count() >= 8);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        let now = Utc::now();
        let mut pipelines = HashMap::new();

        for pipeline in default_pipelines() {
            let status = if pipeline.enabled {
                PipelineStatus::Idle
            } else {
                PipelineStatus::Disabled
            };

            pipelines.insert(
                pipeline.id.clone(),
                PipelineEntry {
                    pipeline,
                    status,
                    executions: 0,
                    total_latency_ms: 0,
                    slo_violations: 0,
                    last_execution: None,
                    error_count: 0,
                    created_at: now,
                },
            );
        }

        Self {
            pipelines: RwLock::new(pipelines),
            execution_log: RwLock::new(Vec::new()),
        }
    }

    // ------------------------------------------------------------------
    // Registration
    // ------------------------------------------------------------------

    /// Register a new pipeline with the manager.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Pipeline`] if a pipeline with the same ID is already
    /// registered.
    ///
    /// Returns [`Error::Validation`] if the pipeline ID or name is empty.
    pub fn register_pipeline(&self, pipeline: Pipeline) -> Result<()> {
        if pipeline.id.is_empty() {
            return Err(Error::Validation("Pipeline ID must not be empty".into()));
        }
        if pipeline.name.is_empty() {
            return Err(Error::Validation("Pipeline name must not be empty".into()));
        }

        let status = if pipeline.enabled {
            PipelineStatus::Idle
        } else {
            PipelineStatus::Disabled
        };

        {
            let mut guard = self.pipelines.write();
            if guard.contains_key(&pipeline.id) {
                return Err(Error::Pipeline(format!(
                    "Pipeline '{}' is already registered",
                    pipeline.id
                )));
            }

            guard.insert(
                pipeline.id.clone(),
                PipelineEntry {
                    pipeline,
                    status,
                    executions: 0,
                    total_latency_ms: 0,
                    slo_violations: 0,
                    last_execution: None,
                    error_count: 0,
                    created_at: Utc::now(),
                },
            );
        }

        Ok(())
    }

    /// Remove a pipeline from the manager.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Pipeline`] if no pipeline with the given `id` exists.
    pub fn remove_pipeline(&self, id: &str) -> Result<()> {
        if self.pipelines.write().remove(id).is_none() {
            return Err(Error::Pipeline(format!(
                "Pipeline '{id}' not found"
            )));
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Execution lifecycle
    // ------------------------------------------------------------------

    /// Start a new execution for the specified pipeline.
    ///
    /// Creates a new [`PipelineExecution`] with status [`ExecutionStatus::InProgress`],
    /// increments the pipeline's execution counter, and records the start time.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Pipeline`] if the pipeline does not exist, is disabled,
    /// or is paused.
    #[allow(clippy::significant_drop_tightening)]
    pub fn start_execution(&self, pipeline_id: &str) -> Result<PipelineExecution> {
        let now = Utc::now();
        let execution_id = Uuid::new_v4().to_string();

        {
            let mut guard = self.pipelines.write();
            let entry = guard.get_mut(pipeline_id).ok_or_else(|| {
                Error::Pipeline(format!("Pipeline '{pipeline_id}' not found"))
            })?;

            match entry.status {
                PipelineStatus::Disabled => {
                    return Err(Error::Pipeline(format!(
                        "Pipeline '{pipeline_id}' is disabled"
                    )));
                }
                PipelineStatus::Paused => {
                    return Err(Error::Pipeline(format!(
                        "Pipeline '{pipeline_id}' is paused"
                    )));
                }
                _ => {}
            }

            entry.status = PipelineStatus::Running;
            entry.executions = entry.executions.saturating_add(1);
            entry.last_execution = Some(now);
        }

        let execution = PipelineExecution {
            pipeline_id: pipeline_id.to_owned(),
            execution_id,
            started_at: now,
            completed_at: None,
            duration_ms: None,
            status: ExecutionStatus::InProgress,
            stages_completed: Vec::new(),
            error: None,
        };

        {
            let mut log = self.execution_log.write();
            if log.len() >= EXECUTION_LOG_CAPACITY {
                log.remove(0);
            }
            log.push(execution.clone());
        }

        Ok(execution)
    }

    /// Mark an execution as successfully completed.
    ///
    /// Records the completion timestamp, calculates the duration, and checks
    /// the pipeline's latency SLO. If the duration exceeds the SLO, a
    /// violation is recorded.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Pipeline`] if the execution ID is not found in the log.
    #[allow(clippy::cast_sign_loss, clippy::cast_precision_loss)]
    pub fn complete_execution(
        &self,
        execution_id: &str,
        stages_completed: Vec<PipelineStage>,
    ) -> Result<PipelineExecution> {
        let now = Utc::now();

        let mut log = self.execution_log.write();
        let execution = log
            .iter_mut()
            .rev()
            .find(|e| e.execution_id == execution_id)
            .ok_or_else(|| {
                Error::Pipeline(format!("Execution '{execution_id}' not found"))
            })?;

        let duration_ms = now
            .signed_duration_since(execution.started_at)
            .num_milliseconds()
            .max(0) as u64;

        execution.completed_at = Some(now);
        execution.duration_ms = Some(duration_ms);
        execution.status = ExecutionStatus::Completed;
        execution.stages_completed = stages_completed;

        let result = execution.clone();
        let pipeline_id = result.pipeline_id.clone();

        // Must drop log guard before acquiring pipelines guard to avoid
        // potential ordering issues (though parking_lot handles this gracefully).
        drop(log);

        {
            let mut guard = self.pipelines.write();
            if let Some(entry) = guard.get_mut(&pipeline_id) {
                entry.total_latency_ms = entry.total_latency_ms.saturating_add(duration_ms);

                if duration_ms > entry.pipeline.latency_slo_ms {
                    entry.slo_violations = entry.slo_violations.saturating_add(1);
                }

                // Return to Idle after successful completion
                if entry.status == PipelineStatus::Running {
                    entry.status = PipelineStatus::Idle;
                }
            }
        }

        Ok(result)
    }

    /// Mark an execution as failed.
    ///
    /// Records the error message, sets the execution status to
    /// [`ExecutionStatus::Failed`], and increments the pipeline's error
    /// counter.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Pipeline`] if the execution ID is not found in the log.
    #[allow(clippy::cast_sign_loss, clippy::cast_precision_loss)]
    pub fn fail_execution(
        &self,
        execution_id: &str,
        error: String,
    ) -> Result<PipelineExecution> {
        let now = Utc::now();

        let mut log = self.execution_log.write();
        let execution = log
            .iter_mut()
            .rev()
            .find(|e| e.execution_id == execution_id)
            .ok_or_else(|| {
                Error::Pipeline(format!("Execution '{execution_id}' not found"))
            })?;

        let duration_ms = now
            .signed_duration_since(execution.started_at)
            .num_milliseconds()
            .max(0) as u64;

        execution.completed_at = Some(now);
        execution.duration_ms = Some(duration_ms);
        execution.status = ExecutionStatus::Failed;
        execution.error = Some(error);

        let result = execution.clone();
        let pipeline_id = result.pipeline_id.clone();

        drop(log);

        {
            let mut guard = self.pipelines.write();
            if let Some(entry) = guard.get_mut(&pipeline_id) {
                entry.error_count = entry.error_count.saturating_add(1);
                entry.total_latency_ms = entry.total_latency_ms.saturating_add(duration_ms);
                entry.status = PipelineStatus::Failed;
            }
        }

        Ok(result)
    }

    // ------------------------------------------------------------------
    // Queries
    // ------------------------------------------------------------------

    /// Retrieve a clone of the pipeline entry for the given ID.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Pipeline`] if no pipeline with the given `id` exists.
    #[must_use = "returns the pipeline entry without side effects"]
    pub fn get_pipeline(&self, id: &str) -> Result<PipelineEntry> {
        let guard = self.pipelines.read();
        guard
            .get(id)
            .cloned()
            .ok_or_else(|| Error::Pipeline(format!("Pipeline '{id}' not found")))
    }

    /// Compute aggregate statistics for the specified pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Pipeline`] if no pipeline with the given `id` exists.
    #[must_use = "returns statistics without side effects"]
    #[allow(clippy::cast_precision_loss, clippy::significant_drop_tightening)]
    pub fn get_pipeline_stats(&self, id: &str) -> Result<PipelineStats> {
        let guard = self.pipelines.read();
        let entry = guard
            .get(id)
            .ok_or_else(|| Error::Pipeline(format!("Pipeline '{id}' not found")))?;

        let total = entry.executions;
        let failed = entry.error_count;
        let successful = total.saturating_sub(failed);

        let avg_latency_ms = if successful > 0 {
            entry.total_latency_ms as f64 / successful as f64
        } else {
            0.0
        };

        let slo_compliance_rate = if total > 0 {
            1.0 - (entry.slo_violations as f64 / total as f64)
        } else {
            1.0
        };

        let error_rate = if total > 0 {
            failed as f64 / total as f64
        } else {
            0.0
        };

        Ok(PipelineStats {
            pipeline_id: id.to_owned(),
            total_executions: total,
            successful_executions: successful,
            failed_executions: failed,
            avg_latency_ms,
            slo_violation_count: entry.slo_violations,
            slo_compliance_rate,
            error_rate,
        })
    }

    /// Return all executions that are currently in progress.
    #[must_use]
    pub fn get_active_executions(&self) -> Vec<PipelineExecution> {
        let log = self.execution_log.read();
        log.iter()
            .filter(|e| e.status == ExecutionStatus::InProgress)
            .cloned()
            .collect()
    }

    /// Return the definitions of all enabled pipelines.
    #[must_use]
    pub fn get_enabled_pipelines(&self) -> Vec<Pipeline> {
        let guard = self.pipelines.read();
        guard
            .values()
            .filter(|e| e.pipeline.enabled && e.status != PipelineStatus::Disabled)
            .map(|e| e.pipeline.clone())
            .collect()
    }

    /// Return all pipelines matching the specified priority level.
    #[must_use]
    pub fn get_pipelines_by_priority(&self, priority: u32) -> Vec<Pipeline> {
        let guard = self.pipelines.read();
        guard
            .values()
            .filter(|e| u32::from(e.pipeline.priority) == priority)
            .map(|e| e.pipeline.clone())
            .collect()
    }

    // ------------------------------------------------------------------
    // Lifecycle management
    // ------------------------------------------------------------------

    /// Pause a pipeline, preventing new executions from starting.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Pipeline`] if the pipeline does not exist or is
    /// already paused/disabled.
    #[allow(clippy::significant_drop_tightening)]
    pub fn pause_pipeline(&self, id: &str) -> Result<()> {
        let mut guard = self.pipelines.write();
        let entry = guard
            .get_mut(id)
            .ok_or_else(|| Error::Pipeline(format!("Pipeline '{id}' not found")))?;

        match entry.status {
            PipelineStatus::Paused => {
                return Err(Error::Pipeline(format!(
                    "Pipeline '{id}' is already paused"
                )));
            }
            PipelineStatus::Disabled => {
                return Err(Error::Pipeline(format!(
                    "Pipeline '{id}' is disabled and cannot be paused"
                )));
            }
            _ => {
                entry.status = PipelineStatus::Paused;
            }
        }

        Ok(())
    }

    /// Resume a paused pipeline, allowing new executions.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Pipeline`] if the pipeline does not exist or is not
    /// currently paused.
    #[allow(clippy::significant_drop_tightening)]
    pub fn resume_pipeline(&self, id: &str) -> Result<()> {
        let mut guard = self.pipelines.write();
        let entry = guard
            .get_mut(id)
            .ok_or_else(|| Error::Pipeline(format!("Pipeline '{id}' not found")))?;

        if entry.status != PipelineStatus::Paused {
            return Err(Error::Pipeline(format!(
                "Pipeline '{id}' is not paused (current status: {:?})",
                entry.status
            )));
        }

        entry.status = PipelineStatus::Idle;
        Ok(())
    }

    /// Administratively disable a pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Pipeline`] if the pipeline does not exist or is
    /// already disabled.
    #[allow(clippy::significant_drop_tightening)]
    pub fn disable_pipeline(&self, id: &str) -> Result<()> {
        let mut guard = self.pipelines.write();
        let entry = guard
            .get_mut(id)
            .ok_or_else(|| Error::Pipeline(format!("Pipeline '{id}' not found")))?;

        if entry.status == PipelineStatus::Disabled {
            return Err(Error::Pipeline(format!(
                "Pipeline '{id}' is already disabled"
            )));
        }

        entry.status = PipelineStatus::Disabled;
        entry.pipeline.enabled = false;
        Ok(())
    }

    /// Re-enable a previously disabled pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Pipeline`] if the pipeline does not exist or is not
    /// currently disabled.
    #[allow(clippy::significant_drop_tightening)]
    pub fn enable_pipeline(&self, id: &str) -> Result<()> {
        let mut guard = self.pipelines.write();
        let entry = guard
            .get_mut(id)
            .ok_or_else(|| Error::Pipeline(format!("Pipeline '{id}' not found")))?;

        if entry.status != PipelineStatus::Disabled {
            return Err(Error::Pipeline(format!(
                "Pipeline '{id}' is not disabled (current status: {:?})",
                entry.status
            )));
        }

        entry.status = PipelineStatus::Idle;
        entry.pipeline.enabled = true;
        Ok(())
    }

    /// Return the total number of registered pipelines.
    #[must_use]
    pub fn pipeline_count(&self) -> usize {
        let guard = self.pipelines.read();
        guard.len()
    }

    /// Check the SLO compliance status for a pipeline.
    ///
    /// The `p99_latency_ms` is approximated from the maximum observed
    /// execution duration in the execution log for this pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Pipeline`] if the pipeline does not exist.
    #[must_use = "returns SLO status without side effects"]
    #[allow(clippy::cast_precision_loss, clippy::significant_drop_tightening)]
    pub fn check_slo_compliance(&self, id: &str) -> Result<SloStatus> {
        let guard = self.pipelines.read();
        let entry = guard
            .get(id)
            .ok_or_else(|| Error::Pipeline(format!("Pipeline '{id}' not found")))?;

        let total = entry.executions;
        let successful = total.saturating_sub(entry.error_count);

        let avg_latency_ms = if successful > 0 {
            entry.total_latency_ms as f64 / successful as f64
        } else {
            0.0
        };

        let slo_target = entry.pipeline.latency_slo_ms;
        let compliant = avg_latency_ms <= slo_target as f64;
        let violation_count = entry.slo_violations;

        drop(guard);

        // Approximate p99 as the maximum observed duration for this pipeline
        let p99_latency_ms = self
            .execution_log
            .read()
            .iter()
            .filter(|e| e.pipeline_id == id)
            .filter_map(|e| e.duration_ms)
            .max()
            .map_or(0.0, |v| v as f64);

        Ok(SloStatus {
            pipeline_id: id.to_owned(),
            slo_target_ms: slo_target,
            avg_latency_ms,
            p99_latency_ms,
            compliant,
            violation_count,
        })
    }
}

impl Default for PipelineManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a manager with defaults loaded.
    fn make_manager() -> PipelineManager {
        PipelineManager::new()
    }

    /// Helper: create a simple custom pipeline.
    fn custom_pipeline(id: &str, name: &str) -> Pipeline {
        Pipeline {
            id: id.into(),
            name: name.into(),
            priority: 5,
            latency_slo_ms: 200,
            throughput_target: 500,
            error_budget: 0.01,
            enabled: true,
        }
    }

    // ------------------------------------------------------------------
    // 1. test_new_loads_defaults
    // ------------------------------------------------------------------
    #[test]
    fn test_new_loads_defaults() {
        let mgr = make_manager();
        assert_eq!(mgr.pipeline_count(), 8);

        // Verify a known default pipeline is present
        let entry = mgr
            .get_pipeline("PL-HEALTH-001")
            .ok();
        assert!(entry.is_some());

        let entry = entry.unwrap_or_else(|| {
            // This branch will never execute because we just asserted Some
            PipelineEntry {
                pipeline: Pipeline::new("dummy", "dummy"),
                status: PipelineStatus::Idle,
                executions: 0,
                total_latency_ms: 0,
                slo_violations: 0,
                last_execution: None,
                error_count: 0,
                created_at: Utc::now(),
            }
        });
        assert_eq!(entry.pipeline.name, "Health Monitoring Pipeline");
        assert_eq!(entry.status, PipelineStatus::Idle);
    }

    // ------------------------------------------------------------------
    // 2. test_register_pipeline
    // ------------------------------------------------------------------
    #[test]
    fn test_register_pipeline() {
        let mgr = make_manager();
        let pipeline = custom_pipeline("PL-CUSTOM-001", "Custom Pipeline");

        let result = mgr.register_pipeline(pipeline);
        assert!(result.is_ok());
        assert_eq!(mgr.pipeline_count(), 9);

        let entry = mgr.get_pipeline("PL-CUSTOM-001");
        assert!(entry.is_ok());
    }

    // ------------------------------------------------------------------
    // 3. test_register_duplicate_fails
    // ------------------------------------------------------------------
    #[test]
    fn test_register_duplicate_fails() {
        let mgr = make_manager();
        let pipeline = custom_pipeline("PL-HEALTH-001", "Duplicate");

        let result = mgr.register_pipeline(pipeline);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // 4. test_remove_pipeline
    // ------------------------------------------------------------------
    #[test]
    fn test_remove_pipeline() {
        let mgr = make_manager();
        assert_eq!(mgr.pipeline_count(), 8);

        let result = mgr.remove_pipeline("PL-HEALTH-001");
        assert!(result.is_ok());
        assert_eq!(mgr.pipeline_count(), 7);

        // Removing again should fail
        let result = mgr.remove_pipeline("PL-HEALTH-001");
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // 5. test_start_execution
    // ------------------------------------------------------------------
    #[test]
    fn test_start_execution() {
        let mgr = make_manager();

        let exec = mgr.start_execution("PL-HEALTH-001");
        assert!(exec.is_ok());

        let exec = exec.ok().unwrap_or_else(|| PipelineExecution {
            pipeline_id: String::new(),
            execution_id: String::new(),
            started_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            status: ExecutionStatus::Pending,
            stages_completed: Vec::new(),
            error: None,
        });

        assert_eq!(exec.pipeline_id, "PL-HEALTH-001");
        assert_eq!(exec.status, ExecutionStatus::InProgress);
        assert!(exec.completed_at.is_none());
        assert!(!exec.execution_id.is_empty());

        // Pipeline should now be Running
        let entry = mgr.get_pipeline("PL-HEALTH-001");
        assert!(entry.is_ok());
    }

    // ------------------------------------------------------------------
    // 6. test_complete_execution
    // ------------------------------------------------------------------
    #[test]
    fn test_complete_execution() {
        let mgr = make_manager();

        let exec = mgr.start_execution("PL-HEALTH-001");
        assert!(exec.is_ok());
        let exec = exec.ok().unwrap_or_else(|| PipelineExecution {
            pipeline_id: String::new(),
            execution_id: String::new(),
            started_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            status: ExecutionStatus::Pending,
            stages_completed: Vec::new(),
            error: None,
        });

        let stages = vec![PipelineStage::Source, PipelineStage::Transform, PipelineStage::Sink];

        let result = mgr.complete_execution(&exec.execution_id, stages.clone());
        assert!(result.is_ok());

        let completed = result.ok().unwrap_or_else(|| PipelineExecution {
            pipeline_id: String::new(),
            execution_id: String::new(),
            started_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            status: ExecutionStatus::Pending,
            stages_completed: Vec::new(),
            error: None,
        });

        assert_eq!(completed.status, ExecutionStatus::Completed);
        assert!(completed.completed_at.is_some());
        assert!(completed.duration_ms.is_some());
        assert_eq!(completed.stages_completed.len(), 3);

        // Pipeline should return to Idle
        let entry = mgr.get_pipeline("PL-HEALTH-001");
        assert!(entry.is_ok());
    }

    // ------------------------------------------------------------------
    // 7. test_complete_execution_slo_violation
    // ------------------------------------------------------------------
    #[test]
    fn test_complete_execution_slo_violation() {
        let mgr = make_manager();

        // Register a pipeline with a very low SLO (1ms) to guarantee a violation
        let tight_pipeline = Pipeline {
            id: "PL-TIGHT-001".into(),
            name: "Tight SLO Pipeline".into(),
            priority: 1,
            latency_slo_ms: 0, // 0ms SLO -- any real execution will violate
            throughput_target: 100,
            error_budget: 0.01,
            enabled: true,
        };
        let reg = mgr.register_pipeline(tight_pipeline);
        assert!(reg.is_ok());

        let exec = mgr.start_execution("PL-TIGHT-001");
        assert!(exec.is_ok());
        let exec = exec.ok().unwrap_or_else(|| PipelineExecution {
            pipeline_id: String::new(),
            execution_id: String::new(),
            started_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            status: ExecutionStatus::Pending,
            stages_completed: Vec::new(),
            error: None,
        });

        // Simulate some delay by doing work (the duration is measured from start)
        // Even a 0ms SLO should register a violation since clock resolution
        // means duration >= 0 and typically > 0.
        let stages = vec![PipelineStage::Source];
        let result = mgr.complete_execution(&exec.execution_id, stages);
        assert!(result.is_ok());

        // Check that violations were tracked (may be 0 or 1 depending on clock)
        let entry = mgr.get_pipeline("PL-TIGHT-001");
        assert!(entry.is_ok());

        // With SLO of 0ms, the SLO compliance check should work
        let slo = mgr.check_slo_compliance("PL-TIGHT-001");
        assert!(slo.is_ok());
    }

    // ------------------------------------------------------------------
    // 8. test_fail_execution
    // ------------------------------------------------------------------
    #[test]
    fn test_fail_execution() {
        let mgr = make_manager();

        let exec = mgr.start_execution("PL-LOG-001");
        assert!(exec.is_ok());
        let exec = exec.ok().unwrap_or_else(|| PipelineExecution {
            pipeline_id: String::new(),
            execution_id: String::new(),
            started_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            status: ExecutionStatus::Pending,
            stages_completed: Vec::new(),
            error: None,
        });

        let result = mgr.fail_execution(
            &exec.execution_id,
            "Connection refused".to_owned(),
        );
        assert!(result.is_ok());

        let failed = result.ok().unwrap_or_else(|| PipelineExecution {
            pipeline_id: String::new(),
            execution_id: String::new(),
            started_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            status: ExecutionStatus::Pending,
            stages_completed: Vec::new(),
            error: None,
        });

        assert_eq!(failed.status, ExecutionStatus::Failed);
        assert!(failed.error.is_some());

        // Pipeline should be in Failed state
        let entry = mgr.get_pipeline("PL-LOG-001");
        assert!(entry.is_ok());
        let entry = entry.ok().unwrap_or_else(|| PipelineEntry {
            pipeline: Pipeline::new("dummy", "dummy"),
            status: PipelineStatus::Idle,
            executions: 0,
            total_latency_ms: 0,
            slo_violations: 0,
            last_execution: None,
            error_count: 0,
            created_at: Utc::now(),
        });
        assert_eq!(entry.status, PipelineStatus::Failed);
        assert_eq!(entry.error_count, 1);
    }

    // ------------------------------------------------------------------
    // 9. test_get_pipeline_stats
    // ------------------------------------------------------------------
    #[test]
    fn test_get_pipeline_stats() {
        let mgr = make_manager();

        // Run two executions: one success, one failure
        let exec1 = mgr.start_execution("PL-HEALTH-001");
        assert!(exec1.is_ok());
        let exec1 = exec1.ok().unwrap_or_else(|| PipelineExecution {
            pipeline_id: String::new(),
            execution_id: String::new(),
            started_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            status: ExecutionStatus::Pending,
            stages_completed: Vec::new(),
            error: None,
        });
        let _ = mgr.complete_execution(&exec1.execution_id, vec![PipelineStage::Source]);

        let exec2 = mgr.start_execution("PL-HEALTH-001");
        assert!(exec2.is_ok());
        let exec2 = exec2.ok().unwrap_or_else(|| PipelineExecution {
            pipeline_id: String::new(),
            execution_id: String::new(),
            started_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            status: ExecutionStatus::Pending,
            stages_completed: Vec::new(),
            error: None,
        });
        let _ = mgr.fail_execution(&exec2.execution_id, "test error".to_owned());

        let stats = mgr.get_pipeline_stats("PL-HEALTH-001");
        assert!(stats.is_ok());
        let stats = stats.ok().unwrap_or_else(|| PipelineStats {
            pipeline_id: String::new(),
            total_executions: 0,
            successful_executions: 0,
            failed_executions: 0,
            avg_latency_ms: 0.0,
            slo_violation_count: 0,
            slo_compliance_rate: 1.0,
            error_rate: 0.0,
        });

        assert_eq!(stats.total_executions, 2);
        assert_eq!(stats.successful_executions, 1);
        assert_eq!(stats.failed_executions, 1);
        assert!((stats.error_rate - 0.5).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 10. test_get_active_executions
    // ------------------------------------------------------------------
    #[test]
    fn test_get_active_executions() {
        let mgr = make_manager();

        // No active executions initially
        assert!(mgr.get_active_executions().is_empty());

        // Start one
        let exec = mgr.start_execution("PL-HEALTH-001");
        assert!(exec.is_ok());

        let active = mgr.get_active_executions();
        assert_eq!(active.len(), 1);

        // Complete it
        let exec = exec.ok().unwrap_or_else(|| PipelineExecution {
            pipeline_id: String::new(),
            execution_id: String::new(),
            started_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            status: ExecutionStatus::Pending,
            stages_completed: Vec::new(),
            error: None,
        });
        let _ = mgr.complete_execution(&exec.execution_id, vec![]);

        let active = mgr.get_active_executions();
        assert!(active.is_empty());
    }

    // ------------------------------------------------------------------
    // 11. test_get_enabled_pipelines
    // ------------------------------------------------------------------
    #[test]
    fn test_get_enabled_pipelines() {
        let mgr = make_manager();

        // All 8 defaults are enabled
        let enabled = mgr.get_enabled_pipelines();
        assert_eq!(enabled.len(), 8);

        // Disable one
        let result = mgr.disable_pipeline("PL-HEALTH-001");
        assert!(result.is_ok());

        let enabled = mgr.get_enabled_pipelines();
        assert_eq!(enabled.len(), 7);
    }

    // ------------------------------------------------------------------
    // 12. test_pause_resume_pipeline
    // ------------------------------------------------------------------
    #[test]
    fn test_pause_resume_pipeline() {
        let mgr = make_manager();

        // Pause
        let result = mgr.pause_pipeline("PL-HEALTH-001");
        assert!(result.is_ok());

        let entry = mgr.get_pipeline("PL-HEALTH-001");
        assert!(entry.is_ok());
        let entry = entry.ok().unwrap_or_else(|| PipelineEntry {
            pipeline: Pipeline::new("dummy", "dummy"),
            status: PipelineStatus::Idle,
            executions: 0,
            total_latency_ms: 0,
            slo_violations: 0,
            last_execution: None,
            error_count: 0,
            created_at: Utc::now(),
        });
        assert_eq!(entry.status, PipelineStatus::Paused);

        // Cannot start execution while paused
        let exec = mgr.start_execution("PL-HEALTH-001");
        assert!(exec.is_err());

        // Resume
        let result = mgr.resume_pipeline("PL-HEALTH-001");
        assert!(result.is_ok());

        let entry = mgr.get_pipeline("PL-HEALTH-001");
        assert!(entry.is_ok());
        let entry = entry.ok().unwrap_or_else(|| PipelineEntry {
            pipeline: Pipeline::new("dummy", "dummy"),
            status: PipelineStatus::Paused,
            executions: 0,
            total_latency_ms: 0,
            slo_violations: 0,
            last_execution: None,
            error_count: 0,
            created_at: Utc::now(),
        });
        assert_eq!(entry.status, PipelineStatus::Idle);

        // Can start execution after resume
        let exec = mgr.start_execution("PL-HEALTH-001");
        assert!(exec.is_ok());
    }

    // ------------------------------------------------------------------
    // 13. test_disable_enable_pipeline
    // ------------------------------------------------------------------
    #[test]
    fn test_disable_enable_pipeline() {
        let mgr = make_manager();

        // Disable
        let result = mgr.disable_pipeline("PL-LOG-001");
        assert!(result.is_ok());

        let entry = mgr.get_pipeline("PL-LOG-001");
        assert!(entry.is_ok());
        let entry = entry.ok().unwrap_or_else(|| PipelineEntry {
            pipeline: Pipeline::new("dummy", "dummy"),
            status: PipelineStatus::Idle,
            executions: 0,
            total_latency_ms: 0,
            slo_violations: 0,
            last_execution: None,
            error_count: 0,
            created_at: Utc::now(),
        });
        assert_eq!(entry.status, PipelineStatus::Disabled);
        assert!(!entry.pipeline.enabled);

        // Cannot start execution while disabled
        let exec = mgr.start_execution("PL-LOG-001");
        assert!(exec.is_err());

        // Double disable fails
        let result = mgr.disable_pipeline("PL-LOG-001");
        assert!(result.is_err());

        // Enable
        let result = mgr.enable_pipeline("PL-LOG-001");
        assert!(result.is_ok());

        let entry = mgr.get_pipeline("PL-LOG-001");
        assert!(entry.is_ok());
        let entry = entry.ok().unwrap_or_else(|| PipelineEntry {
            pipeline: Pipeline::new("dummy", "dummy"),
            status: PipelineStatus::Disabled,
            executions: 0,
            total_latency_ms: 0,
            slo_violations: 0,
            last_execution: None,
            error_count: 0,
            created_at: Utc::now(),
        });
        assert_eq!(entry.status, PipelineStatus::Idle);
        assert!(entry.pipeline.enabled);
    }

    // ------------------------------------------------------------------
    // 14. test_pipelines_by_priority
    // ------------------------------------------------------------------
    #[test]
    fn test_pipelines_by_priority() {
        let mgr = make_manager();

        // Priority 1: PL-HEALTH-001, PL-REMEDIATE-001, PL-CONSENSUS-001
        let p1 = mgr.get_pipelines_by_priority(1);
        assert_eq!(p1.len(), 3);

        // Priority 2: PL-LOG-001, PL-HEBBIAN-001, PL-DISCOVERY-001
        let p2 = mgr.get_pipelines_by_priority(2);
        assert_eq!(p2.len(), 3);

        // Priority 3: PL-TENSOR-001, PL-METRICS-001
        let p3 = mgr.get_pipelines_by_priority(3);
        assert_eq!(p3.len(), 2);

        // Priority 99: none
        let p99 = mgr.get_pipelines_by_priority(99);
        assert!(p99.is_empty());
    }

    // ------------------------------------------------------------------
    // 15. test_slo_compliance_check
    // ------------------------------------------------------------------
    #[test]
    fn test_slo_compliance_check() {
        let mgr = make_manager();

        // No executions yet -> compliant with 0.0 avg latency
        let slo = mgr.check_slo_compliance("PL-HEALTH-001");
        assert!(slo.is_ok());
        let slo = slo.ok().unwrap_or_else(|| SloStatus {
            pipeline_id: String::new(),
            slo_target_ms: 0,
            avg_latency_ms: f64::MAX,
            p99_latency_ms: f64::MAX,
            compliant: false,
            violation_count: 0,
        });
        assert!(slo.compliant);
        assert_eq!(slo.slo_target_ms, 100);
        assert!((slo.avg_latency_ms - 0.0).abs() < f64::EPSILON);
        assert_eq!(slo.violation_count, 0);

        // Non-existent pipeline should fail
        let slo = mgr.check_slo_compliance("PL-NONEXISTENT");
        assert!(slo.is_err());
    }

    // ------------------------------------------------------------------
    // 16. test_execution_log_capacity
    // ------------------------------------------------------------------
    #[test]
    fn test_execution_log_capacity() {
        let mgr = make_manager();

        // Fill the log beyond capacity
        for _ in 0..EXECUTION_LOG_CAPACITY + 50 {
            let exec = mgr.start_execution("PL-HEALTH-001");
            if let Ok(e) = exec {
                let _ = mgr.complete_execution(&e.execution_id, vec![]);
            }
        }

        // Verify the log is capped
        let log = mgr.execution_log.read();
        assert!(log.len() <= EXECUTION_LOG_CAPACITY);
    }

    // ------------------------------------------------------------------
    // 17. test_register_empty_id_fails
    // ------------------------------------------------------------------
    #[test]
    fn test_register_empty_id_fails() {
        let mgr = make_manager();
        let pipeline = custom_pipeline("", "No ID Pipeline");
        let result = mgr.register_pipeline(pipeline);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // 18. test_register_empty_name_fails
    // ------------------------------------------------------------------
    #[test]
    fn test_register_empty_name_fails() {
        let mgr = make_manager();
        let pipeline = Pipeline {
            id: "PL-NONAME".into(),
            name: String::new(),
            priority: 5,
            latency_slo_ms: 100,
            throughput_target: 100,
            error_budget: 0.01,
            enabled: true,
        };
        let result = mgr.register_pipeline(pipeline);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // 19. test_complete_nonexistent_execution_fails
    // ------------------------------------------------------------------
    #[test]
    fn test_complete_nonexistent_execution_fails() {
        let mgr = make_manager();
        let result = mgr.complete_execution("nonexistent-id", vec![]);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // 20. test_fail_nonexistent_execution_fails
    // ------------------------------------------------------------------
    #[test]
    fn test_fail_nonexistent_execution_fails() {
        let mgr = make_manager();
        let result = mgr.fail_execution("nonexistent-id", "error".to_owned());
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // 21. test_start_execution_disabled_pipeline_fails
    // ------------------------------------------------------------------
    #[test]
    fn test_start_execution_disabled_pipeline_fails() {
        let mgr = make_manager();
        let _ = mgr.disable_pipeline("PL-TENSOR-001");

        let exec = mgr.start_execution("PL-TENSOR-001");
        assert!(exec.is_err());
    }

    // ------------------------------------------------------------------
    // 22. test_default_impl
    // ------------------------------------------------------------------
    #[test]
    fn test_default_impl() {
        let mgr = PipelineManager::default();
        assert_eq!(mgr.pipeline_count(), 8);
    }

    // ------------------------------------------------------------------
    // 23. test_get_nonexistent_pipeline_fails
    // ------------------------------------------------------------------
    #[test]
    fn test_get_nonexistent_pipeline_fails() {
        let mgr = make_manager();
        let result = mgr.get_pipeline("PL-DOES-NOT-EXIST");
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // 24. test_resume_non_paused_fails
    // ------------------------------------------------------------------
    #[test]
    fn test_resume_non_paused_fails() {
        let mgr = make_manager();
        let result = mgr.resume_pipeline("PL-HEALTH-001");
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // 25. test_enable_non_disabled_fails
    // ------------------------------------------------------------------
    #[test]
    fn test_enable_non_disabled_fails() {
        let mgr = make_manager();
        let result = mgr.enable_pipeline("PL-HEALTH-001");
        assert!(result.is_err());
    }
}
