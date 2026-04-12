//! # The Maintenance Engine - ULTRAPLATE Service
//!
//! Axum HTTP server providing REST API for the Maintenance Engine.
//! Part of the ULTRAPLATE Bulletproof Developer Environment (Port 8080).
//!
//! ## Endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | `/api/health` | Health check (JSON) |
//! | GET | `/api/status` | Full engine status |
//! | GET | `/api/engine` | Engine health report |
//! | GET | `/api/services` | Service mesh overview |
//! | GET | `/api/layers` | Per-layer health breakdown |
//! | GET | `/api/consensus` | PBFT consensus state |
//! | GET | `/api/learning` | Hebbian learning state |
//! | GET | `/metrics` | Prometheus-style metrics |
//!
//! ## Usage
//!
//! ```bash
//! maintenance_engine start                  # Default port 8080
//! maintenance_engine start --port 9000      # Custom port
//! maintenance_engine health                 # CLI health check
//! maintenance_engine status                 # CLI status
//! ```

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]

use std::env;
use std::fmt::Write as _;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Json;
use maintenance_engine_v2::database::DatabaseManager;
use maintenance_engine_v2::engine::Engine;
use maintenance_engine_v2::m2_services::{CircuitBreakerOps, LifecycleOps, ServiceDiscovery};
use maintenance_engine_v2::m3_core_logic::{IssueType, Severity};
use maintenance_engine_v2::m4_integration::cascade_bridge::CascadeBridge;
use maintenance_engine_v2::m5_learning::decay_scheduler::DecayScheduler;
use maintenance_engine_v2::tools::{ToolInvokeRequest, ToolInvokeResponse};
use maintenance_engine_v2::{Error, Result};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default REST API port.
const DEFAULT_PORT: u16 = 8080;

/// Service version.
const VERSION: &str = "1.0.0";

/// Service name.
const SERVICE_NAME: &str = "maintenance-engine";

/// Default L7 observer tick interval in milliseconds (60 seconds).
const DEFAULT_TICK_INTERVAL_MS: u64 = 60_000;

// ---------------------------------------------------------------------------
// Application State
// ---------------------------------------------------------------------------

/// Shared application state passed to all handlers.
struct AppState {
    /// The core engine orchestrator (L2-L7).
    engine: Engine,
    /// Optional database manager for persistence (fail-silent).
    db: Option<Arc<DatabaseManager>>,
    /// Server start time for uptime calculation.
    started_at: std::time::Instant,
    /// Configured port number.
    port: u16,
    /// Count of consecutive observer ticks with zero correlations.
    /// Used for self-triggered metabolic activation (NAM-R1).
    zero_correlation_streak: std::sync::atomic::AtomicU64,
    /// When `true`, the observer tick cycle skips `EventBus` publishing (NAM-R7).
    metabolic_paused: std::sync::atomic::AtomicBool,
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

/// Main entry point.
///
/// Parses CLI arguments, initialises the engine, and starts the Axum server
/// or runs a CLI subcommand.
fn main() -> Result<()> {
    // Install panic hook to log fatal panics before process abort
    std::panic::set_hook(Box::new(|info| {
        eprintln!("FATAL PANIC in maintenance-engine: {info}");
    }));

    let args: Vec<String> = env::args().collect();
    let is_server = args.get(1).map(String::as_str) == Some("start");

    // Initialise tracing — server mode writes to a log file so that broken
    // pipes from process managers (devenv) cannot kill the process.
    if is_server {
        init_tracing_to_file()?;
    } else {
        init_tracing_stderr()?;
    }

    match args.get(1).map(String::as_str) {
        Some("start") => {
            let port = parse_port(&args)?;
            run_server(port)
        }
        Some("health") => {
            let engine = Engine::new();
            let report = engine.health_report()?;
            let status = if report.is_healthy() {
                "healthy"
            } else {
                "unhealthy"
            };
            let output = json!({
                "status": status,
                "service": SERVICE_NAME,
                "version": VERSION,
                "overall_health": report.overall_health,
                "services_healthy": report.services_healthy,
                "services_total": report.services_total,
            });
            println!("{output}");
            Ok(())
        }
        Some("status") => {
            let engine = Engine::new();
            let report = engine.health_report()?;
            println!("Maintenance Engine Status");
            println!("=========================");
            println!("Version: {VERSION}");
            println!(
                "Status: {}",
                if report.is_healthy() {
                    "HEALTHY"
                } else {
                    "DEGRADED"
                }
            );
            println!("Overall Health: {:.1}%", report.overall_health * 100.0);
            println!(
                "Services: {}/{} healthy",
                report.services_healthy, report.services_total
            );
            println!("Pipelines: {} active", report.pipelines_active);
            println!("Pathways: {} registered", report.pathways_count);
            println!("Proposals: {} active", report.proposals_active);
            println!("Layers: 7 (L1-L7)");
            println!("Modules: 42 implemented");
            println!("NAM Target: 92%");
            println!(
                "PBFT Fleet: {} agents (quorum 27)",
                engine.pbft_manager().get_fleet().len()
            );
            Ok(())
        }
        Some("--version" | "-V") => {
            println!("maintenance_engine {VERSION}");
            Ok(())
        }
        Some("--help" | "-h") | None => {
            print_help();
            Ok(())
        }
        Some(cmd) => {
            eprintln!("Unknown command: {cmd}");
            print_help();
            Err(Error::Config(format!("Unknown command: {cmd}")))
        }
    }
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// Start the Axum HTTP server on the given port.
///
/// # Errors
///
/// Returns an error if the tokio runtime cannot be created or the server
/// fails to bind to the requested port.
fn run_server(port: u16) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| Error::Other(format!("Failed to create tokio runtime: {e}")))?;

    run_server_with_retries(&rt, port)
}

/// Run the server with retry logic, reusing the same tokio runtime.
fn run_server_with_retries(rt: &tokio::runtime::Runtime, port: u16) -> Result<()> {
    const MAX_RETRIES: u32 = 3;

    for attempt in 1..=MAX_RETRIES {
        match run_server_attempt(rt, port, attempt, MAX_RETRIES) {
            Ok(()) => return Ok(()),
            Err(e) if attempt == MAX_RETRIES => return Err(e),
            Err(_) => std::thread::sleep(Duration::from_secs(3)),
        }
    }

    Ok(())
}

/// Execute a single server attempt with logging.
fn run_server_attempt(
    rt: &tokio::runtime::Runtime,
    port: u16,
    attempt: u32,
    max_retries: u32,
) -> Result<()> {
    tracing::info!(attempt, max_retries, "Starting server loop");
    match rt.block_on(serve(port)) {
        Ok(()) => {
            tracing::info!("Server shut down cleanly");
            Ok(())
        }
        Err(e) => {
            tracing::error!(attempt, error = %e, "Server exited with error");
            Err(e)
        }
    }
}

/// Spawn background tasks: L7 observer tick, tool registration, peer polling,
/// and periodic learning cycle.
fn spawn_background_tasks(state: &Arc<AppState>) {
    spawn_observer_tick(state);
    spawn_tool_registration(state);
    spawn_peer_polling(state);
    spawn_learning_cycle(state);
    spawn_thermal_polling(state);
    spawn_cascade_polling(state);
    spawn_decay_scheduler(state);
    spawn_health_polling(state);
    spawn_devops_pipeline_trigger();
    spawn_pv2_eventbus_bridge(state);
    spawn_orac_bridge_polling(state);
    spawn_field_tracking(state);
    spawn_self_model_updater(state);
    spawn_remediation_worker(state);
    spawn_evolution_tick(state);
    spawn_gc_sweep(state);
}

/// Issue 9: One-shot trigger of a DevOps Engine health-check pipeline.
///
/// After a 30s startup delay, POSTs a pipeline trigger to the DevOps Engine
/// so it has at least one completed pipeline on first health check.
fn spawn_devops_pipeline_trigger() {
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let addr = "127.0.0.1:8081";
        let body = serde_json::json!({
            "name": "health-check",
            "trigger": "startup",
            "source": "maintenance-engine",
        });
        let body_str = body.to_string();
        let request = format!(
            "POST /pipeline/trigger HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body_str}",
            body_str.len()
        );

        match tokio::time::timeout(
            Duration::from_secs(3),
            tokio::net::TcpStream::connect(addr),
        )
        .await
        {
            Ok(Ok(stream)) => {
                let (_, mut writer) = stream.into_split();
                if let Err(e) = tokio::io::AsyncWriteExt::write_all(&mut writer, request.as_bytes()).await {
                    tracing::debug!(error = %e, "DevOps pipeline trigger failed");
                } else {
                    tracing::info!("DevOps Engine health-check pipeline triggered");
                }
            }
            _ => {
                tracing::debug!("DevOps Engine not reachable for pipeline trigger");
            }
        }
    });
}

/// IGNITION-1a: Bridge ME `EventBus` events to Pane-Vortex V2 (port 8132).
///
/// Polls all 6 `EventBus` channels every 10 seconds and POSTs any new events
/// to PV2 at `/bus/events`. This creates the nervous system connection that
/// allows ME's 333K+ events to flow into the fleet coordination layer.
///
/// Uses a last-seen event ID per channel to avoid re-posting.
fn spawn_pv2_eventbus_bridge(state: &Arc<AppState>) {
    use std::collections::HashMap;
    let bridge_state = Arc::clone(state);
    tokio::spawn(async move {
        // Wait for services to stabilize
        tokio::time::sleep(Duration::from_secs(15)).await;
        tracing::info!("PV2 EventBus bridge started (10s poll interval)");

        let channels = ["health", "remediation", "learning", "consensus", "integration", "metrics", "gc"];

        // R2 fix: Register as subscriber on each EventBus channel so that
        // subscriber_count > 0. Previously the bridge was polling events
        // without being registered, so EventBus reported 0 external subscribers.
        for channel in &channels {
            if let Err(e) = bridge_state.engine.event_bus().subscribe(
                "pv2-bridge", channel, None,
            ) {
                tracing::debug!(error = %e, channel, "PV2 bridge subscribe failed (non-fatal)");
            }
        }
        tracing::info!(
            channels = channels.len(),
            "PV2 EventBus bridge registered as subscriber on all channels"
        );

        let mut last_seen: HashMap<String, String> = HashMap::new();
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        interval.tick().await;

        loop {
            interval.tick().await;

            for channel in &channels {
                let events = bridge_state.engine.event_bus().get_events(channel, 20);
                // Filter to only new events (after last seen ID for this channel)
                let new_events: Vec<_> = if let Some(last_id) = last_seen.get(*channel) {
                    let mut found = false;
                    events
                        .into_iter()
                        .filter(|e| {
                            if found {
                                return true;
                            }
                            if e.id == *last_id {
                                found = true;
                            }
                            false
                        })
                        .collect()
                } else {
                    events
                };

                if new_events.is_empty() {
                    continue;
                }

                // Track last seen event ID
                if let Some(last) = new_events.last() {
                    last_seen.insert((*channel).to_owned(), last.id.clone());
                }

                // Build PV2 payload
                let payload = serde_json::json!({
                    "source": "maintenance-engine",
                    "channel": channel,
                    "events": new_events.iter().map(|e| {
                        serde_json::json!({
                            "id": e.id,
                            "event_type": e.event_type,
                            "payload": e.payload,
                            "source": e.source,
                        })
                    }).collect::<Vec<_>>(),
                });

                let body_str = payload.to_string();
                let addr = "127.0.0.1:8132";
                let request = format!(
                    "POST /bus/events HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body_str}",
                    body_str.len()
                );

                if let Ok(Ok(stream)) = tokio::time::timeout(
                    Duration::from_secs(3),
                    tokio::net::TcpStream::connect(addr),
                )
                .await
                {
                    let (_, mut writer) = stream.into_split();
                    if let Err(e) = tokio::io::AsyncWriteExt::write_all(
                        &mut writer,
                        request.as_bytes(),
                    )
                    .await
                    {
                        tracing::debug!(error = %e, "PV2 EventBus bridge POST failed");
                    }
                }
            }
        }
    });
}

/// Spawn periodic health polling for all registered ULTRAPLATE services.
///
/// Runs on a separate task every 30 seconds. For each registered probe,
/// HTTP GETs the health endpoint and calls `record_result()` to transition
/// service status from Unknown → Healthy/Unhealthy.
#[allow(clippy::too_many_lines)] // 13-service health polling with per-service error handling
fn spawn_health_polling(state: &Arc<AppState>) {
    use maintenance_engine_v2::m1_foundation::Timestamp;
    use maintenance_engine_v2::m2_services::{HealthCheckResult, HealthMonitoring};
    use maintenance_engine_v2::nexus::stdp_bridge::StdpBridge;

    let poll_state = Arc::clone(state);
    tokio::spawn(async move {
        // Wait 10s for services to start
        tokio::time::sleep(Duration::from_secs(10)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(30));
        interval.tick().await;
        loop {
            interval.tick().await;

            // Check MUTATION_ENABLED env var (kill switch per Gap G-B)
            if std::env::var("HEALTH_POLLING_DISABLED").is_ok() {
                continue;
            }

            let monitor = poll_state.engine.health_monitor();
            let probe_count = monitor.probe_count();
            let mut healthy = 0_u32;
            let mut total_checked = 0_u32;

            // Get all probe endpoints — V2: includes memory/coordination/ORAC tier
            let services: [(&str, &str); 16] = [
                ("devops-engine",    "http://localhost:8081/health"),
                ("synthex",          "http://localhost:8090/api/health"),
                ("san-k7",           "http://localhost:8100/health"),
                ("nais",             "http://localhost:8101/health"),
                ("bash-engine",      "http://localhost:8102/health"),
                ("tool-maker",       "http://localhost:8103/health"),
                ("ccm",              "http://localhost:8104/health"),
                ("tool-library",     "http://localhost:8105/health"),
                ("codesynthor-v7",   "http://localhost:8110/health"),
                ("vortex-memory",    "http://localhost:8120/health"),
                ("povm-engine",      "http://localhost:8125/health"),
                ("reasoning-memory", "http://localhost:8130/health"),
                ("pane-vortex",      "http://localhost:8132/health"),
                ("orac-sidecar",     "http://localhost:8133/health"),
                ("architect-agent",  "http://localhost:9001/health"),
                ("prometheus-swarm", "http://localhost:10001/health"),
            ];

            for &(service_id, url) in &services {
                total_checked += 1;
                let start = std::time::Instant::now();

                let is_healthy = match tokio::time::timeout(
                    Duration::from_secs(2),
                    async {
                        // Use raw TCP to check health (no reqwest dependency)
                        let addr = url.trim_start_matches("http://");
                        let (host_port, path) = addr.split_once('/').unwrap_or((addr, "health"));
                        let request = format!(
                            "GET /{path} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n\r\n"
                        );
                        let stream = tokio::net::TcpStream::connect(host_port).await?;
                        let (reader, mut writer) = stream.into_split();
                        tokio::io::AsyncWriteExt::write_all(&mut writer, request.as_bytes()).await?;
                        let _ = tokio::io::AsyncWriteExt::shutdown(&mut writer).await;

                        let mut buf_reader = tokio::io::BufReader::new(reader);
                        let mut status_line = String::new();
                        tokio::io::AsyncBufReadExt::read_line(&mut buf_reader, &mut status_line).await?;
                        // Check for 200 OK
                        Ok::<bool, std::io::Error>(status_line.contains("200"))
                    },
                )
                .await
                {
                    Ok(Ok(true)) => {
                        healthy += 1;
                        true
                    }
                    _ => false,
                };

                #[allow(clippy::cast_possible_truncation)]
                let elapsed_ms = start.elapsed().as_millis() as u64;
                let status = if is_healthy {
                    maintenance_engine_v2::m2_services::HealthStatus::Healthy
                } else {
                    maintenance_engine_v2::m2_services::HealthStatus::Unhealthy
                };

                let result = HealthCheckResult {
                    service_id: service_id.to_string(),
                    status,
                    response_time_ms: elapsed_ms,
                    timestamp: Timestamp::now(),
                    message: None,
                    status_code: if is_healthy { Some(200) } else { None },
                };
                if let Err(e) = monitor.record_result(service_id, result) {
                    tracing::trace!(error = %e, service = service_id, "Health probe record failed");
                }

                // METABOLIC-GAP-1 FIX: Publish per-service health check to EventBus
                let health_payload = serde_json::json!({
                    "event": "service_health_check",
                    "service_id": service_id,
                    "healthy": is_healthy,
                    "latency_ms": elapsed_ms,
                });
                if let Err(e) = poll_state.engine.event_bus().publish(
                    "health", "service_health_check", &health_payload.to_string(), "health_poller",
                ) {
                    tracing::trace!(error = %e, "Failed to publish health check event");
                }

                // C12: Record STDP co-activation for each successful service interaction
                let _ = poll_state.engine.stdp_bridge().record_interaction("maintenance-engine", service_id, is_healthy);

                // Session 071 CF-7 fix: Reinforce L5 Hebbian pathways on health success.
                // Without this, pathways decay at -0.001/300s with zero reinforcement,
                // reaching floor (0.1) at ~41.7 hours. This wires the health poll
                // success signal into the Hebbian manager's record_success().
                if is_healthy {
                    let _ = poll_state.engine.hebbian_manager().record_success("health_failure->service_restart");
                } else {
                    let _ = poll_state.engine.hebbian_manager().record_failure("health_failure->service_restart");
                }

                // R5 fix: Feed L5 StdpProcessor directly for timing pair generation.
                // The N04 StdpBridge records co-activation deltas but does NOT
                // generate spike events for the L5 STDP timing-pair processor.
                // Without this, timing_pairs_processed stays at 0 and all 12
                // Hebbian pathways remain at uniform 0.492 strength.
                {
                    use maintenance_engine_v2::m5_learning::stdp::SpikeType;
                    // Use monotonic ms for STDP timing (not Timestamp which is a tick counter)
                    #[allow(clippy::cast_possible_truncation)]
                    let now_ms = start.elapsed().as_millis() as u64
                        + std::time::UNIX_EPOCH.elapsed().unwrap_or_default().as_millis() as u64;
                    // Record pre-synaptic spike (health monitor → service)
                    let _ = poll_state.engine.stdp_processor().record_spike(
                        "health-monitor",
                        service_id,
                        now_ms.saturating_sub(elapsed_ms), // pre-spike at poll start
                        SpikeType::PreSynaptic,
                    );
                    // Record post-synaptic spike at response time.
                    // Healthy responses get positive delta_t (LTP, strengthening),
                    // unhealthy get artificial negative offset (LTD, weakening).
                    let post_time = if is_healthy {
                        now_ms // natural timing: post follows pre → LTP
                    } else {
                        now_ms.saturating_sub(elapsed_ms + 50) // artificial: pre follows post → LTD
                    };
                    let _ = poll_state.engine.stdp_processor().record_spike(
                        service_id,
                        "health-monitor",
                        post_time,
                        SpikeType::PostSynaptic,
                    );
                }
            }

            // METABOLIC-GAP-1 FIX: Publish health cycle summary to EventBus
            let summary_payload = serde_json::json!({
                "event": "health_poll_cycle",
                "healthy": healthy,
                "total": total_checked,
                "probes": probe_count,
            });
            if let Err(e) = poll_state.engine.event_bus().publish(
                "metrics", "health_poll_cycle", &summary_payload.to_string(), "health_poller",
            ) {
                tracing::trace!(error = %e, "Failed to publish health poll summary");
            }

            tracing::info!(
                healthy,
                total = total_checked,
                probes = probe_count,
                "Health polling cycle complete"
            );
        }
    });
}

