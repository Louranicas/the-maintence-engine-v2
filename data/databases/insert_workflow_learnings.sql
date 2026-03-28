-- ME v2 One-Shot Scaffolding Workflow Learnings
-- Generated: 2026-03-06
-- Author: claude-opus-4-6

BEGIN TRANSACTION;

-- ============================================================
-- 1. WORKFLOW DEFINITION
-- ============================================================
INSERT INTO workflow_definitions (
    workflow_id, workflow_name, description, version,
    workflow_type, execution_mode, max_concurrent_tasks, timeout_seconds,
    retry_policy, trigger_type, trigger_config,
    required_approval_tier, allowed_agents,
    is_enabled, is_template, created_by, tags
) VALUES (
    'wf-mev2-oneshot-scaffold',
    'ME v2 One-Shot Scaffolding',
    'Complete scaffolding workflow for Maintenance Engine v2: 7-phase one-shot deployment covering reconnaissance, directory structure, gold standard cloning, database cloning, asset cloning, master plan generation, and documentation. Produces 209 files across 55 directories with 23,907 LOC of exemplar code and 12 cloned databases (5.9MB).',
    '1.0.0',
    'migration',
    'dag',
    6,
    7200,
    '{"strategy":"none","reason":"one-shot scaffolding, no retry needed"}',
    'manual',
    '{"description":"Triggered by architect decision to scaffold ME v2 from ME v1 gold standards"}',
    'L2',
    '["claude-opus-4-6","fleet-recon-1","fleet-recon-2","fleet-recon-3","fleet-recon-4","fleet-recon-5","fleet-recon-6"]',
    1,
    1,
    'claude-opus-4-6',
    '["scaffolding","me-v2","one-shot","migration","gold-standard","48-modules","8-layers"]'
);

-- ============================================================
-- 2. WORKFLOW STEPS (7 Phases)
-- ============================================================

-- Phase 1: Reconnaissance
INSERT INTO workflow_steps (
    step_id, workflow_id, step_name, description, step_order,
    step_type, action_type, action_config,
    timeout_seconds, retry_count, continue_on_failure, depends_on
) VALUES (
    'step-scaffold-p1-recon',
    'wf-mev2-oneshot-scaffold',
    'Phase 1: Reconnaissance',
    '6 parallel fleet agents deployed for intelligence gathering: (1) ME v1 architecture analysis, (2) database forensics across 12 DBs, (3) M1/M2 pattern extraction for gold standard identification, (4) module spec analysis for 48-module blueprint, (5) Nexus+OVM integration pattern discovery, (6) DevOps Engine v2 pattern extraction',
    1,
    'parallel',
    'execute',
    '{"agents":6,"targets":["me-v1-analysis","database-forensics","m1-m2-patterns","module-specs","nexus-ovm-integration","devops-v2-patterns"],"execution":"parallel-fleet","completion_time_minutes":"2-3"}',
    300,
    0,
    0,
    NULL
);

-- Phase 2: Directory Structure
INSERT INTO workflow_steps (
    step_id, workflow_id, step_name, description, step_order,
    step_type, action_type, action_config,
    timeout_seconds, retry_count, continue_on_failure, depends_on
) VALUES (
    'step-scaffold-p2-dirs',
    'wf-mev2-oneshot-scaffold',
    'Phase 2: Directory Structure',
    '54 directories created matching src/ module layout with ai_docs, ai_specs, config, data, migrations, tests, benches, .claude infrastructure. Establishes the complete filesystem skeleton for 48 modules across 8 layers.',
    2,
    'action',
    'execute',
    '{"directories_created":54,"layout":"src-module-matching","includes":["ai_docs","ai_specs","config","data","migrations","tests","benches",".claude"],"layers":8,"modules":48}',
    120,
    0,
    0,
    '["step-scaffold-p1-recon"]'
);

