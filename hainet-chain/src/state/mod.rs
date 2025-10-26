//! <!-- # START OF FILE hainet-chain/src/state/mod.rs -->
//! State Machine for HAI-Net Blockchain
//
// Manages the state of the blockchain and applies transactions.

use anyhow::Result;
use sled::Db;
use tracing::info;
use crate::governance::{GovernancePayload, ProposalId, TallyResult};
use crate::transactions::Transaction;

/// Represents the state of the blockchain.
pub struct StateMachine {
    db: Db,
    // governance: Governance, // TODO: Initialize with RpcClient
}

impl StateMachine {
    /// Create a new StateMachine
    pub fn new(db_path: &str) -> Result<Self> {
        info!("Opening state database at {}", db_path);
        let db = sled::open(db_path)?;
        // TODO: The RPC client needs a URL, which we don't have here.
        // This will be resolved when we integrate with the ABCI server.
        // let rpc_client = RpcClient::new("http://127.0.0.1:26657")?;
        // let governance = Governance::new(rpc_client)?;
        Ok(Self { db })
    }

    /// Apply a block of transactions to the state.
    pub async fn apply_block(&self, transactions: Vec<Transaction>) -> Result<()> {
        for tx in transactions {
            self.apply_transaction(&tx).await?;
        }
        Ok(())
    }

    /// Apply a single transaction to the state
    async fn apply_transaction(&self, transaction: &Transaction) -> Result<()> {
        info!("Applying transaction to state");
        transaction.verify()?;
        let payload: GovernancePayload = bincode::deserialize(&transaction.payload)?;

        match payload {
            GovernancePayload::SubmitProposal(proposal) => {
                let serialized_proposal = bincode::serialize(&proposal)?;
                self.db.insert(&proposal.id, serialized_proposal)?;
            }
            GovernancePayload::CastVote(vote) => {
                let mut vote_key = b"vote_".to_vec();
                vote_key.extend_from_slice(&vote.proposal_id);
                vote_key.extend_from_slice(vote.voter.as_str().as_bytes());
                if self.db.contains_key(&vote_key)? {
                    anyhow::bail!("Voter has already cast a vote on this proposal");
                }
                let serialized_vote = bincode::serialize(&vote)?;
                self.db.insert(vote_key, serialized_vote)?;
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

    /// Tally votes for a proposal
    pub fn tally_votes(&self, proposal_id: ProposalId) -> Result<TallyResult> {
        crate::governance::tally_votes(&self.db, proposal_id)
    }
}
