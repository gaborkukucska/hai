//! <!-- # START OF FILE hainet-chain/src/consensus/rpc_client.rs -->
//! This module provides a client for interacting with a Tendermint node's RPC endpoint.

use tendermint_rpc::{Client, HttpClient};
use crate::transactions::Transaction;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait RpcClientContract: Send + Sync {
    async fn broadcast_tx(&self, tx: &Transaction) -> Result<()>;
    async fn status(&self) -> Result<tendermint_rpc::endpoint::status::Response>;
}


/// A client for connecting to a Tendermint RPC endpoint.
pub struct RpcClient {
    client: HttpClient,
}

impl RpcClient {
    /// Creates a new `RpcClient` connected to the specified `rpc_url`.
    pub fn new(rpc_url: &str) -> Result<Self> {
        let client = HttpClient::new(rpc_url)?;
        Ok(Self { client })
    }
}

#[async_trait]
impl RpcClientContract for RpcClient {
    /// Broadcasts a transaction to the Tendermint network.
    async fn broadcast_tx(&self, tx: &Transaction) -> Result<()> {
        let tx_bytes = bincode::serialize(tx)?;
        self.client.broadcast_tx_async(tx_bytes).await?;
        Ok(())
    }

    /// Checks the status of the connected Tendermint node.
    async fn status(&self) -> Result<tendermint_rpc::endpoint::status::Response> {
        Ok(self.client.status().await?)
    }
}