-- Phase 3: Gold Standard Cloning
INSERT INTO workflow_steps (
    step_id, workflow_id, step_name, description, step_order,
    step_type, action_type, action_config,
    timeout_seconds, retry_count, continue_on_failure, depends_on
) VALUES (
    'step-scaffold-p3-gold',
    'wf-mev2-oneshot-scaffold',
    'Phase 3: Gold Standard Cloning',
    'M1 Foundation (11 files, 16,711 LOC) and M2 Services (5 files, 7,196 LOC) cloned as exemplars. These production-grade modules serve as templates for all remaining 46 modules, establishing coding style, test density, documentation patterns, and architectural conventions.',
    3,
    'action',
    'execute',
    '{"m1_files":11,"m1_loc":16711,"m2_files":5,"m2_loc":7196,"total_loc":23907,"purpose":"exemplar-templates","quality":"production-grade","standards":["zero-clippy-warnings","pedantic","50-tests-per-module","no-unwrap","no-unsafe","doc-comments"]}',
    180,
    0,
    0,
    '["step-scaffold-p2-dirs"]'
);

-- Phase 4: Database Cloning
INSERT INTO workflow_steps (
    step_id, workflow_id, step_name, description, step_order,
    step_type, action_type, action_config,
    timeout_seconds, retry_count, continue_on_failure, depends_on
) VALUES (
    'step-scaffold-p4-databases',
    'wf-mev2-oneshot-scaffold',
    'Phase 4: Database Cloning',
    '12 databases (5.9MB) cloned from ME v1 with full schema preservation. Enables immediate query validation and state verification without rebuilding from scratch.',
    4,
    'action',
    'execute',
    '{"databases_cloned":12,"total_size_mb":5.9,"schema_preserved":true,"source":"me-v1","purpose":"immediate-query-validation"}',
    120,
    0,
    0,
    '["step-scaffold-p2-dirs"]'
);

-- Phase 5: Asset Cloning
INSERT INTO workflow_steps (
    step_id, workflow_id, step_name, description, step_order,
    step_type, action_type, action_config,
    timeout_seconds, retry_count, continue_on_failure, depends_on
) VALUES (
    'step-scaffold-p5-assets',
    'wf-mev2-oneshot-scaffold',
    'Phase 5: Asset Cloning',
    '155+ supporting files cloned: migrations, configs, benchmarks, tests, NAM documentation, AI specs, module documentation, layer documentation, pattern specifications. Provides complete operational context for all 48 modules.',
    5,
    'action',
    'execute',
    '{"files_cloned":155,"categories":["migrations","configs","benchmarks","tests","nam-docs","ai-specs","module-docs","layer-docs","pattern-specs"],"purpose":"operational-context"}',
    180,
    0,
    0,
    '["step-scaffold-p3-gold","step-scaffold-p4-databases"]'
);

-- Phase 6: Master Plan
INSERT INTO workflow_steps (
    step_id, workflow_id, step_name, description, step_order,
    step_type, action_type, action_config,
    timeout_seconds, retry_count, continue_on_failure, depends_on
) VALUES (
    'step-scaffold-p6-masterplan',
    'wf-mev2-oneshot-scaffold',
    'Phase 6: Master Plan',
    'SCAFFOLDING_MASTER_PLAN.md written with 48-module architecture blueprint, 8 layers (L1-Foundation through L8-Nexus), 12 design constraints (C1-C12). Single source of truth for all coding phases. Includes novel L8 Nexus layer with 6 modules (N01-N06) for OVM integration.',
    6,
    'action',
    'execute',
    '{"output":"SCAFFOLDING_MASTER_PLAN.md","modules":48,"layers":8,"constraints":12,"novel_layer":"L8-Nexus","nexus_modules":["N01","N02","N03","N04","N05","N06"],"purpose":"single-source-of-truth"}',
    300,
    0,
    0,
    '["step-scaffold-p5-assets"]'
);

-- Phase 7: Documentation
INSERT INTO workflow_steps (
    step_id, workflow_id, step_name, description, step_order,
    step_type, action_type, action_config,
    timeout_seconds, retry_count, continue_on_failure, depends_on
) VALUES (
    'step-scaffold-p7-docs',
    'wf-mev2-oneshot-scaffold',
    'Phase 7: Documentation',
    'CLAUDE.md, CLAUDE.local.md, architectural schematics, per-layer spec sheets generated. Establishes the complete developer documentation infrastructure for ongoing ME v2 development.',
    7,
    'action',
    'execute',
    '{"outputs":["CLAUDE.md","CLAUDE.local.md","architectural-schematics","per-layer-spec-sheets"],"purpose":"developer-documentation-infrastructure"}',
    300,
    0,
    0,
    '["step-scaffold-p6-masterplan"]'
);

