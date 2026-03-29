//! # M23: Event Bus
//!
//! Pub/sub event distribution system for the Maintenance Engine.
//! Provides channel-based event publishing, subscription management,
//! and event history tracking across all integration layers.
//!
//! ## Layer: L4 (Integration)
//! ## Dependencies: M01 (Error)
//!
//! ## Features
//!
//! - Named channel creation and management
//! - Subscriber registration with optional event type filters
//! - Event publishing with delivery tracking
//! - Rolling event log (capped at 1000 entries)
//! - Default channels for core subsystems
//!
//! ## Default Channels
//!
//! | Channel | Purpose |
//! |---------|---------|
//! | health | Health check events |
//! | remediation | Auto-remediation events |
//! | learning | Hebbian/STDP learning events |
//! | consensus | PBFT consensus events |
//! | integration | Service bridge events |
//! | metrics | Performance metrics events |
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L04_INTEGRATION.md)

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use uuid::Uuid;

use crate::{Error, Result};

/// Maximum number of event records retained in the bus log.
const EVENT_LOG_CAPACITY: usize = 1000;

/// Default channel names created on bus initialization.
const DEFAULT_CHANNELS: &[&str] = &[
    "health",
    "remediation",
    "learning",
    "consensus",
    "integration",
    "metrics",
    "gc",
];

/// A record of a published event, including delivery metadata.
#[derive(Clone, Debug)]
pub struct EventRecord {
    /// Unique event identifier (UUID v4).
    pub id: String,
    /// Channel the event was published to.
    pub channel: String,
    /// Event type tag (application-defined).
    pub event_type: String,
    /// JSON-serialized event payload.
    pub payload: String,
    /// Identifier of the publishing source.
    pub source: String,
    /// Timestamp when the event was published.
    pub timestamp: DateTime<Utc>,
    /// List of subscriber IDs the event was delivered to.
    pub delivered_to: Vec<String>,
}

/// A subscription binding a subscriber to a channel.
#[derive(Clone, Debug)]
pub struct Subscription {
    /// Unique subscriber identifier.
    pub subscriber_id: String,
    /// Channel this subscription is bound to.
    pub channel: String,
    /// Optional event type filter; when `Some`, only events matching this type are delivered.
    pub filter: Option<String>,
    /// Timestamp when the subscription was created.
    pub created_at: DateTime<Utc>,
}

/// Metadata about a named channel.
#[derive(Clone, Debug)]
pub struct ChannelInfo {
    /// Channel name.
    pub name: String,
    /// Total number of events published to this channel.
    pub event_count: u64,
    /// Current number of active subscribers.
    pub subscriber_count: usize,
    /// Timestamp when the channel was created.
    pub created_at: DateTime<Utc>,
}

/// R21: Trait for receiving event callbacks from the `EventBus`.
///
/// Implementors receive synchronous `on_event` calls during `publish()`,
/// enabling active event delivery instead of passive polling. This is
/// the fundamental architectural upgrade that turns the `EventBus` from
/// bookkeeping into an active nervous system.
///
/// # Important
///
/// Callbacks MUST NOT acquire the `EventBus`'s internal locks (deadlock risk).
/// Keep callbacks fast (< 1ms) — defer heavy work to background tasks.
pub trait EventSubscriber: Send + Sync {
    /// Called synchronously during `publish()` for each matching event.
    fn on_event(&self, event: &EventRecord);
}

/// A pub/sub event bus for distributing events across the Maintenance Engine.
///
/// Supports named channels, subscriber management, event publishing with
/// optional type-based filtering, callback delivery (R21), and an audit
/// log of all published events.
pub struct EventBus {
    /// Subscriptions per channel name.
    subscribers: RwLock<HashMap<String, Vec<Subscription>>>,
    /// Rolling event log (capped at [`EVENT_LOG_CAPACITY`]).
    event_log: RwLock<Vec<EventRecord>>,
    /// Channel metadata, keyed by channel name.
    channels: RwLock<HashMap<String, ChannelInfo>>,
    /// R21: Registered callback subscribers keyed by subscriber ID.
    callbacks: RwLock<HashMap<String, std::sync::Arc<dyn EventSubscriber>>>,
}

