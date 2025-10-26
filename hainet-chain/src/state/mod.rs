//! <!-- # START OF FILE hainet-chain/src/state/mod.rs -->
//! State Machine for HAI-Net Blockchain
//
// Manages the state of the blockchain and applies transactions.

use anyhow::Result;
use sled::Db;
use tracing::info;
use crate::governance::{Governance, GovernancePayload, ProposalId, TallyResult};
use crate::transactions::Transaction;
/// Represents the state of the blockchain.
pub struct StateMachine {
    db: Db,
    governance: Governance,
}

impl StateMachine {
    /// Create a new StateMachine
    pub fn new(db_path: &str) -> Result<Self> {
        info!("Opening state database at {}", db_path);
        let db = sled::open(db_path)?;
        let governance = Governance::new(db.clone())?;
        Ok(Self { db, governance })
    }

    /// Apply a transaction to the state
    pub async fn apply_transaction(&self, transaction_bytes: &[u8]) -> Result<()> {
        info!("Applying transaction to state");
        let transaction: Transaction = bincode::deserialize(transaction_bytes)?;

        // Verify the transaction's integrity and signature
        transaction.verify()?;

        // Decode the payload to determine the transaction type
        let payload: GovernancePayload = bincode::deserialize(&transaction.payload)?;

        // Match on the payload type and call the appropriate handler
        match payload {
            GovernancePayload::SubmitProposal(_) => {
                self.governance.submit_proposal(transaction)?;
            }
            GovernancePayload::CastVote(_) => {
                self.governance.cast_vote(transaction)?;
            }
        }

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

    /// Tally votes for a given proposal
    pub fn tally_votes(&self, proposal_id: ProposalId) -> Result<TallyResult> {
        self.governance.tally_votes(proposal_id)
    }
}