-- ============================================================
-- 3. WORKFLOW INSTANCE (Completed execution)
-- ============================================================
INSERT INTO workflow_instances (
    instance_id, workflow_id,
    trigger_source, triggered_by,
    input_parameters,
    output_results,
    context_data,
    status,
    current_step_id, current_step_order, total_steps,
    completed_steps, failed_steps, skipped_steps,
    started_at, completed_at, duration_ms,
    approval_status
) VALUES (
    'inst-mev2-scaffold-20260306',
    'wf-mev2-oneshot-scaffold',
    'manual',
    'claude-opus-4-6',
    '{"source":"the_maintenance_engine","target":"the_maintenance_engine_v2","strategy":"one-shot-scaffold","gold_standards":["M1","M2"]}',
    '{"total_files":209,"total_directories":55,"cloned_loc":23907,"cloned_databases":12,"database_size_mb":5.9,"supporting_assets":155,"recon_agents":6,"master_plan":"SCAFFOLDING_MASTER_PLAN.md","target_loc":"65000+","target_tests":"2400+","target_databases":12,"target_benchmarks":"10+"}',
    '{"architecture":{"layers":8,"modules":48,"novel":"L8-Nexus-6-modules"},"constraints":"C1-C12","evolution_chamber":{"r_delta_threshold":0.05,"morphogenic_adaptation":true},"quality_gate":"cargo-check->clippy->pedantic->test"}',
    'completed',
    'step-scaffold-p7-docs',
    7,
    7,
    7, 0, 0,
    '2026-03-06T10:00:00',
    '2026-03-06T10:45:00',
    2700000,
    'not_required'
);

-- ============================================================
-- 4. STEP EXECUTIONS (7 phases, all completed)
-- ============================================================

INSERT INTO step_executions (
    execution_id, instance_id, step_id, attempt_number,
    status, input_data, output_data,
    started_at, completed_at, duration_ms,
    exit_code, success, assigned_agent, executed_by, log_output
) VALUES (
    'exec-p1-recon-001',
    'inst-mev2-scaffold-20260306',
    'step-scaffold-p1-recon',
    1,
    'completed',
    '{"agents":6,"targets":["me-v1-arch","db-forensics","m1-m2-patterns","module-specs","nexus-ovm","devops-v2"]}',
    '{"agents_completed":6,"agents_failed":0,"intelligence_gathered":true,"me_v1_modules":42,"databases_found":12,"patterns_extracted":"M1+M2 gold standard identified","nexus_patterns":"OVM integration via 12D tensor","devops_patterns":"Hebbian STDP + pipeline CRUD"}',
    '2026-03-06T10:00:00',
    '2026-03-06T10:03:00',
    180000,
    0, 1,
    'fleet-recon-all',
    'claude-opus-4-6',
    '6 parallel fleet agents completed in ~3 minutes. All returned comprehensive intelligence. ME v1: 42 modules across 7 layers. 12 databases identified (5.9MB). M1+M2 identified as gold standard exemplars.'
);

INSERT INTO step_executions (
    execution_id, instance_id, step_id, attempt_number,
    status, input_data, output_data,
    started_at, completed_at, duration_ms,
    exit_code, success, assigned_agent, executed_by, log_output
) VALUES (
    'exec-p2-dirs-001',
    'inst-mev2-scaffold-20260306',
    'step-scaffold-p2-dirs',
    1,
    'completed',
    '{"target_dirs":54}',
    '{"directories_created":54,"structure":"src-module-matching","includes_infra":true}',
    '2026-03-06T10:03:00',
    '2026-03-06T10:05:00',
    120000,
    0, 1,
    'claude-opus-4-6',
    'claude-opus-4-6',
    '54 directories created. Layout matches src/ module structure with ai_docs, ai_specs, config, data, migrations, tests, benches, .claude infrastructure.'
);

INSERT INTO step_executions (
    execution_id, instance_id, step_id, attempt_number,
    status, input_data, output_data,
    started_at, completed_at, duration_ms,
    exit_code, success, assigned_agent, executed_by, log_output
) VALUES (
    'exec-p3-gold-001',
    'inst-mev2-scaffold-20260306',
    'step-scaffold-p3-gold',
    1,
    'completed',
    '{"source_modules":["M1","M2"],"source_path":"the_maintenance_engine"}',
    '{"m1_files":11,"m1_loc":16711,"m2_files":5,"m2_loc":7196,"total_loc":23907,"quality":"production-grade"}',
    '2026-03-06T10:05:00',
    '2026-03-06T10:12:00',
    420000,
    0, 1,
    'claude-opus-4-6',
    'claude-opus-4-6',
    'M1 Foundation: 11 files, 16,711 LOC. M2 Services: 5 files, 7,196 LOC. Total: 23,907 LOC of verified production-grade exemplar code cloned.'
);

