//! # M07: Signal Bus
//!
//! Typed synchronous signal bus for cross-module coordination within L1 Foundation.
//! Provides three signal channels (health, learning, dissent) and a subscriber-based
//! delivery mechanism modelled after [`MetricsRegistry`](super::MetricsRegistry).
//!
//! ## Layer: L1 (Foundation)
//! ## Dependencies: M00 (`shared_types`), NAM primitives (`nam`)
//!
//! ## Signal Channels
//!
//! | Channel | Signal | Purpose |
//! |---------|--------|---------|
//! | Health | [`HealthSignal`] | Module health transitions |
//! | Learning | [`LearningEvent`] | Wrapped [`LearningSignal`](super::LearningSignal) with timing |
//! | Dissent | [`DissentEvent`] | Wrapped [`Dissent`](super::Dissent) with source module |
//!
//! ## Design Invariants
//!
//! - Synchronous delivery (no async, no channels)
//! - Bounded subscriber capacity (default 256)
//! - Subscribers must be `Send + Sync`
//! - Default no-op implementations on [`SignalSubscriber`]

use std::fmt;
use std::sync::Arc;

use parking_lot::RwLock;

use super::nam::{Dissent, LearningSignal};
use super::shared_types::{ModuleId, Timestamp};

// ============================================================================
// SignalContext
// ============================================================================

/// Additional context attached to every signal emission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalContext {
    /// Which module emitted the signal.
    pub source_module: ModuleId,
    /// When the signal was emitted.
    pub timestamp: Timestamp,
    /// Optional correlation ID for tracing.
    pub correlation_id: Option<String>,
}

impl SignalContext {
    /// Create a new context for a given source module.
    #[must_use]
    pub fn new(source_module: ModuleId) -> Self {
        Self {
            source_module,
            timestamp: Timestamp::now(),
            correlation_id: None,
        }
    }

    /// Attach a correlation ID.
    #[must_use]
    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    /// Override the timestamp (for testing).
    #[must_use]
    pub const fn with_timestamp(mut self, timestamp: Timestamp) -> Self {
        self.timestamp = timestamp;
        self
    }
}

impl fmt::Display for SignalContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ctx({} at {})", self.source_module, self.timestamp)
    }
}

// ============================================================================
// HealthSignal
// ============================================================================

/// Signal emitted when a module's health changes.
#[derive(Debug, Clone, PartialEq)]
pub struct HealthSignal {
    /// Which module changed health.
    pub module_id: ModuleId,
    /// Previous health score (0.0–1.0).
    pub previous_health: f64,
    /// Current health score (0.0–1.0).
    pub current_health: f64,
    /// When this transition occurred.
    pub timestamp: Timestamp,
    /// Human-readable reason for the change.
    pub reason: String,
}

impl HealthSignal {
    /// Create a new health signal.
    ///
    /// Both health values are clamped to `[0.0, 1.0]`.
    #[must_use]
    pub fn new(
        module_id: ModuleId,
        previous_health: f64,
        current_health: f64,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            module_id,
            previous_health: previous_health.clamp(0.0, 1.0),
            current_health: current_health.clamp(0.0, 1.0),
            timestamp: Timestamp::now(),
            reason: reason.into(),
        }
    }

    /// Override the timestamp (for testing).
    #[must_use]
    pub const fn with_timestamp(mut self, timestamp: Timestamp) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Whether this represents a degradation (health decreased).
    #[must_use]
    pub fn is_degradation(&self) -> bool {
        self.current_health < self.previous_health
    }

    /// Whether this represents an improvement (health increased).
    #[must_use]
    pub fn is_improvement(&self) -> bool {
        self.current_health > self.previous_health
    }

    /// Signed delta (positive = improvement, negative = degradation).
    #[must_use]
    pub fn delta(&self) -> f64 {
        self.current_health - self.previous_health
    }
}

impl fmt::Display for HealthSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let arrow = if self.is_degradation() {
            "v"
        } else if self.is_improvement() {
            "^"
        } else {
            "="
        };
        write!(
            f,
            "Health({} {:.3}{}{:.3}: {})",
            self.module_id, self.previous_health, arrow, self.current_health, self.reason
        )
    }
}

// ============================================================================
// LearningEvent
// ============================================================================

/// A [`LearningSignal`] wrapped with timing and context for L5 STDP consumption.
#[derive(Debug, Clone, PartialEq)]
pub struct LearningEvent {
    /// The underlying learning signal from NAM.
    pub signal: LearningSignal,
    /// When this event occurred.
    pub timestamp: Timestamp,
    /// Emission context.
    pub context: SignalContext,
}

