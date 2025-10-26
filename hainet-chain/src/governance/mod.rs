//! <!-- # START OF FILE hainet-chain/src/governance/mod.rs -->
//! Governance System for HAI-Net Blockchain
//
// Manages on-chain governance, including proposals and voting.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use crate::identity::{DID, Keypair};
use crate::transactions::Transaction;
use crate::consensus::rpc_client::RpcClientContract;
use sha3::{Digest, Sha3_256};
use sled::Db;

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
pub struct Governance<C: RpcClientContract> {
    rpc_client: C,
}

impl<C: RpcClientContract> Governance<C> {
    /// Create a new Governance service
    pub fn new(rpc_client: C) -> Result<Self> {
        Ok(Self { rpc_client })
    }

    /// Submit a new proposal
    pub async fn submit_proposal(&self, transaction: Transaction) -> Result<()> {
        transaction.verify()?;
        self.rpc_client.broadcast_tx(&transaction).await?;
        Ok(())
    }

    /// Cast a vote on a proposal
    pub async fn cast_vote(&self, transaction: Transaction) -> Result<()> {
        transaction.verify()?;
        self.rpc_client.broadcast_tx(&transaction).await?;
        Ok(())
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

/// Tally the votes for a given proposal
pub fn tally_votes(db: &Db, proposal_id: ProposalId) -> Result<TallyResult> {
    let mut yes_votes = 0;
    let mut no_votes = 0;

    let mut prefix = b"vote_".to_vec();
    prefix.extend_from_slice(&proposal_id);

    for item in db.scan_prefix(prefix) {
        let (_, value) = item?;
        let vote: Vote = bincode::deserialize(&value)?;
        if vote.decision {
            yes_votes += 1;
        } else {
            no_votes += 1;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Keypair;
    use std::sync::{Arc, Mutex};
    use async_trait::async_trait;

    struct MockRpcClient {
        broadcast_tx_called: Arc<Mutex<bool>>,
    }

    impl MockRpcClient {
        fn new(broadcast_tx_called: Arc<Mutex<bool>>) -> Self {
            Self { broadcast_tx_called }
        }
    }

    #[async_trait]
    impl RpcClientContract for MockRpcClient {
        async fn broadcast_tx(&self, _tx: &Transaction) -> Result<()> {
            let mut called = self.broadcast_tx_called.lock().unwrap();
            *called = true;
            Ok(())
        }
        async fn status(&self) -> Result<tendermint_rpc::endpoint::status::Response> {
            unimplemented!();
        }
    }

    #[tokio::test]
    async fn test_submit_proposal_broadcasts_transaction() {
        let broadcast_tx_called = Arc::new(Mutex::new(false));
        let mock_rpc_client = MockRpcClient::new(broadcast_tx_called.clone());
        let governance = Governance::new(mock_rpc_client).unwrap();
        let keypair = Keypair::generate();
        let transaction = create_proposal(
            &keypair,
            "Test".to_string(),
            "Test".to_string(),
            ProposalType::CommunityFundSpend,
            3600,
            vec![],
        ).unwrap();
        governance.submit_proposal(transaction).await.unwrap();
        assert!(*broadcast_tx_called.lock().unwrap());
    }

    #[tokio::test]
    async fn test_cast_vote_broadcasts_transaction() {
        let broadcast_tx_called = Arc::new(Mutex::new(false));
        let mock_rpc_client = MockRpcClient::new(broadcast_tx_called.clone());
        let governance = Governance::new(mock_rpc_client).unwrap();
        let keypair = Keypair::generate();
        let transaction = create_vote(&keypair, [0u8; 32], true).unwrap();
        governance.cast_vote(transaction).await.unwrap();
        assert!(*broadcast_tx_called.lock().unwrap());
    }
}