INSERT INTO step_executions (
    execution_id, instance_id, step_id, attempt_number,
    status, input_data, output_data,
    started_at, completed_at, duration_ms,
    exit_code, success, assigned_agent, executed_by, log_output
) VALUES (
    'exec-p4-db-001',
    'inst-mev2-scaffold-20260306',
    'step-scaffold-p4-databases',
    1,
    'completed',
    '{"source":"the_maintenance_engine/data/databases","target":"the_maintenance_engine_v2/data/databases"}',
    '{"databases_cloned":12,"total_size_mb":5.9,"schema_preserved":true,"integrity_check":"all_ok"}',
    '2026-03-06T10:05:00',
    '2026-03-06T10:08:00',
    180000,
    0, 1,
    'claude-opus-4-6',
    'claude-opus-4-6',
    '12 databases cloned (5.9MB). Full schema preservation confirmed. PRAGMA integrity_check passed on all databases.'
);

INSERT INTO step_executions (
    execution_id, instance_id, step_id, attempt_number,
    status, input_data, output_data,
    started_at, completed_at, duration_ms,
    exit_code, success, assigned_agent, executed_by, log_output
) VALUES (
    'exec-p5-assets-001',
    'inst-mev2-scaffold-20260306',
    'step-scaffold-p5-assets',
    1,
    'completed',
    '{"categories":["migrations","configs","benchmarks","tests","nam-docs","ai-specs","module-docs","layer-docs","pattern-specs"]}',
    '{"files_cloned":155,"categories_covered":9}',
    '2026-03-06T10:12:00',
    '2026-03-06T10:20:00',
    480000,
    0, 1,
    'claude-opus-4-6',
    'claude-opus-4-6',
    '155+ supporting files cloned across 9 categories: migrations, configs, benchmarks, tests, NAM docs, AI specs, module docs, layer docs, pattern specs.'
);

INSERT INTO step_executions (
    execution_id, instance_id, step_id, attempt_number,
    status, input_data, output_data,
    started_at, completed_at, duration_ms,
    exit_code, success, assigned_agent, executed_by, log_output
) VALUES (
    'exec-p6-masterplan-001',
    'inst-mev2-scaffold-20260306',
    'step-scaffold-p6-masterplan',
    1,
    'completed',
    '{"target":"SCAFFOLDING_MASTER_PLAN.md"}',
    '{"output":"SCAFFOLDING_MASTER_PLAN.md","modules":48,"layers":8,"constraints":12,"novel_layer":"L8-Nexus","nexus_modules":6}',
    '2026-03-06T10:20:00',
    '2026-03-06T10:35:00',
    900000,
    0, 1,
    'claude-opus-4-6',
    'claude-opus-4-6',
    'SCAFFOLDING_MASTER_PLAN.md generated: 48-module architecture, 8 layers (L1-L8), 12 design constraints (C1-C12). Novel L8 Nexus layer with N01-N06 for OVM integration. Single source of truth for all coding phases.'
);

INSERT INTO step_executions (
    execution_id, instance_id, step_id, attempt_number,
    status, input_data, output_data,
    started_at, completed_at, duration_ms,
    exit_code, success, assigned_agent, executed_by, log_output
) VALUES (
    'exec-p7-docs-001',
    'inst-mev2-scaffold-20260306',
    'step-scaffold-p7-docs',
    1,
    'completed',
    '{"targets":["CLAUDE.md","CLAUDE.local.md","schematics","spec-sheets"]}',
    '{"files_generated":4,"types":["claude-md","claude-local-md","architectural-schematics","per-layer-spec-sheets"]}',
    '2026-03-06T10:35:00',
    '2026-03-06T10:45:00',
    600000,
    0, 1,
    'claude-opus-4-6',
    'claude-opus-4-6',
    'CLAUDE.md + CLAUDE.local.md + architectural schematics + per-layer spec sheets generated. Complete developer documentation infrastructure established.'
);

-- ============================================================
-- 5. WORKFLOW VARIABLES (Key metrics and learnings)
-- ============================================================

