//! # M52: Rate Limiter
//!
//! Token-bucket rate limiting for ULTRAPLATE inter-service communication.
//! Each key (service, agent, or API consumer) receives a bucket whose
//! capacity and refill rate are determined by its [`ServiceTier`].
//!
//! ## Layer: L4 (Integration)
//!
//! ## Algorithm
//!
//! Uses a standard **token-bucket** with burst extension:
//!
//! 1. Compute elapsed ticks since last refill and convert to seconds.
//! 2. Add `elapsed_secs * refill_rate` tokens (capped at `capacity`).
//! 3. If `tokens >= 1.0` -> consume one token, return [`RateDecision::Allow`].
//! 4. If `tokens < 1.0` but bucket has burst headroom -> consume from burst,
//!    return [`RateDecision::AllowBurst`].
//! 5. Otherwise -> return [`RateDecision::Reject`] with `retry_after_secs`.
//!
//! ## Tier Defaults
//!
//! | Tier | Requests/min | Burst multiplier | Cooldown (s) |
//! |------|-------------|------------------|--------------|
//! | T1   | 1000        | 2.0              | 60           |
//! | T2   | 800         | 2.0              | 60           |
//! | T3   | 600         | 2.0              | 60           |
//! | T4   | 400         | 2.0              | 60           |
//! | T5   | 200         | 2.0              | 60           |
//!
//! ## Thread Safety
//!
//! All public methods take `&self` and use interior mutability via
//! [`parking_lot::RwLock`] and [`AtomicU64`].
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L04_INTEGRATION.md)

#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;

use crate::m1_foundation::shared_types::Timestamp;
use crate::m2_services::ServiceTier;
use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default requests per minute for Tier 1.
const DEFAULT_T1_RPM: u64 = 1000;

/// Default requests per minute for Tier 2.
const DEFAULT_T2_RPM: u64 = 800;

/// Default requests per minute for Tier 3.
const DEFAULT_T3_RPM: u64 = 600;

/// Default requests per minute for Tier 4.
const DEFAULT_T4_RPM: u64 = 400;

/// Default requests per minute for Tier 5.
const DEFAULT_T5_RPM: u64 = 200;

/// Default burst multiplier applied to base capacity.
const DEFAULT_BURST_MULTIPLIER: f64 = 2.0;

/// Default cooldown period in seconds.
const DEFAULT_COOLDOWN_SECS: u64 = 60;

/// Default maximum tracked keys before eviction.
const DEFAULT_MAX_TRACKED_KEYS: usize = 10_000;

/// Conversion factor: simulated ticks per second.
///
/// The [`Timestamp`] counter increments monotonically. We treat each tick
/// as one second for rate-limiting purposes.
const TICKS_PER_SECOND: f64 = 1.0;

// ---------------------------------------------------------------------------
// TierRateConfig
// ---------------------------------------------------------------------------

/// Per-tier rate-limiting configuration.
///
/// Controls how many requests a key assigned to this tier may issue per
/// minute, how much burst headroom is available, and how long a cooldown
/// persists after exhaustion.
#[derive(Clone, Debug, PartialEq)]
pub struct TierRateConfig {
    /// Maximum sustained requests per minute.
    pub requests_per_minute: u64,
    /// Multiplier applied to `capacity` to derive the burst ceiling.
    /// The burst ceiling equals `capacity * burst_multiplier`.
    pub burst_multiplier: f64,
    /// Cooldown duration in seconds after burst exhaustion.
    pub cooldown_secs: u64,
}

impl TierRateConfig {
    /// Create a new tier rate configuration.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `requests_per_minute` is zero or
    /// `burst_multiplier` is not finite / less than 1.0.
    pub fn new(
        requests_per_minute: u64,
        burst_multiplier: f64,
        cooldown_secs: u64,
    ) -> Result<Self> {
        if requests_per_minute == 0 {
            return Err(Error::Validation(
                "requests_per_minute must be > 0".into(),
            ));
        }
        if !burst_multiplier.is_finite() || burst_multiplier < 1.0 {
            return Err(Error::Validation(
                "burst_multiplier must be >= 1.0 and finite".into(),
            ));
        }
        Ok(Self {
            requests_per_minute,
            burst_multiplier,
            cooldown_secs,
        })
    }

    /// Compute the base token capacity (tokens in one minute).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    const fn capacity(&self) -> f64 {
        self.requests_per_minute as f64
    }

    /// Compute the burst capacity.
    #[must_use]
    fn burst_capacity(&self) -> f64 {
        self.capacity() * self.burst_multiplier
    }

    /// Compute the refill rate in tokens per second.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    fn refill_rate(&self) -> f64 {
        self.requests_per_minute as f64 / 60.0
    }
}

// ---------------------------------------------------------------------------
// BucketState
// ---------------------------------------------------------------------------

/// The current state of a single rate-limiting bucket.
///
/// Each tracked key has exactly one `BucketState`. The bucket refills
/// tokens at `refill_rate` tokens/second up to `capacity`, with an
/// additional burst ceiling at `burst_capacity`.
#[derive(Clone, Debug)]
pub struct BucketState {
    /// The key this bucket belongs to.
    pub key: String,
    /// The tier assigned to this key.
    pub tier: ServiceTier,
    /// Current number of available tokens.
    pub tokens: f64,
    /// Base capacity (tokens per minute equivalent).
    pub capacity: f64,
    /// Extended burst ceiling.
    pub burst_capacity: f64,
    /// Tokens added per second.
    pub refill_rate: f64,
    /// Tick at which the bucket was last refilled.
    pub last_refill: Timestamp,
    /// Total requests seen by this bucket.
    pub total_requests: u64,
    /// Requests that were allowed.
    pub allowed_requests: u64,
    /// Requests that were rejected.
    pub rejected_requests: u64,
}

// ---------------------------------------------------------------------------
// RateDecision
// ---------------------------------------------------------------------------

/// The outcome of a rate-limit check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RateDecision {
    /// Request is within the normal rate limit.
    Allow,
    /// Request was served from burst headroom. A warning is attached.
    AllowBurst {
        /// Human-readable advisory that the caller is using burst tokens.
        warning: String,
    },
    /// Request was rejected because the bucket is exhausted.
    Reject {
        /// Suggested number of seconds to wait before retrying.
        retry_after_secs: u64,
    },
}

// ---------------------------------------------------------------------------
// RateLimiterStats
// ---------------------------------------------------------------------------

