//! # HAI-Net Development Tools MCP Server
//!
//! Provides development tools for Worker AI agents.
//! Tools enable git operations, cargo builds, code search, and file reading.

use anyhow::{Context, Result};
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::RoleServer;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::future::Future;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tracing::{debug, info};

/// Git status result
#[derive(Debug, Serialize, Deserialize)]
struct GitStatus {
    repo_path: String,
    branch: String,
    status_lines: Vec<String>,
    is_clean: bool,
    modified_files: Vec<String>,
    untracked_files: Vec<String>,
}

/// Git diff result
#[derive(Debug, Serialize, Deserialize)]
struct GitDiff {
    file_path: String,
    diff_output: String,
}

/// Git commit result
#[derive(Debug, Serialize, Deserialize)]
struct GitCommit {
    success: bool,
    commit_hash: Option<String>,
    message: String,
    error: Option<String>,
}

/// Cargo build result
#[derive(Debug, Serialize, Deserialize)]
struct CargoBuild {
    success: bool,
    package: Option<String>,
    output: String,
    warnings: usize,
    errors: usize,
}

/// Cargo test result
#[derive(Debug, Serialize, Deserialize)]
struct CargoTest {
    success: bool,
    package: Option<String>,
    filter: Option<String>,
    output: String,
    passed: usize,
    failed: usize,
}

/// Code search result
#[derive(Debug, Serialize, Deserialize)]
struct CodeSearch {
    pattern: String,
    search_path: String,
    matches: Vec<SearchMatch>,
    total_matches: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct SearchMatch {
    file: String,
    line_number: usize,
    line_content: String,
}

/// File lines result
#[derive(Debug, Serialize, Deserialize)]
struct FileLines {
    file_path: String,
    start_line: usize,
    end_line: usize,
    lines: Vec<String>,
    total_lines: usize,
}

/// HAI-Net Development Tools Server
#[derive(Clone)]
struct DevServer;

impl DevServer {
    fn new() -> Self {
        Self
    }

    async fn handle_git_status(&self, repo_path: String) -> Result<String> {
        debug!("Getting git status for: {}", repo_path);

        // Validate repo path exists
        if !Path::new(&repo_path).exists() {
            return Ok(serde_json::json!({
                "error": format!("Repository path does not exist: {}", repo_path)
            }).to_string());
        }

        // Get current branch
        let branch_output = Command::new("git")
            .args(&["-C", &repo_path, "branch", "--show-current"])
            .output()
            .context("Failed to get git branch")?;

        let branch = String::from_utf8_lossy(&branch_output.stdout)
            .trim()
            .to_string();

        // Get status
        let status_output = Command::new("git")
            .args(&["-C", &repo_path, "status", "--porcelain"])
            .output()
            .context("Failed to get git status")?;

        let status_text = String::from_utf8_lossy(&status_output.stdout);
        let status_lines: Vec<String> = status_text
            .lines()
            .map(|s| s.to_string())
            .collect();

        let modified_files: Vec<String> = status_lines
            .iter()
            .filter(|l| l.starts_with(" M") || l.starts_with("M "))
            .map(|l| l[2..].trim().to_string())
            .collect();

        let untracked_files: Vec<String> = status_lines
            .iter()
            .filter(|l| l.starts_with("??"))
            .map(|l| l[2..].trim().to_string())
            .collect();

        let result = GitStatus {
            repo_path,
            branch,
            is_clean: status_lines.is_empty(),
            status_lines,
            modified_files,
            untracked_files,
        };

        Ok(serde_json::to_string_pretty(&result)?)
    }

    async fn handle_git_diff(&self, repo_path: String, file_path: Option<String>) -> Result<String> {
        debug!("Getting git diff for: {:?}", file_path);

        let mut args = vec!["-C", &repo_path, "diff"];
        if let Some(ref path) = file_path {
            args.push("--");
            args.push(path);
        }

        let output = Command::new("git")
            .args(&args)
            .output()
            .context("Failed to get git diff")?;

        let diff_output = String::from_utf8_lossy(&output.stdout).to_string();

        let result = GitDiff {
            file_path: file_path.unwrap_or_else(|| "all files".to_string()),
            diff_output,
        };

        Ok(serde_json::to_string_pretty(&result)?)
    }