impl LearningEvent {
    /// Wrap a learning signal with timing and context.
    #[must_use]
    pub fn new(signal: LearningSignal, context: SignalContext) -> Self {
        Self {
            signal,
            timestamp: Timestamp::now(),
            context,
        }
    }

    /// Override the timestamp (for testing).
    #[must_use]
    pub const fn with_timestamp(mut self, timestamp: Timestamp) -> Self {
        self.timestamp = timestamp;
        self
    }
}

impl fmt::Display for LearningEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Learn({} {} mag={:.2} at {})",
            self.signal.source, self.signal.outcome, self.signal.magnitude, self.timestamp
        )
    }
}

// ============================================================================
// DissentEvent
// ============================================================================

/// A [`Dissent`] record wrapped with timing and source module identity.
#[derive(Debug, Clone, PartialEq)]
pub struct DissentEvent {
    /// The underlying dissent record from NAM.
    pub dissent: Dissent,
    /// When this event occurred.
    pub timestamp: Timestamp,
    /// Which module is the source of this dissent event.
    pub source_module: ModuleId,
}

impl DissentEvent {
    /// Wrap a dissent record with timing and source module.
    #[must_use]
    pub fn new(dissent: Dissent, source_module: ModuleId) -> Self {
        Self {
            dissent,
            timestamp: Timestamp::now(),
            source_module,
        }
    }

    /// Override the timestamp (for testing).
    #[must_use]
    pub const fn with_timestamp(mut self, timestamp: Timestamp) -> Self {
        self.timestamp = timestamp;
        self
    }
}

impl fmt::Display for DissentEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Dissent(from {} on '{}' at {})",
            self.source_module, self.dissent.target, self.timestamp
        )
    }
}

// ============================================================================
// Signal (unified enum)
// ============================================================================

/// Unified signal enum wrapping all three channel types.
#[derive(Debug, Clone)]
pub enum Signal {
    /// Health transition signal.
    Health(HealthSignal),
    /// Learning event signal.
    Learning(LearningEvent),
    /// Dissent event signal.
    Dissent(DissentEvent),
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Health(s) => write!(f, "{s}"),
            Self::Learning(s) => write!(f, "{s}"),
            Self::Dissent(s) => write!(f, "{s}"),
        }
    }
}

// ============================================================================
// SignalSubscriber
// ============================================================================

/// Trait for receiving signals from the [`SignalBus`].
///
/// All methods have default no-op implementations. Subscribers only need to
/// override the channels they care about.
pub trait SignalSubscriber: Send + Sync + fmt::Debug {
    /// Human-readable name for this subscriber (for diagnostics).
    fn name(&self) -> &str;

    /// Called when a health signal is emitted.
    fn on_health(&self, _signal: &HealthSignal) {}

    /// Called when a learning event is emitted.
    fn on_learning(&self, _event: &LearningEvent) {}

    /// Called when a dissent event is emitted.
    fn on_dissent(&self, _event: &DissentEvent) {}
}

// ============================================================================
// SignalBusConfig
// ============================================================================

/// Configuration for the [`SignalBus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalBusConfig {
    /// Maximum number of subscribers (default 256).
    pub max_subscribers: usize,
}

impl Default for SignalBusConfig {
    fn default() -> Self {
        Self {
            max_subscribers: 256,
        }
    }
}

impl SignalBusConfig {
    /// Create a config with a custom subscriber limit.
    #[must_use]
    pub const fn with_max_subscribers(mut self, max: usize) -> Self {
        self.max_subscribers = max;
        self
    }
}

impl fmt::Display for SignalBusConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SignalBusConfig(max_subs={})", self.max_subscribers)
    }
}

// ============================================================================
// SignalBusStats
// ============================================================================

/// Aggregate statistics for a [`SignalBus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SignalBusStats {
    /// Total health signals emitted.
    pub health_emitted: u64,
    /// Total learning events emitted.
    pub learning_emitted: u64,
    /// Total dissent events emitted.
    pub dissent_emitted: u64,
    /// Current subscriber count.
    pub subscriber_count: usize,
}

impl SignalBusStats {
    /// Total signals emitted across all channels.
    #[must_use]
    pub const fn total_emitted(&self) -> u64 {
        self.health_emitted + self.learning_emitted + self.dissent_emitted
    }
}

impl fmt::Display for SignalBusStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Stats(health={}, learning={}, dissent={}, subs={})",
            self.health_emitted, self.learning_emitted, self.dissent_emitted, self.subscriber_count
        )
    }
}

