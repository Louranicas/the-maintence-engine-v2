//! # M51: Auth Handler
//!
//! Token-based authentication and authorization for ULTRAPLATE inter-service
//! communication. Provides issuance, verification, revocation, and rotation
//! of service/agent/human/API-key tokens with full security event auditing.
//!
//! ## Layer: L4 (Integration)
//!
//! ## HMAC Note
//!
//! The current implementation uses [`std::collections::hash_map::DefaultHasher`]
//! as a **structural placeholder** for HMAC digest computation. This is NOT
//! cryptographically secure and exists only to validate the token pipeline.
//! Replace with the `hmac` + `sha2` crates before any production deployment.
//!
//! ## Security
//!
//! - Tokens are **never logged** — only token IDs appear in events and audit
//!   summaries.
//! - All public methods take `&self` and use interior mutability via
//!   [`parking_lot::RwLock`] and [`AtomicU64`].
//!
//! ## Features
//!
//! - Four token types with configurable TTL (Service 24h, Agent 1h, Human 7d, API Key 90d)
//! - Concurrent-safe token store via `RwLock<HashMap>`
//! - Security event log with configurable capacity cap
//! - Atomic counters for issued/verified/failure tallies
//! - Token rotation with safe lock ordering (read → drop → write → issue)
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L04_INTEGRATION.md)

#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::m1_foundation::shared_types::Timestamp;
use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default signing key for HMAC simulation.
const DEFAULT_SIGNING_KEY: &str = "ultraplate-me-v2-key";

/// Default TTL for service tokens in hours.
const DEFAULT_SERVICE_TTL_HOURS: u64 = 24;

/// Default TTL for agent tokens in hours.
const DEFAULT_AGENT_TTL_HOURS: u64 = 1;

/// Default TTL for human tokens in days.
const DEFAULT_HUMAN_TTL_DAYS: u64 = 7;

/// Default TTL for API key tokens in days.
const DEFAULT_API_KEY_TTL_DAYS: u64 = 90;

/// Default maximum number of security events to retain.
const DEFAULT_MAX_EVENTS_LOG: usize = 500;

/// Number of ticks per hour (simulated).
const TICKS_PER_HOUR: u64 = 3600;

/// Number of hours per day.
const HOURS_PER_DAY: u64 = 24;

// ---------------------------------------------------------------------------
// Token Type
// ---------------------------------------------------------------------------

/// Classification of authentication tokens by their intended consumer.
///
/// Each variant carries a different default TTL:
/// - `Service`: 24 hours — machine-to-machine within the ULTRAPLATE mesh
/// - `Agent`: 1 hour — short-lived CVA-NAM agent credentials
/// - `Human`: 7 days — human operator sessions
/// - `ApiKey`: 90 days — long-lived programmatic access
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenType {
    /// Machine-to-machine service token (24h default TTL).
    Service,
    /// Short-lived agent token (1h default TTL).
    Agent,
    /// Human operator session token (7d default TTL).
    Human,
    /// Long-lived programmatic API key (90d default TTL).
    ApiKey,
}

impl fmt::Display for TokenType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Service => write!(f, "Service"),
            Self::Agent => write!(f, "Agent"),
            Self::Human => write!(f, "Human"),
            Self::ApiKey => write!(f, "ApiKey"),
        }
    }
}

// ---------------------------------------------------------------------------
// Token Identity
// ---------------------------------------------------------------------------

/// Identity claims embedded within a token.
///
/// Combines a unique identifier, token type classification, authorization
/// tier (0-5), and a list of granted scope strings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenIdentity {
    /// Unique identity string (e.g. service name, agent UUID).
    pub id: String,
    /// Token classification.
    pub token_type: TokenType,
    /// Authorization tier (0 = highest, 5 = lowest).
    pub tier: u8,
    /// Granted scopes (e.g. `["read:health", "write:config"]`).
    pub scopes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Issued Token
// ---------------------------------------------------------------------------

/// A successfully issued authentication token.
///
/// Contains the raw token string, its computed HMAC digest, and all
/// identity/timing metadata. The `raw` field is the only value that
/// should be transmitted to the client — it is **never** persisted to
/// logs or audit trails.
#[derive(Clone, Debug)]
pub struct IssuedToken {
    /// Unique token identifier (UUID v4).
    pub token_id: String,
    /// Raw token string in `{header}.{payload}.{sig}` format.
    ///
    /// **Security:** Must never be logged or persisted in audit records.
    pub raw: String,
    /// The identity claims embedded in this token.
    pub identity: TokenIdentity,
    /// Timestamp when the token was issued.
    pub issued_at: Timestamp,
    /// Timestamp when the token expires.
    pub expires_at: Timestamp,
    /// Hex-encoded HMAC digest of the token payload.
    pub hmac_digest: String,
}

// ---------------------------------------------------------------------------
// Verified Claims
// ---------------------------------------------------------------------------

/// Result of token verification.
///
/// Contains the decoded identity, timing metadata, and validity flags.
#[derive(Clone, Debug)]
pub struct VerifiedClaims {
    /// The token ID that was verified.
    pub token_id: String,
    /// The decoded identity claims.
    pub identity: TokenIdentity,
    /// When the token was originally issued.
    pub issued_at: Timestamp,
    /// When the token expires.
    pub expires_at: Timestamp,
    /// Whether the token is currently valid (not expired, not revoked).
    pub valid: bool,
    /// Whether the token has been explicitly revoked.
    pub revoked: bool,
}

// ---------------------------------------------------------------------------
// Security Event Types
// ---------------------------------------------------------------------------

/// Classification of security-relevant events in the auth subsystem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityEventType {
    /// A token was successfully verified.
    AuthSuccess,
    /// Token verification failed (invalid, expired, or revoked).
    AuthFailure,
    /// A token was found to be expired during verification.
    TokenExpired,
    /// A token was explicitly revoked.
    TokenRevoked,
    /// An operation was denied due to insufficient scopes.
    ScopeViolation,
    /// A request was rejected due to rate limiting.
    RateLimited,
}

impl fmt::Display for SecurityEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthSuccess => write!(f, "AuthSuccess"),
            Self::AuthFailure => write!(f, "AuthFailure"),
            Self::TokenExpired => write!(f, "TokenExpired"),
            Self::TokenRevoked => write!(f, "TokenRevoked"),
            Self::ScopeViolation => write!(f, "ScopeViolation"),
            Self::RateLimited => write!(f, "RateLimited"),
        }
    }
}

// ---------------------------------------------------------------------------
// Security Event
// ---------------------------------------------------------------------------

