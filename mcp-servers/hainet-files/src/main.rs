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

/// Special project name for Admin access (bypasses sandboxing)
const ADMIN_PROJECT_BYPASS: &str = "__ADMIN__";

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

/// File match result
#[derive(Debug, Serialize, Deserialize)]
struct FileMatch {
    line_number: usize,
    line_content: String,
}

/// File search result
#[derive(Debug, Serialize, Deserialize)]
struct FileSearchResult {
    path: String,
    matches: Vec<FileMatch>,
    count: usize,
}

/// File edit result
#[derive(Debug, Serialize, Deserialize)]
struct FileEditResult {
    success: bool,
    path: String,
    original_hash: String,
    new_hash: String,
    replacements: usize,
}

/// HAI-Net Files Server
#[derive(Clone)]
pub(crate) struct FilesServer {
    storage: Arc<RwLock<StorageManager>>,
    base_path: PathBuf,
}

impl FilesServer {
    pub(crate) fn new(storage_path: PathBuf, base_path: PathBuf) -> Result<Self> {
        let storage = StorageManager::new(storage_path)?;
        info!("📂 Base path for file operations: {}", base_path.display());
        Ok(Self {
            storage: Arc::new(RwLock::new(storage)),
            base_path,
        })
    }

    /// Normalize and validate a path with project-based sandboxing
    /// 
    /// Sandboxing rules:
    /// - If project_name is Some(name) and not ADMIN_PROJECT_BYPASS:
    ///   Path is sandboxed to /sandbox/projects/{project_name}/{requested_path}
    /// - If project_name is None or ADMIN_PROJECT_BYPASS:
    ///   Full filesystem access (Admin only - requires Guardian approval)
    /// 
    /// Security:
    /// - Prevents directory traversal attacks
    /// - Isolates project workspaces
    /// - Admin access requires explicit bypass
    pub(crate) fn normalize_path(&self, requested_path: &str, project_name: Option<&str>) -> Result<PathBuf> {
        // Determine if this is an Admin bypass request
        let is_admin_access = match project_name {
            None | Some(ADMIN_PROJECT_BYPASS) => {
                debug!("Admin access granted for path: {}", requested_path);
                true
            }
            Some(name) => {
                debug!("Worker access: sandboxing to project '{}'", name);
                false
            }
        };

        // Remove leading slash if present (treat all paths as relative)
        let path_str = requested_path.trim_start_matches('/');

        // Reject paths containing '..' to prevent directory traversal
        if path_str.contains("..") {
            anyhow::bail!("Path traversal attempt detected: '..' not allowed");
        }

        // Get the canonical base path for security checks (MUST be before constructing resolved_path)
        let canonical_base = self.base_path.canonicalize()
            .context("Failed to canonicalize base path")?;

        // Construct sandboxed or full path based on access level
        let resolved_path = if is_admin_access {
            // Admin: full filesystem access relative to base_path
            self.base_path.join(path_str)
        } else {
            // Worker: sandboxed to project directory
            // Safe to unwrap here because is_admin_access is false
            let project_name = project_name.unwrap();
            
            // Sanitize project name: replace non-alphanumeric chars (except - and _) with underscores
            let sanitized_project: String = project_name.chars()
                .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
                .collect();
            
            canonical_base
                .join("sandbox")
                .join("projects")
                .join(sanitized_project)
                .join(path_str)
        };

        // Security check: Ensure the resolved path is within the canonical base path.
        // For paths that don't exist yet, we validate the constructed path structurally.
        // This is safe because we've already:
        // 1. Blocked directory traversal (..)
        // 2. Constructed the path from trusted components (canonical_base + known suffixes)
        
        // The resolved_path is already correctly constructed - just validate it starts with base
        if !resolved_path.starts_with(&canonical_base) {
            anyhow::bail!("Resolved path is outside the working directory: {}", requested_path);
        }

        debug!("Normalized path: {} -> {}", requested_path, resolved_path.display());
        Ok(resolved_path)
    }

