//! # START OF FILE hainet-social/src/lib.rs
//! HAI-Net Social Mesh — Privacy-first decentralized social networking
//!
//! Ported from gChat v1.5.0 (Node.js/TypeScript) to native Rust.
//! Provides: gossip protocol, E2E encryption, media transport, social features.

pub mod packets;
pub mod identity;
pub mod crypto;
pub mod gossip;
pub mod firewall;
pub mod dedup;

pub mod downloads;
pub mod congestion;
pub mod relay;
pub mod recovery;
pub mod feed;
pub mod interactions;
pub mod groups;
pub mod messaging;

// Future modules (Phase 4 full port):
// pub mod sync;
// pub mod presence;

use thiserror::Error;

/// Social mesh errors
#[derive(Error, Debug)]
pub enum SocialError {
    #[error("Cryptographic operation failed: {0}")]
    CryptoError(String),

    #[error("Invalid packet: {0}")]
    InvalidPacket(String),

    #[error("Untrusted peer: {0}")]
    UntrustedPeer(String),

    #[error("Duplicate packet: {0}")]
    DuplicatePacket(String),

    #[error("Media transfer error: {0}")]
    MediaTransfer(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type SocialResult<T> = Result<T, SocialError>;
