//! Keypair Management for HAI-Net
//!
//! Wrapper around Ed25519 keypairs for simplified usage within HAI-Net.

use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use serde::{Deserialize, Serialize};

/// Keypair wrapper for Ed25519 cryptographic operations
#[derive(Clone)]
pub struct Keypair {
    signing_key: SigningKey,
}

impl Keypair {
    /// Generate a new random keypair
    ///
    /// # Example
    /// ```
    /// use hainet_chain::identity::Keypair;
    ///
    /// let keypair = Keypair::generate();
    /// ```
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();
        let secret_bytes: [u8; 32] = rand::Rng::gen(&mut rng);
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        
        Self { signing_key }
    }

    /// Create a keypair from existing bytes
    ///
    /// # Errors
    /// Returns error if the bytes are invalid (must be exactly 32 bytes)
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(bytes);
        Self { signing_key }
    }

    /// Get the signing key bytes
    pub fn to_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// Get the public key (verifying key)
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    /// Verify a signature
    pub fn verify(&self, message: &[u8], signature: &Signature) -> anyhow::Result<()> {
        self.verifying_key()
            .verify(message, signature)
            .map_err(|e| anyhow::anyhow!("Signature verification failed: {}", e))
    }
}

impl std::fmt::Debug for Keypair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keypair")
            .field("public_key", &self.verifying_key())
            .finish_non_exhaustive()
    }
}

/// Serializable signature for storage and transmission
#[derive(Debug, Clone)]
pub struct SerializableSignature {
    bytes: [u8; 64],
}

impl Serialize for SerializableSignature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.bytes)
    }
}

impl<'de> Deserialize<'de> for SerializableSignature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = serde::Deserialize::deserialize(deserializer)?;
        if bytes.len() != 64 {
            return Err(serde::de::Error::custom(format!("Invalid signature length: {}", bytes.len())));
        }
        let mut array = [0u8; 64];
        array.copy_from_slice(&bytes);
        Ok(Self { bytes: array })
    }
}

impl From<Signature> for SerializableSignature {
    fn from(sig: Signature) -> Self {
        Self {
            bytes: sig.to_bytes(),
        }
    }
}

impl From<SerializableSignature> for Signature {
    fn from(sig: SerializableSignature) -> Self {
        Signature::from_bytes(&sig.bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let keypair = Keypair::generate();
        let public_key = keypair.verifying_key();
        
        // Verify we can get public key
        assert_eq!(public_key.to_bytes().len(), 32);
    }

    #[test]
    fn test_keypair_from_bytes() {
        let bytes = [42u8; 32];
        let keypair = Keypair::from_bytes(&bytes);
        
        assert_eq!(keypair.to_bytes(), bytes);
    }

    #[test]
    fn test_sign_and_verify() {
        let keypair = Keypair::generate();
        let message = b"Hello, HAI-Net!";
        
        let signature = keypair.sign(message);
        let result = keypair.verify(message, &signature);
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_wrong_signature() {
        let keypair1 = Keypair::generate();
        let keypair2 = Keypair::generate();
        let message = b"Hello, HAI-Net!";
        
        let signature = keypair1.sign(message);
        let result = keypair2.verify(message, &signature);
        
        assert!(result.is_err());
    }

    #[test]
    fn test_serializable_signature() {
        let keypair = Keypair::generate();
        let message = b"Test message";
        let signature = keypair.sign(message);
        
        let serializable: SerializableSignature = signature.into();
        let deserialized: Signature = serializable.into();
        
        assert!(keypair.verify(message, &deserialized).is_ok());
    }
}
