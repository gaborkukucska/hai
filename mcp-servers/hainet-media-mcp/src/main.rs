//! # HAI-Net Media MCP Server - Official SDK Implementation
//!
//! Provides ComfyUI and FFmpeg tools.
//! Implements the Model Context Protocol (MCP) using the official rmcp SDK.

use anyhow::Result;
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::RoleServer;
use std::borrow::Cow;
use std::future::Future;
use std::sync::Arc;
use tracing::info;

pub mod comfyui;
pub mod ffmpeg;

use comfyui::ComfyUIHandler;
use ffmpeg::FFmpegHandler;

/// HAI-Net Media Server
#[derive(Clone)]
pub(crate) struct MediaServer {
    comfyui: ComfyUIHandler,
    ffmpeg: FFmpegHandler,
}

impl MediaServer {
    pub(crate) fn new(shared_drive_path: &str) -> Result<Self> {
        info!("🎬 Initializing Media MCP Server (Shared Drive: {})", shared_drive_path);
        
        let comfyui = ComfyUIHandler::new(shared_drive_path);
        let ffmpeg = FFmpegHandler::new(shared_drive_path);
        
        Ok(Self {
            comfyui,
            ffmpeg,
        })
    }
}

impl ServerHandler for MediaServer {
    fn list_tools(
        &self,
        _params: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        async move {
            let generate_schema = Arc::new({
                let mut map = serde_json::Map::new();
                map.insert("type".to_string(), serde_json::json!("object"));
                let mut props = serde_json::Map::new();
                props.insert("prompt".to_string(), serde_json::json!({ "type": "string", "description": "The image description" }));
                map.insert("properties".to_string(), serde_json::Value::Object(props));
                map.insert("required".to_string(), serde_json::json!(["prompt"]));
                map
            });

            let convert_schema = Arc::new({
                let mut map = serde_json::Map::new();
                map.insert("type".to_string(), serde_json::json!("object"));
                let mut props = serde_json::Map::new();
                props.insert("input_path".to_string(), serde_json::json!({ "type": "string" }));
                props.insert("output_path".to_string(), serde_json::json!({ "type": "string" }));
                map.insert("properties".to_string(), serde_json::Value::Object(props));
                map.insert("required".to_string(), serde_json::json!(["input_path", "output_path"]));
                map
            });

            Ok(ListToolsResult {
                tools: vec![
                    Tool {
                        name: Cow::Borrowed("comfyui_generate"),
                        title: Some("ComfyUI Image Generation".to_string()),
                        description: Some(Cow::Borrowed("Generate an image using ComfyUI")),
                        input_schema: generate_schema,
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("video_convert"),
                        title: Some("FFmpeg Video Convert".to_string()),
                        description: Some(Cow::Borrowed("Convert video files using FFmpeg")),
                        input_schema: convert_schema,
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                ],
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
            let args = request.arguments.unwrap_or_else(|| serde_json::Map::new());

            let result_text = match request.name.as_ref() {
                "comfyui_generate" => {
                    let prompt = args.get("prompt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("A beautiful landscape");
                        
                    self.comfyui.generate_image(prompt).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("ComfyUI generation failed: {}", e)),
                            data: None,
                        })?
                }
                "video_convert" => {
                    let input_path = args.get("input_path").and_then(|v| v.as_str()).unwrap_or("");
                    let output_path = args.get("output_path").and_then(|v| v.as_str()).unwrap_or("");
                    
                    self.ffmpeg.convert_video(input_path, output_path).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("FFmpeg conversion failed: {}", e)),
                            data: None,
                        })?
                }
                _ => {
                    return Err(ErrorData {
                        code: ErrorCode::METHOD_NOT_FOUND,
                        message: Cow::Owned(format!("Unknown tool: {}", request.name)),
                        data: None,
                    });
                }
            };

            Ok(CallToolResult {
                content: vec![Annotated::new(
                    RawContent::Text(RawTextContent {
                        text: result_text,
                        meta: None,
                    }),
                    None
                )],
                is_error: None,
                structured_content: None,
                meta: None,
            })
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = hainet_core::logging::initialize_logging("hainet-media-mcp", "debug")?;

    info!("🎬 Starting HAI-Net Media MCP Server (rmcp SDK)");
    
    let shared_drive_path = std::env::var("HAINET_SHARED_DRIVE")
        .unwrap_or_else(|_| "/media/hai-drive".to_string());
    
    let server = MediaServer::new(&shared_drive_path)?;

    info!("📡 Starting MCP server on stdio transport...");

    use rmcp::service::ServiceExt;
    let running_service = server.serve(rmcp::transport::io::stdio()).await?;

    running_service.waiting().await?;

    info!("🛑 HAI-Net Media MCP Server shutting down");
    Ok(())
}
