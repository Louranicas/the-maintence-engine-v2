//! # L3 Core Logic Integration Tests
//!
//! Comprehensive integration tests for the L3 Core Logic layer (M13-M18).
//! Covers pipeline management, remediation engine, confidence calculation,
//! action execution, outcome recording, feedback loops, and cross-module
//! workflows.
//!
//! ## Test Categories
//!
//! | Category | Module | Tests |
//! |----------|--------|-------|
//! | Remediation Engine | M14 | 8 |
//! | Confidence Calculator | M15 | 7 |
//! | Action Executor | M16 | 8 |
//! | Outcome Recorder | M17 | 7 |
//! | Feedback Loop | M18 | 6 |
//! | Pipeline Manager | M13 | 6 |
//! | Cross-Module Workflows | L3 | 3 |
//! | **Total** | | **45** |

mod common;

use maintenance_engine::m3_core_logic::action::{ActionExecutor, ActionStatus};
use maintenance_engine::m3_core_logic::confidence::ConfidenceCalculator;
use maintenance_engine::m3_core_logic::feedback::{FeedbackLoop, RecommendationType, SignalType};
use maintenance_engine::m3_core_logic::outcome::OutcomeRecorder;
use maintenance_engine::m3_core_logic::pipeline::{ExecutionStatus, PipelineManager};
use maintenance_engine::m3_core_logic::remediation::{RemediationEngine, RemediationStatus};
use maintenance_engine::m3_core_logic::{
    IssueType, PipelineStage, RemediationAction, RemediationRequest, Severity,
    calculate_confidence, default_pipelines, determine_tier,
};
use maintenance_engine::EscalationTier;

// =========================================================================
// 1. RemediationEngine Tests (8 tests)
// =========================================================================

#[test]
fn remediation_submit_request_creates_pending_entry() {
    let engine = RemediationEngine::new();

    let request = engine
        .submit_request(
            "synthex",
            IssueType::HealthFailure,
            Severity::Medium,
            "health check failed",
        )
        .expect("submit_request should succeed for valid inputs");

    assert_eq!(request.service_id, "synthex");
    assert_eq!(request.issue_type, IssueType::HealthFailure);
    assert_eq!(request.severity, Severity::Medium);
    assert!(request.confidence > 0.0, "confidence should be positive");
    assert!(request.confidence <= 1.0, "confidence should be at most 1.0");
    assert!(!request.id.is_empty(), "request ID should not be empty");
    assert_eq!(engine.pending_count(), 1, "pending queue should have 1 entry");
}

#[test]
fn remediation_submit_rejects_empty_service_id() {
    let engine = RemediationEngine::new();

    let result = engine.submit_request(
        "",
        IssueType::LatencySpike,
        Severity::Low,
        "spike detected",
    );
    assert!(result.is_err(), "empty service_id should be rejected");
}

#[test]
fn remediation_submit_rejects_empty_description() {
    let engine = RemediationEngine::new();

    let result = engine.submit_request(
        "synthex",
        IssueType::LatencySpike,
        Severity::Low,
        "",
    );
    assert!(result.is_err(), "empty description should be rejected");
}

#[test]
fn remediation_process_next_moves_pending_to_active() {
    let engine = RemediationEngine::new();

    engine
        .submit_request(
            "san-k7",
            IssueType::ErrorRateHigh,
            Severity::High,
            "error rate above threshold",
        )
        .expect("submit_request should succeed");

    assert_eq!(engine.pending_count(), 1);
    assert_eq!(engine.active_count(), 0);

    let active = engine
        .process_next()
        .expect("process_next should not fail")
        .expect("process_next should return an active remediation");

    assert_eq!(active.request.service_id, "san-k7");
    assert_eq!(active.status, RemediationStatus::Executing);
    assert_eq!(engine.pending_count(), 0);
    assert_eq!(engine.active_count(), 1);
}

#[test]
fn remediation_process_next_returns_none_when_empty() {
    let engine = RemediationEngine::new();

    let result = engine
        .process_next()
        .expect("process_next should not fail on empty queue");

    assert!(result.is_none(), "should return None when pending queue is empty");
}

#[test]
fn remediation_complete_request_success_records_outcome() {
    let engine = RemediationEngine::new();

    let request = engine
        .submit_request("nais", IssueType::Timeout, Severity::Medium, "request timed out")
        .expect("submit_request should succeed");

    let request_id = request.id;

    engine
        .process_next()
        .expect("process_next should succeed")
        .expect("should have a pending request to process");

    let outcome = engine
        .complete_request(&request_id, true, 150, None)
        .expect("complete_request should succeed for active request");

    assert!(outcome.success, "outcome should be successful");
    assert_eq!(outcome.duration_ms, 150);
    assert!(
        outcome.pathway_delta > 0.0,
        "successful outcome should have positive pathway delta"
    );
    assert!(outcome.error.is_none(), "successful outcome should have no error");
    assert_eq!(engine.active_count(), 0, "active count should be 0 after completion");
    assert_eq!(engine.completed_count(), 1, "completed count should be 1");
}