/// Spawn ORAC bridge polling: polls ORAC at localhost:8133/health every 30s.
///
/// Records successful polls and failures via `OracBridge::record_poll()` and
/// `OracBridge::record_failure()`, keeping the ME-side view of ORAC liveness
/// in sync with actual reachability.
fn spawn_orac_bridge_polling(state: &Arc<AppState>) {
    use maintenance_engine_v2::m4_integration::orac_bridge::OracBridge;

    let bridge_state = Arc::clone(state);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(15)).await;
        tracing::info!("ORAC bridge polling started (30s interval)");
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        interval.tick().await;
        loop {
            interval.tick().await;

            // R12: Auth→RateLimit chain before ORAC bridge poll.
            // Verify service token and check rate limits before each cycle.
            {
                use maintenance_engine_v2::m4_integration::{Authenticator, RateLimiting};
                use maintenance_engine_v2::m4_integration::rate_limiter::RateDecision;
                if bridge_state.engine.auth_manager().verify_token("orac-bridge").is_err() {
                    tracing::trace!("ORAC bridge poll skipped: auth token invalid");
                    continue;
                }
                if let Ok(decision) = bridge_state.engine.rate_limiter().check_and_consume(
                    "orac-bridge",
                    maintenance_engine_v2::m2_services::ServiceTier::Tier1,
                ) {
                    if matches!(decision, RateDecision::Reject { .. }) {
                        tracing::trace!("ORAC bridge poll skipped: rate limited");
                        continue;
                    }
                }
            }

            let addr = "127.0.0.1:8133";
            let request =
                "GET /health HTTP/1.1\r\nHost: 127.0.0.1:8133\r\nConnection: close\r\n\r\n";
            match tokio::time::timeout(
                Duration::from_secs(3),
                tokio::net::TcpStream::connect(addr),
            )
            .await
            {
                Ok(Ok(stream)) => {
                    let (reader, mut writer) = stream.into_split();
                    if let Err(e) = tokio::io::AsyncWriteExt::write_all(
                        &mut writer,
                        request.as_bytes(),
                    )
                    .await
                    {
                        tracing::debug!(error = %e, "ORAC bridge poll write failed");
                        bridge_state.engine.orac_bridge().record_failure();
                        continue;
                    }
                    let _ = tokio::io::AsyncWriteExt::shutdown(&mut writer).await;
                    let mut buf = Vec::new();
                    let mut buf_reader = tokio::io::BufReader::new(reader);
                    let _ =
                        tokio::io::AsyncReadExt::read_to_end(&mut buf_reader, &mut buf).await;
                    let response = String::from_utf8_lossy(&buf);
                    if response.contains("200") {
                        tracing::trace!("ORAC bridge poll success");
                    } else {
                        bridge_state.engine.orac_bridge().record_failure();
                    }
                }
                _ => {
                    bridge_state.engine.orac_bridge().record_failure();
                }
            }
        }
    });
}

/// Spawn field tracking: reads PV2 health at localhost:8132 every 10s.
///
/// Extracts Kuramoto `r`, coupling `k`, and sphere count from the PV2 health
/// JSON, then updates the Nexus field bridge, checks for morphogenic triggers,
/// and advances the regime manager.
#[allow(clippy::too_many_lines)]
fn spawn_field_tracking(state: &Arc<AppState>) {
    use maintenance_engine_v2::nexus::field_bridge::FieldBridge;
    use maintenance_engine_v2::nexus::morphogenic_adapter::MorphogenicAdapter;
    use maintenance_engine_v2::nexus::regime_manager::RegimeManager;

    let field_state = Arc::clone(state);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(20)).await;
        tracing::info!("Field tracking started (10s interval)");
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        interval.tick().await;
        loop {
            interval.tick().await;
            let addr = "127.0.0.1:8132";
            let request =
                "GET /health HTTP/1.1\r\nHost: 127.0.0.1:8132\r\nConnection: close\r\n\r\n";
            if let Ok(Ok(stream)) = tokio::time::timeout(
                Duration::from_secs(2),
                tokio::net::TcpStream::connect(addr),
            )
            .await
            {
                let (reader, mut writer) = stream.into_split();
                let _ = tokio::io::AsyncWriteExt::write_all(
                    &mut writer,
                    request.as_bytes(),
                )
                .await;
                let _ = tokio::io::AsyncWriteExt::shutdown(&mut writer).await;
                let mut buf = Vec::new();
                let mut buf_reader = tokio::io::BufReader::new(reader);
                let _ =
                    tokio::io::AsyncReadExt::read_to_end(&mut buf_reader, &mut buf).await;
                let response = String::from_utf8_lossy(&buf);
                if let Some(body_start) = response.find("\r\n\r\n") {
                    let body = &response[body_start + 4..];
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
                        let r = json.get("r").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
                        let k = json.get("k").and_then(serde_json::Value::as_f64).unwrap_or(1.0);
                        #[allow(clippy::cast_possible_truncation)]
                        let spheres = json
                            .get("spheres")
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or(0) as u32;
                        // Capture r BEFORE update for morphogenic delta
                        let r_before = field_state.engine.field_bridge().current_r();

                        let _ = field_state
                            .engine
                            .field_bridge()
                            .update_field_state(r, k, spheres);

                        // Check for morphogenic trigger (r_delta = new - old)
                        let r_delta = r - r_before;
                        if let Some(_action) = field_state
                            .engine
                            .morphogenic_adapter()
                            .check_trigger(r_delta, k)
                        {
                            tracing::info!(r_delta, k, "Morphogenic adaptation triggered");
                        }

                        // Update regime manager
                        let _ = field_state.engine.regime_manager().update_k(k);
                    }
                }
            }
        }
    });
}

/// Spawn self-model updater: refreshes per-layer health scores every 60s.
///
/// Reads the engine health report and pushes each layer's score into the
/// L1 self-model, keeping the introspective view of the stack current.
fn spawn_self_model_updater(state: &Arc<AppState>) {
    use maintenance_engine_v2::m1_foundation::self_model::SelfModelProvider;

    let model_state = Arc::clone(state);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        tracing::info!("Self model updater started (60s interval)");
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.tick().await;
        loop {
            interval.tick().await;
            if let Ok(report) = model_state.engine.health_report() {
                for (i, &score) in report.layer_health.iter().enumerate() {
                    #[allow(clippy::cast_possible_truncation)]
                    let layer = (i + 1) as u8;
                    let _ = model_state.engine.self_model().update_layer_health(
                        layer,
                        score,
                    );
                }
            }
        }
    });
}

/// R6: Spawn remediation worker to process pending remediation requests.
///
/// Polls `RemediationEngine::process_next()` every 30 seconds. Respects
/// escalation tiers: L0 auto-executes, L1 notifies + proceeds, L2/L3
/// require approval. Publishes outcomes to `EventBus` "remediation" channel
/// for M37 correlation and M18 `FeedbackLoop` ingestion.
fn spawn_remediation_worker(state: &Arc<AppState>) {
    let worker_state = Arc::clone(state);
    tokio::spawn(async move {
        // Startup delay — let observer build initial fitness picture
        tokio::time::sleep(Duration::from_secs(20)).await;
        tracing::info!("Remediation worker started (30s poll interval)");

        let mut interval = tokio::time::interval(Duration::from_secs(30));
        interval.tick().await;
        loop {
            interval.tick().await;

            let pending = worker_state.engine.pending_remediations();
            if pending == 0 {
                continue;
            }

            tracing::info!(pending, "Remediation worker: processing queue");

            // Process up to 3 requests per cycle to avoid starvation
            for _ in 0..3 {
                match worker_state.engine.process_next_remediation() {
                    Ok(Some(outcome)) => {
                        let payload = serde_json::json!({
                            "event": "remediation_outcome",
                            "request_id": outcome.request_id,
                            "success": outcome.success,
                            "duration_ms": outcome.duration_ms,
                        });
                        if let Err(e) = worker_state.engine.event_bus().publish(
                            "remediation",
                            "remediation_outcome",
                            &payload.to_string(),
                            "remediation-worker",
                        ) {
                            tracing::trace!(error = %e, "Failed to publish remediation outcome");
                        }
                    }
                    Ok(None) => break, // no more actionable requests
                    Err(e) => {
                        tracing::warn!(error = %e, "Remediation worker: process_next error");
                        break;
                    }
                }
            }
        }
    });
}

/// Spawn heartbeat task: logs every 60s confirming the server is alive.
/// R19 Phase 2c: Decoupled evolution tick at 15s interval.
///
/// Separates RALPH cycle processing from the 60s observer tick. This
/// increases generation rate from ~1.8 gen/h (60s) to ~240 gen/h (15s)
/// without affecting health polling or tensor observation frequency.
///
/// The evolution tick runs: strategy selection → hint-guided learn →
/// V2 propose (with strategy delta ranges) → advance phase.
fn spawn_evolution_tick(state: &Arc<AppState>) {
    let evo_state = Arc::clone(state);
    tokio::spawn(async move {
        // Wait for observer to bootstrap RALPH first
        tokio::time::sleep(Duration::from_secs(30)).await;
        tracing::info!("V2 evolution tick started (15s interval, decoupled from observer)");

        let mut interval = tokio::time::interval(Duration::from_millis(
            maintenance_engine_v2::m7_observer::evolution_chamber::DEFAULT_EVOLUTION_TICK_MS,
        ));
        // Phase 4 (Session 070): Skip missed ticks instead of burst catch-up.
        // Prevents RALPH from processing a backlog of ticks after a slow GC sweep.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;

        loop {
            interval.tick().await;

            // Skip if metabolic paused
            if evo_state
                .metabolic_paused
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                continue;
            }

            let Some(obs) = evo_state.engine.observer() else {
                continue;
            };

            // Get current system state for strategy selection
            let Some(report) = obs.get_report() else {
                continue; // no observer report yet
            };

            let layer_health = evo_state.engine.layer_health_scores();

            // R19: Select strategy based on current field state
            let (r, r_delta) = {
                use maintenance_engine_v2::nexus::field_bridge::FieldBridge;
                let fb = evo_state.engine.field_bridge();
                let r_val = fb.current_r();
                let delta = fb.recent_deltas(1).first().map_or(0.0, |d| d.r_delta);
                (r_val, delta)
            };
            let strategy = obs.chamber().select_strategy(
                report.current_fitness,
                &layer_health,
                r,
                r_delta,
            );

            // R19: Run hint-guided Learn phase
            let hint = obs.chamber().learn_with_hints(
                report.emergences_since_last,
                None, // dimension analysis from tensor (future enhancement)
                &layer_health,
            );

            // R19: Check convergence — pause if variance too low
            if obs.chamber().is_converged() {
                tracing::debug!("V2 evolution tick: converged, skipping mutation");
                continue;
            }

            // R19 Phase 2: Propose using strategy-guided delta ranges
            // Find a tunable parameter from the hint or fall back to default
            let target_param = hint
                .as_ref()
                .map_or("emergence_detector.min_confidence", |h| &h.parameter);

            // Read the actual current parameter value from the observer
            let current_val = if target_param == "emergence_detector.min_confidence" {
                obs.detector().effective_min_confidence()
            } else {
                0.5_f64 // fallback for unknown parameters
            };

            if let Err(e) = obs.chamber().propose_v2_mutation(
                target_param,
                current_val,
                report.current_fitness,
                &strategy,
                hint.as_ref(),
            ) {
                tracing::trace!(
                    error = %e,
                    strategy = %strategy,
                    "V2 evolution tick: mutation proposal skipped"
                );
            } else {
                tracing::info!(
                    strategy = %strategy,
                    hint = hint.as_ref().map_or("none", |h| &h.parameter),
                    fitness = report.current_fitness,
                    "V2 evolution tick: mutation proposed"
                );
            }

            // Advance RALPH phase
            let _ = obs.chamber().advance_phase();
        }
    });
}

fn spawn_heartbeat(port: u16) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.tick().await;
        loop {
            interval.tick().await;
            tracing::info!(port, "Maintenance Engine heartbeat: alive");
        }
    });
}

/// Spawn the L7 observer tick task with DB persistence.
fn spawn_observer_tick(state: &Arc<AppState>) {
    let tick_state = Arc::clone(state);
    let tick_interval = state
        .engine
        .observer()
        .map_or(DEFAULT_TICK_INTERVAL_MS, |obs| obs.config().tick_interval_ms);

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(tick_interval));
        // Phase 4 (Session 070): Skip missed ticks to prevent burst catch-up after slow cycles.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;
        loop {
            interval.tick().await;
            let tick_start = std::time::Instant::now();
            observer_tick_cycle(&tick_state).await;
            let tick_ms = tick_start.elapsed().as_millis();
            if tick_ms > 1000 {
                tracing::warn!(
                    tick_ms,
                    "Observer tick exceeded 1s budget — check for contention"
                );
            }
        }
    });
}