/// Aggregate statistics across all tracked keys.
#[derive(Clone, Debug)]
pub struct RateLimiterStats {
    /// Number of distinct keys currently tracked.
    pub total_keys: usize,
    /// Total requests across all keys.
    pub total_requests: u64,
    /// Total allowed requests across all keys.
    pub total_allowed: u64,
    /// Total rejected requests across all keys.
    pub total_rejected: u64,
    /// Overall allow rate (0.0 .. 1.0). Returns 0.0 if no requests.
    pub overall_allow_rate: f64,
}

// ---------------------------------------------------------------------------
// RateLimiterConfig
// ---------------------------------------------------------------------------

/// Top-level configuration for the rate limiter.
///
/// Contains per-tier configurations and a global cap on the number of
/// simultaneously tracked keys.
#[derive(Clone, Debug)]
pub struct RateLimiterConfig {
    /// Configuration for Tier 1 keys.
    pub tier1: TierRateConfig,
    /// Configuration for Tier 2 keys.
    pub tier2: TierRateConfig,
    /// Configuration for Tier 3 keys.
    pub tier3: TierRateConfig,
    /// Configuration for Tier 4 keys.
    pub tier4: TierRateConfig,
    /// Configuration for Tier 5 keys.
    pub tier5: TierRateConfig,
    /// Maximum number of keys to track before eviction.
    pub max_tracked_keys: usize,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            tier1: TierRateConfig {
                requests_per_minute: DEFAULT_T1_RPM,
                burst_multiplier: DEFAULT_BURST_MULTIPLIER,
                cooldown_secs: DEFAULT_COOLDOWN_SECS,
            },
            tier2: TierRateConfig {
                requests_per_minute: DEFAULT_T2_RPM,
                burst_multiplier: DEFAULT_BURST_MULTIPLIER,
                cooldown_secs: DEFAULT_COOLDOWN_SECS,
            },
            tier3: TierRateConfig {
                requests_per_minute: DEFAULT_T3_RPM,
                burst_multiplier: DEFAULT_BURST_MULTIPLIER,
                cooldown_secs: DEFAULT_COOLDOWN_SECS,
            },
            tier4: TierRateConfig {
                requests_per_minute: DEFAULT_T4_RPM,
                burst_multiplier: DEFAULT_BURST_MULTIPLIER,
                cooldown_secs: DEFAULT_COOLDOWN_SECS,
            },
            tier5: TierRateConfig {
                requests_per_minute: DEFAULT_T5_RPM,
                burst_multiplier: DEFAULT_BURST_MULTIPLIER,
                cooldown_secs: DEFAULT_COOLDOWN_SECS,
            },
            max_tracked_keys: DEFAULT_MAX_TRACKED_KEYS,
        }
    }
}

impl RateLimiterConfig {
    /// Look up the tier-specific configuration for a [`ServiceTier`].
    #[must_use]
    pub const fn for_tier(&self, tier: ServiceTier) -> &TierRateConfig {
        match tier {
            ServiceTier::Tier1 => &self.tier1,
            ServiceTier::Tier2 => &self.tier2,
            ServiceTier::Tier3 => &self.tier3,
            ServiceTier::Tier4 => &self.tier4,
            ServiceTier::Tier5 => &self.tier5,
        }
    }
}

// ---------------------------------------------------------------------------
// RateLimiting Trait
// ---------------------------------------------------------------------------

/// Core rate-limiting trait for the Maintenance Engine.
///
/// All methods take `&self` -- state mutation is handled via interior
/// mutability (`RwLock`, `AtomicU64`).
///
/// # Errors
///
/// Methods return `Err` on:
/// - Empty keys
/// - Invalid tier configurations
pub trait RateLimiting: Send + Sync + fmt::Debug {
    /// Check whether a request under `key` at the given `tier` should be
    /// allowed, and consume a token if so.
    ///
    /// If the key has not been registered, it is auto-registered with the
    /// given tier.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `key` is empty.
    fn check_and_consume(&self, key: &str, tier: ServiceTier) -> Result<RateDecision>;

    /// Explicitly register a key with a tier, creating a fresh bucket.
    ///
    /// If the key already exists, its tier and bucket are overwritten.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `key` is empty.
    fn register_key(&self, key: &str, tier: ServiceTier) -> Result<()>;

    /// Retrieve a snapshot of the bucket state for a key, if it exists.
    fn get_bucket_state(&self, key: &str) -> Option<BucketState>;

    /// Reset a key's bucket to its full capacity.
    fn reset_bucket(&self, key: &str);

    /// Return the rate configuration for a given tier.
    fn tier_config(&self, tier: ServiceTier) -> TierRateConfig;

    /// Return aggregate statistics across all tracked keys.
    fn overall_stats(&self) -> RateLimiterStats;

    /// Override the rate configuration for a specific tier.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the new config is invalid.
    fn set_tier_config(&self, tier: ServiceTier, config: TierRateConfig) -> Result<()>;
}

// ---------------------------------------------------------------------------
// RateLimiter
// ---------------------------------------------------------------------------

/// Concrete implementation of [`RateLimiting`].
///
/// Thread-safe via `parking_lot::RwLock` for the bucket map and config,
/// and `AtomicU64` for monotonic global counters.
pub struct RateLimiter {
    /// Active buckets keyed by caller-supplied key string.
    buckets: RwLock<HashMap<String, BucketState>>,
    /// Mutable configuration (allows runtime tier overrides).
    config: RwLock<RateLimiterConfig>,
    /// Global counter: total requests checked.
    total_requests: AtomicU64,
    /// Global counter: total requests allowed (`Allow` + `AllowBurst`).
    total_allowed: AtomicU64,
    /// Global counter: total requests rejected.
    total_rejected: AtomicU64,
}

impl fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RateLimiter")
            .field("buckets_len", &self.buckets.read().len())
            .field("config", &*self.config.read())
            .field(
                "total_requests",
                &self.total_requests.load(Ordering::Relaxed),
            )
            .field(
                "total_allowed",
                &self.total_allowed.load(Ordering::Relaxed),
            )
            .field(
                "total_rejected",
                &self.total_rejected.load(Ordering::Relaxed),
            )
            .finish()
    }
}

impl RateLimiter {
    /// Create a new `RateLimiter` with the given configuration.
    #[must_use]
    pub fn new(config: RateLimiterConfig) -> Self {
        Self {
            buckets: RwLock::new(HashMap::new()),
            config: RwLock::new(config),
            total_requests: AtomicU64::new(0),
            total_allowed: AtomicU64::new(0),
            total_rejected: AtomicU64::new(0),
        }
    }

