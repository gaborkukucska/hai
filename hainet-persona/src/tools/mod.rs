//! # Tools module
//!
//! This module provides external tools that agents can use via the Model Context Protocol (MCP).

pub mod mcp;

pub use mcp::MCPClientManager;

// Re-export common MCP types from rmcp
pub use rmcp::model::{Tool, Resource, Prompt, Content};