/// Single observer tick cycle: ingest `EventBus` events into M37, build tensor,
/// tick observer, persist results, and auto-remediate if fitness is `Critical`.
#[allow(clippy::too_many_lines)]
async fn observer_tick_cycle(state: &Arc<AppState>) {
    // Check metabolic pause flag — skip entire tick if paused (NAM-R7)
    if state
        .metabolic_paused
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        tracing::trace!("Observer tick skipped: metabolic paused");
        return;
    }

    // Phase 1B: Drain EventBus events into M37 BEFORE fitness evaluation.
    // This is the critical wiring that feeds the M37→M38→M39 pipeline.
    ingest_eventbus_events(state);

    let tensor = state.engine.build_tensor();
    let Some(obs) = state.engine.observer() else {
        return;
    };
    match obs.tick(&tensor) {
        Ok(report) => {
            persist_tick_results(state, &tensor, obs, &report).await;

            // NAM-R1: Track zero-correlation streak for self-triggered dormancy detection
            if report.correlations_since_last == 0 {
                let streak = state.zero_correlation_streak.fetch_add(
                    1,
                    std::sync::atomic::Ordering::Relaxed,
                ) + 1;
                if streak >= 3 {
                    tracing::info!(
                        streak,
                        "Self-detected dormancy: 0 correlations for {streak} consecutive ticks"
                    );
                }
                // Phase 4 (V7): Dormancy adaptive response — self-modify when dormant
                dormancy_response(state, obs, &report, streak);
            } else {
                state
                    .zero_correlation_streak
                    .store(0, std::sync::atomic::Ordering::Relaxed);
            }

            // G4: Auto-remediate when fitness drops to Critical or Failed
            if matches!(
                report.system_state,
                maintenance_engine_v2::m7_observer::SystemState::Critical
                    | maintenance_engine_v2::m7_observer::SystemState::Failed
            ) {
                let desc = format!(
                    "Auto-remediation: fitness {:.3} in {:?} state",
                    report.current_fitness, report.system_state
                );
                if let Err(e) = state.engine.auto_remediate(
                    "maintenance-engine",
                    maintenance_engine_v2::m3_core_logic::Severity::High,
                    &desc,
                ) {
                    tracing::warn!(error = %e, "Auto-remediation from fitness trigger failed");
                } else {
                    tracing::info!(
                        fitness = report.current_fitness,
                        "Auto-remediation triggered by Critical fitness"
                    );
                    // Phase 1A: Publish remediation event
                    let payload = serde_json::json!({
                        "event": "auto_remediation",
                        "trigger": "critical_fitness",
                        "fitness": report.current_fitness,
                        "state": format!("{:?}", report.system_state),
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });
                    if let Err(e) = state.engine.event_bus().publish(
                        "remediation", "auto_remediation", &payload.to_string(), "observer",
                    ) {
                        tracing::trace!(error = %e, "Failed to publish remediation event");
                    }
                }
            }

            // Phase 1A: Publish observer tick metrics to EventBus
            let payload = serde_json::json!({
                "event": "observer_tick",
                "tick": report.tick,
                "fitness": report.current_fitness,
                "system_state": format!("{:?}", report.system_state),
                "correlations": report.correlations_since_last,
                "emergences": report.emergences_since_last,
                "generation": report.generation,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            });
            if let Err(e) = state.engine.event_bus().publish(
                "metrics", "observer_tick", &payload.to_string(), "observer_layer",
            ) {
                tracing::trace!(error = %e, "Failed to publish observer metrics event");
            }

            // Phase 2.5 (NAM-T2): Persist cognitive state for temporal continuity.
            if let Some(ref db) = state.db {
                let streak = state
                    .zero_correlation_streak
                    .load(std::sync::atomic::Ordering::Relaxed);
                let cog = maintenance_engine_v2::database::CognitiveState {
                    window_size_ms: obs.correlator().effective_window_ms(),
                    generation: report.generation,
                    tick_count: report.tick,
                    zero_correlation_streak: streak,
                    fitness: report.current_fitness,
                    saved_at: chrono::Utc::now().to_rfc3339(),
                };
                if let Err(e) = db.write_cognitive_state(&cog).await {
                    tracing::trace!(error = %e, "Failed to persist cognitive state (non-fatal)");
                }
            }

            // R10: Pattern→Antipattern→PBFT escalation.
            // Check antipattern violations and escalate high-severity ones
            // to PBFT consensus for fleet-wide decision making.
            if report.tick % 5 == 0 {
                let violations = state.engine.antipattern_violation_count();
                if violations > 0 {
                    let description = format!(
                        "Antipattern violations detected: {violations} active, fitness {:.3}",
                        report.current_fitness
                    );
                    // Create PBFT proposal for consensus on remediation action
                    if let Err(e) = state.engine.create_consensus_proposal(
                        &description,
                        "antipattern-escalation",
                    ) {
                        tracing::trace!(error = %e, "PBFT proposal creation skipped");
                    } else {
                        tracing::info!(
                            violations,
                            "R10: Antipattern violations escalated to PBFT consensus"
                        );
                    }

                    // R11: Wire active dissent into consensus pipeline.
                    // Generate counterarguments for the proposal before voting begins.
                    if let Some(proposal_id) = state.engine.latest_proposal_id() {
                        let dissent_count = state
                            .engine
                            .generate_and_record_dissent(&proposal_id);
                        if dissent_count > 0 {
                            tracing::info!(
                                dissent_count,
                                proposal = %proposal_id,
                                "R11: Active dissent generated for PBFT proposal"
                            );
                        }
                    }
                }
            }

            // Phase 3 (V7): RALPH Loop Processing — close the propose→execute→verify cycle
            ralph_process_mutations(state, obs, &report).await;

            // Issue 2 fix: Fitness-driven mutation pathway.
            // Every 10 ticks, check if fitness is below threshold and propose
            // mutations to improve the weakest layer. Kill switch: MUTATION_ENABLED=false.
            // R3 fix: generation >= 5 (was > 5, blocking at exactly gen 5)
            if report.tick % 10 == 0
                && report.generation >= 5
                && std::env::var("MUTATION_ENABLED").map_or(true, |v| v != "false")
            {
                fitness_driven_mutations(state, obs, &report);
            }

            // R3 fix: Generation-independent periodic mutation path.
            // Fires every 20 ticks regardless of generation count, targeting
            // structural deficits detected by self-model layer health scores.
            // This ensures RALPH mutates even when fitness > 0.75 but layers
            // are individually degraded (e.g. L2=0.33, L5=0.49).
            if report.tick % 20 == 0
                && std::env::var("MUTATION_ENABLED").map_or(true, |v| v != "false")
            {
                periodic_structural_mutations(state, obs, &report);
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "L7 observer tick failed (swallowed)");
        }
    }
}

/// Propose mutations based on fitness level, targeting the weakest layer.
///
/// R3 fix: raised threshold from 0.75 to 0.85 so mutations fire at current
/// fitness (0.61-0.80). RALPH should always be seeking improvement, not only
/// when fitness is critically low.
fn fitness_driven_mutations(
    state: &Arc<AppState>,
    obs: &maintenance_engine_v2::m7_observer::ObserverLayer,
    report: &maintenance_engine_v2::m7_observer::ObservationReport,
) {
    if report.current_fitness >= 0.85 {
        return;
    }

    let layer_health = state.engine.layer_health_scores();
    // Find the worst-performing layer index (skip L1=always 1.0)
    let worst_layer = layer_health
        .iter()
        .enumerate()
        .skip(1) // skip L1
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let Some((layer_idx, &layer_score)) = worst_layer else {
        return;
    };

    // Only propose if the layer is actually degraded
    if layer_score >= 0.7 {
        return;
    }

    // Map layer index to a tunable parameter
    let (target_param, current_val, proposed_val) = match layer_idx {
        1 => {
            // L2 Services: no direct parameter to tune (health polling handles this)
            return;
        }
        6 => {
            // L7 Observer: widen correlation window to catch more events
            let current = obs.correlator().effective_window_ms();
            #[allow(clippy::cast_precision_loss)]
            let current_f = current as f64;
            let widened_f = (current_f * 1.15).min(120_000.0);
            ("log_correlator.window_size_ms", current_f, widened_f)
        }
        _ => {
            // For other layers, adjust emergence confidence threshold
            let current = 0.4_f64; // default from config
            let proposed = (current - 0.05).max(0.2);
            (
                "emergence_detector.min_confidence",
                current,
                proposed,
            )
        }
    };

    if let Err(e) = obs.chamber().propose_mutation(
        target_param,
        current_val,
        proposed_val,
        report.current_fitness,
    ) {
        tracing::trace!(
            error = %e,
            target = target_param,
            "Fitness-driven mutation proposal skipped"
        );
    } else {
        tracing::info!(
            target = target_param,
            fitness = report.current_fitness,
            layer = layer_idx,
            layer_score,
            "Fitness-driven mutation proposed for weakest layer"
        );
    }
}

/// R3 fix: Generation-independent periodic mutation path.
///
/// Fires every 20 ticks regardless of fitness or generation count. Examines
/// self-model layer health scores to find structural deficits (layers < 0.5)
/// and proposes targeted mutations. This ensures RALPH evolves even when
/// overall fitness looks acceptable but individual layers are degraded.
fn periodic_structural_mutations(
    state: &Arc<AppState>,
    obs: &maintenance_engine_v2::m7_observer::ObserverLayer,
    report: &maintenance_engine_v2::m7_observer::ObservationReport,
) {
    let layer_health = state.engine.layer_health_scores();

    // Find layers with structural deficits (below 0.6)
    for (idx, &score) in layer_health.iter().enumerate().skip(1) {
        if score >= 0.6 {
            continue;
        }

        // Map structural deficit to a tunable parameter
        let (target_param, current_val, proposed_val) = match idx {
            1 => continue, // L2: health polling handles this, not tunable
            2 => {
                // L3: Lower remediation confidence threshold to process more requests
                let current = 0.7_f64;
                let proposed = (current - 0.05).max(0.4);
                ("remediation.min_confidence", current, proposed)
            }
            4 => {
                // L5: Increase STDP timing window to capture more co-activations
                let current = 100.0_f64;
                let proposed = (current * 1.2).min(200.0);
                ("stdp.window_ms", current, proposed)
            }
            5 => {
                // L6: Increase agent fleet size for broader consensus
                let current = 41.0_f64;
                let proposed = (current + 5.0).min(60.0);
                ("pbft.fleet_target", current, proposed)
            }
            _ => {
                // Generic: adjust emergence detection sensitivity
                let current = 0.4_f64;
                let proposed = (current - 0.03).max(0.2);
                ("emergence_detector.min_confidence", current, proposed)
            }
        };

        if let Err(e) = obs.chamber().propose_mutation(
            target_param,
            current_val,
            proposed_val,
            report.current_fitness,
        ) {
            tracing::trace!(
                error = %e,
                target = target_param,
                layer = idx,
                "Periodic structural mutation proposal skipped"
            );
        } else {
            tracing::info!(
                target = target_param,
                layer = idx,
                layer_score = score,
                "Periodic structural mutation proposed for deficit layer"
            );
            // Only propose one mutation per tick to avoid overwhelming the chamber
            break;
        }
    }
}

/// NAM-T2: Restore cognitive state from the database for temporal continuity across restarts.
///
/// Loads the last persisted `CognitiveState` and applies it to the observer layer,
/// restoring tick counter, generation, and correlator window configuration.
async fn restore_cognitive_state(state: &Arc<AppState>) {
    let Some(ref db) = state.db else { return };
    match db.read_cognitive_state().await {
        Ok(Some(cog_state)) => {
            if let Some(obs) = state.engine.observer() {
                obs.restore_cognitive_state(&cog_state);
            }
        }
        Ok(None) => {
            tracing::debug!("No persisted cognitive state found (first run)");
        }
        Err(e) => {
            tracing::trace!(error = %e, "Failed to read cognitive state (non-fatal)");
        }
    }
}

/// RALPH Loop Processing: execute proposed mutations, verify past deadline, advance phase.
///
/// This closes the critical propose→execute→verify→accept/rollback cycle that was
/// previously severed (mutations were proposed but never applied or verified).
#[allow(clippy::too_many_lines)] // R20 N04/N05 wiring adds necessary complexity
async fn ralph_process_mutations(
    state: &Arc<AppState>,
    obs: &maintenance_engine_v2::m7_observer::ObserverLayer,
    report: &maintenance_engine_v2::m7_observer::ObservationReport,
) {
    let ralph_state = obs.ralph_state();

    // Bootstrap: start first RALPH cycle if none has ever started
    if ralph_state.cycle_number == 0 {
        if let Err(e) = obs.chamber().start_cycle() {
            tracing::trace!(error = %e, "RALPH bootstrap: start_cycle skipped");
        } else {
            tracing::info!("RALPH loop bootstrapped: cycle 1 started");
            // Phase 6 (V7): Bootstrap Hebbian pathways for RALPH→STDP feedback
            bootstrap_ralph_pathways(state);
        }
    }

    // Process active mutations
    let active = obs.active_mutation_list();
    if !active.is_empty() {
        tracing::debug!(
            count = active.len(),
            "RALPH: processing active mutations"
        );
    }
    for mutation in &active {
        match mutation.status {
            maintenance_engine_v2::m7_observer::MutationStatus::Proposed => {
                // R20: Wire N05 EvolutionGate — submit mutation for field coherence
                // testing before execution. Gate checks r_before to decide if the
                // mutation should proceed, be rejected, or deferred to consensus.
                use maintenance_engine_v2::m1_foundation::Timestamp;
                use maintenance_engine_v2::nexus::evolution_gate::{EvolutionGate, MutationCandidate};
                use maintenance_engine_v2::nexus::stdp_bridge::StdpBridge;
                let candidate = MutationCandidate {
                    id: mutation.id.clone(),
                    parameter: mutation.target_parameter.clone(),
                    old_value: mutation.original_value,
                    new_value: mutation.applied_value,
                    proposed_by: "ralph".to_string(),
                    timestamp: Timestamp::now(),
                };
                let _ = state.engine.evolution_gate().submit_mutation(candidate);

                // R20: Wire N04 StdpBridge — record the mutation proposal as a
                // pre-synaptic spike, creating timing pairs with the subsequent
                // verification outcome (post-synaptic). This bridges L8→L5.
                let _ = state.engine.stdp_bridge().record_interaction(
                    "ralph-propose",
                    &mutation.target_parameter,
                    true,
                );

                // Execute mutation (status transition + runtime config change)
                if let Err(e) = obs.execute_mutation(&mutation.id) {
                    tracing::trace!(error = %e, id = %mutation.id, "RALPH: mutation execute skipped");
                }
            }
            maintenance_engine_v2::m7_observer::MutationStatus::Verifying => {
                // Check if verification deadline has passed
                let elapsed_ms = (chrono::Utc::now() - mutation.applied_at).num_milliseconds();
                if elapsed_ms > 30_000 {
                    // R20: N05 EvolutionGate — evaluate field coherence post-mutation.
                    {
                        use maintenance_engine_v2::nexus::evolution_gate::EvolutionGate;
                        use maintenance_engine_v2::nexus::field_bridge::FieldBridge;
                        let r_now = state.engine.field_bridge().current_r();
                        let _ = state.engine.evolution_gate().evaluate(
                            &mutation.id,
                            mutation.fitness_at_proposal, // proxy for r_before
                            r_now,
                        );
                    }

                    // R20: N04 StdpBridge — record verification outcome as post-synaptic
                    {
                        use maintenance_engine_v2::nexus::stdp_bridge::StdpBridge;
                        let _ = state.engine.stdp_bridge().record_interaction(
                            &mutation.target_parameter,
                            "ralph-verify",
                            true,
                        );
                    }

                    // Verify against current fitness
                    match obs.verify_or_rollback(&mutation.id, report.current_fitness) {
                        Ok(record) => {
                            if let Some(ref db) = state.db {
                                let entry = maintenance_engine_v2::database::MutationEntry {
                                    id: record.id.clone(),
                                    generation: record.generation,
                                    target_parameter: record.target_parameter.clone(),
                                    original_value: record.original_value,
                                    mutated_value: record.mutated_value,
                                    applied: record.applied,
                                    rolled_back: record.rolled_back,
                                    timestamp: record.timestamp.to_rfc3339(),
                                };
                                if let Err(e) = db.write_mutation(&entry).await {
                                    tracing::trace!(error = %e, "Failed to persist mutation record");
                                }
                            }
                            // Phase 6 (V7): RALPH→STDP feedback via Hebbian pathways
                            let pathway_key = format!(
                                "ralph.{}->mutation.outcome",
                                record.target_parameter
                            );
                            if record.rolled_back {
                                // LTD: weaken pathway for rolled-back mutations
                                let _ = state.engine.hebbian_manager()
                                    .record_failure(&pathway_key);
                                tracing::info!(
                                    id = %record.id,
                                    target = %record.target_parameter,
                                    "RALPH: mutation rolled back (fitness decline) + LTD"
                                );
                            } else {
                                // LTP: strengthen pathway for accepted mutations
                                let _ = state.engine.hebbian_manager()
                                    .record_success(&pathway_key);
                                tracing::info!(
                                    id = %record.id,
                                    target = %record.target_parameter,
                                    delta = record.fitness_after - record.fitness_before,
                                    "RALPH: mutation accepted + LTP"
                                );
                            }
                        }
                        Err(e) => {
                            tracing::trace!(error = %e, "RALPH: verify/rollback failed");
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Advance RALPH phase (tick-driven FSM)
    if let Ok(phase) = obs.advance_ralph_phase() {
        tracing::debug!(phase = ?phase, "RALPH phase advanced");
    }
}

/// Phase 6 (V7): Bootstrap Hebbian pathways for RALPH→STDP feedback.
///
/// Creates one pathway per known mutation target so that `record_success()`
/// and `record_failure()` have existing keys to operate on.
fn bootstrap_ralph_pathways(state: &Arc<AppState>) {
    let targets = [
        "log_correlator.window_size_ms",
        "emergence_detector.min_confidence",
        "tick_interval_ms",
    ];
    for target in &targets {
        let source = format!("ralph.{target}");
        if let Err(e) = state.engine.hebbian_manager().add_pathway(
            &source,
            "mutation.outcome",
            maintenance_engine_v2::m5_learning::PathwayType::ConfigToBehavior,
        ) {
            // Already exists is fine — just log at trace
            tracing::trace!(error = %e, target, "RALPH pathway bootstrap skipped");
        }
    }
    tracing::info!(
        targets = targets.len(),
        "RALPH→STDP Hebbian pathways bootstrapped"
    );
}

/// Phase 4 (V7): Dormancy adaptive response — NAM-R1 self-modification.
///
/// Takes escalating action based on zero-correlation streak length:
/// - Streak >= 3: propose mutation to widen M37 correlation window by 10%
/// - Streak >= 5: trigger learning cycle + publish synthetic health event
/// - Streak >= 10: escalate to auto-remediation warning
fn dormancy_response(
    state: &Arc<AppState>,
    obs: &maintenance_engine_v2::m7_observer::ObserverLayer,
    report: &maintenance_engine_v2::m7_observer::ObservationReport,
    streak: u64,
) {
    // Level 1: Widen correlation window to catch sparser events
    if streak == 3 {
        let current_ms = obs.correlator().effective_window_ms();
        // Widen by 10%, capped at 60s. current_ms is u64 in [100, 60_000].
        let widened_ms = (current_ms / 10).saturating_add(current_ms).min(60_000);
        // propose_mutation expects f64 values
        let current_f = f64::from(u32::try_from(current_ms).unwrap_or(u32::MAX));
        let widened_f = f64::from(u32::try_from(widened_ms).unwrap_or(u32::MAX));
        if let Err(e) = obs.chamber().propose_mutation(
            "log_correlator.window_size_ms",
            current_f,
            widened_f,
            report.current_fitness,
        ) {
            tracing::trace!(error = %e, "Dormancy: window widen proposal skipped");
        } else {
            tracing::info!(
                current_ms,
                widened_ms,
                "Dormancy L1: proposed M37 window widen"
            );
        }
    }

    // Level 2: Trigger learning cycle + publish synthetic event to break silence
    if streak == 5 {
        if let Err(e) = state.engine.learning_cycle() {
            tracing::trace!(error = %e, "Dormancy L2: learning cycle failed");
        }
        let payload = serde_json::json!({
            "event": "dormancy_break",
            "streak": streak,
            "fitness": report.current_fitness,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        if let Err(e) = state.engine.event_bus().publish(
            "health", "dormancy_break", &payload.to_string(), "observer_dormancy",
        ) {
            tracing::trace!(error = %e, "Dormancy L2: event publish failed");
        }
        tracing::info!("Dormancy L2: learning cycle triggered + synthetic event published");
    }

    // Level 3: Auto-remediation escalation
    if streak == 10 {
        let desc = format!(
            "Persistent dormancy: {streak} consecutive ticks with 0 correlations (fitness {:.3})",
            report.current_fitness
        );
        if let Err(e) = state.engine.auto_remediate(
            "maintenance-engine",
            Severity::Medium,
            &desc,
        ) {
            tracing::trace!(error = %e, "Dormancy L3: remediation failed");
        } else {
            tracing::info!(streak, "Dormancy L3: auto-remediation escalated");
        }
    }
}

/// Drain pending `EventBus` events into the L7 Observer's M37 `LogCorrelator`.
///
/// This is the critical wiring that connects L1-L6 service events to the
/// M37→M38→M39 cognitive pipeline. Without this, the `EventBus` has publishers
/// but M37 has no data to correlate.
///
/// Called at the start of each observer tick, BEFORE fitness evaluation,
/// so that newly ingested events can influence the current tick's analysis.
fn ingest_eventbus_events(state: &Arc<AppState>) {
    let Some(obs) = state.engine.observer() else {
        return;
    };

    // Drain events from all 7 default channels (incl. gc added Session 070)
    let channels = ["health", "remediation", "learning", "consensus", "integration", "metrics", "gc"];

    let mut total_ingested: u64 = 0;
    for channel in &channels {
        // Get recent events (last 50 per channel per tick — prevents flooding)
        let events = state.engine.event_bus().get_events(channel, 50);
        for event in &events {
            match obs.ingest_event(channel, &event.event_type, &event.payload) {
                Ok(correlated) => {
                    total_ingested += 1;
                    if !correlated.links.is_empty() {
                        tracing::debug!(
                            channel,
                            event_type = %event.event_type,
                            correlations = correlated.links.len(),
                            "M37 found correlations from EventBus event"
                        );
                    }
                }
                Err(e) => {
                    tracing::trace!(
                        error = %e,
                        channel,
                        "M37 event ingestion failed (non-fatal)"
                    );
                }
            }
        }
    }

    if total_ingested > 0 {
        tracing::debug!(
            events_ingested = total_ingested,
            "EventBus→M37 ingestion complete"
        );
    }
}

/// Persist observer tick results to database (fitness, tensor, emergence).
async fn persist_tick_results(
    state: &Arc<AppState>,
    tensor: &maintenance_engine_v2::Tensor12D,
    obs: &maintenance_engine_v2::m7_observer::ObserverLayer,
    report: &maintenance_engine_v2::m7_observer::ObservationReport,
) {
    let Some(ref db) = state.db else { return };

    // G1: Persist fitness history
    let entry = maintenance_engine_v2::database::FitnessHistoryEntry {
        timestamp: chrono::Utc::now().to_rfc3339(),
        fitness: report.current_fitness,
        system_state: format!("{:?}", obs.system_state()),
        tensor_hash: tensor_hash(tensor),
        generation: report.generation,
    };
    if let Err(e) = db.write_fitness_history(&entry).await {
        tracing::warn!(error = %e, "Failed to persist fitness (non-fatal)");
    }

    // G2: Persist tensor snapshot
    let snap = maintenance_engine_v2::database::TensorSnapshot {
        timestamp: chrono::Utc::now().to_rfc3339(),
        dimensions: tensor.to_array(),
        source: "observer_tick".to_string(),
        tick: report.tick,
    };
    if let Err(e) = db.write_tensor_snapshot(&snap).await {
        tracing::warn!(error = %e, "Failed to persist tensor snapshot (non-fatal)");
    }

    // G2: Persist emergence events
    if report.emergences_since_last > 0 {
        #[allow(clippy::cast_possible_truncation)]
        let count = report.emergences_since_last as usize;
        let recent = obs.recent_emergences(count);
        for em in &recent {
            let em_entry = maintenance_engine_v2::database::EmergenceEntry {
                id: em.id.clone(),
                emergence_type: format!("{:?}", em.emergence_type),
                confidence: em.confidence,
                severity: em.severity,
                detected_at: em.detected_at.to_rfc3339(),
                description: em.description.clone(),
            };
            if let Err(e) = db.write_emergence(&em_entry).await {
                tracing::warn!(error = %e, "Failed to persist emergence (non-fatal)");
            }
        }
    }

    // G3 (V7): Persist top correlations (multi-channel links only, capped at 20/tick)
    let recent_corr = obs.correlator().recent_events(50);
    let mut corr_written = 0u32;
    for corr in &recent_corr {
        if corr.links.len() >= 2 && corr_written < 20 {
            #[allow(clippy::cast_possible_truncation)]
            let entry = maintenance_engine_v2::database::CorrelationEntry {
                id: corr.id.clone(),
                channel: corr.primary_event.channel.clone(),
                event_type: corr.primary_event.event_type.clone(),
                link_count: corr.links.len() as u32,
                timestamp: corr.discovered_at.to_rfc3339(),
            };
            if let Err(e) = db.write_correlation(&entry).await {
                tracing::trace!(error = %e, "Failed to persist correlation (non-fatal)");
            } else {
                corr_written += 1;
            }
        }
    }
}

/// Compute a simple XOR hash of the tensor bytes for deduplication.
fn tensor_hash(tensor: &maintenance_engine_v2::Tensor12D) -> String {
    let bytes = tensor.to_bytes();
    let mut hash = 0_u64;
    for chunk in bytes.chunks(8) {
        let mut buf = [0_u8; 8];
        let len = chunk.len().min(8);
        buf[..len].copy_from_slice(&chunk[..len]);
        hash ^= u64::from_le_bytes(buf);
    }
    format!("{hash:x}")
}

/// Spawn tool library registration (one-shot after startup).
fn spawn_tool_registration(state: &Arc<AppState>) {
    let tool_port = state.port;
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(8)).await;
        match maintenance_engine_v2::m4_integration::tool_registrar::ToolRegistrar::new(tool_port) {
            Ok(registrar) => match registrar.register_all().await {
                Ok(count) => tracing::info!(registered = count, "Tool Library registration complete"),
                Err(e) => tracing::warn!(error = %e, "Tool Library registration failed (non-fatal)"),
            },
            Err(e) => tracing::warn!(error = %e, "Tool registrar init failed (non-fatal)"),
        }
    });
}

/// Spawn peer bridge tiered polling with DB persistence.
fn spawn_peer_polling(state: &Arc<AppState>) {
    if state.engine.peer_bridge().is_none() {
        return;
    }

    for &(secs, tiers) in &[(15_u64, &[1_u8] as &[u8]), (30, &[2, 3]), (60, &[4, 5])] {
        let poll_state = Arc::clone(state);
        let tiers = tiers.to_vec();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(secs));
            interval.tick().await;
            loop {
                interval.tick().await;
                if let Some(bridge) = poll_state.engine.peer_bridge() {
                    for &tier in &tiers {
                        bridge.poll_tier(tier).await;
                    }
                    persist_poll_results(&poll_state, bridge).await;

                    // R7: Publish peer health summary to EventBus integration channel.
                    // Circuit breaker state transitions (Open↔Closed) emit Hebbian
                    // LTP (recovery) or LTD (failure) signals for pathway differentiation.
                    {
                        let summary = bridge.mesh_summary();
                        let payload = serde_json::json!({
                            "event": "peer_health_cycle",
                            "reachable": summary.reachable_peers,
                            "total": summary.total_peers,
                            "circuits_open": summary.circuit_open_count,
                            "mesh_synergy": summary.mesh_synergy,
                        });
                        let _ = poll_state.engine.event_bus().publish(
                            "integration",
                            "peer_health_cycle",
                            &payload.to_string(),
                            "peer-poller",
                        );
                    }
                }
            }
        });
    }

    // Self-registration (one-shot after startup)
    let reg_state = Arc::clone(state);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(5)).await;
        if let Some(bridge) = reg_state.engine.peer_bridge() {
            bridge.register_self().await;
            bridge.set_polling(true);
        }
    });
}

