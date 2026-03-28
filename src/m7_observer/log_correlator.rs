//! # M37: Log Correlator
//!
//! Cross-layer event correlation engine for the L7 Observer Layer.
//! Subscribes to all 6 `EventBus` channels and detects temporal, causal,
//! semantic, and recurring correlations between events from different
//! layers (L1-L6).
//!
//! ## Layer: L7 (Observer)
//! ## Lock Order: 2 (after `ObserverBus`, before `EmergenceDetector`)
//! ## Dependencies: M23 (`EventBus`), M01 (Error)
//!
//! ## Correlation Types
//!
//! | Type | Formula |
//! |------|---------|
//! | Temporal | `1.0 - (\|delta_ms\| / temporal_tolerance_ms)` |
//! | Causal | `0.8 * (1.0 - delta_ms / window_size_ms)` |
//! | Semantic | `layers_count / 6.0` |
//! | Recurring | `1.0 - (stddev / mean)` |
//!
//! ## Related Documentation
//! - [Log Correlator Spec](../../ai_specs/evolution_chamber_ai_specs/LOG_CORRELATOR_SPEC.md)
//! - [Layer Specification](../../ai_docs/layers/L07_OBSERVER.md)

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result};

/// Default correlation window size in milliseconds.
const DEFAULT_WINDOW_SIZE_MS: u64 = 5000;

/// Default maximum events buffered.
const DEFAULT_MAX_BUFFER_SIZE: usize = 10_000;

/// Default minimum correlation confidence.
const DEFAULT_MIN_CONFIDENCE: f64 = 0.6;

/// Default minimum recurring count.
const DEFAULT_MIN_RECURRING_COUNT: u32 = 3;

/// Default temporal tolerance in milliseconds.
const DEFAULT_TEMPORAL_TOLERANCE_MS: u64 = 500;

/// Default maximum correlations per event.
const DEFAULT_MAX_CORRELATIONS_PER_EVENT: usize = 20;

/// Maximum number of recurring patterns retained.
const MAX_RECURRING_PATTERNS: usize = 100;

/// Channel-to-layer mapping for origin detection.
fn channel_to_layer(channel: &str) -> u8 {
    match channel {
        "metrics" => 1,
        "health" => 2,
        "remediation" => 3,
        "integration" => 4,
        "learning" => 5,
        "consensus" => 6,
        _ => 0,
    }
}

/// Types of correlation links between events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CorrelationLinkType {
    /// Events occurred within temporal tolerance across different channels.
    Temporal,
    /// Events share causal relationship (same service, cascading errors).
    Causal,
    /// Events share semantic similarity (same event type, related services).
    Semantic,
    /// Events form a recurring pattern.
    Recurring,
}

impl std::fmt::Display for CorrelationLinkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Temporal => write!(f, "temporal"),
            Self::Causal => write!(f, "causal"),
            Self::Semantic => write!(f, "semantic"),
            Self::Recurring => write!(f, "recurring"),
        }
    }
}

/// An event ingested from the `EventBus` for correlation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IngestedEvent {
    /// Original `EventBus` event ID.
    pub event_id: String,
    /// Source channel.
    pub channel: String,
    /// Event type tag.
    pub event_type: String,
    /// JSON payload.
    pub payload: String,
    /// Source layer (1-6).
    pub source_layer: u8,
    /// Ingestion timestamp.
    pub ingested_at: DateTime<Utc>,
}

/// A link between two correlated events.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrelationLink {
    /// Source event ID.
    pub source_event_id: String,
    /// Target event ID.
    pub target_event_id: String,
    /// Link type classification.
    pub link_type: CorrelationLinkType,
    /// Link confidence [0.0, 1.0].
    pub confidence: f64,
    /// Temporal offset in milliseconds (signed).
    pub temporal_offset_ms: i64,
}

/// A correlated event with discovered links.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrelatedEvent {
    /// Correlation ID (UUID v4).
    pub id: String,
    /// Primary event.
    pub primary_event: IngestedEvent,
    /// Related events found during correlation.
    pub related_events: Vec<IngestedEvent>,
    /// Correlation links.
    pub links: Vec<CorrelationLink>,
    /// Overall correlation confidence [0.0, 1.0].
    pub confidence: f64,
    /// Discovery timestamp.
    pub discovered_at: DateTime<Utc>,
}

/// A time-bounded window grouping correlated events.
#[derive(Clone, Debug)]
pub struct CorrelationWindow {
    /// Unique window identifier (UUID v4).
    pub window_id: String,
    /// Events in this window.
    pub events: Vec<IngestedEvent>,
    /// Correlation links within this window.
    pub links: Vec<CorrelationLink>,
    /// Window start time.
    pub start_time: DateTime<Utc>,
    /// Window end time.
    pub end_time: DateTime<Utc>,
    /// Total correlations found in this window.
    pub correlation_count: u32,
    /// Whether this window has been finalized.
    pub finalized: bool,
}

