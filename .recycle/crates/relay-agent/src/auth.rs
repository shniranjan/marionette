use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub struct AuthState {
    pub session_key: Option<Vec<u8>>,
    pub session_id: Option<String>,
}

impl AuthState {
    pub fn new() -> Self {
        Self {
            session_key: None,
            session_id: None,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.session_key.is_some()
    }

    pub fn sign(&self, message: &[u8]) -> Option<Vec<u8>> {
        let key = self.session_key.as_ref()?;
        let mut mac = HmacSha256::new_from_slice(key).ok()?;
        mac.update(message);
        Some(mac.finalize().into_bytes().to_vec())
    }

    pub fn verify(&self, message: &[u8], signature: &[u8]) -> bool {
        if let Some(expected) = self.sign(message) {
            // Constant-time comparison to prevent timing attacks
            if expected.len() != signature.len() {
                return false;
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_roundtrip() {
        let state = AuthState {
            session_key: Some(vec![0x42u8; 32]),
            session_id: None,
        };
        let msg = b"test message";
        let sig = state.sign(msg).unwrap();
        assert!(sig.len() == 32);
        assert!(state.verify(msg, &sig));
    }

    #[test]
    fn tampered_message_rejected() {
        let state = AuthState {
            session_key: Some(vec![0x42u8; 32]),
            session_id: None,
        };
        let msg = b"test message";
        let sig = state.sign(msg).unwrap();
        assert!(!state.verify(b"tampered message", &sig));
    }

    #[test]
    fn tampered_signature_rejected() {
        let state = AuthState {
            session_key: Some(vec![0x42u8; 32]),
            session_id: None,
        };
        let msg = b"test message";
        let mut sig = state.sign(msg).unwrap();
        sig[0] ^= 1;
        assert!(!state.verify(msg, &sig));
    }

    #[test]
    fn wrong_key_rejected() {
        let state_a = AuthState {
            session_key: Some(vec![0x42u8; 32]),
            session_id: None,
        };
        let state_b = AuthState {
            session_key: Some(vec![0x99u8; 32]),
            session_id: None,
        };
        let msg = b"test message";
        let sig = state_a.sign(msg).unwrap();
        assert!(!state_b.verify(msg, &sig));
    }

    #[test]
    fn unauthenticated_state_cannot_sign() {
        let state = AuthState::new();
        assert!(state.sign(b"msg").is_none());
        assert!(!state.is_authenticated());
    }
}