impl EventBus {
    /// Create a new `EventBus` with the default set of channels.
    ///
    /// The following channels are created automatically:
    /// `health`, `remediation`, `learning`, `consensus`, `integration`, `metrics`.
    #[must_use]
    pub fn new() -> Self {
        let mut channels_map = HashMap::new();
        let mut subscribers_map = HashMap::new();

        for &name in DEFAULT_CHANNELS {
            channels_map.insert(
                name.into(),
                ChannelInfo {
                    name: name.into(),
                    event_count: 0,
                    subscriber_count: 0,
                    created_at: Utc::now(),
                },
            );
            subscribers_map.insert(name.to_string(), Vec::new());
        }

        Self {
            subscribers: RwLock::new(subscribers_map),
            event_log: RwLock::new(Vec::with_capacity(EVENT_LOG_CAPACITY)),
            channels: RwLock::new(channels_map),
            callbacks: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new named channel.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if:
    /// - The channel name is empty.
    /// - A channel with the same name already exists.
    pub fn create_channel(&self, name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(Error::Validation("channel name must not be empty".into()));
        }

        {
            let mut channels = self.channels.write();
            if channels.contains_key(name) {
                return Err(Error::Validation(format!(
                    "channel '{name}' already exists"
                )));
            }
            channels.insert(
                name.into(),
                ChannelInfo {
                    name: name.into(),
                    event_count: 0,
                    subscriber_count: 0,
                    created_at: Utc::now(),
                },
            );
        } // channels guard dropped

        self.subscribers.write().insert(name.into(), Vec::new());

        Ok(())
    }

    /// Subscribe to a channel with an optional event type filter.
    ///
    /// If `filter` is `Some`, only events whose `event_type` matches the filter
    /// string will be considered as delivered to this subscriber.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `subscriber_id` is empty.
    /// Returns [`Error::ServiceNotFound`] if the channel does not exist.
    pub fn subscribe(
        &self,
        subscriber_id: &str,
        channel: &str,
        filter: Option<String>,
    ) -> Result<()> {
        if subscriber_id.is_empty() {
            return Err(Error::Validation("subscriber_id must not be empty".into()));
        }

        // Verify channel exists
        if !self.channels.read().contains_key(channel) {
            return Err(Error::ServiceNotFound(format!(
                "channel '{channel}' does not exist"
            )));
        }

        let subscription = Subscription {
            subscriber_id: subscriber_id.into(),
            channel: channel.into(),
            filter,
            created_at: Utc::now(),
        };

        let new_count = {
            let mut subs = self.subscribers.write();
            let channel_subs = subs.entry(channel.into()).or_default();

            // Prevent duplicate subscriptions from the same subscriber
            let already_subscribed = channel_subs
                .iter()
                .any(|s| s.subscriber_id == subscriber_id);
            if !already_subscribed {
                channel_subs.push(subscription);
            }
            let count = channel_subs.len();
            drop(subs);
            count
        };

        // Update channel subscriber count
        if let Some(info) = self.channels.write().get_mut(channel) {
            info.subscriber_count = new_count;
        }

        Ok(())
    }

    /// Unsubscribe a subscriber from a channel.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] if the channel does not exist.
    pub fn unsubscribe(&self, subscriber_id: &str, channel: &str) -> Result<()> {
        // Verify channel exists
        if !self.channels.read().contains_key(channel) {
            return Err(Error::ServiceNotFound(format!(
                "channel '{channel}' does not exist"
            )));
        }

        let new_count = {
            let mut subs = self.subscribers.write();
            let count = subs.get_mut(channel).map_or(0, |channel_subs| {
                channel_subs.retain(|s| s.subscriber_id != subscriber_id);
                channel_subs.len()
            });
            drop(subs);
            count
        };

        // Update channel subscriber count
        if let Some(info) = self.channels.write().get_mut(channel) {
            info.subscriber_count = new_count;
        }

        Ok(())
    }

    /// R21: Register a callback subscriber for active event delivery.
    ///
    /// When an event is published to a channel where this subscriber is
    /// registered, the callback's `on_event` method is called synchronously.
    /// The subscriber must also be subscribed to the channel via `subscribe()`.
    ///
    /// # Panics
    ///
    /// Does not panic — duplicate registrations silently overwrite.
    pub fn register_callback(
        &self,
        subscriber_id: &str,
        callback: std::sync::Arc<dyn EventSubscriber>,
    ) {
        self.callbacks
            .write()
            .insert(subscriber_id.to_string(), callback);
    }

    /// Publish an event to a channel.
    ///
    /// The event is delivered to all subscribers on the channel. If a subscriber
    /// has a filter set, the event is only considered delivered if `event_type`
    /// matches the filter.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `event_type` or `source` is empty.
    /// Returns [`Error::ServiceNotFound`] if the channel does not exist.
    pub fn publish(
        &self,
        channel: &str,
        event_type: &str,
        payload: &str,
        source: &str,
    ) -> Result<EventRecord> {
        if event_type.is_empty() {
            return Err(Error::Validation("event_type must not be empty".into()));
        }
        if source.is_empty() {
            return Err(Error::Validation("source must not be empty".into()));
        }

        // Verify channel exists
        if !self.channels.read().contains_key(channel) {
            return Err(Error::ServiceNotFound(format!(
                "channel '{channel}' does not exist"
            )));
        }

        // Determine delivery targets
        let delivered_to: Vec<String> = self
            .subscribers
            .read()
            .get(channel)
            .map(|subs| {
                subs.iter()
                    .filter(|s| s.filter.as_ref().is_none_or(|f| f == event_type))
                    .map(|s| s.subscriber_id.clone())
                    .collect()
            })
            .unwrap_or_default();

        let record = EventRecord {
            id: Uuid::new_v4().to_string(),
            channel: channel.into(),
            event_type: event_type.into(),
            payload: payload.into(),
            source: source.into(),
            timestamp: Utc::now(),
            delivered_to,
        };

        // Append to event log (enforce capacity)
        {
            let mut log = self.event_log.write();
            if log.len() >= EVENT_LOG_CAPACITY {
                log.remove(0);
            }
            log.push(record.clone());
        } // log guard dropped

        // Increment channel event count
        if let Some(info) = self.channels.write().get_mut(channel) {
            info.event_count += 1;
        }

        // R21: Invoke registered callbacks for matching subscribers.
        // Callbacks are called synchronously — keep them fast (< 1ms).
        // Lock ordering: callbacks lock acquired AFTER event_log and channels
        // locks are released, preventing deadlock.
        {
            let cbs = self.callbacks.read();
            for subscriber_id in &record.delivered_to {
                if let Some(cb) = cbs.get(subscriber_id) {
                    cb.on_event(&record);
                }
            }
        }

        Ok(record)
    }

    /// Retrieve the most recent events for a channel, up to `limit`.
    ///
    /// Events are returned in chronological order (oldest first).
    #[must_use]
    pub fn get_events(&self, channel: &str, limit: usize) -> Vec<EventRecord> {
        let channel_events: Vec<EventRecord> = self
            .event_log
            .read()
            .iter()
            .filter(|e| e.channel == channel)
            .cloned()
            .collect();

        // Return the last `limit` events
        let start = channel_events.len().saturating_sub(limit);
        channel_events[start..].to_vec()
    }

    /// Retrieve all current subscriptions for a channel.
    #[must_use]
    pub fn get_subscribers(&self, channel: &str) -> Vec<Subscription> {
        self.subscribers
            .read()
            .get(channel)
            .cloned()
            .unwrap_or_default()
    }

    /// Return the total number of channels.
    #[must_use]
    pub fn channel_count(&self) -> usize {
        self.channels.read().len()
    }

    /// Return the total number of events published across all channels.
    #[must_use]
    pub fn total_events(&self) -> u64 {
        self.channels.read().values().map(|info| info.event_count).sum()
    }

    /// Retrieve metadata for a specific channel.
    #[must_use]
    pub fn get_channel_info(&self, name: &str) -> Option<ChannelInfo> {
        self.channels.read().get(name).cloned()
    }

    /// List all channel names.
    #[must_use]
    pub fn list_channels(&self) -> Vec<String> {
        self.channels.read().keys().cloned().collect()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_channel() {
        let bus = EventBus::new();
        let initial = bus.channel_count();
        assert!(bus.create_channel("custom").is_ok());
        assert_eq!(bus.channel_count(), initial + 1);

        // Duplicate should fail
        assert!(bus.create_channel("custom").is_err());

        // Empty name should fail
        assert!(bus.create_channel("").is_err());
    }

    #[test]
    fn test_subscribe() {
        let bus = EventBus::new();
        assert!(bus.subscribe("agent-1", "health", None).is_ok());

        let subs = bus.get_subscribers("health");
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].subscriber_id, "agent-1");
        assert!(subs[0].filter.is_none());
    }