    async fn handle_git_commit(&self, repo_path: String, message: String) -> Result<String> {
        info!("Committing changes with message: {}", message);

        // Add all changes
        let add_output = Command::new("git")
            .args(&["-C", &repo_path, "add", "-A"])
            .output()
            .context("Failed to stage changes")?;

        if !add_output.status.success() {
            let result = GitCommit {
                success: false,
                commit_hash: None,
                message: "Failed to stage changes".to_string(),
                error: Some(String::from_utf8_lossy(&add_output.stderr).to_string()),
            };
            return Ok(serde_json::to_string_pretty(&result)?);
        }

        // Commit
        let commit_output = Command::new("git")
            .args(&["-C", &repo_path, "commit", "-m", &message])
            .output()
            .context("Failed to commit")?;

        if !commit_output.status.success() {
            let result = GitCommit {
                success: false,
                commit_hash: None,
                message: "Commit failed".to_string(),
                error: Some(String::from_utf8_lossy(&commit_output.stderr).to_string()),
            };
            return Ok(serde_json::to_string_pretty(&result)?);
        }

        // Get commit hash
        let hash_output = Command::new("git")
            .args(&["-C", &repo_path, "rev-parse", "HEAD"])
            .output()
            .context("Failed to get commit hash")?;

        let commit_hash = String::from_utf8_lossy(&hash_output.stdout)
            .trim()
            .to_string();

        let result = GitCommit {
            success: true,
            commit_hash: Some(commit_hash),
            message: "Changes committed successfully".to_string(),
            error: None,
        };

        Ok(serde_json::to_string_pretty(&result)?)
    }

    async fn handle_cargo_build(&self, package: Option<String>, release: bool) -> Result<String> {
        info!("Building cargo package: {:?}", package);

        let mut args = vec!["build"];
        if release {
            args.push("--release");
        }
        if let Some(ref pkg) = package {
            args.push("--package");
            args.push(pkg);
        }

        let output = Command::new("cargo")
            .args(&args)
            .output()
            .context("Failed to execute cargo build")?;

        let output_text = String::from_utf8_lossy(&output.stderr).to_string();
        let warnings = output_text.matches("warning:").count();
        let errors = output_text.matches("error:").count();

        let result = CargoBuild {
            success: output.status.success(),
            package,
            output: output_text,
            warnings,
            errors,
        };

        Ok(serde_json::to_string_pretty(&result)?)
    }

    async fn handle_cargo_test(&self, package: Option<String>, filter: Option<String>) -> Result<String> {
        info!("Running cargo tests: package={:?}, filter={:?}", package, filter);

        let mut args = vec!["test"];
        if let Some(ref pkg) = package {
            args.push("--package");
            args.push(pkg);
        }
        if let Some(ref flt) = filter {
            args.push(flt);
        }

        let output = Command::new("cargo")
            .args(&args)
            .output()
            .context("Failed to execute cargo test")?;

        let output_text = String::from_utf8_lossy(&output.stdout).to_string();
        let passed = output_text.matches("test result: ok.").count();
        let failed = output_text.matches("test result: FAILED.").count();

        let result = CargoTest {
            success: output.status.success(),
            package,
            filter,
            output: output_text,
            passed,
            failed,
        };

        Ok(serde_json::to_string_pretty(&result)?)
    }