/// A recorded security event in the auth subsystem.
///
/// Events are capped at [`AuthConfig::max_events_log`] entries. Once the
/// cap is reached, the oldest event is evicted to make room.
#[derive(Clone, Debug)]
pub struct SecurityEvent {
    /// Unique event identifier (UUID v4).
    pub id: String,
    /// The type of security event.
    pub event_type: SecurityEventType,
    /// The source that triggered this event (token ID, service name, etc.).
    pub source_id: String,
    /// When this event occurred.
    pub timestamp: Timestamp,
    /// Human-readable description of the event.
    pub details: String,
}

// ---------------------------------------------------------------------------
// Auth Audit Summary
// ---------------------------------------------------------------------------

/// Aggregate counters for the authentication subsystem.
///
/// All counters are monotonically increasing (except `active_tokens` and
/// `event_count` which reflect current state).
#[derive(Clone, Debug)]
pub struct AuthAuditSummary {
    /// Total tokens issued since startup.
    pub total_issued: u64,
    /// Total token verifications performed.
    pub total_verified: u64,
    /// Total tokens explicitly revoked.
    pub total_revoked: u64,
    /// Total failed verification attempts.
    pub total_failures: u64,
    /// Number of currently active (non-revoked, non-expired) tokens.
    pub active_tokens: usize,
    /// Number of security events in the log.
    pub event_count: usize,
}

// ---------------------------------------------------------------------------
// Auth Config
// ---------------------------------------------------------------------------

/// Configuration for the authentication subsystem.
///
/// Sensible defaults are provided via [`Default`].
#[derive(Clone, Debug)]
pub struct AuthConfig {
    /// Key used for HMAC digest computation.
    ///
    /// **Note:** Uses `DefaultHasher` as a structural placeholder.
    /// Replace with a real HMAC key before production.
    pub signing_key: String,
    /// TTL for `TokenType::Service` tokens (hours).
    pub service_ttl_hours: u64,
    /// TTL for `TokenType::Agent` tokens (hours).
    pub agent_ttl_hours: u64,
    /// TTL for `TokenType::Human` tokens (days).
    pub human_ttl_days: u64,
    /// TTL for `TokenType::ApiKey` tokens (days).
    pub api_key_ttl_days: u64,
    /// Maximum number of security events to keep in memory.
    pub max_events_log: usize,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            signing_key: DEFAULT_SIGNING_KEY.to_owned(),
            service_ttl_hours: DEFAULT_SERVICE_TTL_HOURS,
            agent_ttl_hours: DEFAULT_AGENT_TTL_HOURS,
            human_ttl_days: DEFAULT_HUMAN_TTL_DAYS,
            api_key_ttl_days: DEFAULT_API_KEY_TTL_DAYS,
            max_events_log: DEFAULT_MAX_EVENTS_LOG,
        }
    }
}

// ---------------------------------------------------------------------------
// Authenticator Trait
// ---------------------------------------------------------------------------

/// Core authentication trait for the Maintenance Engine.
///
/// All methods take `&self` — state mutation is handled via interior
/// mutability (`RwLock`, `AtomicU64`).
///
/// # Errors
///
/// Methods return `Err` on:
/// - Empty or invalid identity IDs
/// - Token not found for verification/revocation/rotation
/// - Expired or revoked tokens during verification
pub trait Authenticator: Send + Sync + fmt::Debug {
    /// Issue a new token for the given identity.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the identity ID is empty.
    fn issue_token(&self, identity: &TokenIdentity) -> Result<IssuedToken>;

    /// Verify a raw token string and return decoded claims.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the raw token is empty or malformed,
    /// or if the referenced token ID is not found.
    fn verify_token(&self, raw: &str) -> Result<VerifiedClaims>;

    /// Revoke a token by its ID, preventing future verification.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the token ID is empty or not found.
    fn revoke_token(&self, token_id: &str) -> Result<()>;

    /// Check whether a token ID is currently valid (exists and not revoked).
    fn is_token_valid(&self, token_id: &str) -> bool;

    /// Rotate a token: revoke the old token and issue a new one with the
    /// same identity claims.
    ///
    /// Lock ordering: read tokens → drop guard → write revoked → issue new.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the token ID is empty or not found.
    fn rotate_token(&self, token_id: &str) -> Result<IssuedToken>;

    /// Return all recorded security events.
    fn security_events(&self) -> Vec<SecurityEvent>;

    /// Return an aggregate audit summary.
    fn audit_summary(&self) -> AuthAuditSummary;
}

// ---------------------------------------------------------------------------
// Auth Manager
// ---------------------------------------------------------------------------

/// Concrete implementation of [`Authenticator`].
///
/// Thread-safe via `parking_lot::RwLock` for collections and
/// `AtomicU64` for monotonic counters.
pub struct AuthManager {
    /// Active (non-revoked) tokens keyed by token ID.
    tokens: RwLock<HashMap<String, IssuedToken>>,
    /// Set of revoked token IDs.
    revoked: RwLock<HashSet<String>>,
    /// Capped log of security events.
    events: RwLock<Vec<SecurityEvent>>,
    /// Configuration.
    config: AuthConfig,
    /// Monotonic counter: total tokens issued.
    issued_count: AtomicU64,
    /// Monotonic counter: total verification attempts.
    verified_count: AtomicU64,
    /// Monotonic counter: total failed verifications.
    failure_count: AtomicU64,
    /// Monotonic counter: total revocations.
    revoked_count: AtomicU64,
}

impl fmt::Debug for AuthManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthManager")
            .field("active_tokens", &self.tokens.read().len())
            .field("revoked_count", &self.revoked.read().len())
            .field("event_count", &self.events.read().len())
            .field("issued_count", &self.issued_count.load(Ordering::Relaxed))
            .field("verified_count", &self.verified_count.load(Ordering::Relaxed))
            .field("failure_count", &self.failure_count.load(Ordering::Relaxed))
            .field("config", &self.config)
            .finish()
    }
}

impl AuthManager {
    /// Create a new `AuthManager` with the given configuration.
    #[must_use]
    pub fn new(config: AuthConfig) -> Self {
        Self {
            tokens: RwLock::new(HashMap::new()),
            revoked: RwLock::new(HashSet::new()),
            events: RwLock::new(Vec::new()),
            config,
            issued_count: AtomicU64::new(0),
            verified_count: AtomicU64::new(0),
            failure_count: AtomicU64::new(0),
            revoked_count: AtomicU64::new(0),
        }
    }