    #[test]
    fn test_subscribe_with_filter() {
        let bus = EventBus::new();
        assert!(
            bus.subscribe("agent-2", "health", Some("critical".into()))
                .is_ok()
        );

        let subs = bus.get_subscribers("health");
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].filter.as_deref(), Some("critical"));
    }

    #[test]
    fn test_subscribe_nonexistent_channel() {
        let bus = EventBus::new();
        let result = bus.subscribe("agent-1", "nonexistent", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_subscribe_prevents_duplicates() {
        let bus = EventBus::new();
        assert!(bus.subscribe("agent-1", "health", None).is_ok());
        assert!(bus.subscribe("agent-1", "health", None).is_ok()); // no error, but no duplicate

        let subs = bus.get_subscribers("health");
        assert_eq!(subs.len(), 1);
    }

    #[test]
    fn test_publish() {
        let bus = EventBus::new();
        assert!(bus.subscribe("agent-1", "health", None).is_ok());
        assert!(bus.subscribe("agent-2", "health", None).is_ok());

        let result = bus.publish(
            "health",
            "status_change",
            r#"{"service":"synthex","status":"degraded"}"#,
            "monitor",
        );
        assert!(result.is_ok());

        let record = result.unwrap_or_else(|_| EventRecord {
            id: String::new(),
            channel: String::new(),
            event_type: String::new(),
            payload: String::new(),
            source: String::new(),
            timestamp: Utc::now(),
            delivered_to: Vec::new(),
        });

        assert_eq!(record.channel, "health");
        assert_eq!(record.event_type, "status_change");
        assert_eq!(record.delivered_to.len(), 2);
        assert!(record.delivered_to.contains(&"agent-1".to_string()));
        assert!(record.delivered_to.contains(&"agent-2".to_string()));
    }

    #[test]
    fn test_event_delivery_with_filter() {
        let bus = EventBus::new();

        // agent-1 subscribes to all events
        assert!(bus.subscribe("agent-1", "health", None).is_ok());
        // agent-2 subscribes only to "critical" events
        assert!(
            bus.subscribe("agent-2", "health", Some("critical".into()))
                .is_ok()
        );

        // Publish a non-critical event
        let result = bus.publish("health", "info", r#"{"msg":"ok"}"#, "monitor");
        assert!(result.is_ok());
        let record = result.unwrap_or_else(|_| EventRecord {
            id: String::new(),
            channel: String::new(),
            event_type: String::new(),
            payload: String::new(),
            source: String::new(),
            timestamp: Utc::now(),
            delivered_to: Vec::new(),
        });
        // Only agent-1 should receive (no filter), agent-2 filters for "critical"
        assert_eq!(record.delivered_to.len(), 1);
        assert!(record.delivered_to.contains(&"agent-1".to_string()));

        // Publish a critical event
        let result2 = bus.publish("health", "critical", r#"{"msg":"down"}"#, "monitor");
        assert!(result2.is_ok());
        let record2 = result2.unwrap_or_else(|_| EventRecord {
            id: String::new(),
            channel: String::new(),
            event_type: String::new(),
            payload: String::new(),
            source: String::new(),
            timestamp: Utc::now(),
            delivered_to: Vec::new(),
        });
        // Both should receive
        assert_eq!(record2.delivered_to.len(), 2);
    }

    #[test]
    fn test_unsubscribe() {
        let bus = EventBus::new();
        assert!(bus.subscribe("agent-1", "health", None).is_ok());
        assert!(bus.subscribe("agent-2", "health", None).is_ok());

        assert_eq!(bus.get_subscribers("health").len(), 2);

        assert!(bus.unsubscribe("agent-1", "health").is_ok());
        assert_eq!(bus.get_subscribers("health").len(), 1);
        assert_eq!(bus.get_subscribers("health")[0].subscriber_id, "agent-2");
    }

    #[test]
    fn test_unsubscribe_nonexistent_channel() {
        let bus = EventBus::new();
        let result = bus.unsubscribe("agent-1", "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_events() {
        let bus = EventBus::new();
        assert!(bus.subscribe("agent-1", "metrics", None).is_ok());

        for i in 0..10 {
            let _r = bus.publish(
                "metrics",
                "cpu",
                &format!(r#"{{"value":{i}}}"#),
                "collector",
            );
        }

        // Get last 5 events
        let events = bus.get_events("metrics", 5);
        assert_eq!(events.len(), 5);

        // Get all events
        let all_events = bus.get_events("metrics", 100);
        assert_eq!(all_events.len(), 10);

        // No events on a different channel
        let empty = bus.get_events("health", 10);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_channel_count() {
        let bus = EventBus::new();
        assert_eq!(bus.channel_count(), DEFAULT_CHANNELS.len());

        assert!(bus.create_channel("extra").is_ok());
        assert_eq!(bus.channel_count(), DEFAULT_CHANNELS.len() + 1);
    }

    #[test]
    fn test_default_channels() {
        let bus = EventBus::new();
        let channels = bus.list_channels();
        for &expected in DEFAULT_CHANNELS {
            assert!(
                channels.contains(&expected.to_string()),
                "missing default channel: {expected}"
            );
        }
    }

    #[test]
    fn test_total_events() {
        let bus = EventBus::new();
        assert_eq!(bus.total_events(), 0);

        let _r1 = bus.publish("health", "ping", "{}", "test");
        let _r2 = bus.publish("metrics", "cpu", "{}", "test");
        let _r3 = bus.publish("health", "pong", "{}", "test");

        assert_eq!(bus.total_events(), 3);
    }

    #[test]
    fn test_publish_to_nonexistent_channel() {
        let bus = EventBus::new();
        let result = bus.publish("nonexistent", "test", "{}", "source");
        assert!(result.is_err());
    }

    #[test]
    fn test_publish_empty_event_type() {
        let bus = EventBus::new();
        let result = bus.publish("health", "", "{}", "source");
        assert!(result.is_err());
    }

    #[test]
    fn test_event_log_capacity() {
        let bus = EventBus::new();

        for i in 0..(EVENT_LOG_CAPACITY + 100) {
            let _r = bus.publish(
                "health",
                "tick",
                &format!(r#"{{"seq":{i}}}"#),
                "test",
            );
        }

        let log_len = bus.event_log.read().len();
        assert!(
            log_len <= EVENT_LOG_CAPACITY,
            "log exceeded capacity: {log_len} > {EVENT_LOG_CAPACITY}",
        );
    }

    #[test]
    fn test_channel_info() {
        let bus = EventBus::new();
        let info = bus.get_channel_info("health");
        assert!(info.is_some());
        let info = info.unwrap_or_else(|| ChannelInfo {
            name: String::new(),
            event_count: u64::MAX,
            subscriber_count: usize::MAX,
            created_at: Utc::now(),
        });
        assert_eq!(info.name, "health");
        assert_eq!(info.event_count, 0);
    }

    // --- Additional tests to reach 50+ ---

    #[test]
    fn test_default_creates_same_as_new() {
        let d = EventBus::default();
        let n = EventBus::new();
        assert_eq!(d.channel_count(), n.channel_count());
    }

    #[test]
    fn test_subscribe_empty_subscriber_id_fails() {
        let bus = EventBus::new();
        let result = bus.subscribe("", "health", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_publish_empty_source_fails() {
        let bus = EventBus::new();
        let result = bus.publish("health", "tick", "{}", "");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_channel_then_subscribe() {
        let bus = EventBus::new();
        assert!(bus.create_channel("custom").is_ok());
        assert!(bus.subscribe("agent-1", "custom", None).is_ok());
        let subs = bus.get_subscribers("custom");
        assert_eq!(subs.len(), 1);
    }

    #[test]
    fn test_create_channel_then_publish() {
        let bus = EventBus::new();
        assert!(bus.create_channel("custom-events").is_ok());
        let result = bus.publish("custom-events", "tick", r#"{"v":1}"#, "source");
        assert!(result.is_ok());
    }

    #[test]
    fn test_publish_increments_channel_event_count() {
        let bus = EventBus::new();
        let _r1 = bus.publish("health", "ping", "{}", "src");
        let _r2 = bus.publish("health", "pong", "{}", "src");
        let info = bus.get_channel_info("health");
        assert!(info.is_some());
        let info = info.unwrap_or_else(|| ChannelInfo {
            name: String::new(),
            event_count: 0,
            subscriber_count: 0,
            created_at: Utc::now(),
        });
        assert_eq!(info.event_count, 2);
    }

    #[test]
    fn test_subscribe_updates_channel_subscriber_count() {
        let bus = EventBus::new();
        assert!(bus.subscribe("a", "health", None).is_ok());
        assert!(bus.subscribe("b", "health", None).is_ok());
        let info = bus.get_channel_info("health");
        assert!(info.is_some());
        let info = info.unwrap_or_else(|| ChannelInfo {
            name: String::new(),
            event_count: 0,
            subscriber_count: 0,
            created_at: Utc::now(),
        });
        assert_eq!(info.subscriber_count, 2);
    }

    #[test]
    fn test_unsubscribe_updates_channel_subscriber_count() {
        let bus = EventBus::new();
        assert!(bus.subscribe("a", "health", None).is_ok());
        assert!(bus.subscribe("b", "health", None).is_ok());
        assert!(bus.unsubscribe("a", "health").is_ok());
        let info = bus.get_channel_info("health");
        assert!(info.is_some());
        let info = info.unwrap_or_else(|| ChannelInfo {
            name: String::new(),
            event_count: 0,
            subscriber_count: 0,
            created_at: Utc::now(),
        });
        assert_eq!(info.subscriber_count, 1);
    }

    #[test]
    fn test_unsubscribe_nonexistent_subscriber_is_ok() {
        let bus = EventBus::new();
        // Unsubscribing a non-existent subscriber from existing channel should succeed
        let result = bus.unsubscribe("ghost", "health");
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_events_pagination_limit() {
        let bus = EventBus::new();
        for i in 0..20 {
            let _r = bus.publish("health", "tick", &format!("{i}"), "src");
        }
        let events = bus.get_events("health", 5);
        assert_eq!(events.len(), 5);
        // Should be the last 5
        assert_eq!(events[0].payload, "15");
        assert_eq!(events[4].payload, "19");
    }

    #[test]
    fn test_get_events_limit_larger_than_total() {
        let bus = EventBus::new();
        let _r = bus.publish("health", "tick", "{}", "src");
        let events = bus.get_events("health", 1000);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_get_events_zero_limit_returns_empty() {
        let bus = EventBus::new();
        let _r = bus.publish("health", "tick", "{}", "src");
        let events = bus.get_events("health", 0);
        assert!(events.is_empty());
    }

    #[test]
    fn test_get_events_empty_channel() {
        let bus = EventBus::new();
        let events = bus.get_events("remediation", 10);
        assert!(events.is_empty());
    }

    #[test]
    fn test_get_subscribers_empty_channel() {
        let bus = EventBus::new();
        let subs = bus.get_subscribers("health");
        assert!(subs.is_empty());
    }

    #[test]
    fn test_get_subscribers_nonexistent_channel() {
        let bus = EventBus::new();
        let subs = bus.get_subscribers("nonexistent");
        assert!(subs.is_empty());
    }

    #[test]
    fn test_get_channel_info_nonexistent() {
        let bus = EventBus::new();
        let info = bus.get_channel_info("nonexistent");
        assert!(info.is_none());
    }

    #[test]
    fn test_list_channels_includes_all_defaults() {
        let bus = EventBus::new();
        let channels = bus.list_channels();
        assert_eq!(channels.len(), DEFAULT_CHANNELS.len());
    }

    #[test]
    fn test_list_channels_after_create() {
        let bus = EventBus::new();
        assert!(bus.create_channel("new-channel").is_ok());
        let channels = bus.list_channels();
        assert!(channels.contains(&"new-channel".to_string()));
    }

    #[test]
    fn test_event_record_has_uuid_id() {
        let bus = EventBus::new();
        let result = bus.publish("health", "tick", "{}", "src");
        assert!(result.is_ok());
        let record = result.unwrap_or_else(|_| EventRecord {
            id: String::new(),
            channel: String::new(),
            event_type: String::new(),
            payload: String::new(),
            source: String::new(),
            timestamp: Utc::now(),
            delivered_to: Vec::new(),
        });
        assert_eq!(record.id.len(), 36);
    }

    #[test]
    fn test_event_record_contains_payload() {
        let bus = EventBus::new();
        let result = bus.publish("health", "tick", r#"{"key":"value"}"#, "src");
        assert!(result.is_ok());
        let record = result.unwrap_or_else(|_| EventRecord {
            id: String::new(),
            channel: String::new(),
            event_type: String::new(),
            payload: String::new(),
            source: String::new(),
            timestamp: Utc::now(),
            delivered_to: Vec::new(),
        });
        assert_eq!(record.payload, r#"{"key":"value"}"#);
    }

    #[test]
    fn test_event_record_contains_source() {
        let bus = EventBus::new();
        let result = bus.publish("health", "tick", "{}", "monitor-42");
        assert!(result.is_ok());
        let record = result.unwrap_or_else(|_| EventRecord {
            id: String::new(),
            channel: String::new(),
            event_type: String::new(),
            payload: String::new(),
            source: String::new(),
            timestamp: Utc::now(),
            delivered_to: Vec::new(),
        });
        assert_eq!(record.source, "monitor-42");
    }

    #[test]
    fn test_publish_no_subscribers_delivers_to_none() {
        let bus = EventBus::new();
        let result = bus.publish("health", "tick", "{}", "src");
        assert!(result.is_ok());
        let record = result.unwrap_or_else(|_| EventRecord {
            id: String::new(),
            channel: String::new(),
            event_type: String::new(),
            payload: String::new(),
            source: String::new(),
            timestamp: Utc::now(),
            delivered_to: Vec::new(),
        });
        assert!(record.delivered_to.is_empty());
    }

    #[test]
    fn test_multiple_filters_on_same_channel() {
        let bus = EventBus::new();
        assert!(bus.subscribe("a1", "health", Some("critical".into())).is_ok());
        assert!(bus.subscribe("a2", "health", Some("warning".into())).is_ok());
        assert!(bus.subscribe("a3", "health", None).is_ok());

        let result = bus.publish("health", "critical", "{}", "src");
        assert!(result.is_ok());
        let record = result.unwrap_or_else(|_| EventRecord {
            id: String::new(),
            channel: String::new(),
            event_type: String::new(),
            payload: String::new(),
            source: String::new(),
            timestamp: Utc::now(),
            delivered_to: Vec::new(),
        });
        // a1 (filter=critical matches) + a3 (no filter) = 2
        assert_eq!(record.delivered_to.len(), 2);
        assert!(record.delivered_to.contains(&"a1".to_string()));
        assert!(record.delivered_to.contains(&"a3".to_string()));
    }

    #[test]
    fn test_total_events_across_channels() {
        let bus = EventBus::new();
        let _r1 = bus.publish("health", "t", "{}", "s");
        let _r2 = bus.publish("metrics", "t", "{}", "s");
        let _r3 = bus.publish("learning", "t", "{}", "s");
        let _r4 = bus.publish("learning", "t", "{}", "s");
        assert_eq!(bus.total_events(), 4);
    }

    #[test]
    fn test_channel_count_unchanged_after_publish() {
        let bus = EventBus::new();
        let before = bus.channel_count();
        let _r = bus.publish("health", "t", "{}", "s");
        assert_eq!(bus.channel_count(), before);
    }

    #[test]
    fn test_subscribe_to_multiple_channels() {
        let bus = EventBus::new();
        assert!(bus.subscribe("agent-x", "health", None).is_ok());
        assert!(bus.subscribe("agent-x", "metrics", None).is_ok());
        let health_subs = bus.get_subscribers("health");
        let metrics_subs = bus.get_subscribers("metrics");
        assert_eq!(health_subs.len(), 1);
        assert_eq!(metrics_subs.len(), 1);
    }

    #[test]
    fn test_unsubscribe_only_removes_from_target_channel() {
        let bus = EventBus::new();
        assert!(bus.subscribe("agent-x", "health", None).is_ok());
        assert!(bus.subscribe("agent-x", "metrics", None).is_ok());
        assert!(bus.unsubscribe("agent-x", "health").is_ok());
        assert!(bus.get_subscribers("health").is_empty());
        assert_eq!(bus.get_subscribers("metrics").len(), 1);
    }

    #[test]
    fn test_event_log_capacity_at_boundary() {
        let bus = EventBus::new();
        // Fill to exactly capacity
        for i in 0..EVENT_LOG_CAPACITY {
            let _r = bus.publish("health", "tick", &format!("{i}"), "s");
        }
        let log_len = bus.event_log.read().len();
        assert_eq!(log_len, EVENT_LOG_CAPACITY);

        // One more should evict the first
        let _r = bus.publish("health", "tick", "overflow", "s");
        let log_len = bus.event_log.read().len();
        assert_eq!(log_len, EVENT_LOG_CAPACITY);
    }

    #[test]
    fn test_subscription_clone() {
        let sub = Subscription {
            subscriber_id: "agent-1".into(),
            channel: "health".into(),
            filter: Some("critical".into()),
            created_at: Utc::now(),
        };
        let cloned = sub.clone();
        assert_eq!(cloned.subscriber_id, "agent-1");
        assert_eq!(cloned.channel, "health");
        assert_eq!(cloned.filter.as_deref(), Some("critical"));
    }

    #[test]
    fn test_channel_info_clone() {
        let info = ChannelInfo {
            name: "test".into(),
            event_count: 42,
            subscriber_count: 5,
            created_at: Utc::now(),
        };
        let cloned = info.clone();
        assert_eq!(cloned.name, "test");
        assert_eq!(cloned.event_count, 42);
        assert_eq!(cloned.subscriber_count, 5);
    }

    #[test]
    fn test_event_record_clone() {
        let record = EventRecord {
            id: "test-id".into(),
            channel: "health".into(),
            event_type: "tick".into(),
            payload: "{}".into(),
            source: "src".into(),
            timestamp: Utc::now(),
            delivered_to: vec!["a".into(), "b".into()],
        };
        let cloned = record.clone();
        assert_eq!(cloned.id, "test-id");
        assert_eq!(cloned.delivered_to.len(), 2);
    }

    #[test]
    fn test_many_subscribers_one_channel() {
        let bus = EventBus::new();
        for i in 0..20 {
            let sub_id = format!("agent-{i}");
            assert!(bus.subscribe(&sub_id, "health", None).is_ok());
        }
        let subs = bus.get_subscribers("health");
        assert_eq!(subs.len(), 20);

        let result = bus.publish("health", "tick", "{}", "src");
        assert!(result.is_ok());
        let record = result.unwrap_or_else(|_| EventRecord {
            id: String::new(),
            channel: String::new(),
            event_type: String::new(),
            payload: String::new(),
            source: String::new(),
            timestamp: Utc::now(),
            delivered_to: Vec::new(),
        });
        assert_eq!(record.delivered_to.len(), 20);
    }

    #[test]
    fn test_create_channel_duplicate_fails() {
        let bus = EventBus::new();
        // "health" already exists as default
        assert!(bus.create_channel("health").is_err());
    }
}
