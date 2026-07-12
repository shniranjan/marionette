//! HMAC session management for relay-agent.
//!
//! Provides session lifecycle (creation, expiry checks), registration payloads,
//! and HMAC-SHA256 signing/verification primitives used to authenticate every
//! message after session establishment.

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

// ── Session ──────────────────────────────────────────────────────────

/// An authenticated session between the relay agent and the Marionette controller.
///
/// After registration, the controller issues a session key used to sign every
/// subsequent message. Sessions have a bounded lifetime and must be refreshed
/// before expiry.
#[derive(Debug, Clone)]
pub struct Session {
    /// Unique session identifier (UUID v4 string).
    pub session_id: String,
    /// Shared secret for HMAC-SHA256 message authentication.
    pub session_key: String,
    /// When this session was created (UTC).
    pub created_at: DateTime<Utc>,
}

impl Session {
    /// Create a new session with the given id and shared key.
    pub fn new(session_id: String, session_key: String) -> Self {
        Self {
            session_id,
            session_key,
            created_at: Utc::now(),
        }
    }

    /// Returns `true` if the session is older than `ttl_secs`.
    pub fn is_expired(&self, ttl_secs: u64) -> bool {
        let age = Utc::now()
            .signed_duration_since(self.created_at)
            .num_seconds();
        age < 0 || age as u64 > ttl_secs
    }
}

// ── Registration ──────────────────────────────────────────────────────

/// Payload sent by the relay agent during the initial registration handshake.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationPayload {
    /// Shared registration token (pre-shared secret).
    pub token: String,
    /// Hostname of the machine running this relay agent.
    pub hostname: String,
    /// CPU architecture (e.g. "aarch64", "x86_64").
    pub arch: String,
    /// Operating system name (e.g. "linux").
    pub os: String,
    /// Relay agent version string.
    pub relay_version: String,
}

// ── HMAC primitives ───────────────────────────────────────────────────

/// Compute an HMAC-SHA256 signature over `message` using `key`.
///
/// Returns the raw 32-byte authentication tag.
pub fn sign_message(key: &[u8], message: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key)
        .expect("HMAC-SHA256 accepts keys of any size");
    mac.update(message);
    mac.finalize().into_bytes().to_vec()
}

/// Verify an HMAC-SHA256 `signature` over `message` using `key`.
///
/// Uses constant-time comparison to resist timing side-channels.
pub fn verify_signature(key: &[u8], message: &[u8], signature: &[u8]) -> bool {
    let expected = sign_message(key, message);
    if expected.len() != signature.len() {
        return false;
    }
    // Constant-time byte comparison
    expected
        .iter()
        .zip(signature.iter())
        .fold(0, |acc, (a, b)| acc | (a ^ b))
        == 0
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Session tests ────────────────────────────────────────

    #[test]
    fn session_is_not_expired_when_fresh() {
        let s = Session::new("id-1".into(), "key-1".into());
        assert!(!s.is_expired(300));
    }

    #[test]
    fn session_is_not_expired_within_ttl() {
        let s = Session::new("id-1".into(), "key-1".into());
        // session is brand new, 3600 sec TTL is far in the future
        assert!(!s.is_expired(3600));
    }

    #[test]
    fn session_expires_after_ttl() {
        let s = Session::new("id-1".into(), "key-1".into());
        assert!(s.is_expired(0)); // 0 sec TTL = instantly expired (allow 1s slop)
    }

    // ── HMAC tests ───────────────────────────────────────────

    #[test]
    fn sign_and_verify_roundtrip() {
        let key = vec![0x42u8; 32];
        let msg = b"test message";
        let sig = sign_message(&key, msg);
        assert_eq!(sig.len(), 32);
        assert!(verify_signature(&key, msg, &sig));
    }

    #[test]
    fn tampered_message_rejected() {
        let key = vec![0x42u8; 32];
        let msg = b"test message";
        let sig = sign_message(&key, msg);
        assert!(!verify_signature(&key, b"tampered message", &sig));
    }

    #[test]
    fn tampered_signature_rejected() {
        let key = vec![0x42u8; 32];
        let msg = b"test message";
        let mut sig = sign_message(&key, msg);
        sig[0] ^= 1;
        assert!(!verify_signature(&key, msg, &sig));
    }

    #[test]
    fn wrong_key_rejected() {
        let key_a = vec![0x42u8; 32];
        let key_b = vec![0x99u8; 32];
        let msg = b"test message";
        let sig = sign_message(&key_a, msg);
        assert!(!verify_signature(&key_b, msg, &sig));
    }

    #[test]
    fn empty_message_supported() {
        let key = vec![0x7fu8; 32];
        let msg = b"";
        let sig = sign_message(&key, msg);
        assert_eq!(sig.len(), 32);
        assert!(verify_signature(&key, msg, &sig));
    }
}