-- Instance-scoped metrics
INSERT INTO workflow_variables (variable_id, scope, scope_id, variable_name, variable_type, variable_value, description) VALUES
    ('var-scaffold-total-files', 'instance', 'inst-mev2-scaffold-20260306', 'total_files_created', 'number', '209', 'Total files created during scaffolding');
INSERT INTO workflow_variables (variable_id, scope, scope_id, variable_name, variable_type, variable_value, description) VALUES
    ('var-scaffold-total-dirs', 'instance', 'inst-mev2-scaffold-20260306', 'total_directories', 'number', '55', 'Total directories created');
INSERT INTO workflow_variables (variable_id, scope, scope_id, variable_name, variable_type, variable_value, description) VALUES
    ('var-scaffold-cloned-loc', 'instance', 'inst-mev2-scaffold-20260306', 'cloned_loc', 'number', '23907', 'Lines of code cloned from gold standard modules (M1: 16711 + M2: 7196)');
INSERT INTO workflow_variables (variable_id, scope, scope_id, variable_name, variable_type, variable_value, description) VALUES
    ('var-scaffold-cloned-dbs', 'instance', 'inst-mev2-scaffold-20260306', 'cloned_databases', 'number', '12', 'Number of databases cloned from ME v1 (5.9MB total)');
INSERT INTO workflow_variables (variable_id, scope, scope_id, variable_name, variable_type, variable_value, description) VALUES
    ('var-scaffold-assets', 'instance', 'inst-mev2-scaffold-20260306', 'supporting_assets', 'number', '155', 'Supporting files cloned (migrations, configs, benchmarks, tests, docs, specs)');
INSERT INTO workflow_variables (variable_id, scope, scope_id, variable_name, variable_type, variable_value, description) VALUES
    ('var-scaffold-recon-agents', 'instance', 'inst-mev2-scaffold-20260306', 'reconnaissance_agents', 'number', '6', 'Parallel fleet agents deployed for intelligence gathering');
INSERT INTO workflow_variables (variable_id, scope, scope_id, variable_name, variable_type, variable_value, description) VALUES
    ('var-scaffold-target-loc', 'instance', 'inst-mev2-scaffold-20260306', 'target_loc', 'string', '65000+', 'Target LOC for complete ME v2');
INSERT INTO workflow_variables (variable_id, scope, scope_id, variable_name, variable_type, variable_value, description) VALUES
    ('var-scaffold-target-tests', 'instance', 'inst-mev2-scaffold-20260306', 'target_tests', 'string', '2400+', 'Target test count (50 per module x 48 modules)');
INSERT INTO workflow_variables (variable_id, scope, scope_id, variable_name, variable_type, variable_value, description) VALUES
    ('var-scaffold-target-benchmarks', 'instance', 'inst-mev2-scaffold-20260306', 'target_benchmarks', 'string', '10+', 'Target benchmark count');

-- Workflow-scoped design constraints
INSERT INTO workflow_variables (variable_id, scope, scope_id, variable_name, variable_type, variable_value, description) VALUES
    ('var-scaffold-constraints', 'workflow', 'wf-mev2-oneshot-scaffold', 'design_constraints', 'json',
     '{"C1":"zero-clippy-warnings-pedantic","C2":"no-unwrap-outside-tests","C3":"no-unsafe","C4":"no-chrono-timestamps-cycle-counters-only","C5":"doc-comments-all-public-items","C6":"50-tests-per-module","C7":"meaningful-aligned-tests","C8":"production-grade-every-module","C9":"quality-gate-check-clippy-pedantic-test","C10":"hebbian-stdp-integration","C11":"12d-tensor-encoding","C12":"pbft-consensus-n40-f13-q27"}',
     'Design constraints C1-C12 documented in SCAFFOLDING_MASTER_PLAN.md');

-- Workflow-scoped architecture
INSERT INTO workflow_variables (variable_id, scope, scope_id, variable_name, variable_type, variable_value, description) VALUES
    ('var-scaffold-architecture', 'workflow', 'wf-mev2-oneshot-scaffold', 'architecture', 'json',
     '{"layers":{"L1":"Foundation-M1-M6","L2":"Services-M7-M12","L3":"Core-Logic-M13-M18","L4":"Integration-M19-M24","L5":"Learning-M25-M30","L6":"Consensus-M31-M36","L7":"Orchestration-M37-M42","L8":"Nexus-N01-N06"},"total_modules":48,"novel":"L8-Nexus-adds-6-OVM-integration-modules","evolution_chamber":{"r_delta_threshold":0.05,"morphogenic_adaptation":true}}',
     'Complete 8-layer 48-module architecture with novel L8 Nexus layer');

