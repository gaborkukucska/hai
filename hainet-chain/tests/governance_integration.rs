//! <!-- # START OF FILE hainet-chain/tests/governance_integration.rs -->
//! Integration Tests for the Governance Workflow

use hainet_chain::governance::{create_proposal, create_vote, ProposalType, GovernancePayload, Vote};
use hainet_chain::identity::Keypair;
use hainet_chain::state::StateMachine;
use tempfile::tempdir;

#[tokio::test]
async fn test_governance_workflow() {
    // 1. Setup
    let dir = tempdir().unwrap();
    let db_path = dir.path().to_str().unwrap();
    let state_machine = StateMachine::new(db_path).unwrap();
    let keypair1 = Keypair::generate();
    let keypair2 = Keypair::generate();
    let keypair3 = Keypair::generate();

    // 2. Create a proposal
    let proposal_tx = create_proposal(
        &keypair1,
        "Test Proposal".to_string(),
        "A proposal to test the governance workflow.".to_string(),
        ProposalType::CommunityFundSpend,
        3600,
        vec![1, 2, 3],
    )
    .unwrap();

    // Extract proposal ID from the transaction
    let proposal_payload: GovernancePayload =
        bincode::deserialize(&proposal_tx.payload).unwrap();
    let proposal = match proposal_payload {
        GovernancePayload::SubmitProposal(p) => p,
        _ => panic!("Unexpected payload type"),
    };
    let proposal_id = proposal.id;

    // 3. Create votes
    let vote1_tx = create_vote(&keypair1, proposal_id, true).unwrap(); // Yes
    let vote2_tx = create_vote(&keypair2, proposal_id, true).unwrap(); // Yes
    let vote3_tx = create_vote(&keypair3, proposal_id, false).unwrap(); // No

    // 4. Apply all transactions in a single block
    let block_txs = vec![proposal_tx, vote1_tx, vote2_tx, vote3_tx];
    state_machine.apply_block(block_txs).await.unwrap();

    // Explicitly drop the state machine to release the database lock
    drop(state_machine);

    // 5. Tally votes manually by scanning the database
    let db = sled::open(db_path).unwrap();
    let mut yes_votes = 0;
    let mut no_votes = 0;
    let prefix = b"vote_";
    for item in db.scan_prefix(prefix) {
        let (_, value) = item.unwrap();
        let vote: Vote = bincode::deserialize(&value).unwrap();
        if vote.proposal_id == proposal_id {
            if vote.decision {
                yes_votes += 1;
            } else {
                no_votes += 1;
            }
        }
    }

    // 6. Verify the outcome
    assert_eq!(yes_votes, 2);
    assert_eq!(no_votes, 1);
}