#[test]
fn remediation_complete_request_failure_records_negative_delta() {
    let engine = RemediationEngine::new();

    let request = engine
        .submit_request(
            "devops-engine",
            IssueType::Crash,
            Severity::Critical,
            "service crashed",
        )
        .expect("submit_request should succeed");

    let request_id = request.id;

    engine
        .process_next()
        .expect("process_next should succeed")
        .expect("should have request to process");

    let outcome = engine
        .complete_request(&request_id, false, 5000, Some("restart failed".into()))
        .expect("complete_request should succeed");

    assert!(!outcome.success, "outcome should indicate failure");
    assert!(
        outcome.pathway_delta < 0.0,
        "failed outcome should have negative pathway delta"
    );
    assert_eq!(
        outcome.error.as_deref(),
        Some("restart failed"),
        "error message should be preserved"
    );
}

#[test]
fn remediation_cancel_request_removes_from_pending() {
    let engine = RemediationEngine::new();

    let req = engine
        .submit_request(
            "tool-library",
            IssueType::MemoryPressure,
            Severity::Low,
            "high memory usage",
        )
        .expect("submit_request should succeed");

    assert_eq!(engine.pending_count(), 1);

    engine
        .cancel_request(&req.id)
        .expect("cancel_request should succeed for pending request");

    assert_eq!(
        engine.pending_count(),
        0,
        "pending count should be 0 after cancellation"
    );

    let result = engine.cancel_request(&req.id);
    assert!(result.is_err(), "cancelling non-existent request should fail");
}

#[test]
fn remediation_success_rate_computes_correctly() {
    let engine = RemediationEngine::new();

    for (i, success) in [true, true, false].iter().enumerate() {
        let req = engine
            .submit_request(
                "synthex",
                IssueType::HealthFailure,
                Severity::Low,
                &format!("test issue {i}"),
            )
            .expect("submit_request should succeed");

        engine
            .process_next()
            .expect("process_next should succeed")
            .expect("should have pending request");

        engine
            .complete_request(&req.id, *success, 100, None)
            .expect("complete_request should succeed");
    }

    let rate = engine.success_rate();
    let expected = 2.0 / 3.0;
    assert!(
        (rate - expected).abs() < 1e-10,
        "success rate should be ~0.667, got {rate}"
    );
}

#[test]
fn remediation_max_concurrent_respected() {
    let engine = RemediationEngine::with_max_concurrent(2)
        .expect("with_max_concurrent(2) should succeed");

    for i in 0..3 {
        engine
            .submit_request(
                "synthex",
                IssueType::LatencySpike,
                Severity::Low,
                &format!("spike {i}"),
            )
            .expect("submit_request should succeed");
    }

    engine
        .process_next()
        .expect("process 1 should succeed")
        .expect("should return active remediation");
    engine
        .process_next()
        .expect("process 2 should succeed")
        .expect("should return active remediation");

    let third = engine.process_next().expect("process 3 should not error");
    assert!(
        third.is_none(),
        "should return None when at max_concurrent capacity"
    );
    assert_eq!(engine.active_count(), 2);
    assert_eq!(engine.pending_count(), 1);
}

// =========================================================================
// 2. ConfidenceCalculator Tests (7 tests)
// =========================================================================

#[test]
fn confidence_calculate_high_inputs_produces_high_score() {
    let calc = ConfidenceCalculator::new();

    for _ in 0..5 {
        calc.record_outcome("synthex", "health_failure", "restart", true, 0.9)
            .expect("record_outcome should succeed");
    }

    let factors = calc
        .calculate("synthex", IssueType::HealthFailure, Severity::Critical)
        .expect("calculate should succeed");

    assert!(
        factors.calibrated_confidence > 0.5,
        "high-history confidence should be above 0.5, got {}",
        factors.calibrated_confidence
    );
    assert!(
        factors.calibrated_confidence <= 1.0,
        "confidence must not exceed 1.0"
    );
}

#[test]
fn confidence_calculate_low_inputs_produces_lower_score() {
    let calc = ConfidenceCalculator::new();

    for _ in 0..5 {
        calc.record_outcome("bad-service", "timeout", "fallback", false, 0.2)
            .expect("record_outcome should succeed");
    }

    let factors = calc
        .calculate("bad-service", IssueType::Timeout, Severity::Low)
        .expect("calculate should succeed");

    assert!(
        factors.historical_success_rate < 0.5,
        "all-failure history should have rate < 0.5, got {}",
        factors.historical_success_rate
    );
    assert!(
        factors.calibrated_confidence >= 0.0,
        "confidence must be at least 0.0"
    );
}

