//! <!-- # START OF FILE hainet-chain/tests/governance_integration.rs -->
//! Integration Tests for the Governance Workflow

use hainet_chain::governance::{create_proposal, create_vote, ProposalType, GovernancePayload};
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

    // 5. Tally votes using the state machine
    let tally_result = state_machine.tally_votes(proposal_id).unwrap();

    // 6. Verify the outcome
    assert_eq!(tally_result.yes_votes, 2);
    assert_eq!(tally_result.no_votes, 1);
    assert_eq!(tally_result.status, hainet_chain::governance::ProposalStatus::Passed);
}
