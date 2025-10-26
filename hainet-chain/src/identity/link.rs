//! Human-AI Cryptographic Link Implementation
//!
//! Implements blockchain-secured binding between human users and their
//! AI personas, ensuring verifiable ownership and constitutional compliance.

use super::{Keypair, DID};
use ed25519_dalek::Signature;
use serde::{Deserialize, Serialize, Serializer, Deserializer};
use sha3::{Digest, Sha3_256};
use std::time::SystemTime;

/// Helper module for Signature serialization
mod signature_serde {
    use super::*;
    use serde::de::Error;

    pub fn serialize<S>(sig: &Signature, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes = sig.to_bytes();
        serializer.serialize_bytes(&bytes)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Signature, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: Vec<u8> = serde::Deserialize::deserialize(deserializer)?;
        if bytes.len() != 64 {
            return Err(D::Error::custom(format!("Invalid signature length: {}", bytes.len())));
        }
        let mut array = [0u8; 64];
        array.copy_from_slice(&bytes);
        Ok(Signature::from_bytes(&array))
    }
}

/// Hash type for link records
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hash([u8; 32]);

impl Hash {
    /// Create a hash from bytes
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut hasher = Sha3_256::new();
        hasher.update(bytes);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        Self(hash)
    }

    /// Create a zero hash (genesis)
    pub fn zero() -> Self {
        Self([0u8; 32])
    }

    /// Get hash as bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Version information for link records
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
}

/// Blockchain record linking a human user to their AI persona
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkRecord {
    /// Human user's DID
    pub user_did: DID,
    
    /// AI persona's DID
    pub persona_did: DID,
    
    /// Creation timestamp
    pub created_at: SystemTime,
    
    /// Hash of the link data
    pub link_hash: Hash,
    
    /// User's signature of the link
    #[serde(with = "signature_serde")]
    pub user_signature: Signature,
    
    /// AI persona's signature of the link
    #[serde(with = "signature_serde")]
    pub persona_signature: Signature,
    
    /// Human-chosen name for the persona
    pub persona_name: String,
    
    /// Version of the link format
    pub version: Version,
    
    /// Current state hash (for continuity verification)
    pub current_state_hash: Hash,
}

impl LinkRecord {
    /// Create link data for signing
    fn create_link_data(user_did: &DID, persona_did: &DID, persona_name: &str) -> Vec<u8> {
        format!("{}:{}:{}", user_did.as_str(), persona_did.as_str(), persona_name)
            .into_bytes()
    }

    /// Verify the integrity of this link record
    pub fn verify(&self) -> anyhow::Result<bool> {
        // Reconstruct link data
        let link_data = Self::create_link_data(
            &self.user_did,
            &self.persona_did,
            &self.persona_name,
        );

        // Verify expected hash
        let expected_hash = Hash::from_bytes(&link_data);
        if self.link_hash != expected_hash {
            return Ok(false);
        }

        // Verify user signature
        let user_pubkey = self.user_did.to_public_key()?;
        user_pubkey
            .verify_strict(self.link_hash.as_bytes(), &self.user_signature)
            .map_err(|_| anyhow::anyhow!("User signature verification failed"))?;

        // Verify persona signature
        let persona_pubkey = self.persona_did.to_public_key()?;
        persona_pubkey
            .verify_strict(self.link_hash.as_bytes(), &self.persona_signature)
            .map_err(|_| anyhow::anyhow!("Persona signature verification failed"))?;

        Ok(true)
    }
}

/// Blockchain-secured link between human and AI persona
pub struct PersonaLink {
    /// Human identity
    user_did: DID,

    /// AI persona identity
    persona_did: DID,
    persona_keypair: Keypair,

    /// Link record
    link_record: LinkRecord,
}