    /// Create a new `AuthManager` with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(AuthConfig::default())
    }

    /// Compute TTL in ticks for the given token type, based on config.
    const fn ttl_ticks(&self, token_type: TokenType) -> u64 {
        match token_type {
            TokenType::Service => self.config.service_ttl_hours * TICKS_PER_HOUR,
            TokenType::Agent => self.config.agent_ttl_hours * TICKS_PER_HOUR,
            TokenType::Human => {
                self.config.human_ttl_days * HOURS_PER_DAY * TICKS_PER_HOUR
            }
            TokenType::ApiKey => {
                self.config.api_key_ttl_days * HOURS_PER_DAY * TICKS_PER_HOUR
            }
        }
    }

    /// Compute a hex-encoded HMAC digest using `DefaultHasher`.
    ///
    /// **NOT cryptographically secure** — structural placeholder only.
    fn compute_digest(&self, payload: &str) -> String {
        let mut hasher = DefaultHasher::new();
        self.config.signing_key.hash(&mut hasher);
        payload.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// Encode bytes as a simple hex string (lowercase).
    fn to_hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;
        let mut hex = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            let _ = write!(hex, "{b:02x}");
        }
        hex
    }

    /// Build the raw token string: `hex(header).hex(payload).hex(digest)`.
    fn build_raw_token(token_id: &str, identity: &TokenIdentity, digest: &str) -> String {
        let header = format!("typ={},ver=1", identity.token_type);
        let payload = format!(
            "id={},tid={},tier={},scopes={}",
            identity.id,
            token_id,
            identity.tier,
            identity.scopes.join(";")
        );
        let header_hex = Self::to_hex(header.as_bytes());
        let payload_hex = Self::to_hex(payload.as_bytes());
        format!("{header_hex}.{payload_hex}.{digest}")
    }

    /// Extract the token ID from a raw token string.
    ///
    /// The token ID is encoded in the payload segment (second dot-delimited
    /// part) as a `tid=<uuid>` field.
    fn extract_token_id(raw: &str) -> Result<String> {
        let parts: Vec<&str> = raw.split('.').collect();
        if parts.len() != 3 {
            return Err(Error::Validation(
                "malformed token: expected 3 dot-separated segments".into(),
            ));
        }

        let payload_hex = parts[1];
        let payload_bytes = Self::from_hex(payload_hex)?;
        let payload = String::from_utf8(payload_bytes).map_err(|e| {
            Error::Validation(format!("invalid UTF-8 in token payload: {e}"))
        })?;

        for field in payload.split(',') {
            if let Some(tid) = field.strip_prefix("tid=") {
                return Ok(tid.to_owned());
            }
        }

        Err(Error::Validation(
            "malformed token: missing tid field in payload".into(),
        ))
    }

    /// Decode a hex string into bytes.
    fn from_hex(hex: &str) -> Result<Vec<u8>> {
        if !hex.len().is_multiple_of(2) {
            return Err(Error::Validation(
                "invalid hex: odd number of characters".into(),
            ));
        }

        let mut bytes = Vec::with_capacity(hex.len() / 2);
        let mut chars = hex.chars();

        while let Some(high) = chars.next() {
            let low = chars.next().ok_or_else(|| {
                Error::Validation("invalid hex: unexpected end of string".into())
            })?;

            let byte = u8::from_str_radix(&format!("{high}{low}"), 16).map_err(|e| {
                Error::Validation(format!("invalid hex digit: {e}"))
            })?;
            bytes.push(byte);
        }

        Ok(bytes)
    }

    /// Record a security event, evicting the oldest entry if the log is full.
    fn record_event(
        &self,
        event_type: SecurityEventType,
        source_id: &str,
        details: &str,
    ) {
        let event = SecurityEvent {
            id: Uuid::new_v4().to_string(),
            event_type,
            source_id: source_id.to_owned(),
            timestamp: Timestamp::now(),
            details: details.to_owned(),
        };

        let mut events = self.events.write();
        if events.len() >= self.config.max_events_log {
            events.remove(0);
        }
        events.push(event);
    }

    /// Issue a token without recording events (used internally by rotate).
    fn issue_token_inner(&self, identity: &TokenIdentity) -> Result<IssuedToken> {
        if identity.id.is_empty() {
            return Err(Error::Validation("identity ID must not be empty".into()));
        }

        let token_id = Uuid::new_v4().to_string();
        let issued_at = Timestamp::now();
        let ttl = self.ttl_ticks(identity.token_type);
        let expires_at = Timestamp::from_raw(issued_at.ticks() + ttl);

        let payload = format!(
            "id={},tid={},tier={},scopes={},iat={},exp={}",
            identity.id,
            token_id,
            identity.tier,
            identity.scopes.join(";"),
            issued_at.ticks(),
            expires_at.ticks()
        );

        let digest = self.compute_digest(&payload);
        let raw = Self::build_raw_token(&token_id, identity, &digest);

        let issued = IssuedToken {
            token_id: token_id.clone(),
            raw,
            identity: identity.clone(),
            issued_at,
            expires_at,
            hmac_digest: digest,
        };

        self.tokens.write().insert(token_id, issued.clone());
        self.issued_count.fetch_add(1, Ordering::Relaxed);

        Ok(issued)
    }
}

impl Authenticator for AuthManager {
    fn issue_token(&self, identity: &TokenIdentity) -> Result<IssuedToken> {
        let issued = self.issue_token_inner(identity)?;
        self.record_event(
            SecurityEventType::AuthSuccess,
            &issued.token_id,
            &format!("token issued for identity '{}' (type={})", identity.id, identity.token_type),
        );
        Ok(issued)
    }