#[test]
fn confidence_rejects_empty_service_id() {
    let calc = ConfidenceCalculator::new();

    let result = calc.calculate("", IssueType::Crash, Severity::High);
    assert!(result.is_err(), "empty service_id should be rejected");
}

#[test]
fn confidence_clamped_to_unit_interval() {
    let high = calculate_confidence(1.0, 1.0, 1.0, 1.0, 1.0);
    assert!(high <= 1.0, "confidence must be <= 1.0, got {high}");
    assert!(high >= 0.0, "confidence must be >= 0.0, got {high}");

    let low = calculate_confidence(0.0, 0.0, 0.0, 0.0, 0.0);
    assert!(low >= 0.0, "confidence must be >= 0.0, got {low}");
    assert!(low <= 1.0, "confidence must be <= 1.0, got {low}");
}

#[test]
fn confidence_record_outcome_updates_history() {
    let calc = ConfidenceCalculator::new();

    assert_eq!(calc.service_count(), 0, "no services initially");
    assert_eq!(calc.total_records(), 0, "no records initially");

    calc.record_outcome("synthex", "health_failure", "restart", true, 0.85)
        .expect("record_outcome should succeed");

    assert_eq!(calc.service_count(), 1, "one service after recording");
    assert_eq!(calc.total_records(), 1, "one record after recording");

    let rate = calc.get_historical_success_rate("synthex");
    assert!(
        (rate - 1.0).abs() < 1e-10,
        "1 success out of 1 total should give rate 1.0, got {rate}"
    );
}

#[test]
fn confidence_time_factor_defaults_without_history() {
    let calc = ConfidenceCalculator::new();

    let tf = calc.calculate_time_factor("unknown-service");
    assert!(
        (tf - 0.5).abs() < 1e-10,
        "time factor for unknown service should be default 0.5, got {tf}"
    );
}

#[test]
fn confidence_calibration_offset_adjusts_with_data() {
    let calc = ConfidenceCalculator::new();

    for _ in 0..5 {
        calc.record_outcome("overconfident-svc", "error_rate_high", "reset", false, 0.9)
            .expect("record_outcome should succeed");
    }

    let offset = calc.get_calibration_offset("overconfident-svc");
    assert!(
        offset < 0.0,
        "overconfident system should have negative offset, got {offset}"
    );
}

// =========================================================================
// 3. ActionExecutor Tests (8 tests)
// =========================================================================

fn make_remediation_request(
    service_id: &str,
    tier: EscalationTier,
    action: RemediationAction,
) -> RemediationRequest {
    RemediationRequest {
        id: uuid::Uuid::new_v4().to_string(),
        service_id: service_id.into(),
        issue_type: IssueType::HealthFailure,
        severity: Severity::Medium,
        confidence: 0.85,
        suggested_action: action,
        tier,
        context: std::collections::HashMap::new(),
    }
}

#[test]
fn action_dispatch_l0_creates_approved_execution() {
    let executor = ActionExecutor::new();

    let request = make_remediation_request(
        "synthex",
        EscalationTier::L0AutoExecute,
        RemediationAction::CacheCleanup {
            service_id: "synthex".into(),
            threshold_percent: 50,
        },
    );

    let execution = executor
        .dispatch(&request)
        .expect("dispatch should succeed for L0 tier");

    assert_eq!(
        execution.status,
        ActionStatus::Approved,
        "L0 tier should be auto-approved"
    );
    assert_eq!(execution.service_id, "synthex");
    assert_eq!(execution.tier, EscalationTier::L0AutoExecute);
    assert_eq!(executor.get_active_count(), 1);
}

#[test]
fn action_dispatch_l2_creates_pending_execution() {
    let executor = ActionExecutor::new();

    let request = make_remediation_request(
        "san-k7",
        EscalationTier::L2RequireApproval,
        RemediationAction::ServiceRestart {
            service_id: "san-k7".into(),
            graceful: true,
        },
    );

    let execution = executor
        .dispatch(&request)
        .expect("dispatch should succeed");

    assert_eq!(
        execution.status,
        ActionStatus::Pending,
        "L2 tier should require approval"
    );
}

#[test]
fn action_approve_transitions_pending_to_approved() {
    let executor = ActionExecutor::new();

    let request = make_remediation_request(
        "nais",
        EscalationTier::L2RequireApproval,
        RemediationAction::CircuitBreakerReset {
            service_id: "nais".into(),
        },
    );

    let dispatched = executor
        .dispatch(&request)
        .expect("dispatch should succeed");

    assert_eq!(dispatched.status, ActionStatus::Pending);

    let approved = executor
        .approve(&dispatched.execution_id)
        .expect("approve should succeed for pending execution");

    assert_eq!(approved.status, ActionStatus::Approved);
}