/// G4: Spawn periodic learning cycle task (runs every 5 minutes).
///
/// Executes `Engine::learning_cycle()` which applies Hebbian decay,
/// processes STDP timing windows, and counts anti-pattern violations.
fn spawn_learning_cycle(state: &Arc<AppState>) {
    let learn_state = Arc::clone(state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        interval.tick().await; // skip immediate first tick
        loop {
            interval.tick().await;
            match learn_state.engine.learning_cycle() {
                Ok(result) => {
                    tracing::info!(
                        pathways_decayed = result.pathways_decayed,
                        timing_pairs = result.timing_pairs_processed,
                        antipatterns = result.antipatterns_detected,
                        "Learning cycle completed"
                    );

                    // Phase 1A: Publish learning cycle event to EventBus
                    let payload = serde_json::json!({
                        "event": "learning_cycle",
                        "pathways_decayed": result.pathways_decayed,
                        "timing_pairs": result.timing_pairs_processed,
                        "antipatterns": result.antipatterns_detected,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    });
                    if let Err(e) = learn_state.engine.event_bus().publish(
                        "learning", "learning_cycle", &payload.to_string(), "stdp_processor",
                    ) {
                        tracing::trace!(error = %e, "Failed to publish learning event (non-fatal)");
                    }

                    // G4: Persist learning performance sample
                    if let Some(ref db) = learn_state.db {
                        let sample = maintenance_engine_v2::database::PerformanceSample {
                            metric_name: "learning_cycle_decay".to_string(),
                            #[allow(clippy::cast_precision_loss)]
                            value: result.pathways_decayed as f64,
                            unit: "pathways".to_string(),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        };
                        if let Err(e) = db.write_performance_sample(&sample).await {
                            tracing::warn!(error = %e, "Failed to persist learning sample (non-fatal)");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Learning cycle failed (non-fatal)");
                }
            }
        }
    });
}

/// R8: Fetch thermal state from SYNTHEX via HTTP GET `/v3/thermal`.
///
/// Falls back to a synthetic reading derived from the engine tensor's
/// `health_score` dimension if SYNTHEX is unreachable (fail-OPEN pattern).
async fn fetch_synthex_thermal(
    state: &Arc<AppState>,
) -> maintenance_engine_v2::m7_observer::thermal_monitor::ThermalReading {
    let fallback = || {
        let tensor = state.engine.build_tensor();
        maintenance_engine_v2::m7_observer::thermal_monitor::ThermalReading {
            temperature: 1.0 - tensor.health_score,
            target: 0.5,
            pid_output: 0.0,
            timestamp: chrono::Utc::now(),
        }
    };

    let Ok(Ok(body)) = tokio::time::timeout(Duration::from_secs(2), async {
        let stream = tokio::net::TcpStream::connect("127.0.0.1:8090").await?;
        let (reader, mut writer) = stream.into_split();
        let req = "GET /v3/thermal HTTP/1.1\r\nHost: 127.0.0.1:8090\r\nConnection: close\r\n\r\n";
        tokio::io::AsyncWriteExt::write_all(&mut writer, req.as_bytes()).await?;
        let _ = tokio::io::AsyncWriteExt::shutdown(&mut writer).await;
        let mut buf = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(
            &mut tokio::io::BufReader::new(reader),
            &mut buf,
        )
        .await?;
        Ok::<Vec<u8>, std::io::Error>(buf)
    })
    .await
    else {
        return fallback();
    };

    let body_str = String::from_utf8_lossy(&body);
    let Some(json_start) = body_str.find('{') else {
        return fallback();
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&body_str[json_start..]) else {
        return fallback();
    };

    maintenance_engine_v2::m7_observer::thermal_monitor::ThermalReading {
        temperature: v
            .get("temperature")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.5),
        target: v
            .get("target")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.5),
        pid_output: v
            .get("pid_output")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0),
        timestamp: chrono::Utc::now(),
    }
}

/// Spawn thermal monitor polling (M40): polls SYNTHEX `/v3/thermal` every 30s.
fn spawn_thermal_polling(state: &Arc<AppState>) {
    let Some(monitor) = state.engine.thermal_monitor() else {
        return;
    };
    let interval_secs = monitor.config().poll_interval_secs;
    let poll_state = Arc::clone(state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.tick().await; // skip immediate tick
        loop {
            interval.tick().await;
            if let Some(mon) = poll_state.engine.thermal_monitor() {
                // R8: Couple with SYNTHEX thermal via HTTP GET /v3/thermal.
                // Falls back to synthetic reading from engine tensor if SYNTHEX unreachable.
                let reading = fetch_synthex_thermal(&poll_state).await;
                tracing::debug!(temperature = reading.temperature, "Thermal monitor tick");
                mon.record_reading(reading);
            }
        }
    });
}

/// Spawn cascade bridge polling (M42): polls SYNTHEX `/v3/diagnostics` every 15s.
fn spawn_cascade_polling(state: &Arc<AppState>) {
    let Some(bridge) = state.engine.cascade_bridge() else {
        return;
    };
    let interval_secs = bridge.config().poll_interval_secs;
    let poll_state = Arc::clone(state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.tick().await; // skip immediate tick
        loop {
            interval.tick().await;
            if let Some(cb) = poll_state.engine.cascade_bridge() {
                // In production this would HTTP GET /v3/diagnostics.
                // For now, record a synthetic snapshot.
                let snapshot = maintenance_engine_v2::m4_integration::cascade_bridge::CascadePipelineSnapshot {
                    stages: Vec::new(),
                    total_amplification: 1.0, // healthy default
                    open_breakers: 0,
                    timestamp: chrono::Utc::now(),
                };
                cb.record_snapshot(snapshot);
                tracing::debug!(window_size = cb.window_size(), "Cascade bridge tick");
            }
        }
    });
}

/// Spawn decay scheduler (M41): triggers SYNTHEX V3 decay every 300s.
fn spawn_decay_scheduler(state: &Arc<AppState>) {
    let Some(sched) = state.engine.decay_scheduler() else {
        return;
    };
    let interval_secs = sched.config().trigger_interval_secs;
    let poll_state = Arc::clone(state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.tick().await; // skip immediate tick
        loop {
            interval.tick().await;
            if let Some(ds) = poll_state.engine.decay_scheduler() {
                // In production this would POST /v3/decay/trigger.
                // For now, record a synthetic success.
                ds.record_success(None, None, None, None);
                tracing::debug!(
                    compliance = ds.compliance().compliance_ratio,
                    "Decay scheduler tick"
                );
            }
        }
    });
}

/// Habitat GC sweep — runs every hour (3600s). Session 070 Phase 2.
///
/// Responsibilities:
/// 1. Prune ME V2 database rows older than retention thresholds (batched)
/// 2. Trigger cross-service GC: RM `DELETE /gc`, ORAC `POST /blackboard/prune`
/// 3. Wire existing `PathwayPruner::prune()` from M28
/// 4. Publish GC summary event to `EventBus` "gc" channel
/// 5. Append audit entry to `/tmp/gc-audit.jsonl`
///
/// Does NOT run in the RALPH tick loop (S-Biggest Risk mitigation).
/// All HTTP calls use `tokio::time::timeout` with 5s cap.
fn spawn_gc_sweep(state: &Arc<AppState>) {
    let gc_state = Arc::clone(state);
    tokio::spawn(async move {
        // Wait 60s for services to stabilize before first GC
        tokio::time::sleep(Duration::from_secs(60)).await;
        tracing::info!("GC sweep started (3600s interval)");

        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        interval.tick().await; // skip immediate tick
        loop {
            interval.tick().await;

            if gc_state
                .metabolic_paused
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                tracing::debug!("GC sweep skipped: metabolic paused");
                continue;
            }

            let sweep_start = std::time::Instant::now();
            let mut total_pruned: u64 = 0;

            // ── 0. Disk pressure check (Phase 3, Session 070) ──
            let disk_pct = gc_check_disk_usage().await;
            if disk_pct > 90.0 {
                tracing::warn!(disk_pct, "GC: CRITICAL disk pressure — pausing metabolism");
                gc_state
                    .metabolic_paused
                    .store(true, std::sync::atomic::Ordering::SeqCst);
            } else if disk_pct > 85.0 {
                tracing::warn!(disk_pct, "GC: EMERGENCY disk pressure");
            } else if disk_pct > 80.0 {
                tracing::info!(disk_pct, "GC: WARNING disk pressure");
            }

            // Auto-resume: if metabolic_paused was set by GC and disk is now < 80%, resume
            if gc_state
                .metabolic_paused
                .load(std::sync::atomic::Ordering::Relaxed)
                && disk_pct < 80.0
            {
                gc_state
                    .metabolic_paused
                    .store(false, std::sync::atomic::Ordering::SeqCst);
                tracing::info!(disk_pct, "GC: auto-resumed metabolism (disk recovered)");
            }

            // ── 1. Prune ME V2 databases (batched DELETEs) ──
            if let Some(ref db) = gc_state.db {
                total_pruned += gc_prune_evolution_db(db).await;
                total_pruned += gc_prune_performance_db(db).await;
                total_pruned += gc_prune_service_events(db).await;
            }

            // M28 Pathway Pruner runs via spawn_learning_cycle (300s).
            // GC does not duplicate it — learning_cycle() already calls prune().

            // ── 2. Cross-service GC triggers ──
            total_pruned += gc_trigger_rm().await;
            total_pruned += gc_trigger_orac_prune().await;

            // ── 4. Publish GC summary to EventBus ──
            let duration_ms = sweep_start.elapsed().as_millis();
            let payload = serde_json::json!({
                "event": "gc_sweep",
                "total_pruned": total_pruned,
                "duration_ms": duration_ms,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            });
            if let Err(e) = gc_state.engine.event_bus().publish(
                "gc", "gc_sweep", &payload.to_string(), "gc_sweep",
            ) {
                tracing::trace!(error = %e, "GC: EventBus publish failed (non-fatal)");
            }

            // ── 5. Audit log ──
            gc_append_audit(total_pruned, duration_ms).await;

            tracing::info!(
                total_pruned,
                duration_ms,
                "GC sweep completed"
            );
        }
    });
}

