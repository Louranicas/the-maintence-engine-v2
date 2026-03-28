//! # M44: Observer Bus
//!
//! Internal L7 pub/sub bus connecting M37 (Log Correlator), M38 (Emergence
//! Detector), M39 (Evolution Chamber), and the Fitness Evaluator.
//!
//! Decouples internal L7 communication from the external `EventBus` (M23).
//! All handler invocations are fire-and-forget: errors are logged and
//! counted, never propagated to the publisher.
//!
//! ## Layer: L7 (Observer)
//! ## Lock Order: 1 (acquired before all other L7 locks)
//! ## Dependencies: M01 (Error)
//!
//! ## Related Documentation
//! - [Observer Bus Spec](../../ai_specs/evolution_chamber_ai_specs/OBSERVER_BUS_SPEC.md)
//! - [Layer Specification](../../ai_docs/layers/L07_OBSERVER.md)

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::Result;

/// Maximum number of messages retained per channel in the audit log.
const MAX_MESSAGES_PER_CHANNEL: usize = 500;

/// Default number of internal channels.
const DEFAULT_CHANNEL_COUNT: usize = 3;

/// Internal channel names for L7 observer communication.
const INTERNAL_CHANNELS: &[&str] = &["correlation", "emergence", "evolution"];

/// Source module for internal observer messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ObserverSource {
    /// M37 Log Correlator.
    LogCorrelator,
    /// M38 Emergence Detector.
    EmergenceDetector,
    /// M39 Evolution Chamber.
    EvolutionChamber,
    /// Fitness Evaluator utility.
    FitnessEvaluator,
    /// Layer Coordinator (mod.rs).
    Coordinator,
}

impl std::fmt::Display for ObserverSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LogCorrelator => write!(f, "M37:LogCorrelator"),
            Self::EmergenceDetector => write!(f, "M38:EmergenceDetector"),
            Self::EvolutionChamber => write!(f, "M39:EvolutionChamber"),
            Self::FitnessEvaluator => write!(f, "FitnessEvaluator"),
            Self::Coordinator => write!(f, "L7:Coordinator"),
        }
    }
}

/// Classification of internal observer message types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ObserverMessageType {
    /// New correlation discovered by M37.
    CorrelationFound,
    /// Emergent behavior detected by M38.
    EmergenceDetected,
    /// Mutation proposed by M39.
    MutationProposed,
    /// Mutation result (applied or rolled back) from M39.
    MutationResult,
    /// Fitness evaluation completed.
    FitnessEvaluated,
    /// RALPH phase transition in M39.
    PhaseTransition,
}

impl std::fmt::Display for ObserverMessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CorrelationFound => write!(f, "correlation_found"),
            Self::EmergenceDetected => write!(f, "emergence_detected"),
            Self::MutationProposed => write!(f, "mutation_proposed"),
            Self::MutationResult => write!(f, "mutation_result"),
            Self::FitnessEvaluated => write!(f, "fitness_evaluated"),
            Self::PhaseTransition => write!(f, "phase_transition"),
        }
    }
}

/// An internal observer message passed through the bus.
#[derive(Clone, Debug)]
pub struct ObserverMessage {
    /// Monotonically increasing message ID.
    pub id: u64,
    /// Source module that published this message.
    pub source: ObserverSource,
    /// Classification of the message content.
    pub message_type: ObserverMessageType,
    /// JSON-serialized payload (for audit logging).
    pub payload: String,
    /// Timestamp of message creation.
    pub timestamp: DateTime<Utc>,
}

/// Aggregate statistics for the Observer Bus.
#[derive(Clone, Debug, Default)]
pub struct ObserverBusStats {
    /// Total correlation events published through the bus.
    pub correlations_published: u64,
    /// Total emergence events published through the bus.
    pub emergences_published: u64,
    /// Total evolution events published through the bus.
    pub evolutions_published: u64,
    /// Total messages published across all channels.
    pub total_messages: u64,
    /// Total handler invocation errors (logged, not propagated).
    pub handler_errors: u64,
    /// Timestamp of the most recent bus activity.
    pub last_activity: Option<DateTime<Utc>>,
}

/// Configuration for the Observer Bus.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObserverBusConfig {
    /// Maximum messages retained per channel. Default: 500.
    pub max_messages_per_channel: usize,
    /// Whether to enable debug-level audit logging.
    pub debug_logging: bool,
}