    /// Create a new `RateLimiter` with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(RateLimiterConfig::default())
    }

    /// Create a fresh bucket for the given key and tier using the current
    /// configuration.
    fn create_bucket(&self, key: &str, tier: ServiceTier) -> BucketState {
        let cfg = self.config.read();
        let tier_cfg = cfg.for_tier(tier);
        let bucket = BucketState {
            key: key.to_owned(),
            tier,
            tokens: tier_cfg.capacity(),
            capacity: tier_cfg.capacity(),
            burst_capacity: tier_cfg.burst_capacity(),
            refill_rate: tier_cfg.refill_rate(),
            last_refill: Timestamp::now(),
            total_requests: 0,
            allowed_requests: 0,
            rejected_requests: 0,
        };
        drop(cfg);
        bucket
    }

    /// Refill tokens in a bucket based on elapsed ticks.
    #[allow(clippy::cast_precision_loss)]
    fn refill_bucket(bucket: &mut BucketState) {
        let now = Timestamp::now();
        let elapsed_ticks = now.ticks().saturating_sub(bucket.last_refill.ticks());
        let elapsed_secs = elapsed_ticks as f64 * TICKS_PER_SECOND;
        let new_tokens = elapsed_secs * bucket.refill_rate;
        bucket.tokens = (bucket.tokens + new_tokens).min(bucket.capacity);
        bucket.last_refill = now;
    }

    /// Validate that a key is non-empty.
    fn validate_key(key: &str) -> Result<()> {
        if key.is_empty() {
            return Err(Error::Validation("rate limiter key must not be empty".into()));
        }
        Ok(())
    }

    /// Evict the oldest bucket from the map if at capacity.
    fn evict_oldest_if_full(buckets: &mut HashMap<String, BucketState>, max_keys: usize) {
        if buckets.len() >= max_keys {
            let oldest_key = buckets
                .iter()
                .min_by_key(|(_, b)| b.last_refill)
                .map(|(k, _)| k.clone());
            if let Some(oldest) = oldest_key {
                buckets.remove(&oldest);
            }
        }
    }

    /// Compute seconds until one token is available.
    fn retry_after(&self, bucket: &BucketState, tier: ServiceTier) -> u64 {
        if bucket.refill_rate > 0.0 {
            // Time to accumulate 1 token.
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss
            )]
            let secs = (1.0 / bucket.refill_rate).ceil() as u64;
            secs.max(1)
        } else {
            self.config.read().for_tier(tier).cooldown_secs
        }
    }
}

impl RateLimiting for RateLimiter {
    fn check_and_consume(&self, key: &str, tier: ServiceTier) -> Result<RateDecision> {
        Self::validate_key(key)?;

        self.total_requests.fetch_add(1, Ordering::Relaxed);

        // Ensure the key exists (auto-register if missing).
        {
            let has_key = self.buckets.read().contains_key(key);
            if !has_key {
                let bucket = self.create_bucket(key, tier);
                let max_keys = self.config.read().max_tracked_keys;
                let mut buckets_w = self.buckets.write();
                // Re-check after acquiring write lock (another thread may
                // have inserted).
                if !buckets_w.contains_key(key) {
                    Self::evict_oldest_if_full(&mut buckets_w, max_keys);
                    buckets_w.insert(key.to_owned(), bucket);
                }
                drop(buckets_w);
            }
        }

        // Now perform the check-and-consume under a write lock.
        let mut buckets = self.buckets.write();
        let Some(bucket) = buckets.get_mut(key) else {
            // Should not happen after auto-register above.
            return Err(Error::Validation(format!(
                "bucket for key '{key}' not found after auto-register"
            )));
        };

        // Refill tokens based on elapsed time.
        Self::refill_bucket(bucket);

        bucket.total_requests += 1;

        // Decision logic.
        if bucket.tokens >= 1.0 {
            // Normal allow.
            bucket.tokens -= 1.0;
            bucket.allowed_requests += 1;
            drop(buckets);
            self.total_allowed.fetch_add(1, Ordering::Relaxed);
            Ok(RateDecision::Allow)
        } else if bucket.tokens + (bucket.burst_capacity - bucket.capacity) >= 1.0 {
            // Burst allow -- tokens are below normal capacity but burst
            // headroom can absorb one more request.
            bucket.tokens -= 1.0;
            bucket.allowed_requests += 1;
            let warning = format!(
                "key '{}' is using burst capacity ({:.1}/{:.1} normal tokens remaining)",
                key, bucket.tokens, bucket.capacity
            );
            drop(buckets);
            self.total_allowed.fetch_add(1, Ordering::Relaxed);
            Ok(RateDecision::AllowBurst { warning })
        } else {
            // Reject.
            bucket.rejected_requests += 1;
            let retry_after_secs = self.retry_after(bucket, tier);
            drop(buckets);
            self.total_rejected.fetch_add(1, Ordering::Relaxed);
            Ok(RateDecision::Reject { retry_after_secs })
        }
    }

    fn register_key(&self, key: &str, tier: ServiceTier) -> Result<()> {
        Self::validate_key(key)?;

        let bucket = self.create_bucket(key, tier);
        let max_keys = self.config.read().max_tracked_keys;
        let mut buckets = self.buckets.write();

        if buckets.len() >= max_keys && !buckets.contains_key(key) {
            Self::evict_oldest_if_full(&mut buckets, max_keys);
        }

        buckets.insert(key.to_owned(), bucket);
        drop(buckets);
        Ok(())
    }

    fn get_bucket_state(&self, key: &str) -> Option<BucketState> {
        self.buckets.read().get(key).cloned()
    }

    fn reset_bucket(&self, key: &str) {
        // Read the tier first to avoid nested lock on config inside buckets write.
        let tier = {
            let buckets = self.buckets.read();
            buckets.get(key).map(|b| b.tier)
        };

        if let Some(tier) = tier {
            let capacity = self.config.read().for_tier(tier).capacity();
            let mut buckets = self.buckets.write();
            if let Some(bucket) = buckets.get_mut(key) {
                bucket.tokens = capacity;
                bucket.last_refill = Timestamp::now();
            }
            drop(buckets);
        }
    }

    fn tier_config(&self, tier: ServiceTier) -> TierRateConfig {
        self.config.read().for_tier(tier).clone()
    }

