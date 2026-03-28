//! # PBFT Consensus Benchmarks
//!
//! Benchmarks for PBFT consensus operations using real crate types (M31-M36).
//! Target SLO: <5s for full consensus round.
//! Configuration: n=40 agents, f=13 Byzantine tolerance, q=27 quorum.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

use maintenance_engine::m6_consensus::pbft::PbftManager;
use maintenance_engine::m6_consensus::{ConsensusAction, VoteType};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a PbftManager with a single pre-created proposal and return
/// `(manager, proposal_id)`. Returns `None` if proposal creation fails.
fn manager_with_proposal(action: ConsensusAction) -> Option<(PbftManager, String)> {
    let mgr = PbftManager::new();
    let proposal = mgr.create_proposal(action, "@0.A").ok()?;
    Some((mgr, proposal.id))
}

/// Submit approval votes from the first `n` agents in the fleet.
/// Returns the number of votes successfully submitted.
fn submit_n_approvals(mgr: &PbftManager, proposal_id: &str, n: usize) -> usize {
    let fleet = mgr.get_fleet();
    let mut count = 0_usize;
    for agent in fleet.iter().take(n) {
        if mgr
            .submit_vote(proposal_id, &agent.id, VoteType::Approve, None)
            .is_ok()
        {
            count += 1;
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Benchmark: Creating proposals
// ---------------------------------------------------------------------------

fn bench_create_proposal(c: &mut Criterion) {
    let mut group = c.benchmark_group("pbft_create_proposal");
    group.measurement_time(Duration::from_secs(5));

    // Single proposal creation (fresh manager each iteration)
    group.bench_function("single", |b| {
        b.iter(|| {
            let mgr = PbftManager::new();
            black_box(mgr.create_proposal(ConsensusAction::ServiceTermination, "@0.A").ok())
        });
    });

    // Multiple proposals on the same manager
    for count in [5, 10, 20] {
        group.throughput(Throughput::Elements(count));
        group.bench_with_input(
            BenchmarkId::new("sequential", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let mgr = PbftManager::new();
                    for _ in 0..count {
                        let _ = mgr.create_proposal(
                            ConsensusAction::DatabaseMigration,
                            "@0.A",
                        );
                    }
                    black_box(mgr.proposal_count())
                });
            },
        );
    }

    // Each action type
    let actions = [
        ("service_termination", ConsensusAction::ServiceTermination),
        ("database_migration", ConsensusAction::DatabaseMigration),
        ("credential_rotation", ConsensusAction::CredentialRotation),
        ("cascade_restart", ConsensusAction::CascadeRestart),
        ("config_rollback", ConsensusAction::ConfigRollback),
    ];
    for (name, action) in &actions {
        group.bench_function(format!("action_{name}"), |b| {
            let mgr = PbftManager::new();
            b.iter(|| black_box(mgr.create_proposal(*action, "@0.A").ok()));
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Submitting votes
// ---------------------------------------------------------------------------

fn bench_submit_votes(c: &mut Criterion) {
    let mut group = c.benchmark_group("pbft_submit_votes");
    group.measurement_time(Duration::from_secs(5));

    // Single vote submission
    group.bench_function("single_approve", |b| {
        b.iter_with_setup(
            || manager_with_proposal(ConsensusAction::ServiceTermination),
            |setup| {
                if let Some((mgr, pid)) = setup {
                    black_box(
                        mgr.submit_vote(&pid, "@0.A", VoteType::Approve, None)
                            .ok(),
                    );
                }
            },
        );
    });

    group.bench_function("single_reject_with_reason", |b| {
        b.iter_with_setup(
            || manager_with_proposal(ConsensusAction::ConfigRollback),
            |setup| {
                if let Some((mgr, pid)) = setup {
                    black_box(
                        mgr.submit_vote(
                            &pid,
                            "agent-29",
                            VoteType::Reject,
                            Some("Risk too high".into()),
                        )
                        .ok(),
                    );
                }
            },
        );
    });

    // Batch vote submission (quorum size, full fleet)
    for vote_count in [27_usize, 36, 41] {
        group.throughput(Throughput::Elements(vote_count as u64));
        group.bench_with_input(
            BenchmarkId::new("batch_approve", vote_count),
            &vote_count,
            |b, &count| {
                b.iter_with_setup(
                    || manager_with_proposal(ConsensusAction::CascadeRestart),
                    |setup| {
                        if let Some((mgr, pid)) = setup {
                            black_box(submit_n_approvals(&mgr, &pid, count));
                        }
                    },
                );
            },
        );
    }

    // Mixed votes (approve, reject, abstain)
    group.bench_function("mixed_vote_types", |b| {
        b.iter_with_setup(
            || manager_with_proposal(ConsensusAction::DatabaseMigration),
            |setup| {
                if let Some((mgr, pid)) = setup {
                    let fleet = mgr.get_fleet();
                    for (i, agent) in fleet.iter().enumerate() {
                        let vote_type = match i % 3 {
                            0 => VoteType::Approve,
                            1 => VoteType::Reject,
                            _ => VoteType::Abstain,
                        };
                        let _ = mgr.submit_vote(&pid, &agent.id, vote_type, None);
                    }
                    black_box(());
                }
            },
        );
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Tallying votes
// ---------------------------------------------------------------------------

fn bench_tally_votes(c: &mut Criterion) {
    let mut group = c.benchmark_group("pbft_tally_votes");
    group.measurement_time(Duration::from_secs(5));

    // Tally with various vote counts
    for vote_count in [10_usize, 27, 36, 41] {
        let label = format!("{vote_count}_votes");
        group.bench_function(&label, |b| {
            b.iter_with_setup(
                || {
                    let pair = manager_with_proposal(ConsensusAction::ServiceTermination);
                    if let Some((ref mgr, ref pid)) = pair {
                        submit_n_approvals(mgr, pid, vote_count);
                    }
                    pair
                },
                |setup| {
                    if let Some((mgr, pid)) = setup {
                        black_box(mgr.tally_votes(&pid).ok());
                    }
                },
            );
        });
    }

    // Tally with mixed vote outcomes
    group.bench_function("mixed_quorum_reached", |b| {
        b.iter_with_setup(
            || {
                let pair = manager_with_proposal(ConsensusAction::DatabaseMigration);
                if let Some((ref mgr, ref pid)) = pair {
                    let fleet = mgr.get_fleet();
                    // 30 approve, 11 reject -> quorum reached
                    for (i, agent) in fleet.iter().enumerate() {
                        let vote_type = if i < 30 {
                            VoteType::Approve
                        } else {
                            VoteType::Reject
                        };
                        let _ = mgr.submit_vote(pid, &agent.id, vote_type, None);
                    }
                }
                pair
            },
            |setup| {
                if let Some((mgr, pid)) = setup {
                    black_box(mgr.tally_votes(&pid).ok());
                }
            },
        );
    });

    group.bench_function("mixed_quorum_not_reached", |b| {
        b.iter_with_setup(
            || {
                let pair = manager_with_proposal(ConsensusAction::CredentialRotation);
                if let Some((ref mgr, ref pid)) = pair {
                    let fleet = mgr.get_fleet();
                    // 10 approve, 31 reject -> quorum NOT reached
                    for (i, agent) in fleet.iter().enumerate() {
                        let vote_type = if i < 10 {
                            VoteType::Approve
                        } else {
                            VoteType::Reject
                        };
                        let _ = mgr.submit_vote(pid, &agent.id, vote_type, None);
                    }
                }
                pair
            },
            |setup| {
                if let Some((mgr, pid)) = setup {
                    black_box(mgr.tally_votes(&pid).ok());
                }
            },
        );
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Full consensus round
// ---------------------------------------------------------------------------

fn bench_full_consensus_round(c: &mut Criterion) {
    let mut group = c.benchmark_group("pbft_full_round");
    group.measurement_time(Duration::from_secs(10));

    // Complete round: create proposal -> advance phases -> vote -> tally
    group.bench_function("complete_round_with_quorum", |b| {
        b.iter(|| {
            let mgr = PbftManager::new();

            // 1. Create proposal
            let proposal = mgr
                .create_proposal(ConsensusAction::CascadeRestart, "@0.A")
                .ok();

            if let Some(proposal) = proposal {
                let pid = proposal.id;

                // 2. Advance PrePrepare -> Prepare
                let _ = mgr.advance_phase(&pid);

                // 3. Submit votes (36 agents to include Critics + Integrators)
                let fleet = mgr.get_fleet();
                for agent in fleet.iter().take(36) {
                    let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Approve, None);
                }

                // 4. Advance Prepare -> Commit (validates quorum)
                let _ = mgr.advance_phase(&pid);

                // 5. Advance Commit -> Execute (validates enhanced consensus)
                let _ = mgr.advance_phase(&pid);

                // 6. Advance Execute -> Complete
                let _ = mgr.advance_phase(&pid);

                // 7. Tally votes
                black_box(mgr.tally_votes(&pid).ok());
            }
        });
    });

    // Multiple sequential rounds on the same manager
    for round_count in [3_u64, 5, 10] {
        group.throughput(Throughput::Elements(round_count));
        group.bench_with_input(
            BenchmarkId::new("sequential_rounds", round_count),
            &round_count,
            |b, &count| {
                b.iter(|| {
                    let mgr = PbftManager::new();
                    let fleet = mgr.get_fleet();

                    for _ in 0..count {
                        if let Some(proposal) = mgr
                            .create_proposal(ConsensusAction::ConfigRollback, "@0.A")
                            .ok()
                        {
                            let pid = proposal.id;
                            let _ = mgr.advance_phase(&pid);

                            for agent in fleet.iter().take(36) {
                                let _ = mgr.submit_vote(
                                    &pid,
                                    &agent.id,
                                    VoteType::Approve,
                                    None,
                                );
                            }

                            let _ = mgr.advance_phase(&pid);
                            let _ = mgr.advance_phase(&pid);
                            let _ = mgr.advance_phase(&pid);
                            let _ = mgr.tally_votes(&pid);
                        }
                    }

                    black_box(mgr.proposal_count());
                });
            },
        );
    }

    // Round that fails quorum (partial votes, cannot advance past Prepare)
    group.bench_function("round_quorum_failure", |b| {
        b.iter(|| {
            let mgr = PbftManager::new();

            if let Some(proposal) = mgr
                .create_proposal(ConsensusAction::ServiceTermination, "@0.A")
                .ok()
            {
                let pid = proposal.id;
                let _ = mgr.advance_phase(&pid); // PrePrepare -> Prepare

                // Submit only 10 votes (below quorum of 27)
                let fleet = mgr.get_fleet();
                for agent in fleet.iter().take(10) {
                    let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Approve, None);
                }

                // Attempt Prepare -> Commit (should fail)
                let result = mgr.advance_phase(&pid);
                black_box(result.is_err());

                // Tally should show quorum not reached
                black_box(mgr.tally_votes(&pid).ok());
            }
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Fleet operations
// ---------------------------------------------------------------------------

fn bench_fleet_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("pbft_fleet");
    group.measurement_time(Duration::from_secs(5));

    // Get full fleet
    group.bench_function("get_fleet", |b| {
        let mgr = PbftManager::new();
        b.iter(|| black_box(mgr.get_fleet()));
    });

    // Get specific agents
    group.bench_function("get_agent_human", |b| {
        let mgr = PbftManager::new();
        b.iter(|| black_box(mgr.get_agent("@0.A").ok()));
    });

    group.bench_function("get_agent_validator", |b| {
        let mgr = PbftManager::new();
        b.iter(|| black_box(mgr.get_agent("agent-01").ok()));
    });

    group.bench_function("get_agent_critic", |b| {
        let mgr = PbftManager::new();
        b.iter(|| black_box(mgr.get_agent("agent-29").ok()));
    });

    group.bench_function("get_agent_integrator", |b| {
        let mgr = PbftManager::new();
        b.iter(|| black_box(mgr.get_agent("agent-35").ok()));
    });

    // Active proposals query
    group.bench_function("get_active_proposals_empty", |b| {
        let mgr = PbftManager::new();
        b.iter(|| black_box(mgr.get_active_proposals()));
    });

    group.bench_function("get_active_proposals_10", |b| {
        let mgr = PbftManager::new();
        for _ in 0..10 {
            let _ = mgr.create_proposal(ConsensusAction::ConfigRollback, "@0.A");
        }
        b.iter(|| black_box(mgr.get_active_proposals()));
    });

    group.bench_function("get_active_proposals_50", |b| {
        let mgr = PbftManager::new();
        for _ in 0..50 {
            let _ = mgr.create_proposal(ConsensusAction::DatabaseMigration, "@0.A");
        }
        b.iter(|| black_box(mgr.get_active_proposals()));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion harness
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_create_proposal,
    bench_submit_votes,
    bench_tally_votes,
    bench_full_consensus_round,
    bench_fleet_operations
);

criterion_main!(benches);
