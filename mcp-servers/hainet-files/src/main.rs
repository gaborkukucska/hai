//! # HAI-Net Files MCP Server - Official SDK Implementation
//!
//! Provides file operation tools integrated with content-addressed storage.
//! Implements the Model Context Protocol (MCP) using the official rmcp SDK.

use anyhow::{Context, Result};
use hainet_core::storage::StorageManager;
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::RoleServer;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// File content response
#[derive(Debug, Serialize, Deserialize)]
struct FileContent {
    content: String,
    hash: String,
    size: usize,
}

/// File write result
#[derive(Debug, Serialize, Deserialize)]
struct WriteResult {
    success: bool,
    path: String,
    hash: String,
    size: usize,
}

/// File list result
#[derive(Debug, Serialize, Deserialize)]
struct FileList {
    path: String,
    entries: Vec<String>,
    count: usize,
}

/// File metadata result
#[derive(Debug, Serialize, Deserialize)]
struct FileMetadata {
    path: String,
    size: u64,
    is_file: bool,
    is_dir: bool,
    readonly: bool,
}

/// HAI-Net Files Server
#[derive(Clone)]
struct FilesServer {
    storage: Arc<RwLock<StorageManager>>,
}

impl FilesServer {
    fn new(storage_path: PathBuf) -> Result<Self> {
        let storage = StorageManager::new(storage_path)?;
        Ok(Self {
            storage: Arc::new(RwLock::new(storage)),
        })
    }

    async fn handle_file_read(&self, path: String) -> Result<String> {
        debug!("Reading file: {}", path);

        // Read file content
        let content = tokio::fs::read_to_string(&path)
            .await
            .context("Failed to read file")?;

        // Store in CAS for deduplication
        let storage = self.storage.read().await;
        let hash = storage
            .store()
            .put(content.as_bytes(), Some(PathBuf::from(&path)))
            .await
            .context("Failed to store in CAS")?;

        let result = FileContent {
            content: content.clone(),
            hash: hash.to_hex(),
            size: content.len(),
        };

        Ok(serde_json::to_string(&result)?)
    }

    async fn handle_file_write(&self, path: String, content: String) -> Result<String> {
        debug!("Writing file: {}", path);

        // Write file
        tokio::fs::write(&path, &content)
            .await
            .context("Failed to write file")?;

        // Store in CAS
        let storage = self.storage.read().await;
        let hash = storage
            .store()
            .put(content.as_bytes(), Some(PathBuf::from(&path)))
            .await
            .context("Failed to store in CAS")?;

        let result = WriteResult {
            success: true,
            path,
            hash: hash.to_hex(),
            size: content.len(),
        };

        Ok(serde_json::to_string(&result)?)
    }

    async fn handle_file_list(&self, path: String) -> Result<String> {
        debug!("Listing directory: {}", path);

        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&path)
            .await
            .context("Failed to read directory")?;

        while let Some(entry) = read_dir.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                entries.push(name.to_string());
            }
        }

        let result = FileList {
            path,
            count: entries.len(),
            entries,
        };

        Ok(serde_json::to_string(&result)?)
    }

    async fn handle_file_metadata(&self, path: String) -> Result<String> {
        debug!("Getting metadata for: {}", path);

        let metadata = tokio::fs::metadata(&path)
            .await
            .context("Failed to get metadata")?;

        let result = FileMetadata {
            path,
            size: metadata.len(),
            is_file: metadata.is_file(),
            is_dir: metadata.is_dir(),
            readonly: metadata.permissions().readonly(),
        };

        Ok(serde_json::to_string(&result)?)
    }
}

