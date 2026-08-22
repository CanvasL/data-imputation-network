//! Append-only event envelope per PROTOCOL.md §2.
//!
//! Events are the source of truth; all state (lineage, payments, imputed
//! values) is a projection of the event stream. Signatures are real
//! Ed25519 (ed25519-dalek), and `prev_hash` chains events against
//! reordering.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

use crate::model::{AgentId, hash_bytes};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    Contribute,
    Subscribe,
    Payment,
    Attribute,
    Challenge,
    Resolve,
}

/// Ed25519 signing identity bound to an `AgentId`.
#[derive(Clone)]
pub struct Keypair {
    pub agent: AgentId,
    signing: SigningKey,
}

impl Keypair {
    pub fn random(agent: AgentId) -> Self {
        let mut csprng = OsRng;
        Self {
            agent,
            signing: SigningKey::generate(&mut csprng),
        }
    }

    /// Deterministic keys for reproducible tests.
    pub fn from_seed(agent: AgentId, seed: [u8; 32]) -> Self {
        Self {
            agent,
            signing: SigningKey::from_bytes(&seed),
        }
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing.verifying_key()
    }

    /// Hex-encoded public key, for storage in a registry.
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.verifying_key().to_bytes())
    }

    /// Hex-encoded private key (dev convenience / key export).
    pub fn secret_key_hex(&self) -> String {
        hex::encode(self.signing.to_bytes())
    }

    pub fn sign(&self, msg: &[u8]) -> Signature {
        self.signing.sign(msg)
    }
}

/// Event envelope with hash-chaining (`prev_hash`) for anti-reordering.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Envelope {
    pub event_id: u64,
    pub event_type: EventType,
    pub authored_by: AgentId,
    pub payload: String,
    pub payload_hash: String,
    pub prev_hash: String,
    /// Hex-encoded Ed25519 signature over `signature_input()`.
    pub sig: String,
}

impl Envelope {
    pub fn new(
        event_id: u64,
        event_type: EventType,
        authored_by: AgentId,
        payload: String,
        prev_hash: String,
    ) -> Self {
        let payload_hash = hash_bytes(payload.as_bytes());
        Envelope {
            event_id,
            event_type,
            authored_by,
            payload,
            payload_hash,
            prev_hash,
            sig: String::new(),
        }
    }

    /// What a signature must cover (and what the hash chain is built on).
    pub fn signature_input(&self) -> String {
        format!(
            "{}|{:?}|{}|{}",
            self.event_id, self.event_type, self.payload_hash, self.prev_hash
        )
    }

    /// Canonical hash of this envelope; used as `prev_hash` by the next event.
    pub fn self_hash(&self) -> String {
        hash_bytes(self.signature_input().as_bytes())
    }

    pub fn sign(&mut self, keypair: &Keypair) {
        let sig = keypair.sign(self.signature_input().as_bytes());
        self.sig = hex::encode(sig.to_bytes());
    }

    pub fn verify(&self, vk: &VerifyingKey) -> Result<(), String> {
        let bytes = hex::decode(&self.sig).map_err(|e| format!("bad sig hex: {e}"))?;
        let sig = Signature::from_slice(&bytes).map_err(|e| format!("bad sig bytes: {e}"))?;
        vk.verify(self.signature_input().as_bytes(), &sig)
            .map_err(|e| format!("signature verification failed: {e}"))
    }

    /// Verify against a public key given as hex (as stored in a registry).
    pub fn verify_hex(&self, key_hex: &str) -> Result<(), String> {
        let bytes = hex::decode(key_hex).map_err(|e| format!("bad key hex: {e}"))?;
        let vk = VerifyingKey::from_bytes(
            bytes
                .as_slice()
                .try_into()
                .map_err(|_| "bad public key length".to_string())?,
        )
        .map_err(|e| e.to_string())?;
        self.verify(&vk)
    }

    /// Simplified Merkle root over a batch of payload hashes.
    pub fn merkle_root(payload_hashes: &[String]) -> String {
        let mut acc = String::new();
        for h in payload_hashes {
            acc.push_str(h);
        }
        hash_bytes(acc.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify() {
        let kp = Keypair::random(AgentId("alice".into()));
        let mut env = Envelope::new(
            1,
            EventType::Contribute,
            kp.agent.clone(),
            r#"{"owner":"alice"}"#.into(),
            String::new(),
        );
        env.sign(&kp);
        assert!(env.verify(&kp.verifying_key()).is_ok());

        // Wrong key must fail.
        let mallory = Keypair::random(AgentId("mallory".into()));
        assert!(env.verify(&mallory.verifying_key()).is_err());
    }

    #[test]
    fn hash_chain_is_deterministic() {
        let kp = Keypair::from_seed(AgentId("alice".into()), [7u8; 32]);
        let env = Envelope::new(
            1,
            EventType::Contribute,
            kp.agent.clone(),
            "p".into(),
            String::new(),
        );
        let h1 = env.self_hash();
        let h2 = env.self_hash();
        assert_eq!(h1, h2);
    }
}