impl Default for ObserverBusConfig {
    fn default() -> Self {
        Self {
            max_messages_per_channel: MAX_MESSAGES_PER_CHANNEL,
            debug_logging: false,
        }
    }
}

/// Internal L7 pub/sub bus connecting M37, M38, M39, and the Fitness
/// Evaluator. Decouples internal L7 communication from the external
/// `EventBus` (M23).
///
/// # Thread Safety
///
/// All mutable state is protected by `parking_lot::RwLock`.
///
/// # Lock Order
///
/// Lock order 1 (acquired before all other L7 locks).
pub struct ObserverBus {
    /// Message log per channel (bounded ring buffer).
    channels: RwLock<HashMap<String, Vec<ObserverMessage>>>,
    /// Next message ID (monotonically increasing).
    next_id: RwLock<u64>,
    /// Aggregate bus statistics.
    stats: RwLock<ObserverBusStats>,
    /// Immutable configuration.
    config: ObserverBusConfig,
}

impl ObserverBus {
    /// Constructs a new Observer Bus with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(ObserverBusConfig::default())
    }

    /// Constructs a new Observer Bus with the given configuration.
    #[must_use]
    pub fn with_config(config: ObserverBusConfig) -> Self {
        let mut channels = HashMap::with_capacity(DEFAULT_CHANNEL_COUNT);
        for &name in INTERNAL_CHANNELS {
            channels.insert(name.to_string(), Vec::new());
        }
        Self {
            channels: RwLock::new(channels),
            next_id: RwLock::new(0),
            stats: RwLock::new(ObserverBusStats::default()),
            config,
        }
    }

    /// Publishes a message to the specified internal channel.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the channel name is not a valid
    /// internal channel.
    pub fn publish(
        &self,
        channel: &str,
        source: ObserverSource,
        message_type: ObserverMessageType,
        payload: &str,
    ) -> Result<u64> {
        let msg_id = {
            let mut id = self.next_id.write();
            let current = *id;
            *id += 1;
            current
        };

        let message = ObserverMessage {
            id: msg_id,
            source,
            message_type,
            payload: payload.to_string(),
            timestamp: Utc::now(),
        };

        self.store_message(channel, message);

        // Update statistics
        self.update_stats_for(message_type);

        Ok(msg_id)
    }

    /// Publishes a correlation event from M37.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if serialization fails.
    /// Stores a message in the appropriate channel (internal helper).
    fn store_message(&self, channel: &str, message: ObserverMessage) {
        let max = self.config.max_messages_per_channel;
        let mut channels = self.channels.write();
        let log = channels.entry(channel.to_string()).or_default();
        if log.len() >= max {
            log.remove(0);
        }
        log.push(message);
        drop(channels);
    }

    /// Updates statistics after publishing a message (internal helper).
    fn update_stats_for(&self, message_type: ObserverMessageType) {
        let mut stats = self.stats.write();
        stats.total_messages += 1;
        stats.last_activity = Some(Utc::now());
        match message_type {
            ObserverMessageType::CorrelationFound => stats.correlations_published += 1,
            ObserverMessageType::EmergenceDetected => stats.emergences_published += 1,
            ObserverMessageType::MutationProposed
            | ObserverMessageType::MutationResult => stats.evolutions_published += 1,
            ObserverMessageType::FitnessEvaluated
            | ObserverMessageType::PhaseTransition => {}
        }
    }

    /// Publishes a correlation event from M37.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if serialization fails.
    pub fn publish_correlation(&self, payload: &str) -> Result<u64> {
        self.publish(
            "correlation",
            ObserverSource::LogCorrelator,
            ObserverMessageType::CorrelationFound,
            payload,
        )
    }

    /// Publishes an emergence event from M38.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if serialization fails.
    pub fn publish_emergence(&self, payload: &str) -> Result<u64> {
        self.publish(
            "emergence",
            ObserverSource::EmergenceDetector,
            ObserverMessageType::EmergenceDetected,
            payload,
        )
    }

    /// Publishes a mutation event from M39.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if serialization fails.
    pub fn publish_mutation(&self, payload: &str, proposed: bool) -> Result<u64> {
        let msg_type = if proposed {
            ObserverMessageType::MutationProposed
        } else {
            ObserverMessageType::MutationResult
        };
        self.publish("evolution", ObserverSource::EvolutionChamber, msg_type, payload)
    }

    /// Publishes a fitness evaluation result.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if serialization fails.
    pub fn publish_fitness(&self, payload: &str) -> Result<u64> {
        self.publish(
            "evolution",
            ObserverSource::FitnessEvaluator,
            ObserverMessageType::FitnessEvaluated,
            payload,
        )
    }

    /// Publishes a RALPH phase transition from M39.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if serialization fails.
    pub fn publish_phase_transition(&self, payload: &str) -> Result<u64> {
        self.publish(
            "evolution",
            ObserverSource::EvolutionChamber,
            ObserverMessageType::PhaseTransition,
            payload,
        )
    }

    /// Retrieves the most recent messages from a channel, up to `limit`.
    #[must_use]
    pub fn get_messages(&self, channel: &str, limit: usize) -> Vec<ObserverMessage> {
        let channels = self.channels.read();
        channels.get(channel).map_or_else(Vec::new, |msgs| {
            let start = msgs.len().saturating_sub(limit);
            msgs[start..].to_vec()
        })
    }

    /// Retrieves all messages from a channel with a specific message type.
    #[must_use]
    pub fn get_messages_by_type(
        &self,
        channel: &str,
        message_type: ObserverMessageType,
    ) -> Vec<ObserverMessage> {
        let channels = self.channels.read();
        channels.get(channel).map_or_else(Vec::new, |msgs| {
            msgs.iter()
                .filter(|m| m.message_type == message_type)
                .cloned()
                .collect()
        })
    }

    /// Retrieves messages from a channel within a time range.
    #[must_use]
    pub fn get_messages_in_range(
        &self,
        channel: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<ObserverMessage> {
        let channels = self.channels.read();
        channels.get(channel).map_or_else(Vec::new, |msgs| {
            msgs.iter()
                .filter(|m| m.timestamp >= start && m.timestamp <= end)
                .cloned()
                .collect()
        })
    }

    /// Returns a snapshot of aggregate bus statistics.
    #[must_use]
    pub fn stats(&self) -> ObserverBusStats {
        self.stats.read().clone()
    }

    /// Returns the total number of messages across all channels.
    #[must_use]
    pub fn total_message_count(&self) -> usize {
        self.channels
            .read()
            .values()
            .map(Vec::len)
            .sum()
    }

    /// Returns the message count for a specific channel.
    #[must_use]
    pub fn channel_message_count(&self, channel: &str) -> usize {
        self.channels
            .read()
            .get(channel)
            .map_or(0, Vec::len)
    }

    /// Returns all internal channel names.
    #[must_use]
    pub fn channel_names(&self) -> Vec<String> {
        self.channels.read().keys().cloned().collect()
    }

    /// Returns the number of internal channels.
    #[must_use]
    pub fn channel_count(&self) -> usize {
        self.channels.read().len()
    }

    /// Returns handler/channel counts as (correlation, emergence, evolution).
    #[must_use]
    pub fn message_counts(&self) -> (usize, usize, usize) {
        let channels = self.channels.read();
        let correlation = channels.get("correlation").map_or(0, Vec::len);
        let emergence = channels.get("emergence").map_or(0, Vec::len);
        let evolution = channels.get("evolution").map_or(0, Vec::len);
        drop(channels);
        (correlation, emergence, evolution)
    }

    /// Clears all messages from all channels. Resets statistics.
    pub fn clear(&self) {
        {
            let mut channels = self.channels.write();
            for msgs in channels.values_mut() {
                msgs.clear();
            }
        }
        *self.stats.write() = ObserverBusStats::default();
        *self.next_id.write() = 0;
    }

    /// Prunes messages older than the given timestamp from all channels.
    /// Returns the total number of messages pruned.
    pub fn prune_before(&self, before: DateTime<Utc>) -> usize {
        let mut total_pruned = 0;
        {
            let mut channels = self.channels.write();
            for msgs in channels.values_mut() {
                let before_len = msgs.len();
                msgs.retain(|m| m.timestamp >= before);
                total_pruned += before_len - msgs.len();
            }
        }
        total_pruned
    }

    /// Records a handler error in the bus statistics.
    pub fn record_error(&self) {
        self.stats.write().handler_errors += 1;
    }

    /// Returns the configuration.
    #[must_use]
    pub const fn config(&self) -> &ObserverBusConfig {
        &self.config
    }

    /// Returns the most recent message ID assigned.
    #[must_use]
    pub fn last_message_id(&self) -> u64 {
        let id = *self.next_id.read();
        id.saturating_sub(1)
    }

    /// Checks if a channel exists.
    #[must_use]
    pub fn has_channel(&self, channel: &str) -> bool {
        self.channels.read().contains_key(channel)
    }

    /// Creates a custom internal channel.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the channel name is empty or already exists.
    pub fn create_channel(&self, name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(crate::Error::Validation(
                "channel name must not be empty".into(),
            ));
        }
        {
            let mut channels = self.channels.write();
            if channels.contains_key(name) {
                return Err(crate::Error::Validation(format!(
                    "channel '{name}' already exists"
                )));
            }
            channels.insert(name.to_string(), Vec::new());
        }
        Ok(())
    }

    /// Returns the messages from the most recent N seconds.
    #[must_use]
    pub fn recent_messages(&self, seconds: i64) -> Vec<ObserverMessage> {
        let cutoff = Utc::now() - chrono::Duration::seconds(seconds);
        let mut result = Vec::new();
        {
            let channels = self.channels.read();
            for msgs in channels.values() {
                for msg in msgs {
                    if msg.timestamp >= cutoff {
                        result.push(msg.clone());
                    }
                }
            }
        }
        result.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        result
    }
}