#[test]
fn action_reject_moves_to_completed_history() {
    let executor = ActionExecutor::new();

    let request = make_remediation_request(
        "devops-engine",
        EscalationTier::L3PbftConsensus,
        RemediationAction::DatabaseVacuum {
            database: "test.db".into(),
        },
    );

    let dispatched = executor
        .dispatch(&request)
        .expect("dispatch should succeed");

    let rejected = executor
        .reject(&dispatched.execution_id, "consensus vote rejected")
        .expect("reject should succeed for pending execution");

    assert_eq!(rejected.status, ActionStatus::Rejected);
    assert_eq!(
        executor.get_active_count(),
        0,
        "rejected action should not be active"
    );
    assert_eq!(
        executor.get_completed_count(),
        1,
        "rejected action should be in completed"
    );
}

#[test]
fn action_execute_approved_action_completes() {
    let executor = ActionExecutor::new();

    let request = make_remediation_request(
        "tool-library",
        EscalationTier::L0AutoExecute,
        RemediationAction::CacheCleanup {
            service_id: "tool-library".into(),
            threshold_percent: 80,
        },
    );

    let dispatched = executor
        .dispatch(&request)
        .expect("dispatch should succeed");

    let completed = executor
        .execute(&dispatched.execution_id)
        .expect("execute should succeed for approved action");

    assert!(
        completed.status == ActionStatus::Completed
            || completed.status == ActionStatus::Failed,
        "execution should reach a terminal state, got {:?}",
        completed.status
    );
    assert!(completed.completed_at.is_some(), "completed_at should be set");
    assert!(completed.duration_ms.is_some(), "duration_ms should be set");
    assert!(completed.result.is_some(), "result should be set");
}

#[test]
fn action_execute_pending_action_fails() {
    let executor = ActionExecutor::new();

    let request = make_remediation_request(
        "ccm",
        EscalationTier::L2RequireApproval,
        RemediationAction::SessionRotation {
            session_id: "sess-123".into(),
        },
    );

    let dispatched = executor
        .dispatch(&request)
        .expect("dispatch should succeed");

    let result = executor.execute(&dispatched.execution_id);
    assert!(
        result.is_err(),
        "executing a pending (unapproved) action should fail"
    );
}

#[test]
fn action_rollback_checkpoint_workflow() {
    let executor = ActionExecutor::new();

    let request = make_remediation_request(
        "bash-engine",
        EscalationTier::L0AutoExecute,
        RemediationAction::GracefulDegradation {
            service_id: "bash-engine".into(),
            level: 2,
        },
    );

    let dispatched = executor
        .dispatch(&request)
        .expect("dispatch should succeed");

    let rollback_id = executor
        .save_checkpoint(&dispatched.execution_id, "pre-degradation state snapshot")
        .expect("save_checkpoint should succeed");

    assert!(!rollback_id.is_empty(), "rollback_id should not be empty");

    executor
        .rollback(&rollback_id)
        .expect("rollback should succeed for valid checkpoint");

    let result = executor.rollback(&rollback_id);
    assert!(result.is_err(), "double rollback should fail");
}

#[test]
fn action_max_concurrent_enforced() {
    let executor = ActionExecutor::with_max_concurrent(2);

    for i in 0..2 {
        let request = make_remediation_request(
            &format!("svc-{i}"),
            EscalationTier::L0AutoExecute,
            RemediationAction::CacheCleanup {
                service_id: format!("svc-{i}"),
                threshold_percent: 50,
            },
        );
        executor
            .dispatch(&request)
            .expect("dispatch should succeed within capacity");
    }

    assert!(!executor.can_accept_more(), "should be at capacity");

    let request = make_remediation_request(
        "svc-overflow",
        EscalationTier::L0AutoExecute,
        RemediationAction::CacheCleanup {
            service_id: "svc-overflow".into(),
            threshold_percent: 50,
        },
    );

    let result = executor.dispatch(&request);
    assert!(result.is_err(), "dispatch should fail when at capacity");
}

#[test]
fn action_success_rate_computes_correctly() {
    let executor = ActionExecutor::new();

    for i in 0..3 {
        let request = make_remediation_request(
            &format!("rate-svc-{i}"),
            EscalationTier::L0AutoExecute,
            RemediationAction::RetryWithBackoff {
                max_retries: 3,
                initial_delay_ms: 100,
            },
        );

        let dispatched = executor
            .dispatch(&request)
            .expect("dispatch should succeed");

        let _ = executor.execute(&dispatched.execution_id);
    }

    let rate = executor.get_success_rate();
    assert!(
        (0.0..=1.0).contains(&rate),
        "success rate should be in [0.0, 1.0], got {rate}"
    );
    assert_eq!(
        executor.get_active_count(),
        0,
        "all actions should be completed"
    );
}

// =========================================================================
// 4. OutcomeRecorder Tests (7 tests)
// =========================================================================

