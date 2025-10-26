//! <!-- # START OF FILE hainet-chain/src/consensus/mod.rs -->
//! Consensus Engine for HAI-Net Blockchain
//
// Integrates Tendermint for Byzantine Fault Tolerant consensus.

use anyhow::Result;
use tendermint_rpc::{Client, HttpClient};
use tracing::info;

pub struct ConsensusService {
    rpc_client: HttpClient,
}

impl ConsensusService {
    /// Create a new ConsensusService
    pub async fn new(rpc_url: &str) -> Result<Self> {
        info!("Connecting to Tendermint RPC at {}", rpc_url);
        let rpc_client = HttpClient::new(rpc_url)?;
        Ok(Self { rpc_client })
    }

    /// Check the status of the Tendermint node
    pub async fn check_status(&self) -> Result<()> {
        let status = self.rpc_client.status().await?;
        info!("Tendermint node status: {:?}", status);
        Ok(())
    }

    // TODO: Implement block validation and other consensus logic
}

pub mod validator;
