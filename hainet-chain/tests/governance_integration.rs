//! <!-- # START OF FILE hainet-chain/tests/governance_integration.rs -->
use hainet_chain::governance::{create_proposal, create_vote, ProposalType};
use hainet_chain::identity::Keypair;
use hainet_chain::state::StateMachine;
use tempfile::tempdir;
use anyhow::Result;

#[tokio::test]
async fn test_governance_workflow() -> Result<()> {
    // 1. Setup
    let dir = tempdir()?;
    let db_path = dir.path().to_str().unwrap();
    let state_machine = StateMachine::new(db_path)?;
    let keypair1 = Keypair::generate();
    let keypair2 = Keypair::generate();
    let keypair3 = Keypair::generate();

    // 2. Create and submit a proposal
    let proposal_tx = create_proposal(
        &keypair1,
        "Test Proposal".to_string(),
        "This is a test proposal.".to_string(),
        ProposalType::CommunityFundSpend,
        3600,
        vec![1, 2, 3],
    )?;
    let proposal_tx_bytes = bincode::serialize(&proposal_tx)?;
    state_machine.apply_transaction(&proposal_tx_bytes).await?;

    // Extract proposal ID
    let proposal_payload: hainet_chain::governance::GovernancePayload = bincode::deserialize(&proposal_tx.payload)?;
    let proposal_id = if let hainet_chain::governance::GovernancePayload::SubmitProposal(p) = proposal_payload {
        p.id
    } else {
        panic!("Invalid proposal payload");
    };

    // 3. Cast votes
    let vote1_tx = create_vote(&keypair1, proposal_id, true)?;
    let vote2_tx = create_vote(&keypair2, proposal_id, false)?;
    let vote3_tx = create_vote(&keypair3, proposal_id, true)?;

    let vote1_tx_bytes = bincode::serialize(&vote1_tx)?;
    let vote2_tx_bytes = bincode::serialize(&vote2_tx)?;
    let vote3_tx_bytes = bincode::serialize(&vote3_tx)?;

    state_machine.apply_transaction(&vote1_tx_bytes).await?;
    state_machine.apply_transaction(&vote2_tx_bytes).await?;
    state_machine.apply_transaction(&vote3_tx_bytes).await?;

    // 4. Tally votes
    let tally_result = state_machine.tally_votes(proposal_id)?;
    assert_eq!(tally_result.yes_votes, 2);
    assert_eq!(tally_result.no_votes, 1);
    assert_eq!(tally_result.status, hainet_chain::governance::ProposalStatus::Passed);

    Ok(())
}
