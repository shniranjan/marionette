use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::{HashMap, HashSet};

type HmacSha256 = Hmac<Sha256>;

/// Per-connection session manager.
///
/// Created fresh for every WebSocket connection.  Tracks registered sessions,
/// their HMAC keys, and used nonces to prevent replay attacks.
pub struct SessionManager {
    sessions: HashMap<String, Session>,
}

struct Session {
    #[allow(dead_code)]
    endpoint_id: String,
    session_key: Vec<u8>,
    used_nonces: HashSet<String>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Create a new session, returning the random 32-byte session key.
    pub fn create_session(&mut self, session_id: String, endpoint_id: String) -> Vec<u8> {
        use rand::RngCore;

        let mut key = vec![0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);

        self.sessions.insert(
            session_id.clone(),
            Session {
                endpoint_id,
                session_key: key.clone(),
                used_nonces: HashSet::new(),
            },
        );

        key
    }

    /// Look up a session's HMAC key.
    pub fn get_key(&self, session_id: &str) -> Option<&Vec<u8>> {
        self.sessions.get(session_id).map(|s| &s.session_key)
    }

    /// Check and record a nonce.  Returns `false` if the nonce has been
    /// seen before (replay detected).
    pub fn check_nonce(&mut self, session_id: &str, nonce: &str) -> bool {
        if let Some(session) = self.sessions.get_mut(session_id) {
            if session.used_nonces.contains(nonce) {
                return false; // replay detected
            }
            // Simple rotation: clear if too large
            if session.used_nonces.len() > 10_000 {
                session.used_nonces.clear();
            }
            session.used_nonces.insert(nonce.to_string());
            true
        } else {
            false
        }
    }

    /// HMAC-SHA256 sign a message with the session key.
    pub fn sign(&self, session_id: &str, message: &[u8]) -> Option<Vec<u8>> {
        let key = self.get_key(session_id)?;
        let mut mac = HmacSha256::new_from_slice(key).ok()?;
        mac.update(message);
        Some(mac.finalize().into_bytes().to_vec())
    }

    /// Verify an HMAC-SHA256 signature in constant time.
    pub fn verify(&self, session_id: &str, message: &[u8], signature: &[u8]) -> bool {
        if let Some(expected) = self.sign(session_id, message) {
            if expected.len() != signature.len() {
                return false;
            }
            // Constant-time comparison
            expected
                .iter()
                .zip(signature.iter())
                .fold(0, |acc, (a, b)| acc | (a ^ b))
                == 0
        } else {
            false
        }
    }
}