    fn verify_token(&self, raw: &str) -> Result<VerifiedClaims> {
        if raw.is_empty() {
            self.failure_count.fetch_add(1, Ordering::Relaxed);
            self.record_event(
                SecurityEventType::AuthFailure,
                "unknown",
                "empty token string provided",
            );
            return Err(Error::Validation("raw token must not be empty".into()));
        }

        self.verified_count.fetch_add(1, Ordering::Relaxed);

        let token_id = Self::extract_token_id(raw)?;

        // Check revocation first.
        let is_revoked = self.revoked.read().contains(&token_id);
        if is_revoked {
            self.failure_count.fetch_add(1, Ordering::Relaxed);
            self.record_event(
                SecurityEventType::TokenRevoked,
                &token_id,
                "attempt to verify revoked token",
            );
            // Clone data out of the lock, then drop immediately.
            let snapshot = self.tokens.read().get(&token_id).map(|s| {
                (s.identity.clone(), s.issued_at, s.expires_at)
            });
            if let Some((identity, issued_at, expires_at)) = snapshot {
                return Ok(VerifiedClaims {
                    token_id,
                    identity,
                    issued_at,
                    expires_at,
                    valid: false,
                    revoked: true,
                });
            }
            return Err(Error::Validation(format!(
                "revoked token '{token_id}' not found in store"
            )));
        }

        // Look up the token — clone data out of the lock immediately.
        let snapshot = self.tokens.read().get(&token_id).map(|s| {
            (s.identity.clone(), s.issued_at, s.expires_at)
        });
        let (identity, issued_at, expires_at) = snapshot.ok_or_else(|| {
            self.failure_count.fetch_add(1, Ordering::Relaxed);
            Error::Validation(format!("token '{token_id}' not found"))
        })?;

        // Check expiry.
        let now = Timestamp::now();
        let expired = now.ticks() > expires_at.ticks();

        if expired {
            self.failure_count.fetch_add(1, Ordering::Relaxed);
            self.record_event(
                SecurityEventType::TokenExpired,
                &token_id,
                &format!(
                    "token expired: now={} > expires={}",
                    now.ticks(),
                    expires_at.ticks()
                ),
            );
            return Ok(VerifiedClaims {
                token_id,
                identity,
                issued_at,
                expires_at,
                valid: false,
                revoked: false,
            });
        }

        // Valid token.
        self.record_event(
            SecurityEventType::AuthSuccess,
            &token_id,
            &format!("token verified for identity '{}'", identity.id),
        );

        Ok(VerifiedClaims {
            token_id,
            identity,
            issued_at,
            expires_at,
            valid: true,
            revoked: false,
        })
    }

    fn revoke_token(&self, token_id: &str) -> Result<()> {
        if token_id.is_empty() {
            return Err(Error::Validation("token ID must not be empty".into()));
        }

        // Verify the token exists.
        let exists = self.tokens.read().contains_key(token_id);
        if !exists {
            return Err(Error::Validation(format!(
                "token '{token_id}' not found for revocation"
            )));
        }

        self.revoked.write().insert(token_id.to_owned());
        self.revoked_count.fetch_add(1, Ordering::Relaxed);

        self.record_event(
            SecurityEventType::TokenRevoked,
            token_id,
            "token explicitly revoked",
        );

        Ok(())
    }

    fn is_token_valid(&self, token_id: &str) -> bool {
        if token_id.is_empty() {
            return false;
        }

        if self.revoked.read().contains(token_id) {
            return false;
        }

        let expires_at = self.tokens.read().get(token_id).map(|s| s.expires_at);
        let Some(expires_at) = expires_at else {
            return false;
        };

        let now = Timestamp::now();
        now.ticks() <= expires_at.ticks()
    }

    fn rotate_token(&self, token_id: &str) -> Result<IssuedToken> {
        if token_id.is_empty() {
            return Err(Error::Validation("token ID must not be empty".into()));
        }

        // Step 1: read tokens to clone identity — lock is dropped inline.
        let identity = self.tokens.read().get(token_id).map(|s| s.identity.clone());
        let identity = identity.ok_or_else(|| {
            Error::Validation(format!(
                "token '{token_id}' not found for rotation"
            ))
        })?;

        // Step 2: revoke old token (acquires revoked write lock).
        self.revoke_token(token_id)?;

        // Step 3: issue new token with same identity.
        let new_token = self.issue_token_inner(&identity)?;

        self.record_event(
            SecurityEventType::AuthSuccess,
            &new_token.token_id,
            &format!(
                "token rotated: old={token_id} -> new={}",
                new_token.token_id
            ),
        );

        Ok(new_token)
    }

    fn security_events(&self) -> Vec<SecurityEvent> {
        self.events.read().clone()
    }

    fn audit_summary(&self) -> AuthAuditSummary {
        let active_tokens = {
            let tokens = self.tokens.read();
            let revoked = self.revoked.read();
            let now = Timestamp::now();
            tokens
                .values()
                .filter(|t| {
                    !revoked.contains(&t.token_id)
                        && now.ticks() <= t.expires_at.ticks()
                })
                .count()
        };

        AuthAuditSummary {
            total_issued: self.issued_count.load(Ordering::Relaxed),
            total_verified: self.verified_count.load(Ordering::Relaxed),
            total_revoked: self.revoked_count.load(Ordering::Relaxed),
            total_failures: self.failure_count.load(Ordering::Relaxed),
            active_tokens,
            event_count: self.events.read().len(),
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn service_identity(name: &str) -> TokenIdentity {
        TokenIdentity {
            id: name.to_owned(),
            token_type: TokenType::Service,
            tier: 1,
            scopes: vec!["read:health".to_owned(), "write:config".to_owned()],
        }
    }

    fn agent_identity(name: &str) -> TokenIdentity {
        TokenIdentity {
            id: name.to_owned(),
            token_type: TokenType::Agent,
            tier: 2,
            scopes: vec!["execute:task".to_owned()],
        }
    }

    fn human_identity(name: &str) -> TokenIdentity {
        TokenIdentity {
            id: name.to_owned(),
            token_type: TokenType::Human,
            tier: 0,
            scopes: vec!["admin".to_owned()],
        }
    }

    fn api_key_identity(name: &str) -> TokenIdentity {
        TokenIdentity {
            id: name.to_owned(),
            token_type: TokenType::ApiKey,
            tier: 3,
            scopes: vec!["read:metrics".to_owned()],
        }
    }

    fn make_manager() -> AuthManager {
        AuthManager::with_defaults()
    }

    fn make_manager_with_config(config: AuthConfig) -> AuthManager {
        AuthManager::new(config)
    }

    // -----------------------------------------------------------------------
    // Issue + Verify Roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn issue_and_verify_roundtrip() {
        let mgr = make_manager();
        let identity = service_identity("synthex");
        let issued = mgr.issue_token(&identity);
        assert!(issued.is_ok());
        let issued = issued.unwrap_or_else(|_| panic!("issue failed"));
        assert!(!issued.token_id.is_empty());
        assert!(!issued.raw.is_empty());
        assert!(!issued.hmac_digest.is_empty());

        let claims = mgr.verify_token(&issued.raw);
        assert!(claims.is_ok());
        let claims = claims.unwrap_or_else(|_| panic!("verify failed"));
        assert!(claims.valid);
        assert!(!claims.revoked);
        assert_eq!(claims.token_id, issued.token_id);
        assert_eq!(claims.identity.id, "synthex");
    }

    #[test]
    fn issue_token_returns_correct_identity() {
        let mgr = make_manager();
        let identity = agent_identity("ralph-agent");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        assert_eq!(issued.identity.id, "ralph-agent");
        assert_eq!(issued.identity.token_type, TokenType::Agent);
        assert_eq!(issued.identity.tier, 2);
        assert_eq!(issued.identity.scopes, vec!["execute:task"]);
    }

    #[test]
    fn issue_token_generates_unique_ids() {
        let mgr = make_manager();
        let identity = service_identity("devops-engine");
        let t1 = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue1 failed"));
        let t2 = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue2 failed"));
        assert_ne!(t1.token_id, t2.token_id);
        assert_ne!(t1.raw, t2.raw);
    }

    // -----------------------------------------------------------------------
    // Token Type TTL
    // -----------------------------------------------------------------------

    #[test]
    fn service_token_ttl() {
        let mgr = make_manager();
        let identity = service_identity("san-k7");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let expected_ttl = DEFAULT_SERVICE_TTL_HOURS * TICKS_PER_HOUR;
        let actual_ttl = issued.expires_at.ticks() - issued.issued_at.ticks();
        assert_eq!(actual_ttl, expected_ttl);
    }

    #[test]
    fn agent_token_ttl() {
        let mgr = make_manager();
        let identity = agent_identity("validator-01");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let expected_ttl = DEFAULT_AGENT_TTL_HOURS * TICKS_PER_HOUR;
        let actual_ttl = issued.expires_at.ticks() - issued.issued_at.ticks();
        assert_eq!(actual_ttl, expected_ttl);
    }

    #[test]
    fn human_token_ttl() {
        let mgr = make_manager();
        let identity = human_identity("luke");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let expected_ttl = DEFAULT_HUMAN_TTL_DAYS * HOURS_PER_DAY * TICKS_PER_HOUR;
        let actual_ttl = issued.expires_at.ticks() - issued.issued_at.ticks();
        assert_eq!(actual_ttl, expected_ttl);
    }

    #[test]
    fn api_key_token_ttl() {
        let mgr = make_manager();
        let identity = api_key_identity("ci-runner");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let expected_ttl = DEFAULT_API_KEY_TTL_DAYS * HOURS_PER_DAY * TICKS_PER_HOUR;
        let actual_ttl = issued.expires_at.ticks() - issued.issued_at.ticks();
        assert_eq!(actual_ttl, expected_ttl);
    }

    // -----------------------------------------------------------------------
    // Verify: Expired
    // -----------------------------------------------------------------------

    #[test]
    fn verify_expired_token() {
        let config = AuthConfig {
            agent_ttl_hours: 0, // Immediate expiry: 0 ticks TTL
            ..AuthConfig::default()
        };
        let mgr = make_manager_with_config(config);
        let identity = agent_identity("short-lived");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));

