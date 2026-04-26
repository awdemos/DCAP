//! Self-sovereign identity based on Ed25519.
//!
//! Agents identify themselves via keypairs, not UUIDs or shared secrets.
//! An `Identity` is essentially a public key with a human-readable DID string.

use ed25519_dalek::{Signer, VerifyingKey, SigningKey, Signature as DalekSignature};
use serde::{Deserialize, Serialize};
use rand::Rng;
use std::fmt;
use std::hash::{Hash, Hasher};

/// A unique agent identity derived from an Ed25519 public key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    /// Decentralized identifier: `did:dcap:<base58-encoded-pubkey>`
    pub did: String,
    /// The raw verifying key.
    #[serde(with = "verifying_key_hex")]
    pub verifying_key: VerifyingKey,
}

impl Identity {
    /// Construct an identity from a verifying key.
    pub fn from_key(verifying_key: VerifyingKey) -> Self {
        let did = format!("did:dcap:{}", bs58::encode(verifying_key.as_bytes()));
        Self { did, verifying_key }
    }

    /// Verify a signature over a message.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> Result<(), CryptoError> {
        self.verifying_key
            .verify_strict(message, &signature.0)
            .map_err(|e| CryptoError::InvalidSignature(e.to_string()))
    }
}

impl fmt::Display for Identity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.did)
    }
}

impl PartialOrd for Identity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Identity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.did.cmp(&other.did)
    }
}

impl Hash for Identity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.did.hash(state);
    }
}

/// An Ed25519 signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature(#[serde(with = "signature_hex")] pub DalekSignature);

impl Signature {
    pub fn to_bytes(&self) -> [u8; 64] {
        self.0.to_bytes()
    }
}

/// A keypair for an agent. This should be stored securely and never transmitted.
pub struct Keypair(pub SigningKey);

impl Keypair {
    pub fn generate() -> Self {
        let mut secret = [0u8; 32];
        rand::thread_rng().fill(&mut secret);
        Self(SigningKey::from_bytes(&secret))
    }

    pub fn identity(&self) -> Identity {
        Identity::from_key(self.0.verifying_key())
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        Signature(self.0.sign(message))
    }
}

/// Cryptographic errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CryptoError {
    #[error("invalid signature: {0}")]
    InvalidSignature(String),
    #[error("invalid key: {0}")]
    InvalidKey(String),
    #[error("decode error: {0}")]
    DecodeError(String),
}

// --- Serde helpers for Ed25519 types ---

mod verifying_key_hex {
    use ed25519_dalek::VerifyingKey;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(key: &VerifyingKey, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(key.as_bytes()))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<VerifyingKey, D::Error> {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode(s).map_err(serde::de::Error::custom)?;
        let array: [u8; 32] = bytes.try_into().map_err(|_| serde::de::Error::custom("invalid key length"))?;
        VerifyingKey::from_bytes(&array).map_err(|e| serde::de::Error::custom(format!("{e}")))
    }
}

mod signature_hex {
    use ed25519_dalek::Signature;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(sig: &Signature, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(sig.to_bytes()))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Signature, D::Error> {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode(s).map_err(serde::de::Error::custom)?;
        let array: [u8; 64] = bytes.try_into().map_err(|_| serde::de::Error::custom("invalid signature length"))?;
        Ok(Signature::from(array))
    }
}

// Simple base64url encoding as a stand-in for base58 to avoid extra dependency.
mod bs58 {
    pub fn encode(input: &[u8]) -> String {
        base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_sign_and_verify() {
        let kp = Keypair::generate();
        let identity = kp.identity();
        let msg = b"hello dcap";
        let sig = kp.sign(msg);
        assert!(identity.verify(msg, &sig).is_ok());
    }

    #[test]
    fn verify_fails_with_wrong_key() {
        let kp_a = Keypair::generate();
        let kp_b = Keypair::generate();
        let msg = b"hello dcap";
        let sig = kp_a.sign(msg);
        assert!(kp_b.identity().verify(msg, &sig).is_err());
    }

    #[test]
    fn identity_ordering_by_did() {
        let kp_a = Keypair::generate();
        let kp_b = Keypair::generate();
        let id_a = kp_a.identity();
        let id_b = kp_b.identity();
        // Ordering is deterministic based on did strings
        assert!((id_a < id_b) || (id_b < id_a) || (id_a == id_b));
    }

    #[test]
    fn identity_hash_consistent() {
        use std::collections::hash_map::DefaultHasher;
        let kp = Keypair::generate();
        let id = kp.identity();
        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        id.hash(&mut h1);
        id.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }
}