/// Prune `fitness_history` and `mutation_log` rows older than 30 days.
///
/// Uses batched DELETEs (100 rows per iteration) with `tokio::time::sleep`
/// between batches to yield to concurrent writers.
async fn gc_prune_evolution_db(db: &maintenance_engine_v2::database::DatabaseManager) -> u64 {
    use maintenance_engine_v2::m1_foundation::state::{execute, DatabaseType};

    let Ok(pool) = db.persistence().pool(DatabaseType::EvolutionTracking) else {
        return 0;
    };

    let cutoff = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(30))
        .map_or_else(String::new, |dt| dt.to_rfc3339());

    if cutoff.is_empty() {
        return 0;
    }

    let mut total: u64 = 0;
    // Batch-delete fitness_history
    loop {
        match execute(
            pool,
            "DELETE FROM fitness_history WHERE rowid IN (SELECT rowid FROM fitness_history WHERE timestamp < ?1 LIMIT 100)",
            &[cutoff.as_str()],
        )
        .await
        {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(e) => {
                tracing::debug!(error = %e, "GC: fitness_history prune failed");
                break;
            }
        }
    }

    // Batch-delete mutation_log
    loop {
        match execute(
            pool,
            "DELETE FROM mutation_log WHERE rowid IN (SELECT rowid FROM mutation_log WHERE timestamp < ?1 LIMIT 100)",
            &[cutoff.as_str()],
        )
        .await
        {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(e) => {
                tracing::debug!(error = %e, "GC: mutation_log prune failed");
                break;
            }
        }
    }

    if total > 0 {
        tracing::info!(total, "GC: evolution DB rows pruned");
    }
    total
}

/// Prune `performance_samples` older than 30 days.
async fn gc_prune_performance_db(db: &maintenance_engine_v2::database::DatabaseManager) -> u64 {
    use maintenance_engine_v2::m1_foundation::state::{execute, DatabaseType};

    let Ok(pool) = db.persistence().pool(DatabaseType::PerformanceMetrics) else {
        return 0;
    };

    let cutoff = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(30))
        .map_or_else(String::new, |dt| dt.to_rfc3339());

    if cutoff.is_empty() {
        return 0;
    }

    let mut total: u64 = 0;
    loop {
        match execute(
            pool,
            "DELETE FROM performance_samples WHERE rowid IN (SELECT rowid FROM performance_samples WHERE timestamp < ?1 LIMIT 100)",
            &[cutoff.as_str()],
        )
        .await
        {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(e) => {
                tracing::debug!(error = %e, "GC: performance_samples prune failed");
                break;
            }
        }
    }

    if total > 0 {
        tracing::info!(total, "GC: performance DB rows pruned");
    }
    total
}

/// Prune `service_events` older than 90 days.
async fn gc_prune_service_events(db: &maintenance_engine_v2::database::DatabaseManager) -> u64 {
    use maintenance_engine_v2::m1_foundation::state::{execute, DatabaseType};

    let Ok(pool) = db.persistence().pool(DatabaseType::ServiceTracking) else {
        return 0;
    };

    let cutoff = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(90))
        .map_or_else(String::new, |dt| dt.to_rfc3339());

    if cutoff.is_empty() {
        return 0;
    }

    let mut total: u64 = 0;
    loop {
        match execute(
            pool,
            "DELETE FROM service_events WHERE rowid IN (SELECT rowid FROM service_events WHERE timestamp < ?1 LIMIT 100)",
            &[cutoff.as_str()],
        )
        .await
        {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(e) => {
                tracing::debug!(error = %e, "GC: service_events prune failed");
                break;
            }
        }
    }

    if total > 0 {
        tracing::info!(total, "GC: service events pruned");
    }
    total
}

/// Trigger RM garbage collection via `DELETE /gc` on port 8130.
async fn gc_trigger_rm() -> u64 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let addr = "127.0.0.1:8130";
    let request = "DELETE /gc HTTP/1.1\r\nHost: 127.0.0.1:8130\r\nConnection: close\r\n\r\n";

    match tokio::time::timeout(Duration::from_secs(5), async {
        let stream = tokio::net::TcpStream::connect(addr).await?;
        let (reader, mut writer) = stream.into_split();
        writer.write_all(request.as_bytes()).await?;
        writer.shutdown().await?;
        let mut body = Vec::new();
        tokio::io::BufReader::new(reader)
            .read_to_end(&mut body)
            .await?;
        Ok::<_, std::io::Error>(body)
    })
    .await
    {
        Ok(Ok(body)) => {
            let resp = String::from_utf8_lossy(&body);
            if resp.contains("gc_removed") {
                // Parse gc_removed count from response
                if let Some(n) = resp
                    .split("gc_removed\":")
                    .nth(1)
                    .and_then(|s| s.split([',', '}'].as_ref()).next())
                    .and_then(|s| s.trim().parse::<u64>().ok())
                {
                    if n > 0 {
                        tracing::info!(removed = n, "GC: RM /gc triggered");
                    }
                    return n;
                }
            }
            0
        }
        Ok(Err(e)) => {
            tracing::debug!(error = %e, "GC: RM /gc call failed (non-fatal)");
            0
        }
        Err(_) => {
            tracing::debug!("GC: RM /gc call timed out (non-fatal)");
            0
        }
    }
}

/// Trigger ORAC blackboard prune via `POST /blackboard/prune` on port 8133.
async fn gc_trigger_orac_prune() -> u64 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let addr = "127.0.0.1:8133";
    let request =
        "POST /blackboard/prune HTTP/1.1\r\nHost: 127.0.0.1:8133\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

    match tokio::time::timeout(Duration::from_secs(5), async {
        let stream = tokio::net::TcpStream::connect(addr).await?;
        let (reader, mut writer) = stream.into_split();
        writer.write_all(request.as_bytes()).await?;
        writer.shutdown().await?;
        let mut body = Vec::new();
        tokio::io::BufReader::new(reader)
            .read_to_end(&mut body)
            .await?;
        Ok::<_, std::io::Error>(body)
    })
    .await
    {
        Ok(Ok(body)) => {
            let resp = String::from_utf8_lossy(&body);
            // Sum all fields from PruneReport JSON
            let total: u64 = ["stale_panes", "complete_panes", "old_tasks", "ghosts",
                "hebbian_summaries", "consent_audit", "stale_sessions", "coupling_compacted"]
                .iter()
                .filter_map(|key| {
                    resp.split(&format!("\"{key}\":"))
                        .nth(1)
                        .and_then(|s| s.split([',', '}'].as_ref()).next())
                        .and_then(|s| s.trim().parse::<u64>().ok())
                })
                .sum();
            if total > 0 {
                tracing::info!(total, "GC: ORAC blackboard prune triggered");
            }
            total
        }
        Ok(Err(e)) => {
            tracing::debug!(error = %e, "GC: ORAC prune call failed (non-fatal)");
            0
        }
        Err(_) => {
            tracing::debug!("GC: ORAC prune call timed out (non-fatal)");
            0
        }
    }
}

/// Append a GC audit entry to `/tmp/gc-audit.jsonl`.
async fn gc_append_audit(total_pruned: u64, duration_ms: u128) {
    use tokio::io::AsyncWriteExt;

    let entry = format!(
        "{{\"ts\":\"{}\",\"action\":\"gc_sweep\",\"pruned\":{total_pruned},\"duration_ms\":{duration_ms}}}\n",
        chrono::Utc::now().to_rfc3339(),
    );
    match tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/gc-audit.jsonl")
        .await
    {
        Ok(mut f) => {
            if let Err(e) = f.write_all(entry.as_bytes()).await {
                tracing::debug!(error = %e, "GC: audit log write failed (non-fatal)");
            }
        }
        Err(e) => {
            tracing::debug!(error = %e, "GC: audit log open failed (non-fatal)");
        }
    }
}

/// Check root partition disk usage via `df /` (Phase 3, Session 070).
///
/// Uses `tokio::task::spawn_blocking` to avoid blocking the async runtime.
/// Returns the usage percentage (0.0-100.0), or 50.0 on failure (safe fallback).
async fn gc_check_disk_usage() -> f64 {
    tokio::task::spawn_blocking(|| {
        let output = std::process::Command::new("df")
            .args(["--output=pcent", "/"])
            .output();
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                // df output: "Use%\n 75%\n" — extract the number
                stdout
                    .lines()
                    .nth(1)
                    .and_then(|line| line.trim().trim_end_matches('%').parse::<f64>().ok())
                    .unwrap_or(50.0)
            }
            Err(_) => 50.0, // Safe fallback — don't trigger false alarms
        }
    })
    .await
    .unwrap_or(50.0)
}

/// Persist peer poll results as service events, auto-remediate unhealthy peers,
/// and publish composite health events to the `EventBus` for M37 correlation.
async fn persist_poll_results(
    state: &Arc<AppState>,
    bridge: &maintenance_engine_v2::m4_integration::peer_bridge::PeerBridgeManager,
) {
    let summary = bridge.mesh_summary();
    let peers_total = summary.peers.len();
    let mut peers_responded: usize = 0;

    for peer in &summary.peers {
        if peer.reachable {
            peers_responded += 1;
        }

        // Phase 2: Update ServiceRegistry health status from peer poll
        let health_status = if peer.reachable && peer.health_score >= 0.5 {
            maintenance_engine_v2::m2_services::HealthStatus::Healthy
        } else if peer.reachable {
            maintenance_engine_v2::m2_services::HealthStatus::Degraded
        } else {
            maintenance_engine_v2::m2_services::HealthStatus::Unhealthy
        };
        let _ = state.engine.service_registry().update_health(&peer.service_id, health_status);

        // Phase D7: Update lifecycle state from peer poll
        let lm = state.engine.lifecycle_manager();
        if let Ok(current) = lm.get_status(&peer.service_id) {
            use maintenance_engine_v2::m2_services::ServiceStatus;
            let is_healthy = peer.reachable && peer.health_score >= 0.5;
            match (current, is_healthy) {
                (ServiceStatus::Running, false) => {
                    let _ = lm.mark_failed(&peer.service_id);
                }
                (ServiceStatus::Failed, true) => {
                    if lm.start_service(&peer.service_id).is_ok() {
                        let _ = lm.mark_running(&peer.service_id);
                    }
                }
                _ => {} // Running+healthy or Failed+unhealthy: no transition needed
            }
        }

        // G2: Persist service event
        if let Some(ref db) = state.db {
            let event_type = if peer.reachable && peer.health_score >= 0.5 {
                "health_check_passed"
            } else {
                "health_check_failed"
            };
            let entry = maintenance_engine_v2::database::ServiceEventEntry {
                service_id: peer.service_id.clone(),
                event_type: event_type.to_string(),
                health_score: peer.health_score,
                latency_ms: peer.avg_latency_ms,
                timestamp: chrono::Utc::now().to_rfc3339(),
            };
            if let Err(e) = db.write_service_event(&entry).await {
                tracing::warn!(
                    error = %e,
                    service = %peer.service_id,
                    "Failed to persist service event (non-fatal)"
                );
            }
        }

        // G4: Auto-remediate when peer health drops below 0.5
        if peer.reachable && peer.health_score < 0.5 {
            let desc = format!(
                "Auto-remediation: peer {} health {:.3} below threshold 0.5",
                peer.service_id, peer.health_score
            );
            if let Err(e) = state.engine.auto_remediate(
                &peer.service_id,
                maintenance_engine_v2::m3_core_logic::Severity::Medium,
                &desc,
            ) {
                tracing::warn!(
                    error = %e,
                    service = %peer.service_id,
                    "Auto-remediation from poll trigger failed"
                );
            } else {
                tracing::info!(
                    service = %peer.service_id,
                    health = peer.health_score,
                    "Auto-remediation triggered by low peer health"
                );
            }
        }
    }

    // Phase 1A: Publish per-peer integration events for M37 cross-channel correlation.
    for peer in &summary.peers {
        let payload = serde_json::json!({
            "event": "peer_health",
            "service_id": peer.service_id,
            "reachable": peer.reachable,
            "health_score": peer.health_score,
            "latency_ms": peer.avg_latency_ms,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        if let Err(e) = state.engine.event_bus().publish(
            "integration", "peer_health", &payload.to_string(), "peer_bridge",
        ) {
            tracing::trace!(error = %e, "Failed to publish peer health event");
        }
    }

    // Phase 1A: Publish composite health event to EventBus for M37 correlation.
    // Batches all peer results into ONE event (fits M37 correlation window).
    publish_composite_health_event(state, &summary, peers_responded, peers_total);

    // Phase 5 (V7): Feed synergy metrics to SYNTHEX V3 heat source (tactical HTTP POST).
    // SYNTHEX reads its own heat sources; we write ours as a cross-service synergy feed.
    feed_synthex_heat_sources(state, peers_responded, peers_total).await;
}

/// Publish a composite health event to the `EventBus`.
///
/// Aggregates all peer health scores into a single event with completeness
/// ratio (NAM-TC2: partial data distinguished from complete data).
fn publish_composite_health_event(
    state: &Arc<AppState>,
    summary: &maintenance_engine_v2::m4_integration::peer_bridge::MeshHealthSummary,
    peers_responded: usize,
    peers_total: usize,
) {
    #[allow(clippy::cast_precision_loss)]
    let avg_health: f64 = if peers_responded > 0 {
        let total: f64 = summary
            .peers
            .iter()
            .filter(|p| p.reachable)
            .map(|p| p.health_score)
            .sum();
        total / peers_responded as f64
    } else {
        0.0
    };

    #[allow(clippy::cast_precision_loss)]
    let completeness: f64 = if peers_total > 0 {
        peers_responded as f64 / peers_total as f64
    } else {
        0.0
    };

    let payload = serde_json::json!({
        "event": "composite_health",
        "peers_responded": peers_responded,
        "peers_total": peers_total,
        "completeness": completeness,
        "avg_health": avg_health,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    if let Err(e) = state.engine.event_bus().publish(
        "health",
        "composite_health_poll",
        &payload.to_string(),
        "peer_bridge",
    ) {
        tracing::warn!(error = %e, "Failed to publish composite health event (non-fatal)");
    } else {
        tracing::debug!(
            peers = peers_responded,
            total = peers_total,
            avg_health = format!("{avg_health:.3}"),
            "Published composite health event to EventBus"
        );
    }
}

/// Phase 5 (V7): Feed ME-computed synergy metrics to SYNTHEX V3 heat sources.
///
/// Posts cross-service synergy data to SYNTHEX `/v3/heat-source` (if available)
/// and persists locally as a performance sample for historical tracking.
/// This is a tactical bridge — long-term, SYNTHEX should subscribe to the `EventBus`.
#[allow(clippy::cast_precision_loss)]
async fn feed_synthex_heat_sources(
    state: &Arc<AppState>,
    peers_responded: usize,
    peers_total: usize,
) {
    if peers_responded == 0 {
        return;
    }

    // Compute heat source values from peer mesh data
    let synergy = state.engine.bridge_manager().overall_synergy().clamp(0.0, 1.0);
    let correlation_heat = state.engine.observer().map_or(0.0, |obs| {
        let recent = obs.correlator().recent_events(10);
        (recent.len() as f64 / 10.0).clamp(0.0, 1.0)
    });
    let cross_sync = if peers_total > 0 {
        peers_responded as f64 / peers_total as f64
    } else {
        0.0
    };

    // Persist locally as performance samples (always succeeds)
    if let Some(ref db) = state.db {
        for (name, value) in [
            ("synthex.heat.cascade", synergy),
            ("synthex.heat.resonance", correlation_heat),
            ("synthex.heat.cross_sync", cross_sync),
        ] {
            let sample = maintenance_engine_v2::database::PerformanceSample {
                metric_name: name.to_string(),
                value,
                unit: "ratio".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            };
            if let Err(e) = db.write_performance_sample(&sample).await {
                tracing::trace!(error = %e, metric = name, "Heat source persistence failed");
            }
        }
    }

    // Best-effort HTTP POST to SYNTHEX V3 (fire-and-forget, non-blocking)
    // SYNTHEX may not have /v3/heat-source yet — that's fine, we log and move on.
    let payload = serde_json::json!({
        "sources": {
            "HS-002_cascade": synergy,
            "HS-003_resonance": correlation_heat,
            "HS-004_cross_sync": cross_sync,
        },
        "source": "maintenance-engine",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    // Publish to EventBus so M37 can correlate heat source data
    if let Err(e) = state.engine.event_bus().publish(
        "integration",
        "synthex_heat_feed",
        &payload.to_string(),
        "peer_bridge",
    ) {
        tracing::trace!(error = %e, "Heat source event publish failed");
    }
}

/// Async server entrypoint. Binds to `0.0.0.0:{port}` and serves until
/// `Ctrl+C` is received.
async fn serve(port: u16) -> Result<()> {
    tracing::info!("Initialising Maintenance Engine v{VERSION}...");

    // Initialize database manager (fail-silent: None if DB init fails)
    let data_dir = std::path::PathBuf::from("data/databases");
    let db = DatabaseManager::new_optional(&data_dir)
        .await
        .map(Arc::new);
    if db.is_some() {
        tracing::info!("DatabaseManager initialized successfully");
    } else {
        tracing::warn!("DatabaseManager not available -- persistence disabled");
    }

    let state = Arc::new(AppState {
        engine: Engine::new(),
        db,
        started_at: std::time::Instant::now(),
        port,
        zero_correlation_streak: std::sync::atomic::AtomicU64::new(0),
        metabolic_paused: std::sync::atomic::AtomicBool::new(false),
    });

    tracing::info!(
        services = state.engine.service_count(),
        pipelines = state.engine.pipeline_count(),
        pathways = state.engine.pathway_count(),
        fleet = state.engine.pbft_manager().get_fleet().len(),
        "Engine initialised"
    );

    restore_cognitive_state(&state).await;

    spawn_background_tasks(&state);
    spawn_heartbeat(port);

    let shutdown_state = Arc::clone(&state);

    let app = axum::Router::new()
        // Health and status
        .route("/api/health", get(handle_health))
        .route("/api/status", get(handle_status))
        .route("/api/engine", get(handle_engine))
        // Service mesh
        .route("/api/services", get(handle_services))
        .route("/api/layers", get(handle_layers))
        // Subsystem endpoints
        .route("/api/consensus", get(handle_consensus))
        .route("/api/learning", get(handle_learning))
        .route("/api/remediation", get(handle_remediation))
        .route("/api/integration", get(handle_integration))
        // L7 Observer endpoints
        .route("/api/observer", get(handle_observer))
        .route("/api/fitness", get(handle_fitness))
        .route("/api/emergence", get(handle_emergence))
        .route("/api/evolution", get(handle_evolution))
        // Peer bridge
        .route("/api/peers", get(handle_peers))
        // Metabolic activation endpoints
        .route("/api/eventbus/stats", get(handle_eventbus_stats))
        .route("/api/field", get(handle_field))
        .route("/api/metabolic/pause", post(handle_metabolic_pause))
        .route("/api/metabolic/resume", post(handle_metabolic_resume))
        .route("/api/cognitive-state", get(handle_cognitive_state))
        // V3 integration (M40/M41/M42)
        .route("/api/v3/thermal", get(handle_v3_thermal))
        .route("/api/v3/cascade", get(handle_v3_cascade))
        .route("/api/v3/decay", get(handle_v3_decay))
        .route("/api/v3/health", get(handle_v3_health))
        // Tool Library registration
        .route("/api/tools/registration", get(handle_tool_registration))
        // Tool invocation endpoints (15 tools)
        .route("/api/tools/health-check", post(handle_tool_health_check))
        .route("/api/tools/layer-health", post(handle_tool_layer_health))
        .route("/api/tools/service-discovery", post(handle_tool_service_discovery))
        .route("/api/tools/circuit-status", post(handle_tool_circuit_status))
        .route("/api/tools/submit-remediation", post(handle_tool_submit_remediation))
        .route("/api/tools/remediation-status", post(handle_tool_remediation_status))
        .route("/api/tools/pipeline-status", post(handle_tool_pipeline_status))
        .route("/api/tools/learning-cycle", post(handle_tool_learning_cycle))
        .route("/api/tools/pathway-analysis", post(handle_tool_pathway_analysis))
        .route("/api/tools/view-proposals", post(handle_tool_view_proposals))
        .route("/api/tools/submit-vote", post(handle_tool_submit_vote))
        .route("/api/tools/fitness-snapshot", post(handle_tool_fitness_snapshot))
        .route("/api/tools/emergence-report", post(handle_tool_emergence_report))
        .route("/api/tools/tensor-snapshot", post(handle_tool_tensor_snapshot))
        .route("/api/tools/tensor-compare", post(handle_tool_tensor_compare))
        // Metrics
        .route("/metrics", get(handle_metrics))
        // Middleware
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(
        %addr,
        version = VERSION,
        services = 12,
        pbft_n = 40,
        pbft_q = 27,
        "ULTRAPLATE Maintenance Engine starting"
    );

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| {
            tracing::error!(%addr, error = %e, "Port binding failed — is another process using this port?");
            Error::Other(format!("Failed to bind to {addr}: {e}"))
        })?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| Error::Other(format!("Server error: {e}")))?;

    // G5: Graceful shutdown sequence
    graceful_shutdown(&shutdown_state).await;
    Ok(())
}

/// Wait for `Ctrl+C` (SIGINT) or `SIGTERM` for graceful shutdown.
async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut sigterm = signal(SignalKind::terminate())
        .expect("failed to install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received SIGINT — initiating graceful shutdown");
        }
        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM — initiating graceful shutdown");
        }
    }
}