#[test]
fn outcome_record_and_retrieve() {
    let recorder = OutcomeRecorder::new();

    let action = RemediationAction::CacheCleanup {
        service_id: "synthex".into(),
        threshold_percent: 80,
    };

    let record = recorder
        .record(
            "synthex", "req-001", IssueType::MemoryPressure, Severity::Medium,
            action, true, 200, 0.85, 0.9,
        )
        .expect("record should succeed");

    assert_eq!(record.service_id, "synthex");
    assert_eq!(record.request_id, "req-001");
    assert!(record.success);
    assert_eq!(record.duration_ms, 200);

    let retrieved = recorder
        .get_outcome(&record.id)
        .expect("get_outcome should find the recorded outcome");
    assert_eq!(retrieved.id, record.id);
}

#[test]
fn outcome_service_outcomes_returns_correct_records() {
    let recorder = OutcomeRecorder::new();

    let action = RemediationAction::CircuitBreakerReset {
        service_id: "san-k7".into(),
    };

    for _ in 0..2 {
        recorder
            .record(
                "san-k7", "req-s", IssueType::ConnectionFailure, Severity::High,
                action.clone(), true, 100, 0.8, 0.85,
            )
            .expect("record should succeed");
    }

    recorder
        .record(
            "nais", "req-n", IssueType::LatencySpike, Severity::Low,
            action, true, 50, 0.9, 0.7,
        )
        .expect("record should succeed");

    let san_k7_outcomes = recorder.get_service_outcomes("san-k7");
    assert_eq!(san_k7_outcomes.len(), 2, "san-k7 should have 2 outcomes");

    let nais_outcomes = recorder.get_service_outcomes("nais");
    assert_eq!(nais_outcomes.len(), 1, "nais should have 1 outcome");

    let empty = recorder.get_service_outcomes("unknown-service");
    assert!(empty.is_empty(), "unknown service should have no outcomes");
}

#[test]
fn outcome_aggregate_by_service() {
    let recorder = OutcomeRecorder::new();

    let action = RemediationAction::RetryWithBackoff {
        max_retries: 3,
        initial_delay_ms: 100,
    };

    recorder
        .record(
            "synthex", "req-1", IssueType::Timeout, Severity::Medium,
            action.clone(), true, 150, 0.8, 0.9,
        )
        .expect("record should succeed");

    recorder
        .record(
            "synthex", "req-2", IssueType::Timeout, Severity::Medium,
            action, false, 300, 0.7, 0.3,
        )
        .expect("record should succeed");

    let agg = recorder
        .get_aggregate("synthex")
        .expect("aggregate should exist after recording outcomes");

    assert_eq!(agg.total_outcomes, 2);
    assert_eq!(agg.successful_outcomes, 1);
    assert!(agg.avg_effectiveness > 0.0, "avg effectiveness should be > 0");
}

#[test]
fn outcome_effectiveness_by_action() {
    let recorder = OutcomeRecorder::new();

    let action = RemediationAction::CacheCleanup {
        service_id: "tool-library".into(),
        threshold_percent: 50,
    };

    recorder
        .record(
            "tool-library", "req-1", IssueType::MemoryPressure, Severity::Low,
            action.clone(), true, 100, 0.9, 0.8,
        )
        .expect("record should succeed");

    recorder
        .record(
            "tool-library", "req-2", IssueType::MemoryPressure, Severity::Low,
            action.clone(), true, 120, 0.85, 0.6,
        )
        .expect("record should succeed");

    let eff = recorder.get_effectiveness("tool-library", &action);
    let expected = f64::midpoint(0.8, 0.6);
    assert!(
        (eff - expected).abs() < 1e-10,
        "effectiveness should be {expected}, got {eff}"
    );
}

#[test]
fn outcome_pathway_delta_positive_on_success() {
    let recorder = OutcomeRecorder::new();

    let action = RemediationAction::CircuitBreakerReset {
        service_id: "nais".into(),
    };

    recorder
        .record(
            "nais", "req-1", IssueType::ConnectionFailure, Severity::High,
            action.clone(), true, 200, 0.8, 0.9,
        )
        .expect("record should succeed");

    let delta = recorder.calculate_pathway_delta("nais", &action, true);
    assert!(
        delta > 0.0,
        "success pathway delta should be positive, got {delta}"
    );

    let delta_fail = recorder.calculate_pathway_delta("nais", &action, false);
    assert!(
        delta_fail < 0.0,
        "failure pathway delta should be negative, got {delta_fail}"
    );
}

#[test]
fn outcome_trend_returns_effectiveness_values() {
    let recorder = OutcomeRecorder::new();

    let action = RemediationAction::RetryWithBackoff {
        max_retries: 3,
        initial_delay_ms: 100,
    };

    let effectiveness_values = [0.3, 0.5, 0.7, 0.9];
    for (i, eff) in effectiveness_values.iter().enumerate() {
        recorder
            .record(
                "trend-svc",
                &format!("req-{i}"),
                IssueType::Timeout,
                Severity::Medium,
                action.clone(),
                true,
                100,
                0.8,
                *eff,
            )
            .expect("record should succeed");
    }

    let trend = recorder.get_trend("trend-svc", 3);
    assert_eq!(trend.len(), 3, "should return last 3 effectiveness values");
    assert!(
        (trend[0] - 0.5).abs() < 1e-10,
        "first trend value should be 0.5, got {}",
        trend[0]
    );
    assert!(
        (trend[2] - 0.9).abs() < 1e-10,
        "last trend value should be 0.9, got {}",
        trend[2]
    );
}