/// A recurring pattern detected from repeated event sequences.
#[derive(Clone, Debug)]
pub struct RecurringPattern {
    /// Pattern identifier (UUID v4).
    pub pattern_id: String,
    /// Ordered event type sequence.
    pub event_sequence: Vec<String>,
    /// Ordered channel sequence.
    pub channel_sequence: Vec<String>,
    /// Total occurrences observed.
    pub occurrence_count: u64,
    /// Mean interval between occurrences (ms).
    pub average_interval_ms: u64,
    /// Std dev of intervals between occurrences (ms).
    pub stddev_interval_ms: f64,
    /// Confidence based on regularity.
    pub confidence: f64,
    /// First observation timestamp.
    pub first_seen: DateTime<Utc>,
    /// Most recent observation timestamp.
    pub last_seen: DateTime<Utc>,
}

/// Configuration for the Log Correlator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogCorrelatorConfig {
    /// Correlation window size in milliseconds.
    pub window_size_ms: u64,
    /// Maximum events buffered.
    pub max_buffer_size: usize,
    /// Minimum confidence to emit a correlation.
    pub min_correlation_confidence: f64,
    /// Minimum recurring count to flag a pattern.
    pub min_recurring_count: u32,
    /// Temporal tolerance in milliseconds.
    pub temporal_tolerance_ms: u64,
    /// Maximum correlations per event.
    pub max_correlations_per_event: usize,
}

impl Default for LogCorrelatorConfig {
    fn default() -> Self {
        Self {
            window_size_ms: DEFAULT_WINDOW_SIZE_MS,
            max_buffer_size: DEFAULT_MAX_BUFFER_SIZE,
            min_correlation_confidence: DEFAULT_MIN_CONFIDENCE,
            min_recurring_count: DEFAULT_MIN_RECURRING_COUNT,
            temporal_tolerance_ms: DEFAULT_TEMPORAL_TOLERANCE_MS,
            max_correlations_per_event: DEFAULT_MAX_CORRELATIONS_PER_EVENT,
        }
    }
}

/// Aggregate statistics for the Log Correlator.
#[derive(Clone, Debug, Default)]
pub struct CorrelationStats {
    /// Total events ingested since startup.
    pub total_events_ingested: u64,
    /// Total correlation links discovered.
    pub total_correlations_found: u64,
    /// Number of active (non-expired) windows.
    pub active_windows: usize,
    /// Number of detected recurring patterns.
    pub recurring_patterns: usize,
    /// Buffer utilization ratio.
    pub buffer_utilization: f64,
    /// Average correlations per event.
    pub avg_correlations_per_event: f64,
    /// Breakdown by correlation type.
    pub correlation_type_counts: HashMap<String, u64>,
}

/// M37: Cross-layer event correlation engine.
///
/// Subscribes to all 6 `EventBus` channels and detects temporal, causal,
/// semantic, and recurring correlations between events from different
/// layers (L1-L6).
///
/// # Thread Safety
///
/// All mutable state protected by `parking_lot::RwLock`.
///
/// # Lock Order
///
/// Lock order 2 (after `ObserverBus`, before `EmergenceDetector`).
pub struct LogCorrelator {
    /// Ring buffer of correlated events.
    event_buffer: RwLock<Vec<CorrelatedEvent>>,
    /// Active correlation windows.
    correlation_windows: RwLock<HashMap<String, CorrelationWindow>>,
    /// Detected recurring patterns.
    recurring_patterns: RwLock<Vec<RecurringPattern>>,
    /// Internal ingested event log for correlation.
    ingested_log: RwLock<Vec<IngestedEvent>>,
    /// Aggregate statistics.
    stats: RwLock<CorrelationStats>,
    /// Immutable configuration.
    config: LogCorrelatorConfig,
    /// Runtime-mutable window size override (set by RALPH mutations).
    /// When `Some`, overrides `config.window_size_ms`.
    window_override_ms: RwLock<Option<u64>>,
}

