//! <!-- # START OF FILE hainet-chain/src/transactions/mod.rs -->
//! Transaction Management for HAI-Net Blockchain
//
// Defines the structure of transactions and handles their creation and validation.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::time::SystemTime;
use ed25519_dalek::{Signature, VerifyingKey, Verifier};
use crate::identity::Keypair;

/// Represents a transaction on the HAI-Net blockchain.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Transaction {
    pub id: [u8; 32],
    pub timestamp: SystemTime,
    pub payload: Vec<u8>,
    pub signature: Signature,
    pub public_key: VerifyingKey,
}

impl Transaction {
    /// Create and sign a new transaction
    pub fn new(payload: Vec<u8>, keypair: &Keypair) -> Result<Self> {
        let timestamp = SystemTime::now();
        let payload_hash = Self::hash_payload(&payload, &timestamp);
        let signature = keypair.sign(&payload_hash);
        let public_key = keypair.verifying_key();

        let tx_data = Self::serialize_for_id(&timestamp, &payload, &public_key);
        let id = Self::hash(&tx_data);

        Ok(Self {
            id,
            timestamp,
            payload,
            signature,
            public_key,
        })
    }

    /// Verify the transaction's signature and integrity
    pub fn verify(&self) -> Result<()> {
        let payload_hash = Self::hash_payload(&self.payload, &self.timestamp);
        self.public_key.verify(&payload_hash, &self.signature)?;

        let tx_data = Self::serialize_for_id(&self.timestamp, &self.payload, &self.public_key);
        let expected_id = Self::hash(&tx_data);

        if self.id != expected_id {
            anyhow::bail!("Transaction ID mismatch");
        }

        Ok(())
    }

    /// Hash the transaction payload and timestamp for signing
    fn hash_payload(payload: &[u8], timestamp: &SystemTime) -> [u8; 32] {
        let mut hasher = Sha3_256::new();
        let duration = timestamp.duration_since(SystemTime::UNIX_EPOCH).unwrap();
        hasher.update(&duration.as_secs().to_le_bytes());
        hasher.update(&duration.subsec_nanos().to_le_bytes());
        hasher.update(payload);
        hasher.finalize().into()
    }

    /// Serialize core fields to create the transaction ID
    fn serialize_for_id(timestamp: &SystemTime, payload: &[u8], public_key: &VerifyingKey) -> Vec<u8> {
        let mut data = Vec::new();
        let duration = timestamp.duration_since(SystemTime::UNIX_EPOCH).unwrap();
        data.extend_from_slice(&duration.as_secs().to_le_bytes());
        data.extend_from_slice(&duration.subsec_nanos().to_le_bytes());
        data.extend_from_slice(payload);
        data.extend_from_slice(public_key.as_bytes());
        data
    }

    /// Hash data to create a transaction ID or other hash
    fn hash(data: &[u8]) -> [u8; 32] {
        let mut hasher = Sha3_256::new();
        hasher.update(data);
        hasher.finalize().into()
    }
}