#[test]
fn outcome_overall_success_rate() {
    let recorder = OutcomeRecorder::new();

    let action = RemediationAction::FallbackToCached {
        key: "cache-key".into(),
        ttl_seconds: 300,
    };

    for (i, success) in [true, true, true, false].iter().enumerate() {
        recorder
            .record(
                "rate-svc",
                &format!("req-{i}"),
                IssueType::ErrorRateHigh,
                Severity::High,
                action.clone(),
                *success,
                100,
                0.8,
                if *success { 0.9 } else { 0.1 },
            )
            .expect("record should succeed");
    }

    let rate = recorder.overall_success_rate();
    let expected = 3.0 / 4.0;
    assert!(
        (rate - expected).abs() < 1e-10,
        "overall success rate should be {expected}, got {rate}"
    );
    assert_eq!(recorder.total_outcomes(), 4);
}

// =========================================================================
// 5. FeedbackLoop Tests (6 tests)
// =========================================================================

#[test]
fn feedback_generate_reinforcement_signal_on_high_effectiveness() {
    let feedback = FeedbackLoop::new();

    let signal = feedback
        .generate_signal("synthex", "out-001", true, 0.9, 0.85, "health->synthex")
        .expect("generate_signal should succeed");

    assert_eq!(signal.service_id, "synthex");
    assert!(signal.strength > 0.0, "reinforcement signal should be positive");
    assert_eq!(signal.signal_type, SignalType::Reinforcement);
    assert_eq!(feedback.signal_count(), 1);
}

#[test]
fn feedback_generate_correction_signal_on_failure() {
    let feedback = FeedbackLoop::new();

    let signal = feedback
        .generate_signal("nais", "out-002", false, 0.0, 0.7, "latency->nais")
        .expect("generate_signal should succeed");

    assert_eq!(signal.signal_type, SignalType::Correction);
    assert!(signal.strength < 0.0, "correction signal should be negative");
}

#[test]
fn feedback_generate_exploration_signal_on_weak_success() {
    let feedback = FeedbackLoop::new();

    let signal = feedback
        .generate_signal("san-k7", "out-003", true, 0.4, 0.6, "errors->san-k7")
        .expect("generate_signal should succeed");

    assert_eq!(signal.signal_type, SignalType::Exploration);
    assert!(
        signal.strength > 0.0,
        "exploration signal should be positive, got {}",
        signal.strength
    );
}

#[test]
fn feedback_record_calibration_tracks_offset() {
    let feedback = FeedbackLoop::new();

    for _ in 0..5 {
        feedback
            .record_calibration("overconfident-svc", 0.9, false)
            .expect("record_calibration should succeed");
    }

    let offset = feedback.get_calibration_offset("overconfident-svc");
    assert!(
        offset < 0.0,
        "overconfident system should have negative offset, got {offset}"
    );

    for _ in 0..5 {
        feedback
            .record_calibration("underconfident-svc", 0.2, true)
            .expect("record_calibration should succeed");
    }

    let offset_under = feedback.get_calibration_offset("underconfident-svc");
    assert!(
        offset_under > 0.0,
        "underconfident system should have positive offset, got {offset_under}"
    );
}

#[test]
fn feedback_recommendations_for_consistent_signals() {
    let feedback = FeedbackLoop::new();

    for i in 0..5 {
        feedback
            .generate_signal(
                "well-tuned-svc",
                &format!("out-{i}"),
                true,
                0.95,
                0.9,
                "monitor->well-tuned-svc",
            )
            .expect("generate_signal should succeed");
    }

    let recs = feedback
        .generate_recommendations("well-tuned-svc")
        .expect("generate_recommendations should succeed");

    assert!(
        recs.iter()
            .any(|r| r.recommendation_type == RecommendationType::StrengthenPathway),
        "should have StrengthenPathway recommendation for consistently positive signals"
    );
}

#[test]
fn feedback_recent_signals_returns_latest_n() {
    let feedback = FeedbackLoop::new();

    for i in 0..10 {
        feedback
            .generate_signal(
                "svc",
                &format!("out-{i}"),
                true,
                0.8,
                0.7,
                "path->svc",
            )
            .expect("generate_signal should succeed");
    }

    let recent = feedback.get_recent_signals(3);
    assert_eq!(recent.len(), 3, "should return exactly 3 recent signals");

    let all = feedback.get_recent_signals(100);
    assert_eq!(all.len(), 10, "requesting more than available should return all");
}