impl LogCorrelator {
    /// Creates a new `LogCorrelator` with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(LogCorrelatorConfig::default())
    }

    /// Creates a new `LogCorrelator` with the given configuration.
    #[must_use]
    pub fn with_config(config: LogCorrelatorConfig) -> Self {
        Self {
            event_buffer: RwLock::new(Vec::with_capacity(config.max_buffer_size.min(1000))),
            correlation_windows: RwLock::new(HashMap::new()),
            recurring_patterns: RwLock::new(Vec::new()),
            ingested_log: RwLock::new(Vec::with_capacity(config.max_buffer_size.min(1000))),
            window_override_ms: RwLock::new(None),
            stats: RwLock::new(CorrelationStats::default()),
            config,
        }
    }

    /// Validates the configuration.
    ///
    /// # Errors
    ///
    /// Returns `Error::Config` if any parameter is out of range.
    pub fn validate_config(config: &LogCorrelatorConfig) -> Result<()> {
        if config.window_size_ms < 100 || config.window_size_ms > 60_000 {
            return Err(Error::Config(
                "window_size_ms must be in [100, 60000]".into(),
            ));
        }
        if config.max_buffer_size == 0 {
            return Err(Error::Config("max_buffer_size must be > 0".into()));
        }
        if config.min_correlation_confidence < 0.0 || config.min_correlation_confidence > 1.0 {
            return Err(Error::Config(
                "min_correlation_confidence must be in [0.0, 1.0]".into(),
            ));
        }
        if config.temporal_tolerance_ms == 0 {
            return Err(Error::Config(
                "temporal_tolerance_ms must be > 0".into(),
            ));
        }
        Ok(())
    }

    /// Ingests a raw event from the `EventBus` for correlation.
    ///
    /// Computes correlations against the existing buffer and stores
    /// the enriched `CorrelatedEvent`.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `event_id` or channel is empty.
    pub fn ingest_event(
        &self,
        event_id: &str,
        channel: &str,
        event_type: &str,
        payload: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<CorrelatedEvent> {
        if event_id.is_empty() {
            return Err(Error::Validation("event_id must not be empty".into()));
        }
        if channel.is_empty() {
            return Err(Error::Validation("channel must not be empty".into()));
        }

        let source_layer = channel_to_layer(channel);
        let ingested = IngestedEvent {
            event_id: event_id.to_string(),
            channel: channel.to_string(),
            event_type: event_type.to_string(),
            payload: payload.to_string(),
            source_layer,
            ingested_at: timestamp,
        };

        // Find correlations with existing events in the buffer
        let (links, related) = self.find_correlations(&ingested);

        let confidence = if links.is_empty() {
            0.0
        } else {
            #[allow(clippy::cast_precision_loss)]
            let avg: f64 = links.iter().map(|l| l.confidence).sum::<f64>() / links.len() as f64;
            avg
        };

        let correlated = CorrelatedEvent {
            id: Uuid::new_v4().to_string(),
            primary_event: ingested.clone(),
            related_events: related,
            links,
            confidence,
            discovered_at: Utc::now(),
        };

        // Add to ingested log
        {
            let mut log = self.ingested_log.write();
            if log.len() >= self.config.max_buffer_size {
                log.remove(0);
            }
            log.push(ingested);
        }

        // Add to event buffer
        {
            let mut buffer = self.event_buffer.write();
            if buffer.len() >= self.config.max_buffer_size {
                buffer.remove(0);
            }
            buffer.push(correlated.clone());
        }

        // Update active windows
        self.update_windows(&correlated);

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.total_events_ingested += 1;
            #[allow(clippy::cast_precision_loss)]
            {
                stats.total_correlations_found += correlated.links.len() as u64;
            }
            for link in &correlated.links {
                *stats
                    .correlation_type_counts
                    .entry(link.link_type.to_string())
                    .or_insert(0) += 1;
            }
            #[allow(clippy::cast_precision_loss)]
            {
                let total = stats.total_events_ingested as f64;
                if total > 0.0 {
                    stats.avg_correlations_per_event =
                        stats.total_correlations_found as f64 / total;
                }
                stats.buffer_utilization =
                    self.event_buffer.read().len() as f64 / self.config.max_buffer_size as f64;
            }
            stats.active_windows = self.correlation_windows.read().len();
            stats.recurring_patterns = self.recurring_patterns.read().len();
        }

        Ok(correlated)
    }

    /// Finds correlations between a new event and existing events.
    fn find_correlations(&self, event: &IngestedEvent) -> (Vec<CorrelationLink>, Vec<IngestedEvent>) {
        let log = self.ingested_log.read();
        let mut links = Vec::new();
        let mut related = Vec::new();

        for existing in log.iter().rev().take(self.config.max_buffer_size) {
            if links.len() >= self.config.max_correlations_per_event {
                break;
            }

            // Skip self-correlation
            if existing.event_id == event.event_id {
                continue;
            }

            let delta_ms = (event.ingested_at - existing.ingested_at)
                .num_milliseconds();

            // Only correlate events within the window
            let effective_window = self.effective_window_ms();
            if delta_ms.unsigned_abs() > effective_window {
                continue;
            }

            // Temporal correlation: different channels, close in time
            if existing.channel != event.channel
                && delta_ms.unsigned_abs() <= self.config.temporal_tolerance_ms
            {
                #[allow(clippy::cast_precision_loss)]
                let confidence = 1.0
                    - (delta_ms.unsigned_abs() as f64
                        / self.config.temporal_tolerance_ms as f64);
                if confidence >= self.config.min_correlation_confidence {
                    links.push(CorrelationLink {
                        source_event_id: event.event_id.clone(),
                        target_event_id: existing.event_id.clone(),
                        link_type: CorrelationLinkType::Temporal,
                        confidence,
                        temporal_offset_ms: delta_ms,
                    });
                    related.push(existing.clone());
                }
            }

            // Causal correlation: same event_type in downstream layer
            if existing.event_type == event.event_type
                && existing.source_layer != event.source_layer
                && delta_ms > 0
            {
                #[allow(clippy::cast_precision_loss)]
                let abs_delta = delta_ms.unsigned_abs() as f64;
                #[allow(clippy::cast_precision_loss)]
                let eff_win = self.effective_window_ms() as f64;
                let confidence = 0.8 * (1.0 - abs_delta / eff_win);
                if confidence >= self.config.min_correlation_confidence {
                    links.push(CorrelationLink {
                        source_event_id: event.event_id.clone(),
                        target_event_id: existing.event_id.clone(),
                        link_type: CorrelationLinkType::Causal,
                        confidence,
                        temporal_offset_ms: delta_ms,
                    });
                    if !related.iter().any(|r| r.event_id == existing.event_id) {
                        related.push(existing.clone());
                    }
                }
            }

            // Semantic correlation: same event_type across layers
            if existing.event_type == event.event_type && existing.channel != event.channel {
                // Count how many layers have this event type
                let layers_with_type = self.count_layers_with_type(&event.event_type);
                #[allow(clippy::cast_precision_loss)]
                let confidence = layers_with_type as f64 / 6.0;
                if confidence >= self.config.min_correlation_confidence {
                    // Only add if not already linked
                    let already_linked = links
                        .iter()
                        .any(|l| l.target_event_id == existing.event_id);
                    if !already_linked {
                        links.push(CorrelationLink {
                            source_event_id: event.event_id.clone(),
                            target_event_id: existing.event_id.clone(),
                            link_type: CorrelationLinkType::Semantic,
                            confidence,
                            temporal_offset_ms: delta_ms,
                        });
                        if !related.iter().any(|r| r.event_id == existing.event_id) {
                            related.push(existing.clone());
                        }
                    }
                }
            }
        }
        drop(log);

        (links, related)
    }

    /// Counts how many distinct layers have produced the given event type.
    fn count_layers_with_type(&self, event_type: &str) -> usize {
        let log = self.ingested_log.read();
        let mut layers = std::collections::HashSet::new();
        for event in log.iter() {
            if event.event_type == event_type {
                layers.insert(event.source_layer);
            }
        }
        drop(log);
        layers.len()
    }

    /// Updates active correlation windows with a new event.
    fn update_windows(&self, correlated: &CorrelatedEvent) {
        let now = Utc::now();
        let window_duration = chrono::Duration::milliseconds(
            i64::try_from(self.effective_window_ms()).unwrap_or(5000),
        );

        let mut windows = self.correlation_windows.write();

        // Finalize expired windows
        let expired: Vec<String> = windows
            .iter()
            .filter(|(_, w)| !w.finalized && w.end_time < now)
            .map(|(id, _)| id.clone())
            .collect();

        for id in expired {
            if let Some(w) = windows.get_mut(&id) {
                w.finalized = true;
            }
        }

        // Remove windows that have been finalized for too long
        windows.retain(|_, w| {
            if w.finalized {
                w.end_time + window_duration > now
            } else {
                true
            }
        });

        // Find or create window for this event
        let event_time = correlated.primary_event.ingested_at;
        let matching_window = windows
            .values_mut()
            .find(|w| !w.finalized && w.start_time <= event_time && w.end_time >= event_time);

        if let Some(window) = matching_window {
            window.events.push(correlated.primary_event.clone());
            window.links.extend(correlated.links.clone());
            #[allow(clippy::cast_possible_truncation)]
            {
                window.correlation_count = window.links.len() as u32;
            }
            if event_time > window.end_time {
                window.end_time = event_time;
            }
        } else {
            // Create a new window
            let window = CorrelationWindow {
                window_id: Uuid::new_v4().to_string(),
                events: vec![correlated.primary_event.clone()],
                links: correlated.links.clone(),
                start_time: event_time,
                end_time: event_time + window_duration,
                #[allow(clippy::cast_possible_truncation)]
                correlation_count: correlated.links.len() as u32,
                finalized: false,
            };
            windows.insert(window.window_id.clone(), window);
        }
    }

    /// Returns all correlation links for a specific event.
    #[must_use]
    pub fn get_correlations(&self, event_id: &str) -> Vec<CorrelationLink> {
        let buffer = self.event_buffer.read();
        buffer
            .iter()
            .find(|e| e.primary_event.event_id == event_id)
            .map(|e| e.links.clone())
            .unwrap_or_default()
    }

    /// Returns all detected recurring patterns ordered by confidence.
    #[must_use]
    pub fn get_recurring_patterns(&self) -> Vec<RecurringPattern> {
        let mut patterns = self.recurring_patterns.read().clone();
        patterns.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        patterns
    }

    /// Returns events from the buffer within the given time range.
    #[must_use]
    pub fn get_events_in_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<CorrelatedEvent> {
        let buffer = self.event_buffer.read();
        buffer
            .iter()
            .filter(|e| e.primary_event.ingested_at >= start && e.primary_event.ingested_at <= end)
            .cloned()
            .collect()
    }

    /// Returns a specific correlation window by ID.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the window is not found.
    pub fn get_window(&self, window_id: &str) -> Result<CorrelationWindow> {
        let windows = self.correlation_windows.read();
        windows.get(window_id).cloned().ok_or_else(|| {
            Error::Validation(format!("correlation window '{window_id}' not found"))
        })
    }

    /// Returns current aggregate statistics.
    #[must_use]
    pub fn stats(&self) -> CorrelationStats {
        self.stats.read().clone()
    }

    /// Returns the event buffer length.
    #[must_use]
    pub fn buffer_len(&self) -> usize {
        self.event_buffer.read().len()
    }

    /// Returns the ingested log length.
    #[must_use]
    pub fn ingested_count(&self) -> usize {
        self.ingested_log.read().len()
    }

    /// Returns the number of active windows.
    #[must_use]
    pub fn active_window_count(&self) -> usize {
        self.correlation_windows
            .read()
            .values()
            .filter(|w| !w.finalized)
            .count()
    }

    /// Returns the number of finalized windows.
    #[must_use]
    pub fn finalized_window_count(&self) -> usize {
        self.correlation_windows
            .read()
            .values()
            .filter(|w| w.finalized)
            .count()
    }

    /// Runs periodic pattern detection against the current buffer.
    /// Returns newly detected or updated recurring patterns.
    pub fn detect_periodic_patterns(&self) -> Vec<RecurringPattern> {
        let event_groups: HashMap<String, Vec<DateTime<Utc>>> = {
            let log = self.ingested_log.read();
            if log.len() < 2 {
                return Vec::new();
            }
            let mut groups: HashMap<String, Vec<DateTime<Utc>>> = HashMap::new();
            for event in log.iter() {
                let key = format!("{}:{}", event.event_type, event.channel);
                groups.entry(key).or_default().push(event.ingested_at);
            }
            drop(log);
            groups
        };

        let mut detected = Vec::new();

        for (key, mut timestamps) in event_groups {
            #[allow(clippy::cast_possible_truncation)]
            if (timestamps.len() as u32) < self.config.min_recurring_count {
                continue;
            }

            timestamps.sort();

            // Calculate intervals
            let intervals: Vec<i64> = timestamps
                .windows(2)
                .map(|w| (w[1] - w[0]).num_milliseconds())
                .collect();

            if intervals.is_empty() {
                continue;
            }

            #[allow(clippy::cast_precision_loss)]
            let mean = intervals.iter().sum::<i64>() as f64 / intervals.len() as f64;

            if mean < 1.0 {
                continue;
            }

            #[allow(clippy::cast_precision_loss)]
            let variance: f64 = intervals
                .iter()
                .map(|&i| (i as f64 - mean).powi(2))
                .sum::<f64>()
                / intervals.len() as f64;

            let stddev = variance.sqrt();
            let confidence = (1.0 - (stddev / mean)).clamp(0.0, 1.0);

            if confidence < self.config.min_correlation_confidence {
                continue;
            }

            let parts: Vec<&str> = key.splitn(2, ':').collect();
            let (event_type, channel) = if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                (key.clone(), String::new())
            };

            let pattern = RecurringPattern {
                pattern_id: Uuid::new_v4().to_string(),
                event_sequence: vec![event_type],
                channel_sequence: vec![channel],
                #[allow(clippy::cast_possible_truncation)]
                occurrence_count: timestamps.len() as u64,
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                average_interval_ms: mean as u64,
                stddev_interval_ms: stddev,
                confidence,
                first_seen: timestamps[0],
                last_seen: timestamps[timestamps.len() - 1],
            };

            detected.push(pattern);
        }

        // Store detected patterns
        {
            let mut patterns = self.recurring_patterns.write();
            for new_pattern in &detected {
                // Update existing or add new
                let existing = patterns.iter_mut().find(|p| {
                    p.event_sequence == new_pattern.event_sequence
                        && p.channel_sequence == new_pattern.channel_sequence
                });

                if let Some(existing) = existing {
                    existing.occurrence_count = new_pattern.occurrence_count;
                    existing.average_interval_ms = new_pattern.average_interval_ms;
                    existing.stddev_interval_ms = new_pattern.stddev_interval_ms;
                    existing.confidence = new_pattern.confidence;
                    existing.last_seen = new_pattern.last_seen;
                } else if patterns.len() < MAX_RECURRING_PATTERNS {
                    patterns.push(new_pattern.clone());
                }
            }
        }

        detected
    }

    /// Prunes events older than the given timestamp.
    /// Returns the number of events pruned.
    pub fn prune_before(&self, before: DateTime<Utc>) -> usize {
        let mut pruned = 0;

        {
            let mut buffer = self.event_buffer.write();
            let before_len = buffer.len();
            buffer.retain(|e| e.primary_event.ingested_at >= before);
            pruned += before_len - buffer.len();
        }

        {
            let mut log = self.ingested_log.write();
            let before_len = log.len();
            log.retain(|e| e.ingested_at >= before);
            pruned += before_len - log.len();
        }

        pruned
    }

    /// Clears all buffers and resets statistics.
    pub fn clear(&self) {
        self.event_buffer.write().clear();
        self.correlation_windows.write().clear();
        self.recurring_patterns.write().clear();
        self.ingested_log.write().clear();
        *self.stats.write() = CorrelationStats::default();
    }

    /// Returns the most recent N correlated events.
    #[must_use]
    pub fn recent_events(&self, n: usize) -> Vec<CorrelatedEvent> {
        let buffer = self.event_buffer.read();
        let start = buffer.len().saturating_sub(n);
        buffer[start..].to_vec()
    }

    /// Returns the total number of correlation links in the buffer.
    #[must_use]
    pub fn total_links(&self) -> usize {
        self.event_buffer
            .read()
            .iter()
            .map(|e| e.links.len())
            .sum()
    }

    /// Returns the configuration.
    #[must_use]
    pub const fn config(&self) -> &LogCorrelatorConfig {
        &self.config
    }

    /// Returns the effective window size in milliseconds, accounting for
    /// any runtime override set by RALPH mutations.
    #[must_use]
    pub fn effective_window_ms(&self) -> u64 {
        self.window_override_ms
            .read()
            .unwrap_or(self.config.window_size_ms)
    }

    /// Updates the correlation window size at runtime (called by RALPH mutation executor).
    ///
    /// The override is clamped to `[100, 60_000]` to match validation bounds.
    pub fn update_window_size(&self, ms: u64) {
        let clamped = ms.clamp(100, 60_000);
        *self.window_override_ms.write() = Some(clamped);
    }

    /// Returns a list of all window IDs.
    #[must_use]
    pub fn window_ids(&self) -> Vec<String> {
        self.correlation_windows.read().keys().cloned().collect()
    }
}