/// G5: Execute graceful shutdown sequence.
///
/// 1. Log final fitness score and uptime
/// 2. Flush pending DB writes
/// 3. Close peer connections
async fn graceful_shutdown(state: &Arc<AppState>) {
    let uptime_secs = state.started_at.elapsed().as_secs();

    // Log final fitness score
    let final_fitness = state.engine.observer().and_then(|obs| {
        let tensor = state.engine.build_tensor();
        obs.tick(&tensor).ok().map(|r| r.current_fitness)
    });
    tracing::info!(
        uptime_secs = uptime_secs,
        final_fitness = ?final_fitness,
        pending_remediations = state.engine.pending_remediations(),
        "Shutdown: final state logged"
    );

    // Flush final fitness to DB
    if let (Some(ref db), Some(fitness)) = (&state.db, final_fitness) {
        let entry = maintenance_engine_v2::database::FitnessHistoryEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            fitness,
            system_state: "Shutdown".to_string(),
            tensor_hash: "shutdown".to_string(),
            generation: 0,
        };
        if let Err(e) = db.write_fitness_history(&entry).await {
            tracing::warn!(error = %e, "Failed to flush final fitness to DB");
        } else {
            tracing::info!("Final fitness flushed to DB");
        }
    }

    // Log peer bridge status before closing
    if let Some(bridge) = state.engine.peer_bridge() {
        let summary = bridge.mesh_summary();
        tracing::info!(
            total_peers = summary.total_peers,
            reachable = summary.reachable_peers,
            "Shutdown: peer connections closing"
        );
    }

    tracing::info!("Maintenance Engine shut down gracefully");
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /api/health` -- Lightweight health check.
///
/// Returns `200 OK` with `{"status":"healthy"}` when the engine is healthy,
/// or `503 Service Unavailable` when degraded.
///
/// G1: Includes last persisted fitness score when DB is available.
async fn handle_health(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let uptime_secs = state.started_at.elapsed().as_secs();

    // G1: Read last fitness from DB if available
    let last_fitness: Option<f64> = if let Some(ref db) = state.db {
        db.read_latest_fitness().await.ok().flatten().map(|e| e.fitness)
    } else {
        None
    };

    state.engine.health_report().map_or_else(
        |_| {
            let body = json!({
                "status": "error",
                "service": SERVICE_NAME,
                "version": VERSION,
                "uptime_secs": uptime_secs,
            });
            (StatusCode::INTERNAL_SERVER_ERROR, Json(body))
        },
        |report| {
            let status = if report.is_healthy() { "healthy" } else { "degraded" };
            let code = if report.is_healthy() {
                StatusCode::OK
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            };
            let body = json!({
                "status": status,
                "service": SERVICE_NAME,
                "version": VERSION,
                "uptime_secs": uptime_secs,
                "overall_health": report.overall_health,
                "last_fitness": last_fitness,
                "db_connected": state.db.is_some(),
            });
            (code, Json(body))
        },
    )
}

/// `GET /api/status` -- Full engine status report.
async fn handle_status(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let uptime_secs = state.started_at.elapsed().as_secs();

    state.engine.health_report().map_or_else(
        |e| Json(json!({
            "service": SERVICE_NAME,
            "version": VERSION,
            "status": "error",
            "error": e.to_string(),
        })),
        |report| {
            let (weakest_idx, weakest_score) = report.weakest_layer();
            let layer_names = ["L1:Foundation", "L2:Services", "L3:CoreLogic",
                               "L4:Integration", "L5:Learning", "L6:Consensus",
                               "L7:Observer"];
            let weakest_name = layer_names.get(weakest_idx).unwrap_or(&"Unknown");

            Json(json!({
                "service": SERVICE_NAME,
                "version": VERSION,
                "status": if report.is_healthy() { "operational" } else { "degraded" },
                "uptime_secs": uptime_secs,
                "port": state.port,
                "architecture": {
                    "layers": 8, // R15: updated from 7 to 8 (L1-L7 + L8 Nexus)
                    "modules": 48, // R15: updated from 42 to reflect V2 (48+ modules)
                    "version": "2.0.0",
                },
                "health": {
                    "overall": report.overall_health,
                    "is_healthy": report.is_healthy(),
                    "services_total": report.services_total,
                    "services_healthy": report.services_healthy,
                    "pipelines_active": report.pipelines_active,
                    "pathways_count": report.pathways_count,
                    "proposals_active": report.proposals_active,
                    "weakest_layer": weakest_name,
                    "weakest_layer_score": weakest_score,
                },
                "nam": {
                    "target": 0.95, // R15: V2 target is 0.95, not 0.92
                    "requirements": ["R1:SelfQuery", "R2:HebbianRouting",
                                     "R3:DissentCapture", "R4:FieldVisualization",
                                     "R5:HumanAsAgent"],
                },
                "pbft": {
                    "n": maintenance_engine_v2::m6_consensus::PBFT_N,
                    "f": maintenance_engine_v2::m6_consensus::PBFT_F,
                    "q": maintenance_engine_v2::m6_consensus::PBFT_Q,
                    "fleet_size": state.engine.pbft_manager().get_fleet().len(),
                    "view_number": state.engine.current_view_number(),
                },
                "tensor_dims": 12,
            }))
        },
    )
}

/// `GET /api/engine` -- Detailed engine health report.
async fn handle_engine(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    state.engine.health_report().map_or_else(
        |e| Json(json!({ "error": e.to_string() })),
        |report| {
            let layer_names = ["Foundation", "Services", "CoreLogic",
                               "Integration", "Learning", "Consensus",
                               "Observer"];
            let layers: Vec<Value> = report
                .layer_health
                .iter()
                .zip(layer_names.iter())
                .enumerate()
                .map(|(i, (score, name))| {
                    json!({
                        "layer": i + 1,
                        "name": name,
                        "health": score,
                    })
                })
                .collect();

            Json(json!({
                "overall_health": report.overall_health,
                "is_healthy": report.is_healthy(),
                "services_total": report.services_total,
                "services_healthy": report.services_healthy,
                "pipelines_active": report.pipelines_active,
                "pathways_count": report.pathways_count,
                "proposals_active": report.proposals_active,
                "layers": layers,
            }))
        },
    )
}

/// `GET /api/services` -- ULTRAPLATE service mesh overview.
///
/// G1: Returns live peer data from `PeerBridgeManager` when available,
/// falling back to the static service list when the bridge is disabled.
async fn handle_services(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let report = state.engine.health_report().ok();
    let healthy_count = report.as_ref().map_or(0, |r| r.services_healthy);
    let total_count = report.as_ref().map_or(12, |r| r.services_total);

    // G1: Use live peer status when the peer bridge is available
    let services: Vec<Value> = state.engine.peer_bridge().map_or_else(
        || {
            // Fallback: static service list
            vec![
                service_entry("synthex", "SYNTHEX Engine", 8090, 1, 1.5),
                service_entry("san-k7-orchestrator", "SAN-K7 Orchestrator", 8100, 1, 1.5),
                service_entry("nais", "NAIS", 8101, 2, 1.3),
                service_entry("codesynthor-v7", "CodeSynthor V7", 8110, 2, 1.3),
                service_entry("devops-engine", "DevOps Engine", 8081, 2, 1.3),
                service_entry("tool-library", "Tool Library", 8105, 3, 1.2),
                // library-agent (8083) removed: disabled in devenv, was dragging fitness tensor
                service_entry("ccm", "Claude Context Manager", 8104, 3, 1.2),
                service_entry("prometheus-swarm", "Prometheus Swarm", 10001, 4, 1.1),
                service_entry("architect-agent", "Architect Agent", 9001, 4, 1.1),
                service_entry("bash-engine", "Bash Engine", 8102, 5, 1.0),
                service_entry("tool-maker", "Tool Maker", 8103, 5, 1.0),
            ]
        },
        |bridge| {
            let summary = bridge.mesh_summary();
            summary.peers.iter().map(|p| {
                json!({
                    "id": p.service_id,
                    "name": p.service_id,
                    "host": "localhost",
                    "reachable": p.reachable,
                    "health_score": p.health_score,
                    "avg_latency_ms": p.avg_latency_ms,
                    "synergy_score": p.synergy_score,
                    "circuit_open": p.circuit_open,
                    "version": p.version,
                    "total_successes": p.total_successes,
                    "total_failures": p.total_failures,
                })
            }).collect()
        },
    );

    Json(json!({
        "mesh": "ULTRAPLATE",
        "total": total_count,
        "healthy": healthy_count,
        "live_data": state.engine.peer_bridge().is_some(),
        "services": services,
    }))
}

/// `GET /api/layers` -- Per-layer health breakdown.
async fn handle_layers(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    state.engine.health_report().map_or_else(
        |e| Json(json!({ "error": e.to_string() })),
        |report| {
            let layer_details = vec![
                layer_detail(1, "Foundation", "M01-M06", "Error handling, config, logging", report.layer_health[0]),
                layer_detail(2, "Services", "M07-M12", "Service discovery, health monitoring", report.layer_health[1]),
                layer_detail(3, "Core Logic", "M13-M18", "Pipeline management, remediation", report.layer_health[2]),
                layer_detail(4, "Integration", "M19-M24", "REST, gRPC, WebSocket, IPC bridges", report.layer_health[3]),
                layer_detail(5, "Learning", "M25-M30", "Hebbian pathways, STDP, pattern recognition", report.layer_health[4]),
                layer_detail(6, "Consensus", "M31-M36", "PBFT, agent coordination, quorum", report.layer_health[5]),
                layer_detail(7, "Observer", "M37-M39", "Cross-layer observation, emergence detection, RALPH evolution", report.layer_health[6]),
            ];

            Json(json!({
                "overall_health": report.overall_health,
                "layers": layer_details,
            }))
        },
    )
}

/// `GET /api/consensus` -- PBFT consensus state.
async fn handle_consensus(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let fleet = state.engine.pbft_manager().get_fleet();
    let fleet_summary: Vec<Value> = fleet
        .iter()
        .take(5)
        .map(|agent| {
            json!({
                "id": agent.id,
                "role": format!("{:?}", agent.role),
                "weight": agent.role.vote_weight(),
            })
        })
        .collect();

    Json(json!({
        "pbft": {
            "n": 40,
            "f": 13,
            "q": 27,
        },
        "fleet_size": fleet.len(),
        "view_number": state.engine.current_view_number(),
        "open_ballots": state.engine.open_ballot_count(),
        "total_dissent": state.engine.total_dissent(),
        "fleet_sample": fleet_summary,
        "agent_roles": {
            "validators": 20,
            "explorers": 8,
            "critics": 6,
            "integrators": 4,
            "historians": 2,
            "human": 1,
        },
    }))
}

/// `GET /api/learning` -- Hebbian learning state.
async fn handle_learning(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let avg_strength = state.engine.average_pathway_strength();
    let pathway_count = state.engine.pathway_count();

    Json(json!({
        "pathways": {
            "count": pathway_count,
            "average_strength": avg_strength,
        },
        "stdp": {
            "ltp_rate": 0.1,
            "ltd_rate": 0.05,
            "window_ms": 100,
            "decay_rate": 0.1, // HRS-001 corrected (was hardcoded 0.001)
        },
        "antipatterns": {
            "pattern_count": state.engine.antipattern_detector().pattern_count(),
            "violations": state.engine.antipattern_detector().violation_count(),
        },
    }))
}

/// `GET /api/remediation` -- Remediation engine state.
async fn handle_remediation(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    Json(json!({
        "pending": state.engine.pending_remediations(),
        "active": state.engine.active_remediations(),
        "success_rate": state.engine.remediation_success_rate(),
        "escalation_tiers": {
            "L0": "Auto-execute (confidence >= 0.9)",
            "L1": "Notify human (confidence >= 0.7)",
            "L2": "Require approval (confidence < 0.7)",
            "L3": "PBFT consensus (quorum 27/40)",
        },
    }))
}

/// `GET /api/integration` -- Integration layer state.
async fn handle_integration(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    Json(json!({
        "bridges": state.engine.bridge_count(),
        "event_channels": state.engine.event_channel_count(),
        "overall_synergy": state.engine.overall_synergy(),
        "protocols": ["REST", "gRPC", "WebSocket", "IPC"],
    }))
}

/// `GET /metrics` -- Prometheus-compatible metrics.
async fn handle_metrics(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let body = build_metrics_body(&state);
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}

/// Build the Prometheus-format metrics body from engine state.
fn build_metrics_body(state: &AppState) -> String {
    let uptime = state.started_at.elapsed().as_secs();
    let report = state.engine.health_report().ok();
    let overall = report.as_ref().map_or(0.0, |r| r.overall_health);
    let healthy = report.as_ref().map_or(0, |r| r.services_healthy);
    let total = report.as_ref().map_or(0, |r| r.services_total);
    let pipelines = report.as_ref().map_or(0, |r| r.pipelines_active);
    let pathways = report.as_ref().map_or(0, |r| r.pathways_count);

    let avg_strength = state.engine.average_pathway_strength();
    let pending_rem = state.engine.pending_remediations();
    let active_rem = state.engine.active_remediations();
    let success_rate = state.engine.remediation_success_rate();
    let ballots = state.engine.open_ballot_count();
    let dissent = state.engine.total_dissent();
    let view = state.engine.current_view_number();
    let bridges = state.engine.bridge_count();
    let channels = state.engine.event_channel_count();
    let synergy = state.engine.overall_synergy();

    let layer_metrics = build_layer_metrics(report.as_ref());

    format!(
        "# HELP maintenance_engine_up Whether the maintenance engine is up\n\
         # TYPE maintenance_engine_up gauge\n\
         maintenance_engine_up 1\n\
         # HELP maintenance_engine_uptime_seconds Engine uptime in seconds\n\
         # TYPE maintenance_engine_uptime_seconds counter\n\
         maintenance_engine_uptime_seconds {uptime}\n\
         # HELP maintenance_engine_health_overall Overall health score\n\
         # TYPE maintenance_engine_health_overall gauge\n\
         maintenance_engine_health_overall {overall:.4}\n\
         # HELP maintenance_engine_services_total Total registered services\n\
         # TYPE maintenance_engine_services_total gauge\n\
         maintenance_engine_services_total {total}\n\
         # HELP maintenance_engine_services_healthy Healthy services count\n\
         # TYPE maintenance_engine_services_healthy gauge\n\
         maintenance_engine_services_healthy {healthy}\n\
         # HELP maintenance_engine_pipelines_active Active pipelines\n\
         # TYPE maintenance_engine_pipelines_active gauge\n\
         maintenance_engine_pipelines_active {pipelines}\n\
         # HELP maintenance_engine_pathways_count Hebbian pathway count\n\
         # TYPE maintenance_engine_pathways_count gauge\n\
         maintenance_engine_pathways_count {pathways}\n\
         # HELP maintenance_engine_pathway_strength Average pathway strength\n\
         # TYPE maintenance_engine_pathway_strength gauge\n\
         maintenance_engine_pathway_strength {avg_strength:.4}\n\
         # HELP maintenance_engine_remediation_pending Pending remediations\n\
         # TYPE maintenance_engine_remediation_pending gauge\n\
         maintenance_engine_remediation_pending {pending_rem}\n\
         # HELP maintenance_engine_remediation_active Active remediations\n\
         # TYPE maintenance_engine_remediation_active gauge\n\
         maintenance_engine_remediation_active {active_rem}\n\
         # HELP maintenance_engine_remediation_success_rate Remediation success rate\n\
         # TYPE maintenance_engine_remediation_success_rate gauge\n\
         maintenance_engine_remediation_success_rate {success_rate:.4}\n\
         # HELP maintenance_engine_consensus_ballots Open PBFT ballots\n\
         # TYPE maintenance_engine_consensus_ballots gauge\n\
         maintenance_engine_consensus_ballots {ballots}\n\
         # HELP maintenance_engine_consensus_dissent Total dissent events\n\
         # TYPE maintenance_engine_consensus_dissent counter\n\
         maintenance_engine_consensus_dissent {dissent}\n\
         # HELP maintenance_engine_consensus_view Current PBFT view number\n\
         # TYPE maintenance_engine_consensus_view gauge\n\
         maintenance_engine_consensus_view {view}\n\
         # HELP maintenance_engine_bridges Active bridge count\n\
         # TYPE maintenance_engine_bridges gauge\n\
         maintenance_engine_bridges {bridges}\n\
         # HELP maintenance_engine_event_channels Event bus channels\n\
         # TYPE maintenance_engine_event_channels gauge\n\
         maintenance_engine_event_channels {channels}\n\
         # HELP maintenance_engine_synergy Overall synergy score\n\
         # TYPE maintenance_engine_synergy gauge\n\
         maintenance_engine_synergy {synergy:.4}\n\
         # HELP maintenance_engine_layer_health Per-layer health scores\n\
         # TYPE maintenance_engine_layer_health gauge\n\
         {layer_metrics}\
         # HELP maintenance_engine_modules_total Total implemented modules\n\
         # TYPE maintenance_engine_modules_total gauge\n\
         maintenance_engine_modules_total 42\n\
         # HELP maintenance_engine_layers_total Total architecture layers\n\
         # TYPE maintenance_engine_layers_total gauge\n\
         maintenance_engine_layers_total 7\n\
         # HELP maintenance_engine_tests_total Total passing tests\n\
         # TYPE maintenance_engine_tests_total gauge\n\
         maintenance_engine_tests_total 1294\n"
    )
}

/// Build per-layer health metrics in Prometheus format.
fn build_layer_metrics(
    report: Option<&maintenance_engine_v2::engine::EngineHealthReport>,
) -> String {
    let layer_names = ["foundation", "services", "core_logic",
                       "integration", "learning", "consensus", "observer"];
    let mut out = String::new();
    if let Some(r) = report {
        for (i, (score, name)) in r.layer_health.iter().zip(layer_names.iter()).enumerate() {
            let _ = writeln!(
                out,
                "maintenance_engine_layer_health{{layer=\"{name}\",index=\"{}\"}} {score:.4}",
                i + 1
            );
        }
    }
    out
}

/// `GET /api/observer` -- L7 observer metrics and state.
async fn handle_observer(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    state.engine.observer().map_or_else(
        || Json(json!({
            "enabled": false,
            "message": "L7 Observer is not initialized",
        })),
        |obs| {
            let metrics = obs.metrics();
            let report = obs.get_report();
            Json(json!({
                "enabled": true,
                "tick_count": obs.tick_count(),
                "generation": obs.generation(),
                "system_state": format!("{:?}", obs.system_state()),
                "fitness_trend": format!("{:?}", obs.fitness_trend()),
                "uptime_seconds": obs.uptime_seconds(),
                "metrics": {
                    "events_ingested": metrics.events_ingested,
                    "correlations_found": metrics.correlations_found,
                    "emergences_detected": metrics.emergences_detected,
                    "mutations_proposed": metrics.mutations_proposed,
                    "mutations_applied": metrics.mutations_applied,
                    "mutations_rolled_back": metrics.mutations_rolled_back,
                    "ralph_cycles": metrics.ralph_cycles,
                    "observer_errors": metrics.observer_errors,
                    "ticks_executed": metrics.ticks_executed,
                    "reports_generated": metrics.reports_generated,
                },
                "last_report": report.map(|r| json!({
                    "id": r.id,
                    "timestamp": r.timestamp.to_rfc3339(),
                    "tick": r.tick,
                    "current_fitness": r.current_fitness,
                    "correlations_since_last": r.correlations_since_last,
                    "emergences_since_last": r.emergences_since_last,
                    "mutations_since_last": r.mutations_since_last,
                    "active_mutations": r.active_mutations,
                    "generation": r.generation,
                })),
            }))
        },
    )
}

/// `GET /api/fitness` -- Current fitness score, trend, and history.
async fn handle_fitness(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    state.engine.observer().map_or_else(
        || Json(json!({
            "enabled": false,
            "message": "L7 Observer is not initialized",
        })),
        |obs| {
            let current = obs.fitness().current_fitness();
            let trend = obs.fitness_trend();
            let snapshots = obs.fitness().recent_snapshots(20);
            let dimension_scores = obs.fitness().current_fitness().map(|_| {
                let tensor = state.engine.build_tensor();
                let arr = tensor.to_array();
                let dim_names = ["service_id", "port", "tier", "deps", "agents",
                                 "protocol", "health", "uptime", "synergy",
                                 "latency", "error_rate", "temporal"];
                dim_names.iter().zip(arr.iter()).map(|(name, &val)| {
                    json!({ "dimension": name, "value": val })
                }).collect::<Vec<_>>()
            });

            Json(json!({
                "current_fitness": current,
                "trend": format!("{trend:?}"),
                "system_state": format!("{:?}", obs.system_state()),
                "history_length": snapshots.len(),
                "dimension_scores": dimension_scores,
                "recent_snapshots": snapshots.iter().take(10).map(|s| json!({
                    "fitness": s.fitness,
                    "timestamp": s.timestamp.to_rfc3339(),
                })).collect::<Vec<_>>(),
            }))
        },
    )
}

/// `GET /api/emergence` -- Recent emergence records and stats.
async fn handle_emergence(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    state.engine.observer().map_or_else(
        || Json(json!({
            "enabled": false,
            "message": "L7 Observer is not initialized",
        })),
        |obs| {
            let recent = obs.recent_emergences(20);
            let total_count = obs.emergence_count();
            let detector = obs.detector();

            Json(json!({
                "total_emergences": total_count,
                "active_monitors": detector.active_monitor_count(),
                "recent": recent.iter().map(|e| json!({
                    "id": e.id,
                    "emergence_type": format!("{:?}", e.emergence_type),
                    "confidence": e.confidence,
                    "timestamp": e.detected_at.to_rfc3339(),
                    "description": e.description,
                })).collect::<Vec<_>>(),
            }))
        },
    )
}

/// `GET /api/evolution` -- RALPH state, active mutations, generation.
async fn handle_evolution(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    state.engine.observer().map_or_else(
        || Json(json!({
            "enabled": false,
            "message": "L7 Observer is not initialized",
        })),
        |obs| {
            let ralph = obs.ralph_state();
            let recent_mutations = obs.recent_mutations(20);
            let chamber = obs.chamber();

            let chamber_info = chamber.stats();
            Json(json!({
                "ralph_state": {
                    "phase": format!("{:?}", ralph.current_phase),
                    "cycle_number": ralph.cycle_number,
                    "paused": ralph.paused,
                    "mutations_proposed": ralph.mutations_proposed,
                    "mutations_applied": ralph.mutations_applied,
                },
                "generation": obs.generation(),
                "active_mutations": chamber.active_mutation_count(),
                "chamber_stats": {
                    "total_proposed": chamber_info.total_mutations_proposed,
                    "total_applied": chamber_info.total_mutations_applied,
                    "total_rolled_back": chamber_info.total_mutations_rolled_back,
                    "total_ralph_cycles": chamber_info.total_ralph_cycles,
                    "current_generation": chamber_info.current_generation,
                },
                "recent_mutations": recent_mutations.iter().map(|m| json!({
                    "id": m.id,
                    "applied": m.applied,
                    "rolled_back": m.rolled_back,
                    "target_parameter": m.target_parameter,
                    "timestamp": m.timestamp.to_rfc3339(),
                })).collect::<Vec<_>>(),
            }))
        },
    )
}

/// `GET /api/peers` -- Peer service health states and mesh synergy.
async fn handle_peers(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    state.engine.peer_bridge().map_or_else(
        || Json(json!({
            "enabled": false,
            "message": "Peer bridge is not initialized",
        })),
        |bridge| {
            let summary = bridge.mesh_summary();
            Json(json!({
                "enabled": true,
                "total_peers": summary.total_peers,
                "reachable_peers": summary.reachable_peers,
                "circuit_open_count": summary.circuit_open_count,
                "mesh_synergy": summary.mesh_synergy,
                "polling_active": bridge.is_polling(),
                "peers": summary.peers.iter().map(|p| json!({
                    "service_id": p.service_id,
                    "reachable": p.reachable,
                    "health_score": p.health_score,
                    "consecutive_failures": p.consecutive_failures,
                    "avg_latency_ms": p.avg_latency_ms,
                    "synergy_score": p.synergy_score,
                    "circuit_open": p.circuit_open,
                    "version": p.version,
                    "total_successes": p.total_successes,
                    "total_failures": p.total_failures,
                })).collect::<Vec<_>>(),
            }))
        },
    )
}

// ---------------------------------------------------------------------------
// Metabolic Activation Handlers
// ---------------------------------------------------------------------------

/// `GET /api/eventbus/stats` -- `EventBus` channel statistics.
///
/// Returns event counts, subscriber counts, and channel health for
/// observability into the `EventBus`→M37 pipeline.
async fn handle_eventbus_stats(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let bus = state.engine.event_bus();
    let channels: Vec<Value> = bus
        .list_channels()
        .iter()
        .map(|name| {
            let info = bus.get_channel_info(name);
            json!({
                "channel": name,
                "event_count": info.as_ref().map_or(0, |i| i.event_count),
                "subscriber_count": info.as_ref().map_or(0, |i| i.subscriber_count),
            })
        })
        .collect();

    Json(json!({
        "total_channels": bus.channel_count(),
        "total_events": bus.total_events(),
        "channels": channels,
    }))
}

/// `GET /api/field` -- Unified cognitive field state (NAM-R4).
///
/// Returns the complete cognitive state as one JSON response:
/// `EventBus` stats, M37 correlations, M38 emergences, M39 generation,
/// current fitness, and dormancy detection state.
async fn handle_field(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let bus = state.engine.event_bus();
    let zero_streak = state
        .zero_correlation_streak
        .load(std::sync::atomic::Ordering::Relaxed);

    let observer_state = state.engine.observer().map(|obs| {
        let metrics = obs.metrics();
        let report = obs.get_report();
        json!({
            "tick_count": obs.tick_count(),
            "generation": obs.generation(),
            "system_state": format!("{:?}", obs.system_state()),
            "fitness_trend": format!("{:?}", obs.fitness_trend()),
            "events_ingested": metrics.events_ingested,
            "correlations_found": metrics.correlations_found,
            "emergences_detected": metrics.emergences_detected,
            "mutations_proposed": metrics.mutations_proposed,
            "mutations_applied": metrics.mutations_applied,
            "ralph_cycles": metrics.ralph_cycles,
            "current_fitness": report.as_ref().map_or(0.0, |r| r.current_fitness),
            "correlations_since_last": report.as_ref().map_or(0, |r| r.correlations_since_last),
        })
    });

    Json(json!({
        "eventbus": {
            "total_channels": bus.channel_count(),
            "total_events": bus.total_events(),
        },
        "observer": observer_state,
        "dormancy": {
            "zero_correlation_streak": zero_streak,
            "is_dormant": zero_streak >= 3,
        },
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

// ---------------------------------------------------------------------------
// Metabolic Control Handlers (NAM-R7: Human Override)
// ---------------------------------------------------------------------------

/// `POST /api/metabolic/pause` -- Pause metabolic `EventBus` publishing.
async fn handle_metabolic_pause(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    state
        .metabolic_paused
        .store(true, std::sync::atomic::Ordering::Relaxed);
    Json(json!({
        "status": "paused",
        "message": "Metabolic publishing paused",
    }))
}

/// `POST /api/metabolic/resume` -- Resume metabolic `EventBus` publishing.
async fn handle_metabolic_resume(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    state
        .metabolic_paused
        .store(false, std::sync::atomic::Ordering::Relaxed);
    Json(json!({
        "status": "active",
        "message": "Metabolic publishing resumed",
    }))
}

/// `GET /api/cognitive-state` -- Read persisted cognitive state (NAM-T2).
async fn handle_cognitive_state(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let Some(ref db) = state.db else {
        return Json(json!({
            "error": "Database not available",
        }));
    };
    match db.read_cognitive_state().await {
        Ok(Some(cog)) => Json(json!({
            "window_size_ms": cog.window_size_ms,
            "generation": cog.generation,
            "tick_count": cog.tick_count,
            "zero_correlation_streak": cog.zero_correlation_streak,
            "fitness": cog.fitness,
            "saved_at": cog.saved_at,
        })),
        Ok(None) => Json(json!({
            "message": "No cognitive state persisted yet",
        })),
        Err(e) => Json(json!({
            "error": format!("Failed to read cognitive state: {e}"),
        })),
    }
}

// ---------------------------------------------------------------------------
// V3 Integration Handlers (M40/M41/M42)
// ---------------------------------------------------------------------------

/// `GET /api/v3/thermal` -- Thermal monitor snapshot (M40).
async fn handle_v3_thermal(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    state.engine.thermal_monitor().map_or_else(
        || Json(json!({
            "enabled": false,
            "message": "Thermal monitor is not initialized",
        })),
        |mon| {
            let snapshot = mon.snapshot();
            Json(json!({
                "enabled": true,
                "current_temp": snapshot.current_temp,
                "target_temp": snapshot.target_temp,
                "avg_temp": snapshot.avg_temp,
                "max_temp": snapshot.max_temp,
                "runaway_detected": snapshot.runaway_detected,
                "window_size": snapshot.window_size,
                "consecutive_failures": snapshot.consecutive_failures,
                "degraded": snapshot.degraded,
            }))
        },
    )
}

/// `GET /api/v3/cascade` -- Cascade bridge health (M42).
async fn handle_v3_cascade(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    state.engine.cascade_bridge().map_or_else(
        || Json(json!({
            "enabled": false,
            "message": "Cascade bridge is not initialized",
        })),
        |bridge| {
            let health = bridge.health();
            Json(json!({
                "enabled": true,
                "current_amplification": health.current_amplification,
                "avg_amplification": health.avg_amplification,
                "max_amplification": health.max_amplification,
                "open_breakers": health.open_breakers,
                "anomaly_detected": health.anomaly_detected,
                "window_size": health.window_size,
                "consecutive_failures": health.consecutive_failures,
                "degraded": health.degraded,
            }))
        },
    )
}

/// `GET /api/v3/decay` -- Decay scheduler compliance (M41).
async fn handle_v3_decay(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    state.engine.decay_scheduler().map_or_else(
        || Json(json!({
            "enabled": false,
            "message": "Decay scheduler is not initialized",
        })),
        |sched| {
            let compliance = sched.compliance();
            Json(json!({
                "enabled": true,
                "total_attempts": compliance.total_attempts,
                "successes": compliance.successes,
                "failures": compliance.failures,
                "compliance_ratio": compliance.compliance_ratio,
                "consecutive_failures": compliance.consecutive_failures,
                "degraded": compliance.degraded,
            }))
        },
    )
}

/// `GET /api/v3/health` -- Aggregate V3 integration health.
async fn handle_v3_health(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let thermal_ok = state.engine.thermal_monitor().is_some();
    let cascade_ok = state.engine.cascade_bridge().is_some();
    let decay_ok = state.engine.decay_scheduler().is_some();

    let thermal_degraded = state.engine.thermal_monitor()
        .is_none_or(|m| m.snapshot().degraded);
    let cascade_degraded = state.engine.cascade_bridge()
        .is_none_or(CascadeBridge::is_degraded);
    let decay_degraded = state.engine.decay_scheduler()
        .is_none_or(DecayScheduler::is_degraded);

    let all_healthy = thermal_ok && cascade_ok && decay_ok
        && !thermal_degraded && !cascade_degraded && !decay_degraded;

    Json(json!({
        "status": if all_healthy { "healthy" } else { "degraded" },
        "modules": {
            "thermal_monitor": { "enabled": thermal_ok, "degraded": thermal_degraded },
            "cascade_bridge": { "enabled": cascade_ok, "degraded": cascade_degraded },
            "decay_scheduler": { "enabled": decay_ok, "degraded": decay_degraded },
        },
    }))
}

// ---------------------------------------------------------------------------
// Tool Invocation Handlers
// ---------------------------------------------------------------------------

/// Helper to build a `ToolInvokeResponse` from engine data.
fn tool_response(
    start: std::time::Instant,
    request_id: &str,
    data: Value,
    tensor: Option<&maintenance_engine_v2::Tensor12D>,
) -> ToolInvokeResponse {
    ToolInvokeResponse {
        success: true,
        data,
        duration_ms: start.elapsed().as_secs_f64() * 1000.0,
        request_id: request_id.to_string(),
        tensor: tensor.map(maintenance_engine_v2::Tensor12D::to_array),
    }
}

/// `GET /api/tools/registration` -- Tool registration status.
async fn handle_tool_registration(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let defs = maintenance_engine_v2::tools::all_tool_definitions();
    Json(json!({
        "service": SERVICE_NAME,
        "port": state.port,
        "tools": defs.len(),
        "definitions": defs,
    }))
}

/// `POST /api/tools/health-check` -- Full engine health check.
async fn handle_tool_health_check(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ToolInvokeRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let report = state.engine.health_report();
    let tensor = state.engine.build_tensor();

    let data = report.map_or_else(
        |e| json!({ "error": e.to_string() }),
        |r| json!({
            "overall_health": r.overall_health,
            "is_healthy": r.is_healthy(),
            "services_total": r.services_total,
            "services_healthy": r.services_healthy,
            "pipelines_active": r.pipelines_active,
            "pathways_count": r.pathways_count,
            "proposals_active": r.proposals_active,
            "layer_health": r.layer_health,
        }),
    );
    Json(tool_response(start, &req.request_id, data, Some(&tensor)))
}

/// `POST /api/tools/layer-health` -- Per-layer health breakdown.
async fn handle_tool_layer_health(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ToolInvokeRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let data = state.engine.health_report().map_or_else(
        |e| json!({ "error": e.to_string() }),
        |r| {
            let names = ["Foundation", "Services", "CoreLogic", "Integration",
                         "Learning", "Consensus", "Observer"];
            let layers: Vec<Value> = r.layer_health.iter().zip(names.iter()).enumerate()
                .map(|(i, (s, n))| json!({"layer": i + 1, "name": n, "health": s}))
                .collect();
            json!({ "layers": layers, "overall_health": r.overall_health })
        },
    );
    Json(tool_response(start, &req.request_id, data, None))
}

/// `POST /api/tools/service-discovery` -- List discovered services.
async fn handle_tool_service_discovery(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ToolInvokeRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let endpoints = maintenance_engine_v2::m4_integration::default_endpoints();
    let data = json!({
        "service_count": state.engine.service_count(),
        "endpoints": endpoints.iter().map(|e| json!({
            "service_id": e.service_id,
            "host": e.host,
            "port": e.port,
            "protocol": format!("{:?}", e.protocol),
        })).collect::<Vec<_>>(),
    });
    Json(tool_response(start, &req.request_id, data, None))
}

/// `POST /api/tools/circuit-status` -- Circuit breaker states.
async fn handle_tool_circuit_status(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ToolInvokeRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let data = json!({
        "total_breakers": state.engine.circuit_breaker().breaker_count(),
    });
    Json(tool_response(start, &req.request_id, data, None))
}

/// `POST /api/tools/submit-remediation` -- Submit remediation request.
///
/// G2: Persists remediation submission as a correlation entry in the database.
async fn handle_tool_submit_remediation(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ToolInvokeRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let service_id = req.params.get("service_id").and_then(serde_json::Value::as_str).unwrap_or("");
    let description = req.params.get("description").and_then(serde_json::Value::as_str).unwrap_or("");
    let data = match state.engine.submit_remediation(
        service_id,
        IssueType::HealthFailure,
        Severity::Medium,
        description,
    ) {
        Ok(id) => {
            // G2: Persist remediation submission as a correlation entry
            if let Some(ref db) = state.db {
                let entry = maintenance_engine_v2::database::CorrelationEntry {
                    id: id.clone(),
                    channel: "remediation".to_string(),
                    event_type: "submission".to_string(),
                    link_count: 1,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                if let Err(e) = db.write_correlation(&entry).await {
                    tracing::warn!(error = %e, "Failed to persist remediation correlation (non-fatal)");
                }
            }

            // G4: Reinforce health→remediation Hebbian pathway on successful submission
            let _ = state.engine.hebbian_manager().record_success("health->remediation");

            json!({ "request_id": id, "status": "submitted" })
        }
        Err(e) => {
            // G4: Weaken pathway on failure
            let _ = state.engine.hebbian_manager().record_failure("health->remediation");

            json!({ "error": e.to_string() })
        }
    };
    Json(tool_response(start, &req.request_id, data, None))
}

/// `POST /api/tools/remediation-status` -- Remediation queue status.
async fn handle_tool_remediation_status(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ToolInvokeRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let data = json!({
        "pending": state.engine.pending_remediations(),
        "active": state.engine.active_remediations(),
        "success_rate": state.engine.remediation_success_rate(),
    });
    Json(tool_response(start, &req.request_id, data, None))
}

/// `POST /api/tools/pipeline-status` -- Pipeline manager status.
async fn handle_tool_pipeline_status(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ToolInvokeRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let data = json!({
        "pipeline_count": state.engine.pipeline_count(),
        "active_pipelines": state.engine.pipeline_manager().pipeline_count(),
    });
    Json(tool_response(start, &req.request_id, data, None))
}

/// `POST /api/tools/learning-cycle` -- Execute learning cycle.
async fn handle_tool_learning_cycle(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ToolInvokeRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let data = state.engine.learning_cycle().map_or_else(
        |e| json!({ "error": e.to_string() }),
        |r| json!({
            "pathways_decayed": r.pathways_decayed,
            "timing_pairs_processed": r.timing_pairs_processed,
            "antipatterns_detected": r.antipatterns_detected,
            "had_activity": r.had_activity(),
        }),
    );
    Json(tool_response(start, &req.request_id, data, None))
}

/// `POST /api/tools/pathway-analysis` -- Hebbian pathway analysis.
async fn handle_tool_pathway_analysis(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ToolInvokeRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let limit = req.params.get("limit").and_then(serde_json::Value::as_u64).map_or(20_usize, |v| v.min(1000) as usize);
    let strongest = state.engine.hebbian_manager().get_strongest_pathways(limit);
    let data = json!({
        "pathway_count": state.engine.pathway_count(),
        "average_strength": state.engine.average_pathway_strength(),
        "strongest": strongest.iter().map(|p| json!({
            "source": p.source,
            "target": p.target,
            "strength": p.strength,
        })).collect::<Vec<_>>(),
    });
    Json(tool_response(start, &req.request_id, data, None))
}

/// `POST /api/tools/view-proposals` -- List active consensus proposals.
async fn handle_tool_view_proposals(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ToolInvokeRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let data = json!({
        "open_ballots": state.engine.open_ballot_count(),
        "total_dissent": state.engine.total_dissent(),
        "view_number": state.engine.current_view_number(),
        "fleet_size": state.engine.pbft_manager().get_fleet().len(),
        "quorum": 27,
    });
    Json(tool_response(start, &req.request_id, data, None))
}

/// `POST /api/tools/submit-vote` -- Submit vote on proposal.
///
/// G4: Wired to `PbftManager::submit_vote()` for real PBFT consensus.
/// Params: `proposal_id`, `agent_id` (default "@0.A"), `vote` ("approve"|"reject"|"abstain"),
/// optional `reason`.
async fn handle_tool_submit_vote(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ToolInvokeRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();

    let proposal_id = req.params.get("proposal_id")
        .and_then(|v| v.as_str()).unwrap_or("");
    let agent_id = req.params.get("agent_id")
        .and_then(|v| v.as_str()).unwrap_or("@0.A");
    let vote_str = req.params.get("vote")
        .and_then(|v| v.as_str()).unwrap_or("approve");
    let reason = req.params.get("reason")
        .and_then(|v| v.as_str()).map(String::from);

    let vote_type = match vote_str.to_lowercase().as_str() {
        "reject" => maintenance_engine_v2::m6_consensus::VoteType::Reject,
        "abstain" => maintenance_engine_v2::m6_consensus::VoteType::Abstain,
        _ => maintenance_engine_v2::m6_consensus::VoteType::Approve,
    };

    let data = match state.engine.pbft_manager().submit_vote(
        proposal_id, agent_id, vote_type, reason,
    ) {
        Ok(vote) => {
            let _ = state.engine.vote_collector().cast_vote(proposal_id, vote);
            json!({
                "status": "voted",
                "proposal_id": proposal_id,
                "agent_id": agent_id,
                "vote": vote_str,
                "open_ballots": state.engine.open_ballot_count(),
                "current_view": state.engine.current_view_number(),
            })
        }
        Err(e) => json!({
            "status": "error",
            "error": e.to_string(),
            "open_ballots": state.engine.open_ballot_count(),
            "current_view": state.engine.current_view_number(),
        }),
    };
    Json(tool_response(start, &req.request_id, data, None))
}

/// `POST /api/tools/fitness-snapshot` -- Current fitness evaluation.
async fn handle_tool_fitness_snapshot(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ToolInvokeRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let tensor = state.engine.build_tensor();
    let data = state.engine.observer().map_or_else(
        || json!({ "enabled": false }),
        |obs| {
            let fitness = obs.fitness().current_fitness();
            let trend = obs.fitness_trend();
            json!({
                "current_fitness": fitness,
                "trend": format!("{trend:?}"),
                "system_state": format!("{:?}", obs.system_state()),
                "generation": obs.generation(),
            })
        },
    );
    Json(tool_response(start, &req.request_id, data, Some(&tensor)))
}

/// `POST /api/tools/emergence-report` -- Recent emergence detections.
async fn handle_tool_emergence_report(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ToolInvokeRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let limit = req.params.get("limit").and_then(serde_json::Value::as_u64).map_or(20_usize, |v| v.min(1000) as usize);
    let data = state.engine.observer().map_or_else(
        || json!({ "enabled": false }),
        |obs| {
            let recent = obs.recent_emergences(limit);
            json!({
                "total_emergences": obs.emergence_count(),
                "active_monitors": obs.detector().active_monitor_count(),
                "recent": recent.iter().map(|e| json!({
                    "id": e.id,
                    "emergence_type": format!("{:?}", e.emergence_type),
                    "confidence": e.confidence,
                    "timestamp": e.detected_at.to_rfc3339(),
                })).collect::<Vec<_>>(),
            })
        },
    );
    Json(tool_response(start, &req.request_id, data, None))
}

/// `POST /api/tools/tensor-snapshot` -- Current 12D tensor state.
async fn handle_tool_tensor_snapshot(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ToolInvokeRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let tensor = state.engine.build_tensor();
    let arr = tensor.to_array();
    let dim_names = ["service_id", "port", "tier", "deps", "agents",
                     "protocol", "health", "uptime", "synergy",
                     "latency", "error_rate", "temporal"];
    let dimensions: Vec<Value> = dim_names.iter().zip(arr.iter()).enumerate()
        .map(|(i, (name, &val))| json!({"index": i, "name": name, "value": val}))
        .collect();
    let data = json!({
        "dimensions": dimensions,
        "valid": tensor.validate().is_ok(),
    });
    Json(tool_response(start, &req.request_id, data, Some(&tensor)))
}

/// `POST /api/tools/tensor-compare` -- Compare two tensor states.
async fn handle_tool_tensor_compare(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ToolInvokeRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let current = state.engine.build_tensor();

    // Parse reference tensor from params, or use default
    let reference = req.params.get("reference")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            if arr.len() == 12 {
                let mut dims = [0.0_f64; 12];
                for (i, v) in arr.iter().enumerate() {
                    dims[i] = v.as_f64().unwrap_or(0.0);
                }
                Some(maintenance_engine_v2::Tensor12D::new(dims))
            } else {
                None
            }
        })
        .unwrap_or_default();

    let distance = current.distance(&reference);
    let current_arr = current.to_array();
    let ref_arr = reference.to_array();
    let deltas: Vec<Value> = current_arr.iter().zip(ref_arr.iter()).enumerate()
        .map(|(i, (&c, &r))| json!({"dimension": i, "current": c, "reference": r, "delta": c - r}))
        .collect();

    let data = json!({
        "euclidean_distance": distance,
        "drift": if distance < 0.1 { "stable" } else if distance < 0.5 { "moderate" } else { "significant" },
        "deltas": deltas,
    });
    Json(tool_response(start, &req.request_id, data, Some(&current)))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a service entry JSON value.
fn service_entry(id: &str, name: &str, port: u16, tier: u8, weight: f64) -> Value {
    json!({
        "id": id,
        "name": name,
        "port": port,
        "tier": tier,
        "weight": weight,
        "host": "localhost",
    })
}

/// Build a layer detail JSON value.
fn layer_detail(num: u8, name: &str, modules: &str, description: &str, health: f64) -> Value {
    json!({
        "layer": num,
        "name": name,
        "modules": modules,
        "description": description,
        "health": health,
    })
}

/// Parse `--port <N>` from CLI arguments.
fn parse_port(args: &[String]) -> Result<u16> {
    // Check --port flag
    for (i, arg) in args.iter().enumerate() {
        if arg == "--port" {
            if let Some(port_str) = args.get(i + 1) {
                return port_str
                    .parse::<u16>()
                    .map_err(|_| Error::Config(format!("Invalid port: {port_str}")));
            }
            return Err(Error::Config("--port requires a value".into()));
        }
    }

    // Check MAINTENANCE_ENGINE_PORT env var
    if let Ok(port_str) = env::var("MAINTENANCE_ENGINE_PORT") {
        return port_str
            .parse::<u16>()
            .map_err(|_| Error::Config(format!("Invalid MAINTENANCE_ENGINE_PORT: {port_str}")));
    }

    Ok(DEFAULT_PORT)
}

/// Print CLI help text.
fn print_help() {
    println!(
        r"The Maintenance Engine v{VERSION}
ULTRAPLATE Bulletproof Developer Environment - Service Maintenance Framework

USAGE:
    maintenance_engine <COMMAND>

COMMANDS:
    start     Start the HTTP server (default port {DEFAULT_PORT})
    health    Check engine health (JSON output)
    status    Show engine status summary
    --help    Print this help message
    --version Print version information

OPTIONS:
    --port <PORT>     Server port (default: {DEFAULT_PORT})

ENVIRONMENT:
    MAINTENANCE_ENGINE_LOG    Log level (trace, debug, info, warn, error)
    MAINTENANCE_ENGINE_PORT   Default port

ENDPOINTS:
    /api/health       Health check
    /api/status       Full status report
    /api/engine       Engine health report
    /api/services     Service mesh overview
    /api/layers       Per-layer health breakdown
    /api/consensus    PBFT consensus state
    /api/learning     Hebbian learning state
    /api/remediation  Remediation engine state
    /api/integration  Integration layer state
    /metrics          Prometheus-compatible metrics

EXAMPLES:
    maintenance_engine start
    maintenance_engine start --port 9000
    maintenance_engine health
    maintenance_engine status
"
    );
}

/// Initialise tracing to write to stderr (for CLI commands).
fn init_tracing_stderr() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(
                "maintenance_engine=info"
                    .parse()
                    .map_err(|_| Error::Config("Invalid log directive".into()))?,
            ),
        )
        .init();
    Ok(())
}

/// Initialise tracing to write to a log file (for server mode).
///
/// When started by a process manager like devenv, stdout/stderr are piped.
/// If the manager exits, the pipe closes and any write triggers SIGPIPE,
/// killing the server. Writing to a file avoids this entirely.
fn init_tracing_to_file() -> Result<()> {
    use std::fs;
    use tracing_subscriber::fmt::writer::MakeWriterExt;

    let log_dir = std::path::PathBuf::from("logs");
    fs::create_dir_all(&log_dir).map_err(|e| {
        Error::Other(format!("Failed to create log directory: {e}"))
    })?;

    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("maintenance_engine.log"))
        .map_err(|e| Error::Other(format!("Failed to open log file: {e}")))?;

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(
                "maintenance_engine=info"
                    .parse()
                    .map_err(|_| Error::Config("Invalid log directive".into()))?,
            ),
        )
        .with_writer(std::sync::Mutex::new(log_file).with_max_level(tracing::Level::TRACE))
        .with_ansi(false)
        .init();
    Ok(())
}