#[test]
fn feedback_clear_old_signals_removes_stale() {
    let feedback = FeedbackLoop::new();

    for i in 0..5 {
        feedback
            .generate_signal("svc", &format!("out-{i}"), true, 0.8, 0.7, "p->svc")
            .expect("generate_signal should succeed");
    }

    assert_eq!(feedback.signal_count(), 5);

    let future = chrono::Utc::now() + chrono::Duration::seconds(1);
    let removed = feedback.clear_old_signals(future);
    assert_eq!(removed, 5, "all 5 signals should be removed");
    assert_eq!(
        feedback.signal_count(),
        0,
        "signal count should be 0 after clearing"
    );
}

// =========================================================================
// 6. PipelineManager Tests (6 tests)
// =========================================================================

#[test]
fn pipeline_default_pipelines_loaded() {
    let manager = PipelineManager::new();

    assert_eq!(
        manager.pipeline_count(),
        8,
        "PipelineManager should have 8 default pipelines"
    );

    let defaults = default_pipelines();
    for pl in &defaults {
        let entry = manager
            .get_pipeline(&pl.id)
            .unwrap_or_else(|_| panic!("default pipeline '{}' should exist", pl.id));
        assert_eq!(entry.pipeline.name, pl.name);
    }
}

#[test]
fn pipeline_get_enabled_returns_all_defaults() {
    let manager = PipelineManager::new();

    let enabled = manager.get_enabled_pipelines();
    assert_eq!(
        enabled.len(),
        8,
        "all 8 default pipelines should be enabled"
    );
}

#[test]
fn pipeline_get_by_id_returns_correct_entry() {
    let manager = PipelineManager::new();

    let entry = manager
        .get_pipeline("PL-HEALTH-001")
        .expect("PL-HEALTH-001 should exist");

    assert_eq!(entry.pipeline.name, "Health Monitoring Pipeline");
    assert_eq!(entry.pipeline.priority, 1);
    assert_eq!(entry.pipeline.latency_slo_ms, 100);
}

#[test]
fn pipeline_start_complete_execution_workflow() {
    let manager = PipelineManager::new();

    let execution = manager
        .start_execution("PL-HEALTH-001")
        .expect("start_execution should succeed");

    assert_eq!(execution.pipeline_id, "PL-HEALTH-001");
    assert_eq!(execution.status, ExecutionStatus::InProgress);

    let stages = vec![
        PipelineStage::Source,
        PipelineStage::Ingress,
        PipelineStage::Transform,
        PipelineStage::Sink,
    ];

    let completed = manager
        .complete_execution(&execution.execution_id, stages)
        .expect("complete_execution should succeed");

    assert_eq!(completed.status, ExecutionStatus::Completed);
    assert!(completed.completed_at.is_some(), "completed_at should be set");
    assert!(completed.duration_ms.is_some(), "duration_ms should be set");
    assert_eq!(
        completed.stages_completed.len(),
        4,
        "should have 4 completed stages"
    );
}

#[test]
fn pipeline_fail_execution_records_error() {
    let manager = PipelineManager::new();

    let execution = manager
        .start_execution("PL-LOG-001")
        .expect("start_execution should succeed");

    let failed = manager
        .fail_execution(&execution.execution_id, "disk full".to_owned())
        .expect("fail_execution should succeed");

    assert_eq!(failed.status, ExecutionStatus::Failed);
    assert_eq!(failed.error.as_deref(), Some("disk full"));
    assert!(failed.completed_at.is_some());
}

#[test]
fn pipeline_slo_compliance_check() {
    let manager = PipelineManager::new();

    let execution = manager
        .start_execution("PL-HEALTH-001")
        .expect("start_execution should succeed");

    manager
        .complete_execution(
            &execution.execution_id,
            vec![PipelineStage::Source, PipelineStage::Sink],
        )
        .expect("complete_execution should succeed");

    let slo = manager
        .check_slo_compliance("PL-HEALTH-001")
        .expect("check_slo_compliance should succeed");

    assert_eq!(slo.pipeline_id, "PL-HEALTH-001");
    assert_eq!(slo.slo_target_ms, 100);
    assert!(slo.compliant, "near-instant execution should be SLO compliant");
}

// =========================================================================
// 7. Cross-Module L3 Workflows (3 tests)
// =========================================================================

#[test]
fn workflow_remediation_to_confidence_to_tier_determination() {
    let calc = ConfidenceCalculator::new();

    for _ in 0..10 {
        calc.record_outcome("synthex", "health_failure", "cache_cleanup", true, 0.85)
            .expect("record_outcome should succeed");
    }

    let factors = calc
        .calculate("synthex", IssueType::HealthFailure, Severity::Low)
        .expect("calculate should succeed");

    let action = RemediationAction::CacheCleanup {
        service_id: "synthex".into(),
        threshold_percent: 50,
    };

    let tier = determine_tier(factors.calibrated_confidence, Severity::Low, &action);

    assert!(
        tier == EscalationTier::L0AutoExecute || tier == EscalationTier::L1NotifyHuman,
        "high confidence + low severity should yield L0 or L1, got {tier:?}"
    );
}