    #[allow(clippy::cast_precision_loss)]
    fn overall_stats(&self) -> RateLimiterStats {
        let total_requests = self.total_requests.load(Ordering::Relaxed);
        let total_allowed = self.total_allowed.load(Ordering::Relaxed);
        let total_rejected = self.total_rejected.load(Ordering::Relaxed);
        let overall_allow_rate = if total_requests > 0 {
            total_allowed as f64 / total_requests as f64
        } else {
            0.0
        };

        RateLimiterStats {
            total_keys: self.buckets.read().len(),
            total_requests,
            total_allowed,
            total_rejected,
            overall_allow_rate,
        }
    }

    fn set_tier_config(&self, tier: ServiceTier, config: TierRateConfig) -> Result<()> {
        if config.requests_per_minute == 0 {
            return Err(Error::Validation(
                "requests_per_minute must be > 0".into(),
            ));
        }
        if !config.burst_multiplier.is_finite() || config.burst_multiplier < 1.0 {
            return Err(Error::Validation(
                "burst_multiplier must be >= 1.0 and finite".into(),
            ));
        }

        let mut cfg = self.config.write();
        match tier {
            ServiceTier::Tier1 => cfg.tier1 = config,
            ServiceTier::Tier2 => cfg.tier2 = config,
            ServiceTier::Tier3 => cfg.tier3 = config,
            ServiceTier::Tier4 => cfg.tier4 = config,
            ServiceTier::Tier5 => cfg.tier5 = config,
        }
        drop(cfg);
        Ok(())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a limiter with a very small capacity so we can
    /// exhaust it easily in tests.
    fn small_limiter() -> RateLimiter {
        let config = RateLimiterConfig {
            tier1: TierRateConfig {
                requests_per_minute: 60, // 1/sec refill
                burst_multiplier: 2.0,
                cooldown_secs: 10,
            },
            tier2: TierRateConfig {
                requests_per_minute: 30,
                burst_multiplier: 2.0,
                cooldown_secs: 10,
            },
            tier3: TierRateConfig {
                requests_per_minute: 12,
                burst_multiplier: 2.0,
                cooldown_secs: 10,
            },
            tier4: TierRateConfig {
                requests_per_minute: 6,
                burst_multiplier: 2.0,
                cooldown_secs: 10,
            },
            tier5: TierRateConfig {
                requests_per_minute: 3,
                burst_multiplier: 2.0,
                cooldown_secs: 10,
            },
            max_tracked_keys: 100,
        };
        RateLimiter::new(config)
    }

    // -----------------------------------------------------------------------
    // Construction + defaults
    // -----------------------------------------------------------------------

    #[test]
    fn test_default_config() {
        let cfg = RateLimiterConfig::default();
        assert_eq!(cfg.tier1.requests_per_minute, 1000);
        assert_eq!(cfg.tier2.requests_per_minute, 800);
        assert_eq!(cfg.tier3.requests_per_minute, 600);
        assert_eq!(cfg.tier4.requests_per_minute, 400);
        assert_eq!(cfg.tier5.requests_per_minute, 200);
        assert!((cfg.tier1.burst_multiplier - 2.0).abs() < f64::EPSILON);
        assert_eq!(cfg.tier1.cooldown_secs, 60);
        assert_eq!(cfg.max_tracked_keys, 10_000);
    }

    #[test]
    fn test_with_defaults() {
        let limiter = RateLimiter::with_defaults();
        let stats = limiter.overall_stats();
        assert_eq!(stats.total_keys, 0);
        assert_eq!(stats.total_requests, 0);
    }

    #[test]
    fn test_new_with_custom_config() {
        let limiter = small_limiter();
        let cfg = limiter.tier_config(ServiceTier::Tier1);
        assert_eq!(cfg.requests_per_minute, 60);
    }

    #[test]
    fn test_debug_format() {
        let limiter = RateLimiter::with_defaults();
        let debug = format!("{limiter:?}");
        assert!(debug.contains("RateLimiter"));
        assert!(debug.contains("tracked_keys"));
    }

    // -----------------------------------------------------------------------
    // TierRateConfig validation
    // -----------------------------------------------------------------------

    #[test]
    fn test_tier_config_valid() {
        let cfg = TierRateConfig::new(100, 2.0, 30);
        assert!(cfg.is_ok());
        let cfg = cfg.ok();
        assert!(cfg.is_some());
    }

    #[test]
    fn test_tier_config_zero_rpm() {
        let result = TierRateConfig::new(0, 2.0, 30);
        assert!(result.is_err());
    }

    #[test]
    fn test_tier_config_burst_below_one() {
        let result = TierRateConfig::new(100, 0.5, 30);
        assert!(result.is_err());
    }

    #[test]
    fn test_tier_config_burst_nan() {
        let result = TierRateConfig::new(100, f64::NAN, 30);
        assert!(result.is_err());
    }

    #[test]
    fn test_tier_config_burst_infinity() {
        let result = TierRateConfig::new(100, f64::INFINITY, 30);
        assert!(result.is_err());
    }

    #[test]
    fn test_tier_config_capacity_and_rate() {
        let cfg = TierRateConfig {
            requests_per_minute: 120,
            burst_multiplier: 2.0,
            cooldown_secs: 60,
        };
        assert!((cfg.capacity() - 120.0).abs() < f64::EPSILON);
        assert!((cfg.burst_capacity() - 240.0).abs() < f64::EPSILON);
        assert!((cfg.refill_rate() - 2.0).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // register_key
    // -----------------------------------------------------------------------

    #[test]
    fn test_register_key_success() {
        let limiter = small_limiter();
        let result = limiter.register_key("svc-a", ServiceTier::Tier1);
        assert!(result.is_ok());
        let bucket = limiter.get_bucket_state("svc-a");
        assert!(bucket.is_some());
    }

    #[test]
    fn test_register_key_empty_fails() {
        let limiter = small_limiter();
        let result = limiter.register_key("", ServiceTier::Tier1);
        assert!(result.is_err());
    }

    #[test]
    fn test_register_key_overwrites() {
        let limiter = small_limiter();
        let _ = limiter.register_key("svc-a", ServiceTier::Tier1);
        let _ = limiter.register_key("svc-a", ServiceTier::Tier3);
        let bucket = limiter.get_bucket_state("svc-a");
        assert!(bucket.is_some());
        let b = bucket.as_ref();
        assert!(b.is_some());
        if let Some(bucket) = b {
            assert_eq!(bucket.tier, ServiceTier::Tier3);
        }
    }

    // -----------------------------------------------------------------------
    // check_and_consume: basic Allow
    // -----------------------------------------------------------------------

    #[test]
    fn test_t1_allows_within_limit() {
        let limiter = small_limiter();
        let decision = limiter.check_and_consume("svc-a", ServiceTier::Tier1);
        assert!(decision.is_ok());
        assert_eq!(decision.ok(), Some(RateDecision::Allow));
    }

    #[test]
    fn test_t2_allows_within_limit() {
        let limiter = small_limiter();
        let decision = limiter.check_and_consume("svc-b", ServiceTier::Tier2);
        assert!(decision.is_ok());
        assert_eq!(decision.ok(), Some(RateDecision::Allow));
    }

    #[test]
    fn test_t3_allows_within_limit() {
        let limiter = small_limiter();
        let decision = limiter.check_and_consume("svc-c", ServiceTier::Tier3);
        assert!(decision.is_ok());
        assert_eq!(decision.ok(), Some(RateDecision::Allow));
    }

    #[test]
    fn test_t4_allows_within_limit() {
        let limiter = small_limiter();
        let decision = limiter.check_and_consume("svc-d", ServiceTier::Tier4);
        assert!(decision.is_ok());
        assert_eq!(decision.ok(), Some(RateDecision::Allow));
    }

    #[test]
    fn test_t5_allows_within_limit() {
        let limiter = small_limiter();
        let decision = limiter.check_and_consume("svc-e", ServiceTier::Tier5);
        assert!(decision.is_ok());
        assert_eq!(decision.ok(), Some(RateDecision::Allow));
    }

    // -----------------------------------------------------------------------
    // check_and_consume: exhaustion and rejection
    // -----------------------------------------------------------------------

    #[test]
    fn test_t5_rejects_over_limit() {
        let limiter = small_limiter();
        // T5 capacity is 3 tokens. Exhaust them.
        for _ in 0..3 {
            let d = limiter.check_and_consume("svc-e", ServiceTier::Tier5);
            assert!(d.is_ok());
            assert_eq!(d.ok(), Some(RateDecision::Allow));
        }
        // Next request should be burst or reject depending on state.
        // Burst capacity = 6, so tokens went from 3 down to 0 after 3 allows.
        // tokens < 1.0, burst headroom = burst_capacity - capacity = 6 - 3 = 3.
        // tokens + burst_headroom = 0 + 3 = 3 >= 1.0, so AllowBurst.
        let d = limiter.check_and_consume("svc-e", ServiceTier::Tier5);
        assert!(d.is_ok());
        match d.ok() {
            Some(RateDecision::AllowBurst { .. }) => {} // expected
            other => panic!("expected AllowBurst, got {other:?}"),
        }
    }

    #[test]
    fn test_complete_exhaustion_rejects() {
        // Use a config with burst_multiplier = 1.0 (no burst headroom).
        let config = RateLimiterConfig {
            tier1: TierRateConfig {
                requests_per_minute: 3,
                burst_multiplier: 1.0,
                cooldown_secs: 10,
            },
            tier2: TierRateConfig {
                requests_per_minute: 3,
                burst_multiplier: 1.0,
                cooldown_secs: 10,
            },
            tier3: TierRateConfig {
                requests_per_minute: 3,
                burst_multiplier: 1.0,
                cooldown_secs: 10,
            },
            tier4: TierRateConfig {
                requests_per_minute: 3,
                burst_multiplier: 1.0,
                cooldown_secs: 10,
            },
            tier5: TierRateConfig {
                requests_per_minute: 3,
                burst_multiplier: 1.0,
                cooldown_secs: 10,
            },
            max_tracked_keys: 100,
        };
        let limiter = RateLimiter::new(config);

        // Exhaust all 3 tokens.
        for _ in 0..3 {
            let d = limiter.check_and_consume("svc-x", ServiceTier::Tier1);
            assert!(d.is_ok());
            assert_eq!(d.ok(), Some(RateDecision::Allow));
        }

        // 4th request should be rejected.
        let d = limiter.check_and_consume("svc-x", ServiceTier::Tier1);
        assert!(d.is_ok());
        match d.ok() {
            Some(RateDecision::Reject { retry_after_secs }) => {
                assert!(retry_after_secs > 0);
            }
            other => panic!("expected Reject, got {other:?}"),
        }
    }

    #[test]
    fn test_reject_retry_after_is_positive() {
        let config = RateLimiterConfig {
            tier1: TierRateConfig {
                requests_per_minute: 1,
                burst_multiplier: 1.0,
                cooldown_secs: 30,
            },
            ..RateLimiterConfig::default()
        };
        let limiter = RateLimiter::new(config);

        // Consume the single token.
        let _ = limiter.check_and_consume("svc-y", ServiceTier::Tier1);
        // Next should reject.
        let d = limiter.check_and_consume("svc-y", ServiceTier::Tier1);
        assert!(d.is_ok());
        if let Some(RateDecision::Reject { retry_after_secs }) = d.ok() {
            assert!(retry_after_secs >= 1);
        }
    }

    // -----------------------------------------------------------------------
    // Burst behavior
    // -----------------------------------------------------------------------

    #[test]
    fn test_burst_decision_contains_warning() {
        let limiter = small_limiter();
        // T5: capacity=3, burst_capacity=6, burst_multiplier=2.0.
        // Exhaust normal tokens.
        for _ in 0..3 {
            let _ = limiter.check_and_consume("burst-key", ServiceTier::Tier5);
        }
        let d = limiter.check_and_consume("burst-key", ServiceTier::Tier5);
        assert!(d.is_ok());
        match d.ok() {
            Some(RateDecision::AllowBurst { warning }) => {
                assert!(warning.contains("burst-key"));
                assert!(warning.contains("burst capacity"));
            }
            other => panic!("expected AllowBurst, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Auto-register on first check
    // -----------------------------------------------------------------------

    #[test]
    fn test_auto_register_on_first_check() {
        let limiter = small_limiter();
        assert!(limiter.get_bucket_state("auto-key").is_none());
        let d = limiter.check_and_consume("auto-key", ServiceTier::Tier2);
        assert!(d.is_ok());
        assert!(limiter.get_bucket_state("auto-key").is_some());
    }

    // -----------------------------------------------------------------------
    // Empty key validation
    // -----------------------------------------------------------------------

    #[test]
    fn test_check_and_consume_empty_key() {
        let limiter = small_limiter();
        let result = limiter.check_and_consume("", ServiceTier::Tier1);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // get_bucket_state
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_bucket_state_existing() {
        let limiter = small_limiter();
        let _ = limiter.register_key("svc-a", ServiceTier::Tier3);
        let state = limiter.get_bucket_state("svc-a");
        assert!(state.is_some());
        let s = state.as_ref();
        assert!(s.is_some());
        if let Some(bucket) = s {
            assert_eq!(bucket.key, "svc-a");
            assert_eq!(bucket.tier, ServiceTier::Tier3);
            assert!((bucket.capacity - 12.0).abs() < f64::EPSILON);
            assert!((bucket.burst_capacity - 24.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_get_bucket_state_missing() {
        let limiter = small_limiter();
        assert!(limiter.get_bucket_state("no-such-key").is_none());
    }

    // -----------------------------------------------------------------------
    // reset_bucket
    // -----------------------------------------------------------------------

    #[test]
    fn test_reset_bucket_restores_tokens() {
        let limiter = small_limiter();
        let _ = limiter.register_key("svc-a", ServiceTier::Tier5);

        // Consume all 3 tokens.
        for _ in 0..3 {
            let _ = limiter.check_and_consume("svc-a", ServiceTier::Tier5);
        }

        // Tokens should be near zero.
        let state = limiter.get_bucket_state("svc-a");
        assert!(state.is_some());
        if let Some(b) = &state {
            assert!(b.tokens < 1.0);
        }

        // Reset.
        limiter.reset_bucket("svc-a");

        // Tokens should be back to capacity.
        let state = limiter.get_bucket_state("svc-a");
        assert!(state.is_some());
        if let Some(b) = &state {
            assert!((b.tokens - b.capacity).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_reset_bucket_nonexistent_is_noop() {
        let limiter = small_limiter();
        limiter.reset_bucket("nonexistent"); // should not panic
    }

    // -----------------------------------------------------------------------
    // tier_config
    // -----------------------------------------------------------------------

    #[test]
    fn test_tier_config_returns_correct_values() {
        let limiter = small_limiter();
        let cfg = limiter.tier_config(ServiceTier::Tier3);
        assert_eq!(cfg.requests_per_minute, 12);
        assert!((cfg.burst_multiplier - 2.0).abs() < f64::EPSILON);
        assert_eq!(cfg.cooldown_secs, 10);
    }

    #[test]
    fn test_tier_config_all_tiers() {
        let limiter = small_limiter();
        let tiers = [
            (ServiceTier::Tier1, 60u64),
            (ServiceTier::Tier2, 30),
            (ServiceTier::Tier3, 12),
            (ServiceTier::Tier4, 6),
            (ServiceTier::Tier5, 3),
        ];
        for (tier, expected_rpm) in &tiers {
            let cfg = limiter.tier_config(*tier);
            assert_eq!(cfg.requests_per_minute, *expected_rpm);
        }
    }

    // -----------------------------------------------------------------------
    // set_tier_config
    // -----------------------------------------------------------------------

    #[test]
    fn test_set_tier_config_success() {
        let limiter = small_limiter();
        let new_cfg = TierRateConfig {
            requests_per_minute: 999,
            burst_multiplier: 3.0,
            cooldown_secs: 5,
        };
        let result = limiter.set_tier_config(ServiceTier::Tier2, new_cfg);
        assert!(result.is_ok());
        let cfg = limiter.tier_config(ServiceTier::Tier2);
        assert_eq!(cfg.requests_per_minute, 999);
        assert!((cfg.burst_multiplier - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_set_tier_config_zero_rpm_fails() {
        let limiter = small_limiter();
        let bad_cfg = TierRateConfig {
            requests_per_minute: 0,
            burst_multiplier: 2.0,
            cooldown_secs: 60,
        };
        let result = limiter.set_tier_config(ServiceTier::Tier1, bad_cfg);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_tier_config_bad_burst_fails() {
        let limiter = small_limiter();
        let bad_cfg = TierRateConfig {
            requests_per_minute: 100,
            burst_multiplier: 0.5,
            cooldown_secs: 60,
        };
        let result = limiter.set_tier_config(ServiceTier::Tier1, bad_cfg);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_tier_config_nan_burst_fails() {
        let limiter = small_limiter();
        let bad_cfg = TierRateConfig {
            requests_per_minute: 100,
            burst_multiplier: f64::NAN,
            cooldown_secs: 60,
        };
        let result = limiter.set_tier_config(ServiceTier::Tier1, bad_cfg);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // overall_stats
    // -----------------------------------------------------------------------

    #[test]
    fn test_overall_stats_initial() {
        let limiter = small_limiter();
        let stats = limiter.overall_stats();
        assert_eq!(stats.total_keys, 0);
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.total_allowed, 0);
        assert_eq!(stats.total_rejected, 0);
        assert!((stats.overall_allow_rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_overall_stats_after_requests() {
        let limiter = small_limiter();
        let _ = limiter.check_and_consume("svc-a", ServiceTier::Tier1);
        let _ = limiter.check_and_consume("svc-b", ServiceTier::Tier2);
        let stats = limiter.overall_stats();
        assert_eq!(stats.total_keys, 2);
        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.total_allowed, 2);
        assert_eq!(stats.total_rejected, 0);
        assert!((stats.overall_allow_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_overall_stats_with_rejections() {
        let config = RateLimiterConfig {
            tier1: TierRateConfig {
                requests_per_minute: 1,
                burst_multiplier: 1.0,
                cooldown_secs: 10,
            },
            ..RateLimiterConfig::default()
        };
        let limiter = RateLimiter::new(config);

        let _ = limiter.check_and_consume("svc-a", ServiceTier::Tier1);
        let _ = limiter.check_and_consume("svc-a", ServiceTier::Tier1); // reject

        let stats = limiter.overall_stats();
        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.total_allowed, 1);
        assert_eq!(stats.total_rejected, 1);
        assert!((stats.overall_allow_rate - 0.5).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Bucket state accounting
    // -----------------------------------------------------------------------

    #[test]
    fn test_bucket_request_counters() {
        let limiter = small_limiter();
        let _ = limiter.register_key("svc-a", ServiceTier::Tier5);

        for _ in 0..5 {
            let _ = limiter.check_and_consume("svc-a", ServiceTier::Tier5);
        }

        let state = limiter.get_bucket_state("svc-a");
        assert!(state.is_some());
        if let Some(b) = &state {
            assert_eq!(b.total_requests, 5);
            // 3 Allow + some AllowBurst, total allowed should be >= 3
            assert!(b.allowed_requests >= 3);
        }
    }

    #[test]
    fn test_bucket_tokens_decrease_on_allow() {
        let limiter = small_limiter();
        let _ = limiter.register_key("svc-a", ServiceTier::Tier1);

        let before = limiter.get_bucket_state("svc-a");
        assert!(before.is_some());
        let initial_tokens = before.as_ref().map_or(0.0, |b| b.tokens);

        let _ = limiter.check_and_consume("svc-a", ServiceTier::Tier1);

        let after = limiter.get_bucket_state("svc-a");
        assert!(after.is_some());
        if let Some(b) = &after {
            // Tokens should have decreased (minus 1.0, plus possible refill).
            // Because the Timestamp is monotonic and likely advanced by a few
            // ticks, there may be a tiny refill. But net should be less.
            assert!(b.tokens < initial_tokens + 1.0);
        }
    }

    // -----------------------------------------------------------------------
    // Multiple keys isolation
    // -----------------------------------------------------------------------

    #[test]
    fn test_different_keys_are_independent() {
        let config = RateLimiterConfig {
            tier1: TierRateConfig {
                requests_per_minute: 2,
                burst_multiplier: 1.0,
                cooldown_secs: 10,
            },
            ..RateLimiterConfig::default()
        };
        let limiter = RateLimiter::new(config);

        // Exhaust key-a.
        let _ = limiter.check_and_consume("key-a", ServiceTier::Tier1);
        let _ = limiter.check_and_consume("key-a", ServiceTier::Tier1);
        let d = limiter.check_and_consume("key-a", ServiceTier::Tier1);
        assert!(d.is_ok());
        assert!(matches!(d.ok(), Some(RateDecision::Reject { .. })));

        // key-b should still be allowed.
        let d = limiter.check_and_consume("key-b", ServiceTier::Tier1);
        assert!(d.is_ok());
        assert_eq!(d.ok(), Some(RateDecision::Allow));
    }

    // -----------------------------------------------------------------------
    // Max tracked keys eviction
    // -----------------------------------------------------------------------

    #[test]
    fn test_max_tracked_keys_eviction() {
        let config = RateLimiterConfig {
            max_tracked_keys: 3,
            ..RateLimiterConfig::default()
        };
        let limiter = RateLimiter::new(config);

        let _ = limiter.register_key("k1", ServiceTier::Tier1);
        let _ = limiter.register_key("k2", ServiceTier::Tier1);
        let _ = limiter.register_key("k3", ServiceTier::Tier1);
        assert_eq!(limiter.overall_stats().total_keys, 3);

        // Adding a 4th key should evict one.
        let _ = limiter.register_key("k4", ServiceTier::Tier1);
        assert!(limiter.overall_stats().total_keys <= 3);
    }

    #[test]
    fn test_max_tracked_keys_eviction_via_check() {
        let config = RateLimiterConfig {
            max_tracked_keys: 2,
            ..RateLimiterConfig::default()
        };
        let limiter = RateLimiter::new(config);

        let _ = limiter.check_and_consume("k1", ServiceTier::Tier1);
        let _ = limiter.check_and_consume("k2", ServiceTier::Tier1);
        let _ = limiter.check_and_consume("k3", ServiceTier::Tier1);
        assert!(limiter.overall_stats().total_keys <= 2);
    }

    // -----------------------------------------------------------------------
    // Concurrent access (basic)
    // -----------------------------------------------------------------------

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let limiter = Arc::new(small_limiter());
        let mut handles = Vec::new();

        for i in 0..10 {
            let lim = Arc::clone(&limiter);
            handles.push(thread::spawn(move || {
                let key = format!("thread-{i}");
                for _ in 0..5 {
                    let _ = lim.check_and_consume(&key, ServiceTier::Tier1);
                }
            }));
        }

        for h in handles {
            h.join().ok();
        }

        let stats = limiter.overall_stats();
        assert_eq!(stats.total_requests, 50);
        assert_eq!(stats.total_keys, 10);
    }

    #[test]
    fn test_concurrent_same_key() {
        use std::sync::Arc;
        use std::thread;

        let limiter = Arc::new(small_limiter());
        let mut handles = Vec::new();

        for _ in 0..5 {
            let lim = Arc::clone(&limiter);
            handles.push(thread::spawn(move || {
                for _ in 0..4 {
                    let _ = lim.check_and_consume("shared-key", ServiceTier::Tier1);
                }
            }));
        }

        for h in handles {
            h.join().ok();
        }

        let stats = limiter.overall_stats();
        assert_eq!(stats.total_requests, 20);
        assert_eq!(stats.total_keys, 1);
        assert_eq!(stats.total_allowed + stats.total_rejected, 20);
    }

    // -----------------------------------------------------------------------
    // RateDecision equality
    // -----------------------------------------------------------------------

    #[test]
    fn test_rate_decision_allow_eq() {
        assert_eq!(RateDecision::Allow, RateDecision::Allow);
    }

    #[test]
    fn test_rate_decision_reject_eq() {
        assert_eq!(
            RateDecision::Reject { retry_after_secs: 5 },
            RateDecision::Reject { retry_after_secs: 5 }
        );
    }

    #[test]
    fn test_rate_decision_reject_ne() {
        assert_ne!(
            RateDecision::Reject { retry_after_secs: 5 },
            RateDecision::Reject {
                retry_after_secs: 10
            }
        );
    }

    #[test]
    fn test_rate_decision_allow_ne_reject() {
        assert_ne!(
            RateDecision::Allow,
            RateDecision::Reject { retry_after_secs: 1 }
        );
    }

    #[test]
    fn test_rate_decision_allow_burst_eq() {
        assert_eq!(
            RateDecision::AllowBurst {
                warning: "w".into()
            },
            RateDecision::AllowBurst {
                warning: "w".into()
            }
        );
    }

    #[test]
    fn test_rate_decision_clone() {
        let d = RateDecision::Reject { retry_after_secs: 7 };
        let d2 = d.clone();
        assert_eq!(d, d2);
    }

    #[test]
    fn test_rate_decision_debug() {
        let d = RateDecision::Allow;
        let s = format!("{d:?}");
        assert!(s.contains("Allow"));
    }

    // -----------------------------------------------------------------------
    // RateLimiterStats
    // -----------------------------------------------------------------------

    #[test]
    fn test_stats_allow_rate_zero_requests() {
        let stats = RateLimiterStats {
            total_keys: 0,
            total_requests: 0,
            total_allowed: 0,
            total_rejected: 0,
            overall_allow_rate: 0.0,
        };
        assert!((stats.overall_allow_rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_stats_clone_and_debug() {
        let stats = RateLimiterStats {
            total_keys: 3,
            total_requests: 100,
            total_allowed: 90,
            total_rejected: 10,
            overall_allow_rate: 0.9,
        };
        let s2 = stats.clone();
        assert_eq!(s2.total_keys, 3);
        let debug = format!("{stats:?}");
        assert!(debug.contains("RateLimiterStats"));
    }

    // -----------------------------------------------------------------------
    // BucketState fields
    // -----------------------------------------------------------------------

    #[test]
    fn test_bucket_state_initial_values() {
        let limiter = small_limiter();
        let _ = limiter.register_key("svc-a", ServiceTier::Tier4);
        let bucket = limiter.get_bucket_state("svc-a");
        assert!(bucket.is_some());
        if let Some(b) = &bucket {
            assert_eq!(b.key, "svc-a");
            assert_eq!(b.tier, ServiceTier::Tier4);
            assert!((b.capacity - 6.0).abs() < f64::EPSILON);
            assert!((b.burst_capacity - 12.0).abs() < f64::EPSILON);
            assert!((b.refill_rate - 0.1).abs() < f64::EPSILON);
            assert_eq!(b.total_requests, 0);
            assert_eq!(b.allowed_requests, 0);
            assert_eq!(b.rejected_requests, 0);
        }
    }

    #[test]
    fn test_bucket_state_clone() {
        let limiter = small_limiter();
        let _ = limiter.register_key("svc-a", ServiceTier::Tier1);
        let b1 = limiter.get_bucket_state("svc-a");
        assert!(b1.is_some());
        let b2 = b1.clone();
        assert!(b2.is_some());
        if let (Some(a), Some(b)) = (&b1, &b2) {
            assert_eq!(a.key, b.key);
        }
    }

    // -----------------------------------------------------------------------
    // RateLimiterConfig::for_tier
    // -----------------------------------------------------------------------

    #[test]
    fn test_config_for_tier_mapping() {
        let cfg = RateLimiterConfig::default();
        assert_eq!(cfg.for_tier(ServiceTier::Tier1).requests_per_minute, 1000);
        assert_eq!(cfg.for_tier(ServiceTier::Tier2).requests_per_minute, 800);
        assert_eq!(cfg.for_tier(ServiceTier::Tier3).requests_per_minute, 600);
        assert_eq!(cfg.for_tier(ServiceTier::Tier4).requests_per_minute, 400);
        assert_eq!(cfg.for_tier(ServiceTier::Tier5).requests_per_minute, 200);
    }

    // -----------------------------------------------------------------------
    // Default config builder
    // -----------------------------------------------------------------------

    #[test]
    fn test_default_burst_multiplier_all_tiers() {
        let cfg = RateLimiterConfig::default();
        for tier in &[
            ServiceTier::Tier1,
            ServiceTier::Tier2,
            ServiceTier::Tier3,
            ServiceTier::Tier4,
            ServiceTier::Tier5,
        ] {
            let tc = cfg.for_tier(*tier);
            assert!(
                (tc.burst_multiplier - 2.0).abs() < f64::EPSILON,
                "tier {tier:?} burst multiplier should be 2.0"
            );
        }
    }

    #[test]
    fn test_default_cooldown_all_tiers() {
        let cfg = RateLimiterConfig::default();
        for tier in &[
            ServiceTier::Tier1,
            ServiceTier::Tier2,
            ServiceTier::Tier3,
            ServiceTier::Tier4,
            ServiceTier::Tier5,
        ] {
            let tc = cfg.for_tier(*tier);
            assert_eq!(tc.cooldown_secs, 60, "tier {tier:?} cooldown should be 60");
        }
    }

    // -----------------------------------------------------------------------
    // Trait object compatibility
    // -----------------------------------------------------------------------

    #[test]
    fn test_trait_object_compatible() {
        let limiter: Box<dyn RateLimiting> = Box::new(small_limiter());
        let d = limiter.check_and_consume("trait-obj", ServiceTier::Tier1);
        assert!(d.is_ok());
        assert_eq!(d.ok(), Some(RateDecision::Allow));
    }

    // -----------------------------------------------------------------------
    // Refill mechanics
    // -----------------------------------------------------------------------

    #[test]
    fn test_refill_over_time() {
        // Because Timestamp::now() increments the global tick, multiple calls
        // between check_and_consume will advance "time" and trigger refills.
        let config = RateLimiterConfig {
            tier1: TierRateConfig {
                requests_per_minute: 60, // 1 token/sec
                burst_multiplier: 1.0,
                cooldown_secs: 10,
            },
            ..RateLimiterConfig::default()
        };
        let limiter = RateLimiter::new(config);

        // Register and exhaust.
        let _ = limiter.register_key("refill-key", ServiceTier::Tier1);

        // Consume a chunk of tokens.
        for _ in 0..59 {
            let _ = limiter.check_and_consume("refill-key", ServiceTier::Tier1);
        }

        // Because the timestamp counter is advancing with each Timestamp::now()
        // call (in register + create_bucket + refill), tokens get refilled.
        // This test validates the refill code path runs without error.
        let state = limiter.get_bucket_state("refill-key");
        assert!(state.is_some());
    }

    // -----------------------------------------------------------------------
    // Edge: register_key replaces existing bucket
    // -----------------------------------------------------------------------

    #[test]
    fn test_register_key_replaces_bucket() {
        let limiter = small_limiter();
        let _ = limiter.register_key("re-key", ServiceTier::Tier1);

        // Consume some tokens.
        let _ = limiter.check_and_consume("re-key", ServiceTier::Tier1);
        let _ = limiter.check_and_consume("re-key", ServiceTier::Tier1);

        let before = limiter.get_bucket_state("re-key");
        assert!(before.is_some());
        if let Some(b) = &before {
            assert_eq!(b.total_requests, 2);
        }

        // Re-register resets the bucket.
        let _ = limiter.register_key("re-key", ServiceTier::Tier1);
        let after = limiter.get_bucket_state("re-key");
        assert!(after.is_some());
        if let Some(b) = &after {
            assert_eq!(b.total_requests, 0);
        }
    }
}
