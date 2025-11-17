//! # HAI-Net MCP Server
//!
//! This server acts as a gateway for MCP calls, forwarding them to the appropriate services.

use anyhow::Result;
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::RoleServer;
use serde_json::Value;
use std::borrow::Cow;
use std::future::Future;
use tracing::info;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MCPError {
    #[error("Unknown tool: {0}")]
    UnknownTool(String),
}

/// Service-specific payload types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServicePayload {
    /// MCP tool call request
    MCP {
        server: String,
        tool: String,
        arguments: serde_json::Value,
    },
}

/// HAI-Net MCP Server
#[derive(Clone)]
struct MCPServer;

impl MCPServer {
    fn new() -> Self {
        Self
    }

    async fn handle_request(&self, tool: String, arguments: Value) -> Result<String, MCPError> {
        if tool == "unknown_tool" {
            return Err(MCPError::UnknownTool(tool));
        }
        // Here you would dispatch to the appropriate tool handler
        // For now, we'll just echo the request
        Ok(serde_json::json!({
            "tool": tool,
            "arguments": arguments,
        }).to_string())
    }
}

impl ServerHandler for MCPServer {
    fn list_tools(
        &self,
        _params: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        async move {
            Ok(ListToolsResult {
                tools: vec![],
                next_cursor: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResult, ErrorData>> + Send + '_ {
        async move {
            let tool = request.name.to_string();
            let arguments = Value::Object(request.arguments.unwrap_or_default());

            let result = self.handle_request(tool, arguments).await;

            match result {
                Ok(result_text) => Ok(CallToolResult {
                    content: vec![Annotated::new(
                        RawContent::Text(RawTextContent {
                            text: result_text,
                            meta: None,
                        }),
                        None,
                    )],
                    is_error: None,
                    structured_content: None,
                    meta: None,
                }),
                Err(MCPError::UnknownTool(tool)) => Err(ErrorData {
                    code: ErrorCode::METHOD_NOT_FOUND,
                    message: Cow::Owned(format!("Unknown tool: {}", tool)),
                    data: None,
                }),
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let _guard = hainet_core::logging::initialize_logging("hainet-mcp-server", "debug")?;

    info!("🔧 Starting HAI-Net MCP Server");

    let server = MCPServer::new();

    info!("📡 Starting MCP server on stdio transport...");

    // Run server with stdio transport
    use rmcp::service::ServiceExt;
    let running_service = server.serve(rmcp::transport::io::stdio()).await?;

    running_service.waiting().await?;

    info!("🛑 HAI-Net MCP Server shutting down");
    Ok(())
}