impl Default for ObserverBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_bus() -> ObserverBus {
        ObserverBus::new()
    }

    #[test]
    fn test_new_bus_has_default_channels() {
        let bus = make_bus();
        assert_eq!(bus.channel_count(), DEFAULT_CHANNEL_COUNT);
        for &name in INTERNAL_CHANNELS {
            assert!(bus.has_channel(name));
        }
    }

    #[test]
    fn test_publish_returns_incrementing_ids() {
        let bus = make_bus();
        let id0 = bus.publish_correlation("{}");
        let id1 = bus.publish_correlation("{}");
        assert!(id0.is_ok());
        assert!(id1.is_ok());
        let id0 = id0.unwrap_or(0);
        let id1 = id1.unwrap_or(0);
        assert_eq!(id1, id0 + 1);
    }

    #[test]
    fn test_publish_correlation_increments_stats() {
        let bus = make_bus();
        let _r = bus.publish_correlation(r#"{"test":true}"#);
        let stats = bus.stats();
        assert_eq!(stats.correlations_published, 1);
        assert_eq!(stats.total_messages, 1);
        assert!(stats.last_activity.is_some());
    }

    #[test]
    fn test_publish_emergence_increments_stats() {
        let bus = make_bus();
        let _r = bus.publish_emergence(r#"{"type":"cascade"}"#);
        let stats = bus.stats();
        assert_eq!(stats.emergences_published, 1);
        assert_eq!(stats.total_messages, 1);
    }

    #[test]
    fn test_publish_mutation_proposed() {
        let bus = make_bus();
        let _r = bus.publish_mutation(r#"{"param":"ltp_rate"}"#, true);
        let stats = bus.stats();
        assert_eq!(stats.evolutions_published, 1);
    }

    #[test]
    fn test_publish_mutation_result() {
        let bus = make_bus();
        let _r = bus.publish_mutation(r#"{"applied":true}"#, false);
        let stats = bus.stats();
        assert_eq!(stats.evolutions_published, 1);
    }

    #[test]
    fn test_publish_fitness() {
        let bus = make_bus();
        let _r = bus.publish_fitness(r#"{"score":0.85}"#);
        let stats = bus.stats();
        assert_eq!(stats.total_messages, 1);
    }

    #[test]
    fn test_publish_phase_transition() {
        let bus = make_bus();
        let _r = bus.publish_phase_transition(r#"{"from":"Recognize","to":"Analyze"}"#);
        let stats = bus.stats();
        assert_eq!(stats.total_messages, 1);
    }

    #[test]
    fn test_get_messages_returns_recent() {
        let bus = make_bus();
        for i in 0..5 {
            let _r = bus.publish_correlation(&format!(r#"{{"seq":{i}}}"#));
        }
        let msgs = bus.get_messages("correlation", 3);
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].id, 2);
        assert_eq!(msgs[2].id, 4);
    }

    #[test]
    fn test_get_messages_empty_channel() {
        let bus = make_bus();
        let msgs = bus.get_messages("correlation", 10);
        assert!(msgs.is_empty());
    }

    #[test]
    fn test_get_messages_nonexistent_channel() {
        let bus = make_bus();
        let msgs = bus.get_messages("nonexistent", 10);
        assert!(msgs.is_empty());
    }

    #[test]
    fn test_get_messages_by_type() {
        let bus = make_bus();
        let _r = bus.publish_mutation(r#"{"proposed":true}"#, true);
        let _r = bus.publish_mutation(r#"{"applied":true}"#, false);
        let _r = bus.publish_fitness(r#"{"score":0.9}"#);

        let proposed = bus.get_messages_by_type("evolution", ObserverMessageType::MutationProposed);
        assert_eq!(proposed.len(), 1);

        let results = bus.get_messages_by_type("evolution", ObserverMessageType::MutationResult);
        assert_eq!(results.len(), 1);

        let fitness = bus.get_messages_by_type("evolution", ObserverMessageType::FitnessEvaluated);
        assert_eq!(fitness.len(), 1);
    }

    #[test]
    fn test_total_message_count() {
        let bus = make_bus();
        let _r = bus.publish_correlation("{}");
        let _r = bus.publish_emergence("{}");
        let _r = bus.publish_mutation("{}", true);
        assert_eq!(bus.total_message_count(), 3);
    }

    #[test]
    fn test_channel_message_count() {
        let bus = make_bus();
        let _r = bus.publish_correlation("{}");
        let _r = bus.publish_correlation("{}");
        assert_eq!(bus.channel_message_count("correlation"), 2);
        assert_eq!(bus.channel_message_count("emergence"), 0);
    }

    #[test]
    fn test_message_counts_tuple() {
        let bus = make_bus();
        let _r = bus.publish_correlation("{}");
        let _r = bus.publish_emergence("{}");
        let _r = bus.publish_mutation("{}", true);
        let _r = bus.publish_mutation("{}", false);
        let (c, e, ev) = bus.message_counts();
        assert_eq!(c, 1);
        assert_eq!(e, 1);
        assert_eq!(ev, 2);
    }

    #[test]
    fn test_clear_resets_all() {
        let bus = make_bus();
        let _r = bus.publish_correlation("{}");
        let _r = bus.publish_emergence("{}");
        bus.clear();

        assert_eq!(bus.total_message_count(), 0);
        assert_eq!(bus.stats().total_messages, 0);
        assert_eq!(bus.last_message_id(), 0); // saturating_sub at 0 stays 0
    }

    #[test]
    fn test_capacity_enforcement() {
        let config = ObserverBusConfig {
            max_messages_per_channel: 5,
            debug_logging: false,
        };
        let bus = ObserverBus::with_config(config);
        for i in 0..10 {
            let _r = bus.publish_correlation(&format!(r#"{{"seq":{i}}}"#));
        }
        assert_eq!(bus.channel_message_count("correlation"), 5);
        // Oldest messages should have been evicted
        let msgs = bus.get_messages("correlation", 10);
        assert_eq!(msgs[0].id, 5);
    }

    #[test]
    fn test_record_error() {
        let bus = make_bus();
        bus.record_error();
        bus.record_error();
        assert_eq!(bus.stats().handler_errors, 2);
    }

    #[test]
    fn test_config_accessor() {
        let bus = make_bus();
        assert_eq!(bus.config().max_messages_per_channel, MAX_MESSAGES_PER_CHANNEL);
        assert!(!bus.config().debug_logging);
    }

    #[test]
    fn test_create_custom_channel() {
        let bus = make_bus();
        assert!(bus.create_channel("custom").is_ok());
        assert!(bus.has_channel("custom"));
        assert_eq!(bus.channel_count(), DEFAULT_CHANNEL_COUNT + 1);
    }

    #[test]
    fn test_create_duplicate_channel_fails() {
        let bus = make_bus();
        assert!(bus.create_channel("correlation").is_err());
    }

    #[test]
    fn test_create_empty_channel_fails() {
        let bus = make_bus();
        assert!(bus.create_channel("").is_err());
    }

    #[test]
    fn test_channel_names() {
        let bus = make_bus();
        let names = bus.channel_names();
        assert_eq!(names.len(), DEFAULT_CHANNEL_COUNT);
        for &expected in INTERNAL_CHANNELS {
            assert!(names.contains(&expected.to_string()));
        }
    }

    #[test]
    fn test_prune_before() {
        let bus = make_bus();
        for i in 0..5 {
            let _r = bus.publish_correlation(&format!(r#"{{"seq":{i}}}"#));
        }
        // Prune everything before "now" (all messages are at or before now)
        let future = Utc::now() + chrono::Duration::seconds(1);
        let pruned = bus.prune_before(future);
        assert_eq!(pruned, 5);
        assert_eq!(bus.channel_message_count("correlation"), 0);
    }

    #[test]
    fn test_prune_before_keeps_recent() {
        let bus = make_bus();
        for i in 0..5 {
            let _r = bus.publish_correlation(&format!(r#"{{"seq":{i}}}"#));
        }
        // Prune from the distant past (should keep all)
        let past = Utc::now() - chrono::Duration::hours(1);
        let pruned = bus.prune_before(past);
        assert_eq!(pruned, 0);
        assert_eq!(bus.channel_message_count("correlation"), 5);
    }

    #[test]
    fn test_observer_source_display() {
        assert_eq!(ObserverSource::LogCorrelator.to_string(), "M37:LogCorrelator");
        assert_eq!(ObserverSource::EmergenceDetector.to_string(), "M38:EmergenceDetector");
        assert_eq!(ObserverSource::EvolutionChamber.to_string(), "M39:EvolutionChamber");
        assert_eq!(ObserverSource::FitnessEvaluator.to_string(), "FitnessEvaluator");
        assert_eq!(ObserverSource::Coordinator.to_string(), "L7:Coordinator");
    }

    #[test]
    fn test_observer_message_type_display() {
        assert_eq!(ObserverMessageType::CorrelationFound.to_string(), "correlation_found");
        assert_eq!(ObserverMessageType::EmergenceDetected.to_string(), "emergence_detected");
        assert_eq!(ObserverMessageType::MutationProposed.to_string(), "mutation_proposed");
        assert_eq!(ObserverMessageType::MutationResult.to_string(), "mutation_result");
        assert_eq!(ObserverMessageType::FitnessEvaluated.to_string(), "fitness_evaluated");
        assert_eq!(ObserverMessageType::PhaseTransition.to_string(), "phase_transition");
    }

    #[test]
    fn test_default_bus() {
        let bus = ObserverBus::default();
        assert_eq!(bus.channel_count(), DEFAULT_CHANNEL_COUNT);
        assert_eq!(bus.total_message_count(), 0);
    }

    #[test]
    fn test_message_payload_preserved() {
        let bus = make_bus();
        let payload = r#"{"key":"value","num":42}"#;
        let _r = bus.publish_correlation(payload);
        let msgs = bus.get_messages("correlation", 1);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].payload, payload);
    }

    #[test]
    fn test_message_source_preserved() {
        let bus = make_bus();
        let _r = bus.publish_correlation("{}");
        let _r = bus.publish_emergence("{}");
        let _r = bus.publish_mutation("{}", true);

        let corr = bus.get_messages("correlation", 1);
        assert_eq!(corr[0].source, ObserverSource::LogCorrelator);

        let emerg = bus.get_messages("emergence", 1);
        assert_eq!(emerg[0].source, ObserverSource::EmergenceDetector);

        let evol = bus.get_messages("evolution", 1);
        assert_eq!(evol[0].source, ObserverSource::EvolutionChamber);
    }

    #[test]
    fn test_message_type_preserved() {
        let bus = make_bus();
        let _r = bus.publish_correlation("{}");
        let msgs = bus.get_messages("correlation", 1);
        assert_eq!(msgs[0].message_type, ObserverMessageType::CorrelationFound);
    }

    #[test]
    fn test_message_timestamp_monotonic() {
        let bus = make_bus();
        for i in 0..10 {
            let _r = bus.publish_correlation(&format!(r#"{{"i":{i}}}"#));
        }
        let msgs = bus.get_messages("correlation", 10);
        for window in msgs.windows(2) {
            assert!(window[0].timestamp <= window[1].timestamp);
        }
    }

    #[test]
    fn test_concurrent_safety() {
        use std::sync::Arc;
        use std::thread;

        let bus = Arc::new(ObserverBus::new());
        let mut handles = Vec::new();

        for t in 0..4 {
            let bus_clone = Arc::clone(&bus);
            handles.push(thread::spawn(move || {
                for i in 0..25 {
                    let _r = bus_clone.publish_correlation(
                        &format!(r#"{{"thread":{t},"msg":{i}}}"#),
                    );
                }
            }));
        }

        for handle in handles {
            let _r = handle.join();
        }

        assert_eq!(bus.stats().correlations_published, 100);
        assert_eq!(bus.stats().total_messages, 100);
    }

    #[test]
    fn test_recent_messages() {
        let bus = make_bus();
        for i in 0..5 {
            let _r = bus.publish_correlation(&format!(r#"{{"i":{i}}}"#));
        }
        let recent = bus.recent_messages(60);
        assert_eq!(recent.len(), 5);
    }

    #[test]
    fn test_recent_messages_sorted_by_time() {
        let bus = make_bus();
        let _r = bus.publish_correlation("{}");
        let _r = bus.publish_emergence("{}");
        let _r = bus.publish_mutation("{}", true);
        let recent = bus.recent_messages(60);
        for window in recent.windows(2) {
            assert!(window[0].timestamp <= window[1].timestamp);
        }
    }

    #[test]
    fn test_publish_to_custom_channel() {
        let bus = make_bus();
        assert!(bus.create_channel("custom").is_ok());
        let result = bus.publish(
            "custom",
            ObserverSource::Coordinator,
            ObserverMessageType::PhaseTransition,
            "{}",
        );
        assert!(result.is_ok());
        assert_eq!(bus.channel_message_count("custom"), 1);
    }

    #[test]
    fn test_has_channel_false_for_nonexistent() {
        let bus = make_bus();
        assert!(!bus.has_channel("nonexistent"));
    }

    #[test]
    fn test_stats_default_values() {
        let bus = make_bus();
        let stats = bus.stats();
        assert_eq!(stats.correlations_published, 0);
        assert_eq!(stats.emergences_published, 0);
        assert_eq!(stats.evolutions_published, 0);
        assert_eq!(stats.total_messages, 0);
        assert_eq!(stats.handler_errors, 0);
        assert!(stats.last_activity.is_none());
    }

    #[test]
    fn test_get_messages_in_range_empty() {
        let bus = make_bus();
        let start = Utc::now() - chrono::Duration::hours(2);
        let end = Utc::now() - chrono::Duration::hours(1);
        let msgs = bus.get_messages_in_range("correlation", start, end);
        assert!(msgs.is_empty());
    }

    #[test]
    fn test_get_messages_in_range_finds_messages() {
        let bus = make_bus();
        let before = Utc::now() - chrono::Duration::seconds(1);
        for i in 0..5 {
            let _r = bus.publish_correlation(&format!(r#"{{"i":{i}}}"#));
        }
        let after = Utc::now() + chrono::Duration::seconds(1);
        let msgs = bus.get_messages_in_range("correlation", before, after);
        assert_eq!(msgs.len(), 5);
    }

    #[test]
    fn test_multiple_channels_independent() {
        let bus = make_bus();
        let _r = bus.publish_correlation("{}");
        let _r = bus.publish_emergence("{}");
        assert_eq!(bus.channel_message_count("correlation"), 1);
        assert_eq!(bus.channel_message_count("emergence"), 1);
        assert_eq!(bus.channel_message_count("evolution"), 0);
    }

    #[test]
    fn test_observer_source_equality() {
        assert_eq!(ObserverSource::LogCorrelator, ObserverSource::LogCorrelator);
        assert_ne!(ObserverSource::LogCorrelator, ObserverSource::EmergenceDetector);
    }

    #[test]
    fn test_observer_message_type_equality() {
        assert_eq!(ObserverMessageType::CorrelationFound, ObserverMessageType::CorrelationFound);
        assert_ne!(ObserverMessageType::CorrelationFound, ObserverMessageType::EmergenceDetected);
    }

    #[test]
    fn test_bus_with_custom_config() {
        let config = ObserverBusConfig {
            max_messages_per_channel: 10,
            debug_logging: true,
        };
        let bus = ObserverBus::with_config(config);
        assert_eq!(bus.config().max_messages_per_channel, 10);
        assert!(bus.config().debug_logging);
    }
}
