//! # MCP (Model Context Protocol) module
//!
//! This module provides the client-side implementation of the Model Context Protocol,
//! enabling AI agents to interact with external tool servers.
//!
//! Uses the official `rmcp` SDK for full protocol compliance.

pub mod client;
pub mod config;

pub use client::MCPClientManager;
pub use config::{MCPServersConfig, ServerConfig};
