//! Decentralized Identifier (DID) Implementation
//!
//! DIDs provide unique, cryptographically verifiable identifiers for both
//! human users and AI personas within the HAI-Net network.
//!
//! Format: `did:hainet:{base58_pubkey}`

use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Decentralized Identifier for HAI-Net entities
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DID {
    /// The complete DID string (did:hainet:{identifier})
    identifier: String,
}

impl DID {
    /// Create a DID from an Ed25519 public key
    ///
    /// # Example
    /// ```
    /// use hainet_chain::identity::DID;
    /// use ed25519_dalek::{SigningKey, VerifyingKey};
    ///
    /// let signing_key = SigningKey::from_bytes(&[1u8; 32]);
    /// let verifying_key = signing_key.verifying_key();
    /// let did = DID::from_public_key(&verifying_key);
    /// ```
    pub fn from_public_key(public_key: &VerifyingKey) -> Self {
        let pubkey_bytes = public_key.to_bytes();
        let base58_encoded = bs58::encode(&pubkey_bytes).into_string();
        
        Self {
            identifier: format!("did:hainet:{}", base58_encoded),
        }
    }

    /// Create a DID from a string identifier
    ///
    /// # Errors
    /// Returns error if the DID format is invalid (must start with "did:hainet:")
    pub fn from_string(did_string: String) -> anyhow::Result<Self> {
        if !did_string.starts_with("did:hainet:") {
            anyhow::bail!("Invalid DID format: must start with 'did:hainet:'");
        }

        // Validate base58 encoding of the identifier part
        let parts: Vec<&str> = did_string.split(':').collect();
        if parts.len() != 3 {
            anyhow::bail!("Invalid DID format: expected 'did:hainet:{{identifier}}'");
        }

        // Validate base58 decoding
        bs58::decode(parts[2])
            .into_vec()
            .map_err(|e| anyhow::anyhow!("Invalid base58 encoding: {}", e))?;

        Ok(Self {
            identifier: did_string,
        })
    }

    /// Get the DID as a string
    pub fn as_str(&self) -> &str {
        &self.identifier
    }

    /// Extract the public key from the DID
    ///
    /// # Errors
    /// Returns error if the DID cannot be decoded or is invalid
    pub fn to_public_key(&self) -> anyhow::Result<VerifyingKey> {
        let parts: Vec<&str> = self.identifier.split(':').collect();
        if parts.len() != 3 {
            anyhow::bail!("Invalid DID format");
        }

        let pubkey_bytes = bs58::decode(parts[2])
            .into_vec()
            .map_err(|e| anyhow::anyhow!("Failed to decode base58: {}", e))?;

        if pubkey_bytes.len() != 32 {
            anyhow::bail!("Invalid public key length: expected 32 bytes, got {}", pubkey_bytes.len());
        }

        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&pubkey_bytes);

        VerifyingKey::from_bytes(&bytes)
            .map_err(|e| anyhow::anyhow!("Invalid public key: {}", e))
    }
}

impl fmt::Display for DID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.identifier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    #[test]
    fn test_did_from_public_key() {
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let verifying_key = signing_key.verifying_key();
        
        let did = DID::from_public_key(&verifying_key);
        
        assert!(did.as_str().starts_with("did:hainet:"));
    }

    #[test]
    fn test_did_roundtrip() {
        let signing_key = SigningKey::from_bytes(&[42u8; 32]);
        let verifying_key = signing_key.verifying_key();
        
        let did = DID::from_public_key(&verifying_key);
        let recovered_key = did.to_public_key().expect("Failed to recover public key");
        
        assert_eq!(verifying_key.to_bytes(), recovered_key.to_bytes());
    }

    #[test]
    fn test_did_from_string_valid() {
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let verifying_key = signing_key.verifying_key();
        let did1 = DID::from_public_key(&verifying_key);
        
        let did2 = DID::from_string(did1.as_str().to_string())
            .expect("Failed to create DID from string");
        
        assert_eq!(did1, did2);
    }

    #[test]
    fn test_did_from_string_invalid() {
        let result = DID::from_string("invalid:format:test".to_string());
        assert!(result.is_err());
        
        let result = DID::from_string("did:hainet:invalid_base58!!!".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_did_display() {
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let verifying_key = signing_key.verifying_key();
        let did = DID::from_public_key(&verifying_key);
        
        let displayed = format!("{}", did);
        assert!(displayed.starts_with("did:hainet:"));
    }
}
