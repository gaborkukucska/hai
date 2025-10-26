//! <!-- # START OF FILE hainet-chain/src/consensus/validator.rs -->
//! Block Validator for HAI-Net Blockchain
//
// Verifies the integrity and validity of incoming blocks from the consensus engine.

use anyhow::Result;
use tendermint::Block;
use tracing::{info, warn};
use crate::governance::GovernancePayload;
use crate::transactions::Transaction;
use sled::Db;

pub struct BlockValidator {
    db: Db,
}

impl BlockValidator {
    /// Create a new BlockValidator
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Validate a block and its transactions
    pub async fn validate_block(&self, block: &Block) -> Result<()> {
        info!("Validating block at height {}", block.header.height);

        // The block hash and signatures are typically verified by Tendermint Core.
        // Here, we focus on validating the transactions within the block.

        for transaction_bytes in &block.data {
            match bincode::deserialize::<Transaction>(transaction_bytes) {
                Ok(transaction) => {
                    if let Err(e) = self.validate_transaction(&transaction) {
                        warn!("Invalid transaction found in block: {:?}", e);
                        // Depending on the consensus rules, we might reject the whole block
                        return Err(e);
                    }
                }
                Err(e) => {
                    warn!("Failed to deserialize transaction: {:?}", e);
                    return Err(e.into());
                }
            }
        }

        info!("Block {} is valid", block.header.height);
        Ok(())
    }

    /// Validate a single transaction
    fn validate_transaction(&self, transaction: &Transaction) -> Result<()> {
        // 1. Verify the transaction's own integrity (signature, ID)
        transaction.verify()?;

        // 2. Decode the payload to determine the transaction type
        let payload: GovernancePayload = bincode::deserialize(&transaction.payload)?;

        // 3. Apply governance-specific validation rules
        match payload {
            GovernancePayload::SubmitProposal(proposal) => {
                // Rule: Proposal description must not be empty
                if proposal.description.is_empty() {
                    anyhow::bail!("Proposal description cannot be empty");
                }
                // Rule: Proposal title must not be empty
                if proposal.title.is_empty() {
                    anyhow::bail!("Proposal title cannot be empty");
                }
            }
            GovernancePayload::CastVote(vote) => {
                // Rule: The proposal must exist to be voted on
                if !self.db.contains_key(&vote.proposal_id)? {
                    anyhow::bail!("Proposal not found for vote");
                }
                // Further checks could include ensuring the proposal is in an 'Active' state
            }
        }

        Ok(())
    }
}