impl ServerHandler for FilesServer {
    fn list_tools(
        &self,
        _params: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        async move {
            // Create schema as proper JSON object
            let read_schema = Arc::new({
                let mut map = serde_json::Map::new();
                map.insert("type".to_string(), serde_json::json!("object"));
                let mut props = serde_json::Map::new();
                let mut path_prop = serde_json::Map::new();
                path_prop.insert("type".to_string(), serde_json::json!("string"));
                path_prop.insert("description".to_string(), serde_json::json!("Path to the file"));
                props.insert("path".to_string(), serde_json::Value::Object(path_prop));
                map.insert("properties".to_string(), serde_json::Value::Object(props));
                map.insert("required".to_string(), serde_json::json!(["path"]));
                map
            });

            let write_schema = Arc::new({
                let mut map = serde_json::Map::new();
                map.insert("type".to_string(), serde_json::json!("object"));
                let mut props = serde_json::Map::new();
                let mut path_prop = serde_json::Map::new();
                path_prop.insert("type".to_string(), serde_json::json!("string"));
                props.insert("path".to_string(), serde_json::Value::Object(path_prop));
                let mut content_prop = serde_json::Map::new();
                content_prop.insert("type".to_string(), serde_json::json!("string"));
                props.insert("content".to_string(), serde_json::Value::Object(content_prop));
                map.insert("properties".to_string(), serde_json::Value::Object(props));
                map.insert("required".to_string(), serde_json::json!(["path", "content"]));
                map
            });

            Ok(ListToolsResult {
                tools: vec![
                    Tool {
                        name: Cow::Borrowed("hainet_file_read"),
                        title: Some("Read File".to_string()),
                        description: Some(Cow::Borrowed("Read a file from the local file system")),
                        input_schema: read_schema.clone(),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("hainet_file_write"),
                        title: Some("Write File".to_string()),
                        description: Some(Cow::Borrowed("Write content to a file")),
                        input_schema: write_schema.clone(),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("hainet_file_list"),
                        title: Some("List Files".to_string()),
                        description: Some(Cow::Borrowed("List files in a directory")),
                        input_schema: read_schema.clone(),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("hainet_file_metadata"),
                        title: Some("File Metadata".to_string()),
                        description: Some(Cow::Borrowed("Get file metadata")),
                        input_schema: read_schema,
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
                "hainet_file_read" => {
                    let path = args.get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'path' parameter"),
                            data: None,
                        })?
                        .to_string();
                    self.handle_file_read(path).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("File read error: {}", e)),
                            data: None,
                        })?
                }
                "hainet_file_write" => {
                    let path = args.get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'path' parameter"),
                            data: None,
                        })?
                        .to_string();
                    let content = args.get("content")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'content' parameter"),
                            data: None,
                        })?
                        .to_string();
                    self.handle_file_write(path, content).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("File write error: {}", e)),
                            data: None,
                        })?
                }
                "hainet_file_list" => {
                    let path = args.get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'path' parameter"),
                            data: None,
                        })?
                        .to_string();
                    self.handle_file_list(path).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Directory list error: {}", e)),
                            data: None,
                        })?
                }
                "hainet_file_metadata" => {
                    let path = args.get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'path' parameter"),
                            data: None,
                        })?
                        .to_string();
                    self.handle_file_metadata(path).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Metadata error: {}", e)),
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
    // Create logs directory
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hainet-files");
    let logs_dir = data_dir.join("logs");
    std::fs::create_dir_all(&logs_dir)?;
    
    // Create log file with timestamp
    let log_file = logs_dir.join(format!(
        "hainet-files-{}.log",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    
    // Initialize tracing with file appender
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};
    
    let file_appender = tracing_appender::rolling::never(&logs_dir, log_file.file_name().unwrap());
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);
    
    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(fmt::layer().with_writer(file_writer).with_ansi(false))
        .with(EnvFilter::new("hainet_files=debug,rmcp=info"))
        .init();

    info!("🗂️  Starting HAI-Net Files MCP Server (rmcp SDK)");
    info!("📝 Logs being written to: {}", log_file.display());

    // Initialize storage (use temp directory for now)
    let storage_path = std::env::temp_dir().join("hainet-files-cas");
    let server = FilesServer::new(storage_path)?;

    info!("📡 Starting MCP server on stdio transport...");

    // Run the server with stdio transport
    use rmcp::service::ServiceExt;
    let running_service = server.serve(rmcp::transport::io::stdio()).await?;
    
    // Keep the service running until it's terminated
    running_service.waiting().await?;

    info!("🛑 HAI-Net Files MCP Server shutting down");
    Ok(())
}
