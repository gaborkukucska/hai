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
use sha3::{Digest, Sha3_256};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum GovernancePayload {
    SubmitProposal(Proposal),
    CastVote(Vote),
}

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
}

/// The governance management service
pub struct Governance {
    db: Db,
}

impl Governance {
    /// Create a new Governance service
    pub fn new(db: Db) -> Result<Self> {
        Ok(Self { db })
    }

    /// Submit a new proposal
    pub fn submit_proposal(&self, transaction: Transaction) -> Result<()> {
        transaction.verify()?;
        let payload: GovernancePayload = bincode::deserialize(&transaction.payload)?;
        if let GovernancePayload::SubmitProposal(proposal) = payload {
            let serialized_proposal = bincode::serialize(&proposal)?;
            self.db.insert(&proposal.id, serialized_proposal)?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Invalid payload type for submit_proposal"))
        }
    }

    /// Cast a vote on a proposal
    pub fn cast_vote(&self, transaction: Transaction) -> Result<()> {
        transaction.verify()?;
        let payload: GovernancePayload = bincode::deserialize(&transaction.payload)?;

        if let GovernancePayload::CastVote(vote) = payload {
            // Construct a unique key for the vote to prevent double-voting
            let mut vote_key = b"vote_".to_vec();
            vote_key.extend_from_slice(&vote.proposal_id);
            vote_key.extend_from_slice(vote.voter.as_str().as_bytes());

            // Check if this voter has already voted
            if self.db.contains_key(&vote_key)? {
                anyhow::bail!("Voter has already cast a vote on this proposal");
            }

            // Store the vote transaction
            let serialized_transaction = bincode::serialize(&transaction)?;
            self.db.insert(vote_key, serialized_transaction)?;

            Ok(())
        } else {
            Err(anyhow::anyhow!("Invalid payload type for cast_vote"))
        }
    }

    /// Tally votes for a given proposal
    pub fn tally_votes(&self, proposal_id: ProposalId) -> Result<TallyResult> {
        let mut yes_votes = 0;
        let mut no_votes = 0;

        let prefix = b"vote_".to_vec();
        for item in self.db.scan_prefix(&prefix) {
            let (_, value) = item?;
            let transaction: Transaction = bincode::deserialize(&value)?;
            let payload: GovernancePayload = bincode::deserialize(&transaction.payload)?;

            if let GovernancePayload::CastVote(vote) = payload {
                if vote.proposal_id == proposal_id {
                    if vote.decision {
                        yes_votes += 1;
                    } else {
                        no_votes += 1;
                    }
                }
            }
        }

        let status = if yes_votes > no_votes {
            ProposalStatus::Passed
        } else {
            ProposalStatus::Failed
        };

        Ok(TallyResult {
            yes_votes,
            no_votes,
            status,
        })
    }
}

/// The result of a vote tally
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TallyResult {
    pub yes_votes: u64,
    pub no_votes: u64,
    pub status: ProposalStatus,
}

/// Create and sign a new vote
pub fn create_vote(
    keypair: &Keypair,
    proposal_id: ProposalId,
    decision: bool,
) -> Result<Transaction> {
    let voter_did = DID::from_public_key(&keypair.verifying_key());

    let vote = Vote {
        proposal_id,
        voter: voter_did,
        decision,
    };

    let payload_enum = GovernancePayload::CastVote(vote);
    let payload = bincode::serialize(&payload_enum)?;
    Transaction::new(payload, keypair)
}

/// Create and sign a new proposal
pub fn create_proposal(
    keypair: &Keypair,
    title: String,
    description: String,
    proposal_type: ProposalType,
    voting_duration_secs: u64,
    payload: Vec<u8>,
) -> Result<Transaction> {
    let proposer_did = DID::from_public_key(&keypair.verifying_key());
    let current_time = SystemTime::now();

    let proposal = Proposal {
        id: [0u8; 32], // Placeholder, will be replaced by hash
        proposer: proposer_did,
        title,
        description,
        proposal_type,
        created_at: current_time,
        voting_starts_at: current_time,
        voting_ends_at: current_time + std::time::Duration::from_secs(voting_duration_secs),
        status: ProposalStatus::Pending,
        payload,
    };

    let serialized_proposal = bincode::serialize(&proposal)?;
    let proposal_id = Sha3_256::digest(&serialized_proposal);
    let mut id_array = [0u8; 32];
    id_array.copy_from_slice(&proposal_id);

    let mut proposal_with_id = proposal.clone();
    proposal_with_id.id = id_array;

    let payload_enum = GovernancePayload::SubmitProposal(proposal_with_id);
    let final_payload = bincode::serialize(&payload_enum)?;

    Transaction::new(final_payload, keypair)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Keypair;
    use tempfile::tempdir;

    #[test]
    fn test_create_and_submit_proposal() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let db = sled::open(db_path).unwrap();
        let governance = Governance::new(db).unwrap();
        let keypair = Keypair::generate();

        let transaction = create_proposal(
            &keypair,
            "Test Proposal".to_string(),
            "This is a test proposal.".to_string(),
            ProposalType::CommunityFundSpend,
            3600,
            vec![1, 2, 3],
        )
        .unwrap();

        let result = governance.submit_proposal(transaction);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_and_cast_vote() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let db = sled::open(db_path).unwrap();
        let governance = Governance::new(db).unwrap();
        let keypair = Keypair::generate();
        let proposal_id = [1u8; 32];

        let transaction = create_vote(&keypair, proposal_id, true).unwrap();

        let result = governance.cast_vote(transaction);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tally_votes() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let db = sled::open(db_path).unwrap();
        let governance = Governance::new(db).unwrap();
        let proposal_id = [1u8; 32];

        // Cast some votes
        let keypair1 = Keypair::generate();
        let keypair2 = Keypair::generate();
        let keypair3 = Keypair::generate();
        let vote1 = create_vote(&keypair1, proposal_id, true).unwrap();
        let vote2 = create_vote(&keypair2, proposal_id, false).unwrap();
        let vote3 = create_vote(&keypair3, proposal_id, true).unwrap();
        governance.cast_vote(vote1).unwrap();
        governance.cast_vote(vote2).unwrap();
        governance.cast_vote(vote3).unwrap();

        let result = governance.tally_votes(proposal_id).unwrap();

        assert_eq!(result.yes_votes, 2);
        assert_eq!(result.no_votes, 1);
        assert_eq!(result.status, ProposalStatus::Passed);
    }
}