        // Advance the global tick to simulate time passing.
        let _ = Timestamp::now(); // burn a tick

        let claims = mgr.verify_token(&issued.raw).unwrap_or_else(|_| panic!("verify failed"));
        assert!(!claims.valid);
        assert!(!claims.revoked);
    }

    // -----------------------------------------------------------------------
    // Verify: Revoked
    // -----------------------------------------------------------------------

    #[test]
    fn verify_revoked_token() {
        let mgr = make_manager();
        let identity = service_identity("nais");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));

        let revoke_result = mgr.revoke_token(&issued.token_id);
        assert!(revoke_result.is_ok());

        let claims = mgr.verify_token(&issued.raw).unwrap_or_else(|_| panic!("verify failed"));
        assert!(!claims.valid);
        assert!(claims.revoked);
    }

    // -----------------------------------------------------------------------
    // Revoke
    // -----------------------------------------------------------------------

    #[test]
    fn revoke_token_prevents_validity() {
        let mgr = make_manager();
        let identity = service_identity("bash-engine");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));

        assert!(mgr.is_token_valid(&issued.token_id));
        let _ = mgr.revoke_token(&issued.token_id);
        assert!(!mgr.is_token_valid(&issued.token_id));
    }

    #[test]
    fn revoke_empty_id_fails() {
        let mgr = make_manager();
        let result = mgr.revoke_token("");
        assert!(result.is_err());
    }

    #[test]
    fn revoke_nonexistent_token_fails() {
        let mgr = make_manager();
        let result = mgr.revoke_token("nonexistent-uuid");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Rotate
    // -----------------------------------------------------------------------

    #[test]
    fn rotate_token_issues_new_and_revokes_old() {
        let mgr = make_manager();
        let identity = service_identity("codesynthor-v7");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let old_id = issued.token_id.clone();

        let rotated = mgr.rotate_token(&old_id).unwrap_or_else(|_| panic!("rotate failed"));

        // Old token should be revoked.
        assert!(!mgr.is_token_valid(&old_id));
        // New token should be valid.
        assert!(mgr.is_token_valid(&rotated.token_id));
        // Identity preserved.
        assert_eq!(rotated.identity.id, "codesynthor-v7");
        assert_eq!(rotated.identity.token_type, TokenType::Service);
    }

    #[test]
    fn rotate_empty_id_fails() {
        let mgr = make_manager();
        let result = mgr.rotate_token("");
        assert!(result.is_err());
    }

    #[test]
    fn rotate_nonexistent_token_fails() {
        let mgr = make_manager();
        let result = mgr.rotate_token("nonexistent-uuid");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Security Events
    // -----------------------------------------------------------------------

    #[test]
    fn security_events_logged_on_issue() {
        let mgr = make_manager();
        let identity = service_identity("synthex");
        let _ = mgr.issue_token(&identity);
        let events = mgr.security_events();
        assert!(!events.is_empty());
        assert_eq!(events[0].event_type, SecurityEventType::AuthSuccess);
    }

    #[test]
    fn security_events_logged_on_verify() {
        let mgr = make_manager();
        let identity = service_identity("devops-engine");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let _ = mgr.verify_token(&issued.raw);
        let events = mgr.security_events();
        // At least one AuthSuccess for issue, one for verify
        let auth_success_count = events
            .iter()
            .filter(|e| e.event_type == SecurityEventType::AuthSuccess)
            .count();
        assert!(auth_success_count >= 2);
    }

    #[test]
    fn security_events_logged_on_revoke() {
        let mgr = make_manager();
        let identity = service_identity("san-k7");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let _ = mgr.revoke_token(&issued.token_id);
        let events = mgr.security_events();
        let revoke_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == SecurityEventType::TokenRevoked)
            .collect();
        assert!(!revoke_events.is_empty());
    }

    #[test]
    fn security_events_logged_on_failed_verify_empty() {
        let mgr = make_manager();
        let _ = mgr.verify_token("");
        let events = mgr.security_events();
        let failure_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == SecurityEventType::AuthFailure)
            .collect();
        assert!(!failure_events.is_empty());
    }

    #[test]
    fn security_events_logged_on_expired_verify() {
        let config = AuthConfig {
            service_ttl_hours: 0,
            ..AuthConfig::default()
        };
        let mgr = make_manager_with_config(config);
        let identity = service_identity("test");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let _ = Timestamp::now(); // advance tick
        let _ = mgr.verify_token(&issued.raw);
        let events = mgr.security_events();
        let expired_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == SecurityEventType::TokenExpired)
            .collect();
        assert!(!expired_events.is_empty());
    }

    // -----------------------------------------------------------------------
    // Events Cap
    // -----------------------------------------------------------------------

    #[test]
    fn events_capped_at_max() {
        let config = AuthConfig {
            max_events_log: 5,
            ..AuthConfig::default()
        };
        let mgr = make_manager_with_config(config);
        let identity = service_identity("cap-test");

        // Issue 10 tokens (each generates 1 event).
        for _ in 0..10 {
            let _ = mgr.issue_token(&identity);
        }

        let events = mgr.security_events();
        assert!(events.len() <= 5);
    }

    #[test]
    fn events_cap_evicts_oldest() {
        let config = AuthConfig {
            max_events_log: 3,
            ..AuthConfig::default()
        };
        let mgr = make_manager_with_config(config);
        let identity = service_identity("eviction-test");

        let _ = mgr.issue_token(&identity);
        let _ = mgr.issue_token(&identity);
        let _ = mgr.issue_token(&identity);
        let _ = mgr.issue_token(&identity);

        let events = mgr.security_events();
        assert_eq!(events.len(), 3);
        // The first event should have been evicted; remaining events are
        // the 2nd, 3rd, and 4th issuances.
    }

    // -----------------------------------------------------------------------
    // Audit Summary
    // -----------------------------------------------------------------------

    #[test]
    fn audit_summary_reflects_operations() {
        let mgr = make_manager();
        let identity = service_identity("audit-test");

        let t1 = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let t2 = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let _ = mgr.verify_token(&t1.raw);
        let _ = mgr.revoke_token(&t2.token_id);

        let summary = mgr.audit_summary();
        assert_eq!(summary.total_issued, 2);
        assert_eq!(summary.total_verified, 1);
        assert_eq!(summary.total_revoked, 1);
        assert_eq!(summary.active_tokens, 1);
        assert!(summary.event_count > 0);
    }

    #[test]
    fn audit_summary_counts_failures() {
        let mgr = make_manager();
        let _ = mgr.verify_token(""); // empty token
        let summary = mgr.audit_summary();
        assert_eq!(summary.total_failures, 1);
    }

    #[test]
    fn audit_summary_default_is_zero() {
        let mgr = make_manager();
        let summary = mgr.audit_summary();
        assert_eq!(summary.total_issued, 0);
        assert_eq!(summary.total_verified, 0);
        assert_eq!(summary.total_revoked, 0);
        assert_eq!(summary.total_failures, 0);
        assert_eq!(summary.active_tokens, 0);
        assert_eq!(summary.event_count, 0);
    }

    // -----------------------------------------------------------------------
    // Validation: Empty Identity
    // -----------------------------------------------------------------------

    #[test]
    fn issue_empty_identity_id_fails() {
        let mgr = make_manager();
        let identity = TokenIdentity {
            id: String::new(),
            token_type: TokenType::Service,
            tier: 1,
            scopes: vec![],
        };
        let result = mgr.issue_token(&identity);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Validation: Empty Raw Token
    // -----------------------------------------------------------------------

    #[test]
    fn verify_empty_raw_fails() {
        let mgr = make_manager();
        let result = mgr.verify_token("");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Token Format
    // -----------------------------------------------------------------------

    #[test]
    fn raw_token_has_three_segments() {
        let mgr = make_manager();
        let identity = service_identity("format-test");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let segments: Vec<&str> = issued.raw.split('.').collect();
        assert_eq!(segments.len(), 3);
    }

    #[test]
    fn raw_token_segments_are_valid_hex() {
        let mgr = make_manager();
        let identity = service_identity("hex-test");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        for segment in issued.raw.split('.') {
            assert!(
                segment.len() % 2 == 0,
                "segment length must be even (hex pairs)"
            );
            assert!(
                segment.chars().all(|c| c.is_ascii_hexdigit()),
                "segment must contain only hex digits"
            );
        }
    }

    // -----------------------------------------------------------------------
    // HMAC Digest
    // -----------------------------------------------------------------------

    #[test]
    fn different_payloads_produce_different_digests() {
        let mgr = make_manager();
        let d1 = mgr.compute_digest("payload-a");
        let d2 = mgr.compute_digest("payload-b");
        assert_ne!(d1, d2);
    }

    #[test]
    fn same_payload_produces_same_digest() {
        let mgr = make_manager();
        let d1 = mgr.compute_digest("identical-payload");
        let d2 = mgr.compute_digest("identical-payload");
        assert_eq!(d1, d2);
    }

    #[test]
    fn different_signing_keys_produce_different_digests() {
        let mgr1 = AuthManager::new(AuthConfig {
            signing_key: "key-alpha".to_owned(),
            ..AuthConfig::default()
        });
        let mgr2 = AuthManager::new(AuthConfig {
            signing_key: "key-beta".to_owned(),
            ..AuthConfig::default()
        });
        let d1 = mgr1.compute_digest("same-payload");
        let d2 = mgr2.compute_digest("same-payload");
        assert_ne!(d1, d2);
    }

    // -----------------------------------------------------------------------
    // is_token_valid
    // -----------------------------------------------------------------------

    #[test]
    fn is_token_valid_returns_true_for_active() {
        let mgr = make_manager();
        let identity = service_identity("active-check");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        assert!(mgr.is_token_valid(&issued.token_id));
    }

    #[test]
    fn is_token_valid_returns_false_for_revoked() {
        let mgr = make_manager();
        let identity = service_identity("revoked-check");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let _ = mgr.revoke_token(&issued.token_id);
        assert!(!mgr.is_token_valid(&issued.token_id));
    }

    #[test]
    fn is_token_valid_returns_false_for_empty_id() {
        let mgr = make_manager();
        assert!(!mgr.is_token_valid(""));
    }

    #[test]
    fn is_token_valid_returns_false_for_nonexistent() {
        let mgr = make_manager();
        assert!(!mgr.is_token_valid("does-not-exist"));
    }

    // -----------------------------------------------------------------------
    // Config Customization
    // -----------------------------------------------------------------------

    #[test]
    fn custom_config_ttl_is_respected() {
        let config = AuthConfig {
            service_ttl_hours: 48,
            agent_ttl_hours: 2,
            human_ttl_days: 14,
            api_key_ttl_days: 180,
            ..AuthConfig::default()
        };
        let mgr = make_manager_with_config(config);

        let s = mgr
            .issue_token(&service_identity("s"))
            .unwrap_or_else(|_| panic!("issue failed"));
        assert_eq!(
            s.expires_at.ticks() - s.issued_at.ticks(),
            48 * TICKS_PER_HOUR
        );

        let a = mgr
            .issue_token(&agent_identity("a"))
            .unwrap_or_else(|_| panic!("issue failed"));
        assert_eq!(
            a.expires_at.ticks() - a.issued_at.ticks(),
            2 * TICKS_PER_HOUR
        );

        let h = mgr
            .issue_token(&human_identity("h"))
            .unwrap_or_else(|_| panic!("issue failed"));
        assert_eq!(
            h.expires_at.ticks() - h.issued_at.ticks(),
            14 * HOURS_PER_DAY * TICKS_PER_HOUR
        );

        let k = mgr
            .issue_token(&api_key_identity("k"))
            .unwrap_or_else(|_| panic!("issue failed"));
        assert_eq!(
            k.expires_at.ticks() - k.issued_at.ticks(),
            180 * HOURS_PER_DAY * TICKS_PER_HOUR
        );
    }

    // -----------------------------------------------------------------------
    // Debug Impl
    // -----------------------------------------------------------------------

    #[test]
    fn debug_impl_does_not_leak_tokens() {
        let mgr = make_manager();
        let identity = service_identity("debug-test");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let debug_output = format!("{mgr:?}");
        // The raw token string must NOT appear in debug output.
        assert!(!debug_output.contains(&issued.raw));
        assert!(debug_output.contains("AuthManager"));
    }

    // -----------------------------------------------------------------------
    // Concurrent Issue + Verify
    // -----------------------------------------------------------------------

    #[test]
    fn concurrent_issue_and_verify() {
        use std::sync::Arc;
        use std::thread;

        let mgr = Arc::new(make_manager());
        let mut handles = Vec::new();

        // Spawn 10 threads that each issue and verify a token.
        for i in 0..10 {
            let mgr_clone = Arc::clone(&mgr);
            handles.push(thread::spawn(move || {
                let identity = service_identity(&format!("concurrent-{i}"));
                let issued = mgr_clone
                    .issue_token(&identity)
                    .unwrap_or_else(|_| panic!("issue failed in thread {i}"));
                let claims = mgr_clone
                    .verify_token(&issued.raw)
                    .unwrap_or_else(|_| panic!("verify failed in thread {i}"));
                assert!(claims.valid);
                assert_eq!(claims.identity.id, format!("concurrent-{i}"));
            }));
        }

        for handle in handles {
            handle
                .join()
                .unwrap_or_else(|_| panic!("thread panicked"));
        }

        let summary = mgr.audit_summary();
        assert_eq!(summary.total_issued, 10);
        assert_eq!(summary.total_verified, 10);
    }

    // -----------------------------------------------------------------------
    // Atomic Counter Consistency
    // -----------------------------------------------------------------------

    #[test]
    fn atomic_counters_increment_correctly() {
        let mgr = make_manager();
        let identity = service_identity("counter-test");

        for _ in 0..5 {
            let _ = mgr.issue_token(&identity);
        }
        assert_eq!(mgr.issued_count.load(Ordering::Relaxed), 5);
        assert_eq!(mgr.verified_count.load(Ordering::Relaxed), 0);
        assert_eq!(mgr.failure_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn failure_counter_increments_on_bad_verify() {
        let mgr = make_manager();
        let _ = mgr.verify_token("");
        let _ = mgr.verify_token("");
        assert_eq!(mgr.failure_count.load(Ordering::Relaxed), 2);
    }

    // -----------------------------------------------------------------------
    // Multiple Token Types
    // -----------------------------------------------------------------------

    #[test]
    fn all_token_types_issue_and_verify() {
        let mgr = make_manager();

        let s = mgr.issue_token(&service_identity("svc")).unwrap_or_else(|_| panic!("service failed"));
        let a = mgr.issue_token(&agent_identity("agt")).unwrap_or_else(|_| panic!("agent failed"));
        let h = mgr.issue_token(&human_identity("hmn")).unwrap_or_else(|_| panic!("human failed"));
        let k = mgr.issue_token(&api_key_identity("key")).unwrap_or_else(|_| panic!("apikey failed"));

        for token in [&s, &a, &h, &k] {
            let claims = mgr.verify_token(&token.raw).unwrap_or_else(|_| panic!("verify failed"));
            assert!(claims.valid);
        }
    }

    // -----------------------------------------------------------------------
    // Token Type Display
    // -----------------------------------------------------------------------

    #[test]
    fn token_type_display() {
        assert_eq!(format!("{}", TokenType::Service), "Service");
        assert_eq!(format!("{}", TokenType::Agent), "Agent");
        assert_eq!(format!("{}", TokenType::Human), "Human");
        assert_eq!(format!("{}", TokenType::ApiKey), "ApiKey");
    }

    // -----------------------------------------------------------------------
    // Security Event Type Display
    // -----------------------------------------------------------------------

    #[test]
    fn security_event_type_display() {
        assert_eq!(format!("{}", SecurityEventType::AuthSuccess), "AuthSuccess");
        assert_eq!(format!("{}", SecurityEventType::AuthFailure), "AuthFailure");
        assert_eq!(format!("{}", SecurityEventType::TokenExpired), "TokenExpired");
        assert_eq!(format!("{}", SecurityEventType::TokenRevoked), "TokenRevoked");
        assert_eq!(format!("{}", SecurityEventType::ScopeViolation), "ScopeViolation");
        assert_eq!(format!("{}", SecurityEventType::RateLimited), "RateLimited");
    }

    // -----------------------------------------------------------------------
    // Hex Encoding / Decoding
    // -----------------------------------------------------------------------

    #[test]
    fn hex_roundtrip() {
        let input = b"hello world";
        let hex = AuthManager::to_hex(input);
        let decoded = AuthManager::from_hex(&hex).unwrap_or_else(|_| panic!("decode failed"));
        assert_eq!(decoded, input);
    }

    #[test]
    fn from_hex_invalid_odd_length() {
        let result = AuthManager::from_hex("abc");
        assert!(result.is_err());
    }

    #[test]
    fn from_hex_invalid_characters() {
        let result = AuthManager::from_hex("zzzz");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Token Extraction
    // -----------------------------------------------------------------------

    #[test]
    fn extract_token_id_from_issued() {
        let mgr = make_manager();
        let identity = service_identity("extract-test");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let extracted = AuthManager::extract_token_id(&issued.raw)
            .unwrap_or_else(|_| panic!("extract failed"));
        assert_eq!(extracted, issued.token_id);
    }

    #[test]
    fn extract_token_id_malformed_no_dots() {
        let result = AuthManager::extract_token_id("nodots");
        assert!(result.is_err());
    }

    #[test]
    fn extract_token_id_malformed_two_segments() {
        let result = AuthManager::extract_token_id("aa.bb");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Rotate Preserves Scopes and Tier
    // -----------------------------------------------------------------------

    #[test]
    fn rotate_preserves_scopes_and_tier() {
        let mgr = make_manager();
        let identity = TokenIdentity {
            id: "scope-test".to_owned(),
            token_type: TokenType::Human,
            tier: 0,
            scopes: vec!["admin".to_owned(), "write:all".to_owned()],
        };
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let rotated = mgr.rotate_token(&issued.token_id)
            .unwrap_or_else(|_| panic!("rotate failed"));

        assert_eq!(rotated.identity.tier, 0);
        assert_eq!(rotated.identity.scopes, vec!["admin", "write:all"]);
        assert_eq!(rotated.identity.token_type, TokenType::Human);
    }

    // -----------------------------------------------------------------------
    // Multiple Revocations
    // -----------------------------------------------------------------------

    #[test]
    fn double_revoke_second_is_noop() {
        let mgr = make_manager();
        let identity = service_identity("double-revoke");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let r1 = mgr.revoke_token(&issued.token_id);
        assert!(r1.is_ok());
        // Second revocation should still succeed (idempotent in revoked set,
        // but token still exists in tokens map).
        let r2 = mgr.revoke_token(&issued.token_id);
        assert!(r2.is_ok());
    }

    // -----------------------------------------------------------------------
    // Default Config
    // -----------------------------------------------------------------------

    #[test]
    fn default_config_has_expected_values() {
        let config = AuthConfig::default();
        assert_eq!(config.signing_key, "ultraplate-me-v2-key");
        assert_eq!(config.service_ttl_hours, 24);
        assert_eq!(config.agent_ttl_hours, 1);
        assert_eq!(config.human_ttl_days, 7);
        assert_eq!(config.api_key_ttl_days, 90);
        assert_eq!(config.max_events_log, 500);
    }

    // -----------------------------------------------------------------------
    // With Defaults Constructor
    // -----------------------------------------------------------------------

    #[test]
    fn with_defaults_creates_functional_manager() {
        let mgr = AuthManager::with_defaults();
        let identity = service_identity("default-mgr");
        let issued = mgr.issue_token(&identity);
        assert!(issued.is_ok());
    }

    // -----------------------------------------------------------------------
    // Verify Malformed Token
    // -----------------------------------------------------------------------

    #[test]
    fn verify_malformed_token_fails() {
        let mgr = make_manager();
        let result = mgr.verify_token("not.a.valid-hex");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Concurrent Revocation
    // -----------------------------------------------------------------------

    #[test]
    fn concurrent_revocation() {
        use std::sync::Arc;
        use std::thread;

        let mgr = Arc::new(make_manager());
        let mut tokens = Vec::new();

        // Issue 10 tokens.
        for i in 0..10 {
            let identity = service_identity(&format!("conc-revoke-{i}"));
            let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
            tokens.push(issued.token_id);
        }

        // Revoke all concurrently.
        let mut handles = Vec::new();
        for token_id in tokens {
            let mgr_clone = Arc::clone(&mgr);
            handles.push(thread::spawn(move || {
                let _ = mgr_clone.revoke_token(&token_id);
            }));
        }

        for handle in handles {
            handle.join().unwrap_or_else(|_| panic!("thread panicked"));
        }

        assert_eq!(mgr.audit_summary().active_tokens, 0);
    }

    // -----------------------------------------------------------------------
    // Verify After Rotate Returns Invalid For Old Token
    // -----------------------------------------------------------------------

    #[test]
    fn verify_old_token_after_rotate_shows_revoked() {
        let mgr = make_manager();
        let identity = service_identity("rotate-verify");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let _ = mgr.rotate_token(&issued.token_id);

        let claims = mgr.verify_token(&issued.raw).unwrap_or_else(|_| panic!("verify failed"));
        assert!(!claims.valid);
        assert!(claims.revoked);
    }

    // -----------------------------------------------------------------------
    // Audit Summary After Rotate
    // -----------------------------------------------------------------------

    #[test]
    fn audit_summary_after_rotate() {
        let mgr = make_manager();
        let identity = service_identity("rotate-audit");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let _ = mgr.rotate_token(&issued.token_id);

        let summary = mgr.audit_summary();
        // 1 original + 1 rotated = 2 issued
        assert_eq!(summary.total_issued, 2);
        // 1 revocation from rotation
        assert_eq!(summary.total_revoked, 1);
        // Only the new token is active
        assert_eq!(summary.active_tokens, 1);
    }

    // -----------------------------------------------------------------------
    // Security Events Have Unique IDs
    // -----------------------------------------------------------------------

    #[test]
    fn security_events_have_unique_ids() {
        let mgr = make_manager();
        let identity = service_identity("unique-events");
        for _ in 0..5 {
            let _ = mgr.issue_token(&identity);
        }
        let events = mgr.security_events();
        let ids: HashSet<&str> = events.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids.len(), events.len());
    }

    // -----------------------------------------------------------------------
    // Security Events Contain Source ID
    // -----------------------------------------------------------------------

    #[test]
    fn security_events_contain_source_id() {
        let mgr = make_manager();
        let identity = service_identity("source-id-test");
        let issued = mgr.issue_token(&identity).unwrap_or_else(|_| panic!("issue failed"));
        let events = mgr.security_events();
        assert!(events.iter().any(|e| e.source_id == issued.token_id));
    }

    // -----------------------------------------------------------------------
    // Large Batch Issue
    // -----------------------------------------------------------------------

    #[test]
    fn large_batch_issue_100_tokens() {
        let mgr = make_manager();
        for i in 0..100 {
            let identity = service_identity(&format!("batch-{i}"));
            let result = mgr.issue_token(&identity);
            assert!(result.is_ok());
        }
        let summary = mgr.audit_summary();
        assert_eq!(summary.total_issued, 100);
        assert_eq!(summary.active_tokens, 100);
    }
}
