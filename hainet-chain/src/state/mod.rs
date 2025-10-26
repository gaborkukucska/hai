//! <!-- # START OF FILE hainet-chain/src/state/mod.rs -->
//! State Machine for HAI-Net Blockchain
//
// Manages the state of the blockchain and applies transactions.

use anyhow::Result;
use sled::Db;
use tracing::info;

/// Represents the state of the blockchain.
pub struct StateMachine {
    db: Db,
}

impl StateMachine {
    /// Create a new StateMachine
    pub fn new(db_path: &str) -> Result<Self> {
        info!("Opening state database at {}", db_path);
        let db = sled::open(db_path)?;
        Ok(Self { db })
    }

    /// Apply a transaction to the state
    pub async fn apply_transaction(&self, _transaction: &[u8]) -> Result<()> {
        info!("Applying transaction to state");
        // TODO: Implement transaction application logic
        // 1. Decode the transaction
        // 2. Validate the transaction against the current state
        // 3. Update the state accordingly
        // 4. Handle errors and rollbacks
        Ok(())
    }

    /// Get a value from the state
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let value = self.db.get(key)?.map(|v| v.to_vec());
        Ok(value)
    }

    /// Set a value in the state
    pub fn set(&self, key: &[u8], value: &[u8]) -> Result<()> {
        self.db.insert(key, value)?;
        Ok(())
    }
}