    async fn handle_code_search(&self, pattern: String, search_path: String) -> Result<String> {
        debug!("Searching code for pattern: {}", pattern);

        // Try ripgrep first, fallback to grep
        let use_rg = Command::new("rg").arg("--version").output().is_ok();

        let output = if use_rg {
            Command::new("rg")
                .args(&["-n", "--color", "never", &pattern, &search_path])
                .output()
                .context("Failed to execute ripgrep")?
        } else {
            Command::new("grep")
                .args(&["-rn", &pattern, &search_path])
                .output()
                .context("Failed to execute grep")?
        };

        let output_text = String::from_utf8_lossy(&output.stdout);
        let matches: Vec<SearchMatch> = output_text
            .lines()
            .filter_map(|line| {
                // Parse format: file:line:content
                let parts: Vec<&str> = line.splitn(3, ':').collect();
                if parts.len() == 3 {
                    Some(SearchMatch {
                        file: parts[0].to_string(),
                        line_number: parts[1].parse().unwrap_or(0),
                        line_content: parts[2].to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();

        let result = CodeSearch {
            pattern,
            search_path,
            total_matches: matches.len(),
            matches,
        };

        Ok(serde_json::to_string_pretty(&result)?)
    }

    async fn handle_read_file_lines(
        &self,
        file_path: String,
        start_line: usize,
        end_line: Option<usize>,
    ) -> Result<String> {
        debug!("Reading file lines: {} ({}-{:?})", file_path, start_line, end_line);

        let content = std::fs::read_to_string(&file_path)
            .context("Failed to read file")?;

        let all_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let total_lines = all_lines.len();

        let end = end_line.unwrap_or(start_line).min(total_lines);
        let start = start_line.saturating_sub(1); // Convert to 0-based index

        let lines = if start < total_lines {
            all_lines[start..end].to_vec()
        } else {
            vec![]
        };

        let result = FileLines {
            file_path,
            start_line,
            end_line: end,
            lines,
            total_lines,
        };

        Ok(serde_json::to_string_pretty(&result)?)
    }
}

impl ServerHandler for DevServer {
    fn list_tools(
        &self,
        _params: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        async move {
            Ok(ListToolsResult {
                tools: vec![
                    Tool {
                        name: Cow::Borrowed("git_status"),
                        title: Some("Git Status".to_string()),
                        description: Some(Cow::Borrowed("Get git repository status with modified/untracked files")),
                        input_schema: Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "repo_path": {
                                    "type": "string",
                                    "description": "Path to git repository"
                                }
                            },
                            "required": ["repo_path"]
                        }).as_object().unwrap().clone()),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("git_diff"),
                        title: Some("Git Diff".to_string()),
                        description: Some(Cow::Borrowed("View git diff for file or entire repository")),
                        input_schema: Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "repo_path": {
                                    "type": "string",
                                    "description": "Path to git repository"
                                },
                                "file_path": {
                                    "type": "string",
                                    "description": "Optional: specific file to diff"
                                }
                            },
                            "required": ["repo_path"]
                        }).as_object().unwrap().clone()),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("git_commit"),
                        title: Some("Git Commit".to_string()),
                        description: Some(Cow::Borrowed("Stage and commit all changes with message")),
                        input_schema: Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "repo_path": {
                                    "type": "string",
                                    "description": "Path to git repository"
                                },
                                "message": {
                                    "type": "string",
                                    "description": "Commit message"
                                }
                            },
                            "required": ["repo_path", "message"]
                        }).as_object().unwrap().clone()),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("cargo_build"),
                        title: Some("Cargo Build".to_string()),
                        description: Some(Cow::Borrowed("Build Rust package with cargo")),
                        input_schema: Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "package": {
                                    "type": "string",
                                    "description": "Optional: specific package to build"
                                },
                                "release": {
                                    "type": "boolean",
                                    "description": "Build in release mode",
                                    "default": false
                                }
                            },
                            "required": []
                        }).as_object().unwrap().clone()),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("cargo_test"),
                        title: Some("Cargo Test".to_string()),
                        description: Some(Cow::Borrowed("Run cargo tests with optional filter")),
                        input_schema: Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "package": {
                                    "type": "string",
                                    "description": "Optional: specific package to test"
                                },
                                "filter": {
                                    "type": "string",
                                    "description": "Optional: test name filter"
                                }
                            },
                            "required": []
                        }).as_object().unwrap().clone()),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("code_search"),
                        title: Some("Code Search".to_string()),
                        description: Some(Cow::Borrowed("Search codebase for pattern using ripgrep/grep")),
                        input_schema: Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "pattern": {
                                    "type": "string",
                                    "description": "Search pattern (regex supported)"
                                },
                                "search_path": {
                                    "type": "string",
                                    "description": "Directory to search in"
                                }
                            },
                            "required": ["pattern", "search_path"]
                        }).as_object().unwrap().clone()),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("read_file_lines"),
                        title: Some("Read File Lines".to_string()),
                        description: Some(Cow::Borrowed("Read specific line range from a file")),
                        input_schema: Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "file_path": {
                                    "type": "string",
                                    "description": "Path to file"
                                },
                                "start_line": {
                                    "type": "integer",
                                    "description": "Starting line number (1-based)",
                                    "minimum": 1
                                },
                                "end_line": {
                                    "type": "integer",
                                    "description": "Optional: ending line number"
                                }
                            },
                            "required": ["file_path", "start_line"]
                        }).as_object().unwrap().clone()),
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
                "git_status" => {
                    let repo_path = args.get("repo_path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'repo_path' parameter"),
                            data: None,
                        })?
                        .to_string();
                    
                    self.handle_git_status(repo_path).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Git status error: {}", e)),
                            data: None,
                        })?
                }
                "git_diff" => {
                    let repo_path = args.get("repo_path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'repo_path' parameter"),
                            data: None,
                        })?
                        .to_string();
                    
                    let file_path = args.get("file_path")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    
                    self.handle_git_diff(repo_path, file_path).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Git diff error: {}", e)),
                            data: None,
                        })?
                }
                "git_commit" => {
                    let repo_path = args.get("repo_path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'repo_path' parameter"),
                            data: None,
                        })?
                        .to_string();
                    
                    let message = args.get("message")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'message' parameter"),
                            data: None,
                        })?
                        .to_string();
                    
                    self.handle_git_commit(repo_path, message).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Git commit error: {}", e)),
                            data: None,
                        })?
                }
                "cargo_build" => {
                    let package = args.get("package")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    
                    let release = args.get("release")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    
                    self.handle_cargo_build(package, release).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Cargo build error: {}", e)),
                            data: None,
                        })?
                }
                "cargo_test" => {
                    let package = args.get("package")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    
                    let filter = args.get("filter")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    
                    self.handle_cargo_test(package, filter).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Cargo test error: {}", e)),
                            data: None,
                        })?
                }
                "code_search" => {
                    let pattern = args.get("pattern")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'pattern' parameter"),
                            data: None,
                        })?
                        .to_string();
                    
                    let search_path = args.get("search_path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'search_path' parameter"),
                            data: None,
                        })?
                        .to_string();
                    
                    self.handle_code_search(pattern, search_path).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Code search error: {}", e)),
                            data: None,
                        })?
                }
                "read_file_lines" => {
                    let file_path = args.get("file_path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'file_path' parameter"),
                            data: None,
                        })?
                        .to_string();
                    
                    let start_line = args.get("start_line")
                        .and_then(|v| v.as_u64())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'start_line' parameter"),
                            data: None,
                        })? as usize;
                    
                    let end_line = args.get("end_line")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as usize);
                    
                    self.handle_read_file_lines(file_path, start_line, end_line).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Read file lines error: {}", e)),
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
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hainet-dev");
    let logs_dir = data_dir.join("logs");
    std::fs::create_dir_all(&logs_dir)?;
    
    // Create log file with timestamp
    let log_file = logs_dir.join(format!(
        "hainet-dev-{}.log",
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
        .with(EnvFilter::new("hainet_dev=debug,rmcp=info"))
        .init();

    info!("🛠️  Starting HAI-Net Development Tools MCP Server");
    info!("📝 Logs being written to: {}", log_file.display());

    let server = DevServer::new();

    info!("📡 Starting MCP server on stdio transport...");

    // Run server with stdio transport
    use rmcp::service::ServiceExt;
    let running_service = server.serve(rmcp::transport::io::stdio()).await?;
    
    running_service.waiting().await?;

    info!("🛑 HAI-Net Development Tools MCP Server shutting down");
    Ok(())
}