impl PersonaLink {
    /// Create new human-AI link with cryptographic binding
    ///
    /// # Arguments
    /// * `user_keypair` - Human user's keypair
    /// * `persona_name` - Human-chosen name for the AI persona
    ///
    /// # Example
    /// ```
    /// use hainet_chain::identity::{PersonaLink, Keypair};
    ///
    /// let user_keypair = Keypair::generate();
    /// let link = PersonaLink::create(user_keypair, "MyAssistant".to_string())
    ///     .expect("Failed to create link");
    /// ```
    pub fn create(user_keypair: Keypair, persona_name: String) -> anyhow::Result<Self> {
        // Generate DID for user
        let user_did = DID::from_public_key(&user_keypair.verifying_key());

        // Generate new keypair for AI persona
        let persona_keypair = Keypair::generate();
        let persona_did = DID::from_public_key(&persona_keypair.verifying_key());

        // Create link data
        let link_data = LinkRecord::create_link_data(&user_did, &persona_did, &persona_name);
        let link_hash = Hash::from_bytes(&link_data);

        // Both parties sign the link
        let user_signature = user_keypair.sign(link_hash.as_bytes());
        let persona_signature = persona_keypair.sign(link_hash.as_bytes());

        let link_record = LinkRecord {
            user_did: user_did.clone(),
            persona_did: persona_did.clone(),
            created_at: SystemTime::now(),
            link_hash,
            user_signature,
            persona_signature,
            persona_name,
            version: Version::new(1, 0, 0),
            current_state_hash: Hash::zero(),
        };

        Ok(Self {
            user_did,
            persona_did,
            persona_keypair,
            link_record,
        })
    }

    /// Verify link integrity
    pub fn verify(&self) -> anyhow::Result<bool> {
        self.link_record.verify()
    }

    /// Get the user's DID
    pub fn user_did(&self) -> &DID {
        &self.user_did
    }

    /// Get the persona's DID
    pub fn persona_did(&self) -> &DID {
        &self.persona_did
    }

    /// Get the link record (for blockchain submission)
    pub fn link_record(&self) -> &LinkRecord {
        &self.link_record
    }

    /// Get the persona's keypair (for AI agent operations)
    pub fn persona_keypair(&self) -> &Keypair {
        &self.persona_keypair
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_from_bytes() {
        let data = b"test data";
        let hash = Hash::from_bytes(data);
        
        assert_eq!(hash.as_bytes().len(), 32);
    }

    #[test]
    fn test_hash_zero() {
        let hash = Hash::zero();
        assert_eq!(hash.as_bytes(), &[0u8; 32]);
    }

    #[test]
    fn test_version() {
        let version = Version::new(1, 2, 3);
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 3);
    }

    #[test]
    fn test_persona_link_creation() {
        let user_keypair = Keypair::generate();
        let persona_name = "TestPersona".to_string();

        let link = PersonaLink::create(user_keypair, persona_name)
            .expect("Failed to create link");

        assert!(link.user_did().as_str().starts_with("did:hainet:"));
        assert!(link.persona_did().as_str().starts_with("did:hainet:"));
    }

    #[test]
    fn test_persona_link_verification() {
        let user_keypair = Keypair::generate();
        let link = PersonaLink::create(user_keypair, "Test".to_string())
            .expect("Failed to create link");

        let is_valid = link.verify().expect("Verification failed");
        assert!(is_valid);
    }

    #[test]
    fn test_link_record_verification() {
        let user_keypair = Keypair::generate();
        let link = PersonaLink::create(user_keypair, "Test".to_string())
            .expect("Failed to create link");

        let record = link.link_record();
        let is_valid = record.verify().expect("Record verification failed");
        assert!(is_valid);
    }

    #[test]
    fn test_link_record_fields() {
        let user_keypair = Keypair::generate();
        let persona_name = "MyAI".to_string();
        let link = PersonaLink::create(user_keypair, persona_name.clone())
            .expect("Failed to create link");

        let record = link.link_record();
        assert_eq!(record.persona_name, persona_name);
        assert_eq!(record.version, Version::new(1, 0, 0));
        assert_eq!(record.current_state_hash, Hash::zero());
    }

    #[test]
    fn test_different_links_different_dids() {
        let user_keypair = Keypair::generate();
        let link1 = PersonaLink::create(user_keypair.clone(), "Persona1".to_string())
            .expect("Failed to create link1");
        let link2 = PersonaLink::create(user_keypair, "Persona2".to_string())
            .expect("Failed to create link2");

        // Same user DID
        assert_eq!(link1.user_did(), link2.user_did());
        
        // Different persona DIDs (different keypairs generated)
        assert_ne!(link1.persona_did(), link2.persona_did());
    }
}
