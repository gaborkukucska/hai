//! # START OF FILE hainet-collab/src/lib.rs
//! HAI-Net Collab — Decentralized compute sharing (ported from PPLPWR)

pub mod hardware;
pub mod idle;
pub mod scheduler;
pub mod policy;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CollabError {
    #[error("Hardware detection failed: {0}")]
    HardwareDetection(String),
    #[error("Scheduler error: {0}")]
    Scheduler(String),
    #[error("Policy violation: {0}")]
    PolicyViolation(String),
}

pub type CollabResult<T> = Result<T, CollabError>;
