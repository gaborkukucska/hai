//! <!-- # START OF FILE hainet-chain/tests/governance_integration.rs -->
//! Integration Tests for the Governance Workflow

use hainet_chain::governance::{create_proposal, create_vote, ProposalType};
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

    // 2. Create and submit a proposal
    let proposal_tx = create_proposal(
        &keypair1,
        "Test Proposal".to_string(),
        "A proposal to test the governance workflow.".to_string(),
        ProposalType::CommunityFundSpend,
        3600,
        vec![1, 2, 3],
    )
    .unwrap();
    let proposal_tx_bytes = bincode::serialize(&proposal_tx).unwrap();
    state_machine
        .apply_transaction(&proposal_tx_bytes)
        .await
        .unwrap();

    // Extract proposal ID from the transaction
    let proposal_payload: hainet_chain::governance::GovernancePayload =
        bincode::deserialize(&proposal_tx.payload).unwrap();
    let proposal = match proposal_payload {
        hainet_chain::governance::GovernancePayload::SubmitProposal(p) => p,
        _ => panic!("Unexpected payload type"),
    };
    let proposal_id = proposal.id;

    // 3. Cast votes
    let vote1_tx = create_vote(&keypair1, proposal_id, true).unwrap(); // Yes
    let vote2_tx = create_vote(&keypair2, proposal_id, true).unwrap(); // Yes
    let vote3_tx = create_vote(&keypair3, proposal_id, false).unwrap(); // No

    let vote1_tx_bytes = bincode::serialize(&vote1_tx).unwrap();
    let vote2_tx_bytes = bincode::serialize(&vote2_tx).unwrap();
    let vote3_tx_bytes = bincode::serialize(&vote3_tx).unwrap();

    state_machine
        .apply_transaction(&vote1_tx_bytes)
        .await
        .unwrap();
    state_machine
        .apply_transaction(&vote2_tx_bytes)
        .await
        .unwrap();
    state_machine
        .apply_transaction(&vote3_tx_bytes)
        .await
        .unwrap();

    // 4. Tally votes
    let result = state_machine.tally_votes(proposal_id).unwrap();

    // 5. Verify the outcome
    assert_eq!(result.yes_votes, 2);
    assert_eq!(result.no_votes, 1);
    assert_eq!(
        result.status,
        hainet_chain::governance::ProposalStatus::Passed
    );
}