impl Default for LogCorrelator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_correlator() -> LogCorrelator {
        LogCorrelator::new()
    }

    fn ingest_test_event(
        correlator: &LogCorrelator,
        id: &str,
        channel: &str,
        event_type: &str,
    ) -> Result<CorrelatedEvent> {
        correlator.ingest_event(id, channel, event_type, "{}", Utc::now())
    }

    #[test]
    fn test_new_correlator_defaults() {
        let c = make_correlator();
        assert_eq!(c.buffer_len(), 0);
        assert_eq!(c.ingested_count(), 0);
        assert_eq!(c.stats().total_events_ingested, 0);
    }

    #[test]
    fn test_ingest_single_event() {
        let c = make_correlator();
        let result = ingest_test_event(&c, "ev-1", "health", "status_change");
        assert!(result.is_ok());
        assert_eq!(c.buffer_len(), 1);
        assert_eq!(c.ingested_count(), 1);
    }

    #[test]
    fn test_ingest_empty_event_id_fails() {
        let c = make_correlator();
        let result = c.ingest_event("", "health", "test", "{}", Utc::now());
        assert!(result.is_err());
    }

    #[test]
    fn test_ingest_empty_channel_fails() {
        let c = make_correlator();
        let result = c.ingest_event("ev-1", "", "test", "{}", Utc::now());
        assert!(result.is_err());
    }

    #[test]
    fn test_temporal_correlation() {
        let c = make_correlator();
        let now = Utc::now();
        // Event on health channel
        let _r = c.ingest_event("ev-1", "health", "status", "{}", now);
        // Event on metrics channel, 100ms later (within tolerance)
        let _r = c.ingest_event(
            "ev-2",
            "metrics",
            "threshold_breach",
            "{}",
            now + chrono::Duration::milliseconds(100),
        );

        let links = c.get_correlations("ev-2");
        let temporal = links
            .iter()
            .any(|l| l.link_type == CorrelationLinkType::Temporal);
        assert!(temporal, "expected temporal correlation");
    }

    #[test]
    fn test_causal_correlation() {
        let c = make_correlator();
        let now = Utc::now();
        let _r = c.ingest_event("ev-1", "health", "service_down", "{}", now);
        // Same event_type, different layer, after the first
        let _r = c.ingest_event(
            "ev-2",
            "remediation",
            "service_down",
            "{}",
            now + chrono::Duration::milliseconds(200),
        );

        let links = c.get_correlations("ev-2");
        let causal = links
            .iter()
            .any(|l| l.link_type == CorrelationLinkType::Causal);
        assert!(causal, "expected causal correlation");
    }

    #[test]
    fn test_no_self_correlation() {
        let c = make_correlator();
        let _r = ingest_test_event(&c, "ev-1", "health", "test");
        let event = c.recent_events(1);
        assert!(event[0].links.is_empty(), "should not self-correlate");
    }

    #[test]
    fn test_channel_to_layer_mapping() {
        assert_eq!(channel_to_layer("health"), 2);
        assert_eq!(channel_to_layer("remediation"), 3);
        assert_eq!(channel_to_layer("metrics"), 1);
        assert_eq!(channel_to_layer("learning"), 5);
        assert_eq!(channel_to_layer("consensus"), 6);
        assert_eq!(channel_to_layer("integration"), 4);
        assert_eq!(channel_to_layer("unknown"), 0);
    }

    #[test]
    fn test_stats_updated_on_ingest() {
        let c = make_correlator();
        let _r = ingest_test_event(&c, "ev-1", "health", "test");
        let stats = c.stats();
        assert_eq!(stats.total_events_ingested, 1);
    }

    #[test]
    fn test_buffer_capacity_enforcement() {
        let config = LogCorrelatorConfig {
            max_buffer_size: 5,
            ..Default::default()
        };
        let c = LogCorrelator::with_config(config);
        for i in 0..10 {
            let _r = ingest_test_event(&c, &format!("ev-{i}"), "health", "test");
        }
        assert_eq!(c.buffer_len(), 5);
    }

    #[test]
    fn test_get_correlations_not_found() {
        let c = make_correlator();
        let links = c.get_correlations("nonexistent");
        assert!(links.is_empty());
    }

    #[test]
    fn test_get_events_in_range() {
        let c = make_correlator();
        let before = Utc::now() - chrono::Duration::seconds(1);
        for i in 0..5 {
            let _r = ingest_test_event(&c, &format!("ev-{i}"), "health", "test");
        }
        let after = Utc::now() + chrono::Duration::seconds(1);
        let events = c.get_events_in_range(before, after);
        assert_eq!(events.len(), 5);
    }

    #[test]
    fn test_get_events_in_range_empty() {
        let c = make_correlator();
        let _r = ingest_test_event(&c, "ev-1", "health", "test");
        let start = Utc::now() + chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(2);
        let events = c.get_events_in_range(start, end);
        assert!(events.is_empty());
    }

    #[test]
    fn test_window_creation() {
        let c = make_correlator();
        let _r = ingest_test_event(&c, "ev-1", "health", "test");
        assert!(c.active_window_count() > 0 || c.finalized_window_count() > 0);
    }

    #[test]
    fn test_get_window_not_found() {
        let c = make_correlator();
        let result = c.get_window("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_prune_before() {
        let c = make_correlator();
        for i in 0..5 {
            let _r = ingest_test_event(&c, &format!("ev-{i}"), "health", "test");
        }
        let future = Utc::now() + chrono::Duration::seconds(1);
        let pruned = c.prune_before(future);
        assert!(pruned > 0);
        assert_eq!(c.buffer_len(), 0);
    }

    #[test]
    fn test_clear() {
        let c = make_correlator();
        for i in 0..5 {
            let _r = ingest_test_event(&c, &format!("ev-{i}"), "health", "test");
        }
        c.clear();
        assert_eq!(c.buffer_len(), 0);
        assert_eq!(c.ingested_count(), 0);
        assert_eq!(c.stats().total_events_ingested, 0);
    }

    #[test]
    fn test_recent_events() {
        let c = make_correlator();
        for i in 0..10 {
            let _r = ingest_test_event(&c, &format!("ev-{i}"), "health", "test");
        }
        let recent = c.recent_events(3);
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn test_total_links() {
        let c = make_correlator();
        let _r = ingest_test_event(&c, "ev-1", "health", "test");
        // Single event has no links
        assert_eq!(c.total_links(), 0);
    }

    #[test]
    fn test_default_correlator() {
        let c = LogCorrelator::default();
        assert_eq!(c.buffer_len(), 0);
    }

    #[test]
    fn test_config_accessor() {
        let c = make_correlator();
        assert_eq!(c.config().window_size_ms, DEFAULT_WINDOW_SIZE_MS);
    }

    #[test]
    fn test_config_validation_valid() {
        assert!(LogCorrelator::validate_config(&LogCorrelatorConfig::default()).is_ok());
    }

    #[test]
    fn test_config_validation_invalid_window() {
        let config = LogCorrelatorConfig {
            window_size_ms: 50,
            ..Default::default()
        };
        assert!(LogCorrelator::validate_config(&config).is_err());
    }

    #[test]
    fn test_config_validation_invalid_buffer() {
        let config = LogCorrelatorConfig {
            max_buffer_size: 0,
            ..Default::default()
        };
        assert!(LogCorrelator::validate_config(&config).is_err());
    }

    #[test]
    fn test_config_validation_invalid_confidence() {
        let config = LogCorrelatorConfig {
            min_correlation_confidence: 1.5,
            ..Default::default()
        };
        assert!(LogCorrelator::validate_config(&config).is_err());
    }

    #[test]
    fn test_config_validation_invalid_tolerance() {
        let config = LogCorrelatorConfig {
            temporal_tolerance_ms: 0,
            ..Default::default()
        };
        assert!(LogCorrelator::validate_config(&config).is_err());
    }

    #[test]
    fn test_correlated_event_has_uuid() {
        let c = make_correlator();
        let result = ingest_test_event(&c, "ev-1", "health", "test");
        assert!(result.is_ok());
        let event = result.unwrap_or_else(|_| unreachable!());
        assert!(event.id.contains('-'), "expected UUID format");
    }

    #[test]
    fn test_correlated_event_preserves_source() {
        let c = make_correlator();
        let result = ingest_test_event(&c, "ev-1", "health", "status_change");
        let event = result.unwrap_or_else(|_| unreachable!());
        assert_eq!(event.primary_event.event_id, "ev-1");
        assert_eq!(event.primary_event.channel, "health");
        assert_eq!(event.primary_event.event_type, "status_change");
        assert_eq!(event.primary_event.source_layer, 2);
    }

    #[test]
    fn test_detect_periodic_patterns_insufficient_data() {
        let c = make_correlator();
        let patterns = c.detect_periodic_patterns();
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_detect_periodic_patterns_with_data() {
        let c = LogCorrelator::with_config(LogCorrelatorConfig {
            min_recurring_count: 3,
            min_correlation_confidence: 0.3,
            ..Default::default()
        });
        let base_time = Utc::now() - chrono::Duration::seconds(10);
        for i in 0..5 {
            let _r = c.ingest_event(
                &format!("ev-{i}"),
                "health",
                "heartbeat",
                "{}",
                base_time + chrono::Duration::seconds(i * 2),
            );
        }
        let patterns = c.detect_periodic_patterns();
        // Should detect a periodic pattern for heartbeat events
        assert!(!patterns.is_empty(), "expected periodic pattern detection");
    }

    #[test]
    fn test_recurring_patterns_persistence() {
        let c = LogCorrelator::with_config(LogCorrelatorConfig {
            min_recurring_count: 3,
            min_correlation_confidence: 0.3,
            ..Default::default()
        });
        let base = Utc::now() - chrono::Duration::seconds(20);
        for i in 0..5 {
            let _r = c.ingest_event(
                &format!("ev-{i}"),
                "health",
                "tick",
                "{}",
                base + chrono::Duration::seconds(i * 2),
            );
        }
        let _p = c.detect_periodic_patterns();
        let stored = c.get_recurring_patterns();
        // Patterns should be persisted
        assert_eq!(stored.len(), c.recurring_patterns.read().len());
    }

    #[test]
    fn test_window_ids() {
        let c = make_correlator();
        let _r = ingest_test_event(&c, "ev-1", "health", "test");
        let ids = c.window_ids();
        assert!(!ids.is_empty());
    }

    #[test]
    fn test_buffer_utilization() {
        let config = LogCorrelatorConfig {
            max_buffer_size: 10,
            ..Default::default()
        };
        let c = LogCorrelator::with_config(config);
        for i in 0..5 {
            let _r = ingest_test_event(&c, &format!("ev-{i}"), "health", "test");
        }
        let stats = c.stats();
        assert!((stats.buffer_utilization - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_correlation_type_display() {
        assert_eq!(CorrelationLinkType::Temporal.to_string(), "temporal");
        assert_eq!(CorrelationLinkType::Causal.to_string(), "causal");
        assert_eq!(CorrelationLinkType::Semantic.to_string(), "semantic");
        assert_eq!(CorrelationLinkType::Recurring.to_string(), "recurring");
    }

    #[test]
    fn test_multi_channel_correlation() {
        let c = make_correlator();
        let now = Utc::now();
        let _r = c.ingest_event("ev-1", "health", "alert", "{}", now);
        let _r = c.ingest_event("ev-2", "metrics", "alert", "{}",
            now + chrono::Duration::milliseconds(50));
        let _r = c.ingest_event("ev-3", "remediation", "alert", "{}",
            now + chrono::Duration::milliseconds(100));
        let _r = c.ingest_event("ev-4", "consensus", "alert", "{}",
            now + chrono::Duration::milliseconds(150));

        // The later events should correlate with earlier ones
        let stats = c.stats();
        assert!(stats.total_correlations_found > 0);
    }

    #[test]
    fn test_events_outside_window_not_correlated() {
        let c = make_correlator();
        let now = Utc::now();
        let _r = c.ingest_event("ev-1", "health", "old_event", "{}", now - chrono::Duration::seconds(30));
        let _r = c.ingest_event("ev-2", "metrics", "new_event", "{}", now);

        // Events are too far apart (30s > 5s window)
        let links = c.get_correlations("ev-2");
        assert!(links.is_empty(), "events outside window should not correlate");
    }

    #[test]
    fn test_concurrent_ingest() {
        use std::sync::Arc;
        use std::thread;

        let c = Arc::new(LogCorrelator::new());
        let mut handles = Vec::new();

        for t in 0..4 {
            let c_clone = Arc::clone(&c);
            handles.push(thread::spawn(move || {
                for i in 0..10 {
                    let _r = c_clone.ingest_event(
                        &format!("t{t}-ev-{i}"),
                        "health",
                        "test",
                        "{}",
                        Utc::now(),
                    );
                }
            }));
        }

        for handle in handles {
            let _r = handle.join();
        }

        assert_eq!(c.stats().total_events_ingested, 40);
    }

    #[test]
    fn test_ingested_event_layer_assignment() {
        let c = make_correlator();
        let _r = c.ingest_event("ev-1", "learning", "pathway_update", "{}", Utc::now());
        let events = c.recent_events(1);
        assert_eq!(events[0].primary_event.source_layer, 5);
    }

    #[test]
    fn test_correlation_confidence_range() {
        let c = make_correlator();
        let now = Utc::now();
        let _r = c.ingest_event("ev-1", "health", "alert", "{}", now);
        let _r = c.ingest_event("ev-2", "metrics", "alert", "{}", now + chrono::Duration::milliseconds(10));

        let links = c.get_correlations("ev-2");
        for link in &links {
            assert!(link.confidence >= 0.0 && link.confidence <= 1.0,
                "confidence out of range: {}", link.confidence);
        }
    }
}