-- ============================================================
-- 6. WORKFLOW EVENTS (Key learnings)
-- ============================================================

INSERT INTO workflow_events (
    event_id, source_type, source_id, event_type, severity,
    message, details, workflow_id, instance_id, actor, correlation_id
) VALUES (
    'evt-scaffold-learning-001',
    'workflow', 'inst-mev2-scaffold-20260306', 'completed', 'info',
    'LEARNING: 6 parallel reconnaissance agents complete in ~2-3 minutes, providing comprehensive intelligence across architecture, databases, patterns, specs, integration, and engine analysis',
    '{"category":"reconnaissance","agents":6,"duration_minutes":"2-3","targets":["me-v1-analysis","database-forensics","m1-m2-patterns","module-specs","nexus-ovm-integration","devops-v2-patterns"],"outcome":"all-completed-successfully"}',
    'wf-mev2-oneshot-scaffold', 'inst-mev2-scaffold-20260306',
    'claude-opus-4-6', 'corr-mev2-scaffold-learnings'
);

INSERT INTO workflow_events (
    event_id, source_type, source_id, event_type, severity,
    message, details, workflow_id, instance_id, actor, correlation_id
) VALUES (
    'evt-scaffold-learning-002',
    'workflow', 'inst-mev2-scaffold-20260306', 'completed', 'info',
    'LEARNING: Cloning gold standard layers (M1+M2) provides 23,907 LOC of verified, production-grade exemplar code that serves as template for remaining 46 modules',
    '{"category":"gold-standard-cloning","m1_files":11,"m1_loc":16711,"m2_files":5,"m2_loc":7196,"total_loc":23907,"benefit":"establishes coding style, test density, documentation patterns, architectural conventions"}',
    'wf-mev2-oneshot-scaffold', 'inst-mev2-scaffold-20260306',
    'claude-opus-4-6', 'corr-mev2-scaffold-learnings'
);

INSERT INTO workflow_events (
    event_id, source_type, source_id, event_type, severity,
    message, details, workflow_id, instance_id, actor, correlation_id
) VALUES (
    'evt-scaffold-learning-003',
    'workflow', 'inst-mev2-scaffold-20260306', 'completed', 'info',
    'LEARNING: Database cloning preserves schema + data, enabling immediate query validation without rebuilding from scratch (12 DBs, 5.9MB)',
    '{"category":"database-cloning","databases":12,"size_mb":5.9,"schema_preserved":true,"benefit":"immediate-query-validation-and-state-verification"}',
    'wf-mev2-oneshot-scaffold', 'inst-mev2-scaffold-20260306',
    'claude-opus-4-6', 'corr-mev2-scaffold-learnings'
);

INSERT INTO workflow_events (
    event_id, source_type, source_id, event_type, severity,
    message, details, workflow_id, instance_id, actor, correlation_id
) VALUES (
    'evt-scaffold-learning-004',
    'workflow', 'inst-mev2-scaffold-20260306', 'completed', 'info',
    'LEARNING: SCAFFOLDING_MASTER_PLAN.md serves as single source of truth for all coding phases — must be written before coding begins',
    '{"category":"master-plan","output":"SCAFFOLDING_MASTER_PLAN.md","modules":48,"layers":8,"constraints":12,"principle":"single-source-of-truth-before-coding"}',
    'wf-mev2-oneshot-scaffold', 'inst-mev2-scaffold-20260306',
    'claude-opus-4-6', 'corr-mev2-scaffold-learnings'
);

INSERT INTO workflow_events (
    event_id, source_type, source_id, event_type, severity,
    message, details, workflow_id, instance_id, actor, correlation_id
) VALUES (
    'evt-scaffold-learning-005',
    'workflow', 'inst-mev2-scaffold-20260306', 'completed', 'info',
    'LEARNING: Design constraints C1-C12 must be documented before coding begins — prevents drift and ensures every module meets production quality gate',
    '{"category":"design-constraints","constraints_count":12,"key_constraints":["zero-clippy-pedantic","no-unwrap","no-unsafe","50-tests-per-module","doc-comments-all-public"],"principle":"constraints-before-code"}',
    'wf-mev2-oneshot-scaffold', 'inst-mev2-scaffold-20260306',
    'claude-opus-4-6', 'corr-mev2-scaffold-learnings'
);