#[test]
fn workflow_full_remediation_pipeline() {
    let rem_engine = RemediationEngine::new();
    let request = rem_engine
        .submit_request(
            "synthex",
            IssueType::HealthFailure,
            Severity::Medium,
            "health degraded",
        )
        .expect("submit_request should succeed");

    let active = rem_engine
        .process_next()
        .expect("process_next should succeed")
        .expect("should have pending request");

    let executor = ActionExecutor::new();
    let execution = executor
        .dispatch(&active.request)
        .expect("dispatch should succeed");

    if execution.status == ActionStatus::Approved {
        let result = executor.execute(&execution.execution_id);

        let success = result
            .as_ref()
            .map(|r| r.status == ActionStatus::Completed)
            .unwrap_or(false);

        let outcome = rem_engine
            .complete_request(&request.id, success, 200, None)
            .expect("complete_request should succeed");

        let recorder = OutcomeRecorder::new();
        let record = recorder
            .record(
                "synthex",
                &request.id,
                IssueType::HealthFailure,
                Severity::Medium,
                request.suggested_action.clone(),
                success,
                200,
                request.confidence,
                if success { 0.85 } else { 0.2 },
            )
            .expect("record should succeed");

        let feedback = FeedbackLoop::new();
        let signal = feedback
            .generate_signal(
                "synthex",
                &record.id,
                success,
                record.actual_effectiveness,
                request.confidence,
                "remediation->synthex",
            )
            .expect("generate_signal should succeed");

        if success {
            assert!(
                signal.strength > 0.0,
                "successful outcome should produce positive signal"
            );
        } else {
            assert!(
                signal.strength <= 0.0,
                "failed outcome should produce non-positive signal"
            );
        }

        feedback
            .record_calibration("synthex", request.confidence, success)
            .expect("record_calibration should succeed");

        let delta = outcome.pathway_delta;
        if success {
            assert!(delta > 0.0, "successful pathway delta should be positive");
        } else {
            assert!(delta < 0.0, "failed pathway delta should be negative");
        }
    } else {
        let approved = executor
            .approve(&execution.execution_id)
            .expect("approve should succeed");

        assert_eq!(approved.status, ActionStatus::Approved);

        let _ = executor.execute(&execution.execution_id);

        rem_engine
            .complete_request(&request.id, true, 500, None)
            .expect("complete_request should succeed");
    }
}

#[test]
fn workflow_confidence_feedback_calibration_loop() {
    let calc = ConfidenceCalculator::new();
    let recorder = OutcomeRecorder::new();
    let feedback = FeedbackLoop::new();

    let action = RemediationAction::RetryWithBackoff {
        max_retries: 3,
        initial_delay_ms: 100,
    };

    for i in 0..10 {
        let factors = calc
            .calculate("calibration-svc", IssueType::Timeout, Severity::Medium)
            .expect("calculate should succeed");

        let confidence = factors.calibrated_confidence;

        let success = i % 3 != 2;
        let effectiveness = if success { 0.8 } else { 0.1 };

        let record = recorder
            .record(
                "calibration-svc",
                &format!("req-{i}"),
                IssueType::Timeout,
                Severity::Medium,
                action.clone(),
                success,
                100,
                confidence,
                effectiveness,
            )
            .expect("record should succeed");

        feedback
            .generate_signal(
                "calibration-svc",
                &record.id,
                success,
                effectiveness,
                confidence,
                "timeout->calibration-svc",
            )
            .expect("generate_signal should succeed");

        feedback
            .record_calibration("calibration-svc", confidence, success)
            .expect("record_calibration should succeed");

        calc.record_outcome(
            "calibration-svc",
            "timeout",
            "retry_backoff",
            success,
            confidence,
        )
        .expect("record_outcome should succeed");
    }

    assert_eq!(calc.total_records(), 10, "should have 10 confidence records");
    assert_eq!(
        recorder.total_outcomes(),
        10,
        "should have 10 outcome records"
    );
    assert_eq!(
        feedback.signal_count(),
        10,
        "should have 10 feedback signals"
    );

    let offset = feedback.get_calibration_offset("calibration-svc");
    assert!(
        offset.is_finite(),
        "calibration offset should be finite, got {offset}"
    );

    let recs = feedback
        .generate_recommendations("calibration-svc")
        .expect("generate_recommendations should succeed");

    assert!(
        recs.len() <= 10,
        "should not generate excessive recommendations"
    );

    let success_rate = recorder.overall_success_rate();
    let expected_rate = 7.0 / 10.0;
    assert!(
        (success_rate - expected_rate).abs() < 1e-10,
        "overall success rate should be {expected_rate}, got {success_rate}"
    );
}
