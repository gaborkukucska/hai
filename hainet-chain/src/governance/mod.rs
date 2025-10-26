//! <!-- # START OF FILE hainet-chain/src/governance/mod.rs -->
//! Governance System for HAI-Net Blockchain
//
// Manages on-chain governance, including proposals and voting.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sled::Db;
use std::time::SystemTime;
use crate::identity::{DID, Keypair};
use crate::transactions::Transaction;

/// Unique identifier for a proposal
pub type ProposalId = [u8; 32];

/// The type of a governance proposal
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ProposalType {
    ConstitutionalAmendment,
    ParameterChange,
    CommunityFundSpend,
}

/// The current status of a proposal
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ProposalStatus {
    Pending,
    Active,
    Passed,
    Failed,
    Rejected,
}

/// A governance proposal
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Proposal {
    pub id: ProposalId,
    pub proposer: DID,
    pub title: String,
    pub description: String,
    pub proposal_type: ProposalType,
    pub created_at: SystemTime,
    pub voting_starts_at: SystemTime,
    pub voting_ends_at: SystemTime,
    pub status: ProposalStatus,
    pub payload: Vec<u8>, // The actual change to be executed
}

/// A vote on a proposal
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Vote {
    pub proposal_id: ProposalId,
    pub voter: DID,
    pub decision: bool, // true for Yes, false for No
    pub transaction: Transaction,
}

/// The governance management service
pub struct Governance {
    db: Db,
}

impl Governance {
    /// Create a new Governance service
    pub fn new(db_path: &str) -> Result<Self> {
        let db = sled::open(db_path)?;
        Ok(Self { db })
    }

    /// Submit a new proposal
    pub fn submit_proposal(&self, proposal: Proposal) -> Result<()> {
        let serialized_proposal = bincode::serialize(&proposal)?;
        self.db.insert(proposal.id, serialized_proposal)?;
        Ok(())
    }

    /// Cast a vote on a proposal
    pub fn cast_vote(&self, vote: Vote) -> Result<()> {
        // TODO: Store votes in a way that's easy to query
        // For now, we'll just log that a vote has been cast
        println!("Vote cast: {:?}", vote);
        Ok(())
    }

    // TODO: Implement vote tallying and proposal execution
}