INSERT INTO workflow_events (
    event_id, source_type, source_id, event_type, severity,
    message, details, workflow_id, instance_id, actor, correlation_id
) VALUES (
    'evt-scaffold-learning-006',
    'workflow', 'inst-mev2-scaffold-20260306', 'completed', 'info',
    'LEARNING: New L8 Nexus layer adds 6 novel modules (N01-N06) for OVM integration — extends ME v1 7-layer architecture to 8 layers',
    '{"category":"architecture-extension","new_layer":"L8-Nexus","modules":["N01","N02","N03","N04","N05","N06"],"purpose":"OVM-integration","extends":"ME-v1-7-layer-to-8-layer"}',
    'wf-mev2-oneshot-scaffold', 'inst-mev2-scaffold-20260306',
    'claude-opus-4-6', 'corr-mev2-scaffold-learnings'
);

INSERT INTO workflow_events (
    event_id, source_type, source_id, event_type, severity,
    message, details, workflow_id, instance_id, actor, correlation_id
) VALUES (
    'evt-scaffold-learning-007',
    'workflow', 'inst-mev2-scaffold-20260306', 'completed', 'info',
    'LEARNING: Evolution Chamber integration requires morphogenic adaptation with |r_delta| > 0.05 threshold for meaningful field perturbation',
    '{"category":"evolution-chamber","r_delta_threshold":0.05,"morphogenic_adaptation":true,"principle":"field-perturbation-must-be-meaningful"}',
    'wf-mev2-oneshot-scaffold', 'inst-mev2-scaffold-20260306',
    'claude-opus-4-6', 'corr-mev2-scaffold-learnings'
);

INSERT INTO workflow_events (
    event_id, source_type, source_id, event_type, severity,
    message, details, workflow_id, instance_id, actor, correlation_id
) VALUES (
    'evt-scaffold-learning-008',
    'workflow', 'inst-mev2-scaffold-20260306', 'completed', 'info',
    'LEARNING: Total ME v2 target is 65K+ LOC, 2,400+ tests, 12 databases, 10+ benchmarks — one-shot scaffolding provides 37% of LOC foundation from gold standards alone',
    '{"category":"scale-metrics","total_files":209,"total_directories":55,"cloned_loc":23907,"target_loc":"65000+","loc_foundation_pct":37,"target_tests":"2400+","target_databases":12,"target_benchmarks":"10+"}',
    'wf-mev2-oneshot-scaffold', 'inst-mev2-scaffold-20260306',
    'claude-opus-4-6', 'corr-mev2-scaffold-learnings'
);

-- ============================================================
-- 7. WORKFLOW TEMPLATE (Reusable for future scaffolding)
-- ============================================================
INSERT INTO workflow_templates (
    template_id, template_name, description, category,
    workflow_definition, parameter_schema, default_parameters,
    is_public, usage_count, version, created_by, tags
) VALUES (
    'tmpl-oneshot-scaffold',
    'One-Shot Project Scaffolding',
    'Template for scaffolding a new ULTRAPLATE service from an existing gold standard. 7 phases: Recon, Dirs, Gold Clone, DB Clone, Assets, Master Plan, Docs. Requires ME v1-style project with gold standard modules.',
    'migration',
    '{"phases":7,"phase_names":["Reconnaissance","Directory Structure","Gold Standard Cloning","Database Cloning","Asset Cloning","Master Plan","Documentation"],"execution_mode":"dag","recon_agents":6,"quality_gate":"check-clippy-pedantic-test"}',
    '{"type":"object","required":["source_project","target_project","gold_modules"],"properties":{"source_project":{"type":"string"},"target_project":{"type":"string"},"gold_modules":{"type":"array","items":{"type":"string"}},"recon_agents":{"type":"integer","default":6}}}',
    '{"recon_agents":6,"quality_gate":"mandatory","constraints":"C1-C12"}',
    1,
    1,
    '1.0.0',
    'claude-opus-4-6',
    '["scaffolding","one-shot","migration","template","gold-standard"]'
);

COMMIT;