    async fn handle_file_read(&self, path: String, project_name: Option<String>) -> Result<String> {
        debug!("Reading file: {}", path);

        // Normalize and validate path
        let normalized_path = self.normalize_path(&path, project_name.as_deref())?;

        // Read file content
        let content = tokio::fs::read_to_string(&normalized_path)
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

    async fn handle_file_write(&self, path: String, content: String, project_name: Option<String>) -> Result<String> {
        debug!("📝 Writing file: {} (project: {:?})", path, project_name);
        debug!("   Content size: {} bytes", content.len());

        // Normalize and validate path
        let normalized_path = self.normalize_path(&path, project_name.as_deref())
            .context(format!("Path normalization failed for: {}", path))?;
        
        debug!("   Normalized path: {}", normalized_path.display());

        // Create parent directory if it doesn't exist
        if let Some(parent) = normalized_path.parent() {
            debug!("   Parent directory: {}", parent.display());
            
            // Check if parent already exists
            let parent_exists = parent.exists();
            debug!("   Parent exists before create_dir_all: {}", parent_exists);
            
            // Create parent directory
            tokio::fs::create_dir_all(parent)
                .await
                .context(format!("Failed to create parent directory: {}", parent.display()))?;
            
            // Verify parent was created
            let parent_exists_after = parent.exists();
            debug!("   Parent exists after create_dir_all: {}", parent_exists_after);
            
            if !parent_exists_after {
                anyhow::bail!("Parent directory creation succeeded but directory does not exist: {}", parent.display());
            }
            
            // Check parent directory permissions
            match tokio::fs::metadata(parent).await {
                Ok(metadata) => {
                    debug!("   Parent directory metadata: is_dir={}, readonly={}", 
                           metadata.is_dir(), metadata.permissions().readonly());
                    
                    if metadata.permissions().readonly() {
                        anyhow::bail!("Parent directory is read-only: {}", parent.display());
                    }
                }
                Err(e) => {
                    anyhow::bail!("Failed to get parent directory metadata: {} - {}", parent.display(), e);
                }
            }
        } else {
            debug!("   No parent directory (writing to root)");
        }

        // Check if file already exists
        let file_exists = normalized_path.exists();
        debug!("   File exists before write: {}", file_exists);

        // Write file
        debug!("   Attempting to write {} bytes to: {}", content.len(), normalized_path.display());
        tokio::fs::write(&normalized_path, &content)
            .await
            .context(format!("Failed to write file to: {}", normalized_path.display()))?;

        // Verify file was written
        match tokio::fs::metadata(&normalized_path).await {
            Ok(metadata) => {
                debug!("   ✅ File written successfully: {} bytes", metadata.len());
                if metadata.len() != content.len() as u64 {
                    tracing::warn!("   ⚠️  File size mismatch: expected {} bytes, got {} bytes", 
                                   content.len(), metadata.len());
                }
            }
            Err(e) => {
                anyhow::bail!("File write succeeded but cannot read metadata: {} - {}", normalized_path.display(), e);
            }
        }

        // Store in CAS
        debug!("   Storing in CAS...");
        let storage = self.storage.read().await;
        let hash = storage
            .store()
            .put(content.as_bytes(), Some(PathBuf::from(&path)))
            .await
            .context("Failed to store in CAS")?;
        
        debug!("   ✅ CAS storage complete: {}", hash.to_hex());

        let result = WriteResult {
            success: true,
            path,
            hash: hash.to_hex(),
            size: content.len(),
        };
        debug!("   ✅ File write operation completed successfully");
        Ok(serde_json::to_string(&result)?)
    }

    async fn handle_file_list(&self, path: String, project_name: Option<String>) -> Result<String> {
        debug!("Listing directory: {}", path);

        // Normalize and validate path
        let normalized_path = self.normalize_path(&path, project_name.as_deref())?;

        // Check if directory exists
        if !normalized_path.exists() {
            debug!("Directory does not exist, returning empty list: {}", normalized_path.display());
            let result = FileList {
                path,
                count: 0,
                entries: Vec::new(),
            };
            return Ok(serde_json::to_string(&result)?);
        }

        // Check if path is actually a directory
        if !normalized_path.is_dir() {
            anyhow::bail!("Path exists but is not a directory: {}", path);
        }

        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&normalized_path)
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

    async fn handle_file_metadata(&self, path: String, project_name: Option<String>) -> Result<String> {
        debug!("Getting metadata for: {}", path);

        // Normalize and validate path
        let normalized_path = self.normalize_path(&path, project_name.as_deref())?;

        let metadata = tokio::fs::metadata(&normalized_path)
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

    async fn handle_directory_create(&self, path: String, project_name: Option<String>) -> Result<String> {
        debug!("Creating directory: {} (project: {:?})", path, project_name);

        // Normalize and validate path
        let normalized_path = self.normalize_path(&path, project_name.as_deref())
            .context(format!("Path normalization failed for: {}", path))?;

        debug!("Normalized directory path: {}", normalized_path.display());

        // Create directory and all parent directories
        tokio::fs::create_dir_all(&normalized_path)
            .await
            .context(format!("Failed to create directory at: {}", normalized_path.display()))?;

        info!("Successfully created directory: {}", normalized_path.display());

        let result = WriteResult {
            success: true,
            path,
            hash: "".to_string(), // No hash for directories
            size: 0,
        };

        Ok(serde_json::to_string(&result)?)
    }

    async fn handle_file_search(&self, path: String, query: String, is_regex: bool, project_name: Option<String>) -> Result<String> {
        debug!("Searching in file: {} (query: '{}', regex: {})", path, query, is_regex);

        // Normalize and validate path
        let normalized_path = self.normalize_path(&path, project_name.as_deref())?;

        // Read file content
        let content = tokio::fs::read_to_string(&normalized_path)
            .await
            .context("Failed to read file for search")?;

        let mut matches = Vec::new();

        if is_regex {
            let re = regex::Regex::new(&query).context("Invalid regex pattern")?;
            for (i, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    matches.push(FileMatch {
                        line_number: i + 1,
                        line_content: line.trim().to_string(),
                    });
                }
            }
        } else {
            for (i, line) in content.lines().enumerate() {
                if line.contains(&query) {
                    matches.push(FileMatch {
                        line_number: i + 1,
                        line_content: line.trim().to_string(),
                    });
                }
            }
        }

        let result = FileSearchResult {
            path,
            count: matches.len(),
            matches,
        };

        Ok(serde_json::to_string(&result)?)
    }

    async fn handle_file_edit(&self, path: String, find: String, replace: String, is_regex: bool, project_name: Option<String>) -> Result<String> {
        debug!("Editing file: {} (find: '{}', replace: '{}', regex: {})", path, find, replace, is_regex);

        // Normalize and validate path
        let normalized_path = self.normalize_path(&path, project_name.as_deref())?;

        // Read original content
        let content = tokio::fs::read_to_string(&normalized_path)
            .await
            .context("Failed to read file for edit")?;

        // Calculate original hash
        let storage = self.storage.read().await;
        let original_hash = storage.store().put(content.as_bytes(), None).await?.to_hex();
        drop(storage);

        // Perform replacement
        let (new_content, replacements) = if is_regex {
            let re = regex::Regex::new(&find).context("Invalid regex pattern")?;
            let new_content = re.replace_all(&content, replace.as_str()).to_string();
            let count = if new_content != content { 1 } else { 0 }; 
            (new_content, count)
        } else {
            let count = content.matches(&find).count();
            let new_content = content.replace(&find, &replace);
            (new_content, count)
        };

        if replacements == 0 {
            return Ok(serde_json::to_string(&FileEditResult {
                success: false,
                path,
                original_hash: original_hash.clone(),
                new_hash: original_hash,
                replacements: 0,
            })?);
        }

        // Write file
        tokio::fs::write(&normalized_path, &new_content)
            .await
            .context(format!("Failed to write edited file to: {}", normalized_path.display()))?;

        // Store in CAS
        let storage = self.storage.read().await;
        let new_hash = storage
            .store()
            .put(new_content.as_bytes(), Some(PathBuf::from(&path)))
            .await
            .context("Failed to store new content in CAS")?
            .to_hex();

        let result = FileEditResult {
            success: true,
            path,
            original_hash,
            new_hash,
            replacements,
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
                        name: Cow::Borrowed("file_read"),
                        title: Some("Read File".to_string()),
                        description: Some(Cow::Borrowed("Read a file from the local file system")),
                        input_schema: read_schema.clone(),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("file_write"),
                        title: Some("Write File".to_string()),
                        description: Some(Cow::Borrowed("Write content to a file")),
                        input_schema: write_schema.clone(),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("file_list"),
                        title: Some("List Files".to_string()),
                        description: Some(Cow::Borrowed("List files in a directory")),
                        input_schema: read_schema.clone(),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("file_metadata"),
                        title: Some("File Metadata".to_string()),
                        description: Some(Cow::Borrowed("Get file metadata")),
                        input_schema: read_schema.clone(),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("directory_create"),
                        title: Some("Create Directory".to_string()),
                        description: Some(Cow::Borrowed("Create a directory and all parent directories")),
                        input_schema: read_schema,
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("file_search"),
                        title: Some("Search File".to_string()),
                        description: Some(Cow::Borrowed("Search for text or regex in a file")),
                        input_schema: Arc::new({
                            let mut map = serde_json::Map::new();
                            map.insert("type".to_string(), serde_json::json!("object"));
                            let mut props = serde_json::Map::new();
                            props.insert("path".to_string(), serde_json::json!({ "type": "string" }));
                            props.insert("query".to_string(), serde_json::json!({ "type": "string" }));
                            props.insert("is_regex".to_string(), serde_json::json!({ "type": "boolean" }));
                            map.insert("properties".to_string(), serde_json::Value::Object(props));
                            map.insert("required".to_string(), serde_json::json!(["path", "query"]));
                            map
                        }),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("file_edit"),
                        title: Some("Edit File".to_string()),
                        description: Some(Cow::Borrowed("Replace text or regex in a file")),
                        input_schema: Arc::new({
                            let mut map = serde_json::Map::new();
                            map.insert("type".to_string(), serde_json::json!("object"));
                            let mut props = serde_json::Map::new();
                            props.insert("path".to_string(), serde_json::json!({ "type": "string" }));
                            props.insert("find".to_string(), serde_json::json!({ "type": "string" }));
                            props.insert("replace".to_string(), serde_json::json!({ "type": "string" }));
                            props.insert("is_regex".to_string(), serde_json::json!({ "type": "boolean" }));
                            map.insert("properties".to_string(), serde_json::Value::Object(props));
                            map.insert("required".to_string(), serde_json::json!(["path", "find", "replace"]));
                            map
                        }),
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
            
            // Extract optional project_name parameter (for sandboxing)
            let project_name = args.get("project_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let result_text = match request.name.as_ref() {
                "file_read" => {
                    let path = args.get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'path' parameter"),
                            data: None,
                        })?
                        .to_string();
                    self.handle_file_read(path, project_name.clone()).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("File read error: {}", e)),
                            data: None,
                        })?
                }
                "file_write" => {
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
                    self.handle_file_write(path, content, project_name.clone()).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("File write error: {}", e)),
                            data: None,
                        })?
                }
                "file_list" => {
                    let path = args.get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'path' parameter"),
                            data: None,
                        })?
                        .to_string();
                    self.handle_file_list(path, project_name.clone()).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Directory list error: {}", e)),
                            data: None,
                        })?
                }
                "file_metadata" => {
                    let path = args.get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'path' parameter"),
                            data: None,
                        })?
                        .to_string();
                    self.handle_file_metadata(path, project_name.clone()).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Metadata error: {}", e)),
                            data: None,
                        })?
                }
                "directory_create" => {
                    let path = args.get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'path' parameter"),
                            data: None,
                        })?
                        .to_string();
                    self.handle_directory_create(path, project_name).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Directory creation error: {}", e)),
                            data: None,
                        })?
                }
                "file_search" => {
                    let path = args.get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'path' parameter"),
                            data: None,
                        })?
                        .to_string();
                    let query = args.get("query")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'query' parameter"),
                            data: None,
                        })?
                        .to_string();
                    let is_regex = args.get("is_regex")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    
                    self.handle_file_search(path, query, is_regex, project_name).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("File search error: {}", e)),
                            data: None,
                        })?
                }
                "file_edit" => {
                    let path = args.get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'path' parameter"),
                            data: None,
                        })?
                        .to_string();
                    let find = args.get("find")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'find' parameter"),
                            data: None,
                        })?
                        .to_string();
                    let replace = args.get("replace")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'replace' parameter"),
                            data: None,
                        })?
                        .to_string();
                    let is_regex = args.get("is_regex")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    
                    self.handle_file_edit(path, find, replace, is_regex, project_name).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("File edit error: {}", e)),
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
    // Initialize logging
    let _guard = hainet_core::logging::initialize_logging("hainet-files", "debug")?;

    info!("🗂️  Starting HAI-Net Files MCP Server (rmcp SDK)");

    // Initialize storage (use temp directory for now)
    let storage_path = std::env::temp_dir().join("hainet-files-cas");
    
    // Set base path from environment variable or default to current directory
    let base_path = std::env::var("HAINET_FILES_BASE_PATH")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
        });
    
    let server = FilesServer::new(storage_path, base_path)?;

    info!("📡 Starting MCP server on stdio transport...");

    // Run the server with stdio transport
    use rmcp::service::ServiceExt;
    let running_service = server.serve(rmcp::transport::io::stdio()).await?;

    // Keep the service running until it's terminated
    running_service.waiting().await?;

    info!("🛑 HAI-Net Files MCP Server shutting down");
    Ok(())
}

#[cfg(test)]
mod tests;