// ============================================================================
// SignalBus
// ============================================================================

/// Synchronous signal bus for cross-module coordination.
///
/// Follows the same `Arc<RwLock<Vec<...>>>` pattern as
/// [`MetricsRegistry`](super::MetricsRegistry). Subscribers are delivered
/// signals synchronously in registration order.
#[derive(Debug)]
pub struct SignalBus {
    subscribers: Arc<RwLock<Vec<Arc<dyn SignalSubscriber>>>>,
    config: SignalBusConfig,
    stats: Arc<RwLock<SignalBusStats>>,
}

impl Default for SignalBus {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalBus {
    /// Create a new signal bus with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(SignalBusConfig::default())
    }

    /// Create a new signal bus with custom configuration.
    #[must_use]
    pub fn with_config(config: SignalBusConfig) -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(Vec::new())),
            config,
            stats: Arc::new(RwLock::new(SignalBusStats::default())),
        }
    }

    /// Register a subscriber.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the subscriber limit has been reached.
    pub fn subscribe(
        &self,
        subscriber: Arc<dyn SignalSubscriber>,
    ) -> Result<(), crate::Error> {
        let mut subs = self.subscribers.write();
        if subs.len() >= self.config.max_subscribers {
            return Err(crate::Error::Config(format!(
                "SignalBus subscriber limit reached (max {})",
                self.config.max_subscribers
            )));
        }
        subs.push(subscriber);
        let count = subs.len();
        drop(subs);
        self.stats.write().subscriber_count = count;
        Ok(())
    }

    /// Emit a health signal to all subscribers.
    pub fn emit_health(&self, signal: &HealthSignal) {
        let subs = self.subscribers.read();
        for sub in subs.iter() {
            sub.on_health(signal);
        }
        drop(subs);
        self.stats.write().health_emitted += 1;
    }

    /// Emit a learning event to all subscribers.
    pub fn emit_learning(&self, event: &LearningEvent) {
        let subs = self.subscribers.read();
        for sub in subs.iter() {
            sub.on_learning(event);
        }
        drop(subs);
        self.stats.write().learning_emitted += 1;
    }

    /// Emit a dissent event to all subscribers.
    pub fn emit_dissent(&self, event: &DissentEvent) {
        let subs = self.subscribers.read();
        for sub in subs.iter() {
            sub.on_dissent(event);
        }
        drop(subs);
        self.stats.write().dissent_emitted += 1;
    }

    /// Return a snapshot of bus statistics.
    #[must_use]
    pub fn stats(&self) -> SignalBusStats {
        *self.stats.read()
    }

    /// Return the current subscriber count.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.read().len()
    }

    /// Return the bus configuration.
    #[must_use]
    pub const fn config(&self) -> &SignalBusConfig {
        &self.config
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::m1_foundation::nam::AgentOrigin;
    use std::sync::atomic::{AtomicU64, Ordering};

    // ==== SignalContext tests ====

    #[test]
    fn test_signal_context_new() {
        let ctx = SignalContext::new(ModuleId::M01);
        assert_eq!(ctx.source_module, ModuleId::M01);
        assert!(ctx.correlation_id.is_none());
    }

    #[test]
    fn test_signal_context_with_correlation_id() {
        let ctx = SignalContext::new(ModuleId::M02)
            .with_correlation_id("corr-001");
        assert_eq!(ctx.correlation_id.as_deref(), Some("corr-001"));
    }

    #[test]
    fn test_signal_context_with_timestamp() {
        let ts = Timestamp::from_raw(42);
        let ctx = SignalContext::new(ModuleId::M03).with_timestamp(ts);
        assert_eq!(ctx.timestamp, ts);
    }

    #[test]
    fn test_signal_context_display() {
        let ctx = SignalContext::new(ModuleId::M04)
            .with_timestamp(Timestamp::from_raw(100));
        let display = ctx.to_string();
        assert!(display.contains("M04"));
        assert!(display.contains("T100"));
    }

    #[test]
    fn test_signal_context_clone_eq() {
        let a = SignalContext::new(ModuleId::M05)
            .with_timestamp(Timestamp::from_raw(1));
        let b = a.clone();
        assert_eq!(a, b);
    }

    // ==== HealthSignal tests ====

    #[test]
    fn test_health_signal_new() {
        let sig = HealthSignal::new(ModuleId::M01, 0.8, 0.6, "degraded");
        assert_eq!(sig.module_id, ModuleId::M01);
        assert!((sig.previous_health - 0.8).abs() < f64::EPSILON);
        assert!((sig.current_health - 0.6).abs() < f64::EPSILON);
        assert_eq!(sig.reason, "degraded");
    }

    #[test]
    fn test_health_signal_clamping() {
        let sig = HealthSignal::new(ModuleId::M01, -0.5, 1.5, "clamp test");
        assert!(sig.previous_health.abs() < f64::EPSILON);
        assert!((sig.current_health - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_health_signal_is_degradation() {
        let sig = HealthSignal::new(ModuleId::M01, 0.9, 0.5, "down");
        assert!(sig.is_degradation());
        assert!(!sig.is_improvement());
    }

    #[test]
    fn test_health_signal_is_improvement() {
        let sig = HealthSignal::new(ModuleId::M01, 0.3, 0.8, "up");
        assert!(sig.is_improvement());
        assert!(!sig.is_degradation());
    }

    #[test]
    fn test_health_signal_no_change() {
        let sig = HealthSignal::new(ModuleId::M01, 0.5, 0.5, "stable");
        assert!(!sig.is_degradation());
        assert!(!sig.is_improvement());
    }

    #[test]
    fn test_health_signal_delta() {
        let sig = HealthSignal::new(ModuleId::M01, 0.8, 0.6, "test");
        assert!((sig.delta() - (-0.2)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_health_signal_with_timestamp() {
        let ts = Timestamp::from_raw(42);
        let sig = HealthSignal::new(ModuleId::M01, 0.5, 0.5, "test")
            .with_timestamp(ts);
        assert_eq!(sig.timestamp, ts);
    }

    #[test]
    fn test_health_signal_display() {
        let sig = HealthSignal::new(ModuleId::M01, 0.8, 0.6, "dropping")
            .with_timestamp(Timestamp::from_raw(1));
        let display = sig.to_string();
        assert!(display.contains("M01"));
        assert!(display.contains("dropping"));
    }

    // ==== LearningEvent tests ====

    #[test]
    fn test_learning_event_new() {
        let signal = LearningSignal::success("M01");
        let ctx = SignalContext::new(ModuleId::M01);
        let event = LearningEvent::new(signal.clone(), ctx);
        assert_eq!(event.signal, signal);
    }

    #[test]
    fn test_learning_event_with_timestamp() {
        let signal = LearningSignal::failure("M02");
        let ctx = SignalContext::new(ModuleId::M02);
        let ts = Timestamp::from_raw(99);
        let event = LearningEvent::new(signal, ctx).with_timestamp(ts);
        assert_eq!(event.timestamp, ts);
    }

    #[test]
    fn test_learning_event_display() {
        let signal = LearningSignal::partial("M03", 0.7);
        let ctx = SignalContext::new(ModuleId::M03);
        let event = LearningEvent::new(signal, ctx)
            .with_timestamp(Timestamp::from_raw(50));
        let display = event.to_string();
        assert!(display.contains("M03"));
        assert!(display.contains("Partial"));
    }

    #[test]
    fn test_learning_event_clone_eq() {
        let signal = LearningSignal::success("M04");
        let ctx = SignalContext::new(ModuleId::M04)
            .with_timestamp(Timestamp::from_raw(1));
        let a = LearningEvent::new(signal, ctx)
            .with_timestamp(Timestamp::from_raw(2));
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn test_learning_event_wraps_pathway() {
        let signal = LearningSignal::success("M05").with_pathway("path-001");
        let ctx = SignalContext::new(ModuleId::M05);
        let event = LearningEvent::new(signal, ctx);
        assert_eq!(event.signal.pathway_id.as_deref(), Some("path-001"));
    }

    // ==== DissentEvent tests ====

    #[test]
    fn test_dissent_event_new() {
        let dissent = Dissent::new(AgentOrigin::human(), "d-001", "wrong");
        let event = DissentEvent::new(dissent.clone(), ModuleId::M31);
        assert_eq!(event.dissent, dissent);
        assert_eq!(event.source_module, ModuleId::M31);
    }

    #[test]
    fn test_dissent_event_with_timestamp() {
        let dissent = Dissent::new(AgentOrigin::System, "d-002", "test");
        let ts = Timestamp::from_raw(77);
        let event = DissentEvent::new(dissent, ModuleId::M35)
            .with_timestamp(ts);
        assert_eq!(event.timestamp, ts);
    }

    #[test]
    fn test_dissent_event_display() {
        let dissent = Dissent::new(AgentOrigin::human(), "cfg-port", "too low");
        let event = DissentEvent::new(dissent, ModuleId::M33)
            .with_timestamp(Timestamp::from_raw(10));
        let display = event.to_string();
        assert!(display.contains("M33"));
        assert!(display.contains("cfg-port"));
    }

    #[test]
    fn test_dissent_event_clone_eq() {
        let dissent = Dissent::new(AgentOrigin::System, "x", "y");
        let a = DissentEvent::new(dissent, ModuleId::M01)
            .with_timestamp(Timestamp::from_raw(1));
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn test_dissent_event_preserves_confidence() {
        let dissent = Dissent::new(AgentOrigin::System, "t", "r")
            .with_confidence(0.7);
        let event = DissentEvent::new(dissent, ModuleId::M35);
        assert!((event.dissent.confidence - 0.7).abs() < f64::EPSILON);
    }

    // ==== Signal enum tests ====

    #[test]
    fn test_signal_health_variant() {
        let sig = Signal::Health(HealthSignal::new(ModuleId::M01, 0.5, 0.5, "test"));
        let display = sig.to_string();
        assert!(display.contains("Health"));
    }

    #[test]
    fn test_signal_learning_variant() {
        let signal = LearningSignal::success("M01");
        let ctx = SignalContext::new(ModuleId::M01);
        let sig = Signal::Learning(LearningEvent::new(signal, ctx));
        let display = sig.to_string();
        assert!(display.contains("Learn"));
    }

    #[test]
    fn test_signal_dissent_variant() {
        let dissent = Dissent::new(AgentOrigin::System, "t", "r");
        let sig = Signal::Dissent(DissentEvent::new(dissent, ModuleId::M01));
        let display = sig.to_string();
        assert!(display.contains("Dissent"));
    }

    // ==== SignalBusConfig tests ====

    #[test]
    fn test_config_default() {
        let cfg = SignalBusConfig::default();
        assert_eq!(cfg.max_subscribers, 256);
    }

    #[test]
    fn test_config_custom() {
        let cfg = SignalBusConfig::default().with_max_subscribers(64);
        assert_eq!(cfg.max_subscribers, 64);
    }

    #[test]
    fn test_config_display() {
        let cfg = SignalBusConfig::default();
        assert!(cfg.to_string().contains("256"));
    }

    // ==== SignalBus tests ====

    /// Test subscriber that counts invocations.
    #[derive(Debug)]
    struct CountingSubscriber {
        name: &'static str,
        health_count: AtomicU64,
        learning_count: AtomicU64,
        dissent_count: AtomicU64,
    }

    impl CountingSubscriber {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                health_count: AtomicU64::new(0),
                learning_count: AtomicU64::new(0),
                dissent_count: AtomicU64::new(0),
            }
        }
    }

    impl SignalSubscriber for CountingSubscriber {
        fn name(&self) -> &str {
            self.name
        }

        fn on_health(&self, _signal: &HealthSignal) {
            self.health_count.fetch_add(1, Ordering::Relaxed);
        }

        fn on_learning(&self, _event: &LearningEvent) {
            self.learning_count.fetch_add(1, Ordering::Relaxed);
        }

        fn on_dissent(&self, _event: &DissentEvent) {
            self.dissent_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn test_bus_new_empty() {
        let bus = SignalBus::new();
        assert_eq!(bus.subscriber_count(), 0);
        assert_eq!(bus.stats().total_emitted(), 0);
    }

    #[test]
    fn test_bus_subscribe() {
        let bus = SignalBus::new();
        let sub = Arc::new(CountingSubscriber::new("test"));
        bus.subscribe(sub).unwrap();
        assert_eq!(bus.subscriber_count(), 1);
    }

    /// Helper to create a subscriber and return both the concrete Arc and a trait-object clone.
    fn make_sub(name: &'static str) -> (Arc<CountingSubscriber>, Arc<dyn SignalSubscriber>) {
        let concrete = Arc::new(CountingSubscriber::new(name));
        let trait_obj: Arc<dyn SignalSubscriber> = Arc::clone(&concrete) as _;
        (concrete, trait_obj)
    }

    #[test]
    fn test_bus_subscribe_limit() {
        let bus = SignalBus::with_config(
            SignalBusConfig::default().with_max_subscribers(2),
        );
        let (_, s1) = make_sub("a");
        let (_, s2) = make_sub("b");
        let (_, s3) = make_sub("c");
        bus.subscribe(s1).unwrap();
        bus.subscribe(s2).unwrap();
        assert!(bus.subscribe(s3).is_err());
    }

    #[test]
    fn test_bus_emit_health() {
        let bus = SignalBus::new();
        let (sub, sub_dyn) = make_sub("test");
        bus.subscribe(sub_dyn).unwrap();

        let sig = HealthSignal::new(ModuleId::M01, 0.9, 0.5, "test");
        bus.emit_health(&sig);
        bus.emit_health(&sig);

        assert_eq!(sub.health_count.load(Ordering::Relaxed), 2);
        assert_eq!(bus.stats().health_emitted, 2);
    }

    #[test]
    fn test_bus_emit_learning() {
        let bus = SignalBus::new();
        let (sub, sub_dyn) = make_sub("test");
        bus.subscribe(sub_dyn).unwrap();

        let signal = LearningSignal::success("M01");
        let ctx = SignalContext::new(ModuleId::M01);
        let event = LearningEvent::new(signal, ctx);
        bus.emit_learning(&event);

        assert_eq!(sub.learning_count.load(Ordering::Relaxed), 1);
        assert_eq!(bus.stats().learning_emitted, 1);
    }

    #[test]
    fn test_bus_emit_dissent() {
        let bus = SignalBus::new();
        let (sub, sub_dyn) = make_sub("test");
        bus.subscribe(sub_dyn).unwrap();

        let dissent = Dissent::new(AgentOrigin::System, "t", "r");
        let event = DissentEvent::new(dissent, ModuleId::M35);
        bus.emit_dissent(&event);

        assert_eq!(sub.dissent_count.load(Ordering::Relaxed), 1);
        assert_eq!(bus.stats().dissent_emitted, 1);
    }

    #[test]
    fn test_bus_multiple_subscribers() {
        let bus = SignalBus::new();
        let (s1, s1_dyn) = make_sub("a");
        let (s2, s2_dyn) = make_sub("b");
        bus.subscribe(s1_dyn).unwrap();
        bus.subscribe(s2_dyn).unwrap();

        let sig = HealthSignal::new(ModuleId::M01, 1.0, 0.5, "test");
        bus.emit_health(&sig);

        assert_eq!(s1.health_count.load(Ordering::Relaxed), 1);
        assert_eq!(s2.health_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_bus_stats_total() {
        let bus = SignalBus::new();
        let (_sub, sub_dyn) = make_sub("test");
        bus.subscribe(sub_dyn).unwrap();

        bus.emit_health(&HealthSignal::new(ModuleId::M01, 1.0, 0.5, "h"));
        let signal = LearningSignal::success("M01");
        let ctx = SignalContext::new(ModuleId::M01);
        bus.emit_learning(&LearningEvent::new(signal, ctx));
        let dissent = Dissent::new(AgentOrigin::System, "t", "r");
        bus.emit_dissent(&DissentEvent::new(dissent, ModuleId::M01));

        let stats = bus.stats();
        assert_eq!(stats.total_emitted(), 3);
        assert_eq!(stats.health_emitted, 1);
        assert_eq!(stats.learning_emitted, 1);
        assert_eq!(stats.dissent_emitted, 1);
        assert_eq!(stats.subscriber_count, 1);
    }

    #[test]
    fn test_bus_stats_display() {
        let stats = SignalBusStats {
            health_emitted: 5,
            learning_emitted: 3,
            dissent_emitted: 1,
            subscriber_count: 2,
        };
        let display = stats.to_string();
        assert!(display.contains("health=5"));
        assert!(display.contains("learning=3"));
        assert!(display.contains("dissent=1"));
        assert!(display.contains("subs=2"));
    }

    #[test]
    fn test_bus_default() {
        let bus = SignalBus::default();
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[test]
    fn test_bus_config_accessible() {
        let cfg = SignalBusConfig::default().with_max_subscribers(128);
        let bus = SignalBus::with_config(cfg);
        assert_eq!(bus.config().max_subscribers, 128);
    }

    // ==== [COMPILE] Trait safety tests ====

    #[test]
    fn test_signal_subscriber_is_object_safe() {
        // [COMPILE] SignalSubscriber must be usable as a trait object.
        fn accept_boxed(_sub: Box<dyn SignalSubscriber>) {}
        let sub = Box::new(CountingSubscriber::new("compile-test"));
        accept_boxed(sub);
    }

    #[test]
    fn test_signal_subscriber_is_send_sync() {
        // [COMPILE] SignalSubscriber trait objects must be Send + Sync.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Arc<dyn SignalSubscriber>>();
    }

    #[test]
    fn test_signal_bus_is_send() {
        // [COMPILE] SignalBus must be Send (moveable across threads).
        fn assert_send<T: Send>() {}
        assert_send::<SignalBus>();
    }

    // ==== [INVARIANT] tests ====

    #[test]
    fn test_health_signal_always_clamped_invariant() {
        // [INVARIANT] No matter what values are provided, health is in [0.0, 1.0].
        let sig = HealthSignal::new(ModuleId::M01, -999.0, 999.0, "extreme");
        assert!(sig.previous_health >= 0.0);
        assert!(sig.previous_health <= 1.0);
        assert!(sig.current_health >= 0.0);
        assert!(sig.current_health <= 1.0);
    }

    #[test]
    fn test_signal_bus_default_capacity_invariant() {
        // [INVARIANT] Default bus capacity must be exactly 256.
        let bus = SignalBus::new();
        assert_eq!(bus.config().max_subscribers, 256);
    }

    #[test]
    fn test_signal_bus_stats_zero_initialized() {
        // [INVARIANT] A fresh bus has all stats at zero.
        let bus = SignalBus::new();
        let stats = bus.stats();
        assert_eq!(stats.health_emitted, 0);
        assert_eq!(stats.learning_emitted, 0);
        assert_eq!(stats.dissent_emitted, 0);
        assert_eq!(stats.subscriber_count, 0);
        assert_eq!(stats.total_emitted(), 0);
    }

    // ==== [BOUNDARY] tests ====

    #[test]
    fn test_health_signal_at_exact_0_0() {
        // [BOUNDARY] Both health values at the lower bound.
        let sig = HealthSignal::new(ModuleId::M01, 0.0, 0.0, "floor");
        assert!(sig.previous_health.abs() < f64::EPSILON);
        assert!(sig.current_health.abs() < f64::EPSILON);
        assert!(!sig.is_degradation());
        assert!(!sig.is_improvement());
    }

    #[test]
    fn test_health_signal_at_exact_1_0() {
        // [BOUNDARY] Both health values at the upper bound.
        let sig = HealthSignal::new(ModuleId::M01, 1.0, 1.0, "ceiling");
        assert!((sig.previous_health - 1.0).abs() < f64::EPSILON);
        assert!((sig.current_health - 1.0).abs() < f64::EPSILON);
        assert!(sig.delta().abs() < f64::EPSILON);
    }

    #[test]
    fn test_health_signal_full_range_transition() {
        // [BOUNDARY] Transition from 0.0 to 1.0 — maximum possible delta.
        let sig = HealthSignal::new(ModuleId::M02, 0.0, 1.0, "full range");
        assert!((sig.delta() - 1.0).abs() < f64::EPSILON);
        assert!(sig.is_improvement());
    }

    // ==== [PROPERTY] tests ====

    #[test]
    fn test_health_signal_clamping_property() {
        // [PROPERTY] For any inputs, clamped values are always in [0.0, 1.0].
        let inputs = [
            -1.0, -0.001, 0.0, 0.5, 1.0, 1.001, 100.0, f64::NEG_INFINITY,
        ];
        for &prev in &inputs {
            for &curr in &inputs {
                let sig = HealthSignal::new(ModuleId::M01, prev, curr, "sweep");
                assert!(
                    sig.previous_health >= 0.0 && sig.previous_health <= 1.0,
                    "previous_health out of range for input {prev}"
                );
                assert!(
                    sig.current_health >= 0.0 && sig.current_health <= 1.0,
                    "current_health out of range for input {curr}"
                );
            }
        }
    }

    #[test]
    fn test_signal_bus_stats_total_equals_sum_property() {
        // [PROPERTY] total_emitted() always equals the sum of all channels.
        let stats = SignalBusStats {
            health_emitted: 42,
            learning_emitted: 17,
            dissent_emitted: 5,
            subscriber_count: 3,
        };
        assert_eq!(
            stats.total_emitted(),
            stats.health_emitted + stats.learning_emitted + stats.dissent_emitted
        );
    }

    // ==== [NEGATIVE] tests ====

    #[test]
    fn test_bus_subscribe_beyond_limit_error_message() {
        // [NEGATIVE] Error message must mention the limit.
        let bus = SignalBus::with_config(SignalBusConfig::default().with_max_subscribers(1));
        let (_, s1) = make_sub("first");
        let (_, s2) = make_sub("second");
        bus.subscribe(s1).unwrap();
        let err = bus.subscribe(s2).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("1"), "Error should mention the limit: {msg}");
    }

    #[test]
    fn test_bus_emit_to_zero_subscribers() {
        // [NEGATIVE] Emitting to a bus with no subscribers must not panic.
        let bus = SignalBus::new();
        bus.emit_health(&HealthSignal::new(ModuleId::M01, 1.0, 0.5, "no-subs"));
        let signal = LearningSignal::success("M01");
        let ctx = SignalContext::new(ModuleId::M01);
        bus.emit_learning(&LearningEvent::new(signal, ctx));
        let dissent = Dissent::new(AgentOrigin::System, "t", "r");
        bus.emit_dissent(&DissentEvent::new(dissent, ModuleId::M01));
        assert_eq!(bus.stats().total_emitted(), 3);
    }

    // ==== [INTEGRATION] tests ====

    #[test]
    fn test_bus_cross_channel_emission() {
        // [INTEGRATION] All 3 channels delivered to the same subscriber.
        let bus = SignalBus::new();
        let (sub, sub_dyn) = make_sub("multi");
        bus.subscribe(sub_dyn).unwrap();

        bus.emit_health(&HealthSignal::new(ModuleId::M01, 0.9, 0.5, "h"));
        let signal = LearningSignal::success("M02");
        let ctx = SignalContext::new(ModuleId::M02);
        bus.emit_learning(&LearningEvent::new(signal, ctx));
        let dissent = Dissent::new(AgentOrigin::System, "d", "r");
        bus.emit_dissent(&DissentEvent::new(dissent, ModuleId::M35));

        assert_eq!(sub.health_count.load(Ordering::Relaxed), 1);
        assert_eq!(sub.learning_count.load(Ordering::Relaxed), 1);
        assert_eq!(sub.dissent_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_bus_selective_subscriber() {
        // [INTEGRATION] A subscriber that only overrides on_health.
        #[derive(Debug)]
        struct HealthOnlySubscriber {
            count: AtomicU64,
        }
        impl SignalSubscriber for HealthOnlySubscriber {
            fn name(&self) -> &str {
                "health-only"
            }
            fn on_health(&self, _signal: &HealthSignal) {
                self.count.fetch_add(1, Ordering::Relaxed);
            }
        }

        let bus = SignalBus::new();
        let sub = Arc::new(HealthOnlySubscriber {
            count: AtomicU64::new(0),
        });
        let sub_dyn: Arc<dyn SignalSubscriber> = Arc::clone(&sub) as _;
        bus.subscribe(sub_dyn).unwrap();

        bus.emit_health(&HealthSignal::new(ModuleId::M01, 1.0, 0.5, "h"));
        let signal = LearningSignal::success("M01");
        let ctx = SignalContext::new(ModuleId::M01);
        bus.emit_learning(&LearningEvent::new(signal, ctx));

        // Only the health callback should have fired.
        assert_eq!(sub.count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_signal_context_different_modules_not_equal() {
        // [INTEGRATION] Two contexts with different modules must not be equal.
        let a = SignalContext::new(ModuleId::M01)
            .with_timestamp(Timestamp::from_raw(1));
        let b = SignalContext::new(ModuleId::M02)
            .with_timestamp(Timestamp::from_raw(1));
        assert_ne!(a, b);
    }

    #[test]
    fn test_health_signal_degradation_improvement_exclusive() {
        // [PROPERTY] is_degradation and is_improvement are mutually exclusive.
        let cases = [
            (0.3, 0.7), // improvement
            (0.7, 0.3), // degradation
            (0.5, 0.5), // neither
        ];
        for (prev, curr) in cases {
            let sig = HealthSignal::new(ModuleId::M01, prev, curr, "excl");
            assert!(
                !(sig.is_degradation() && sig.is_improvement()),
                "Both true for {prev} -> {curr}"
            );
        }
    }

    #[test]
    fn test_bus_stats_increment_correctly_after_multiple_emissions() {
        // [INVARIANT] Stats track emissions accurately across many calls.
        let bus = SignalBus::new();
        let (_, sub_dyn) = make_sub("counter");
        bus.subscribe(sub_dyn).unwrap();

        for _ in 0..10 {
            bus.emit_health(&HealthSignal::new(ModuleId::M01, 1.0, 0.5, "h"));
        }
        for _ in 0..5 {
            let signal = LearningSignal::success("M01");
            let ctx = SignalContext::new(ModuleId::M01);
            bus.emit_learning(&LearningEvent::new(signal, ctx));
        }
        for _ in 0..3 {
            let dissent = Dissent::new(AgentOrigin::System, "t", "r");
            bus.emit_dissent(&DissentEvent::new(dissent, ModuleId::M01));
        }

        let stats = bus.stats();
        assert_eq!(stats.health_emitted, 10);
        assert_eq!(stats.learning_emitted, 5);
        assert_eq!(stats.dissent_emitted, 3);
        assert_eq!(stats.total_emitted(), 18);
    }
}
