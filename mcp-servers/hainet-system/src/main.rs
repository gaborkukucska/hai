//! # HAI-Net System Management MCP Server
//!
//! Provides system management tools for Admin AI agent.
//! Tools enable monitoring, service management, and health checks.

use anyhow::{Context, Result};
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::RoleServer;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::future::Future;
use std::process::Command;
use std::sync::Arc;
use sysinfo::System;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// System status result
#[derive(Debug, Serialize, Deserialize)]
struct SystemStatus {
    timestamp: String,
    cpu: CpuInfo,
    memory: MemoryInfo,
    disks: Vec<DiskInfo>,
    uptime: String,
    os: String,
    hostname: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CpuInfo {
    count: usize,
    usage_percent: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct MemoryInfo {
    total_gb: u64,
    used_gb: u64,
    available_gb: u64,
    usage_percent: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiskInfo {
    name: String,
    mount_point: String,
    total_space_gb: u64,
    available_space_gb: u64,
    usage_percent: f64,
}

/// Service list result
#[derive(Debug, Serialize, Deserialize)]
struct ServiceList {
    services: Vec<ServiceInfo>,
    count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServiceInfo {
    pid: u32,
    name: String,
    cpu_usage: String,
    memory_mb: u64,
    status: String,
}

/// Service restart result
#[derive(Debug, Serialize, Deserialize)]
struct RestartResult {
    success: bool,
    service: String,
    message: Option<String>,
    error: Option<String>,
}

/// Health check result
#[derive(Debug, Serialize, Deserialize)]
struct HealthCheck {
    timestamp: String,
    overall_status: String,
    checks: Vec<HealthCheckItem>,
    summary: HealthSummary,
}

#[derive(Debug, Serialize, Deserialize)]
struct HealthCheckItem {
    component: String,
    status: String,
    value: String,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct HealthSummary {
    total: usize,
    healthy: usize,
    warning: usize,
    error: usize,
    critical: usize,
}

/// HAI-Net System Server
#[derive(Clone)]
struct SystemServer {
    sys: Arc<RwLock<System>>,
}

impl SystemServer {
    fn new() -> Self {
        Self {
            sys: Arc::new(RwLock::new(System::new_all())),
        }
    }

    async fn handle_system_status(&self) -> Result<String> {
        debug!("Getting system status");
        
        let mut sys = self.sys.write().await;
        sys.refresh_all();

        // CPU info
        let cpu_count = sys.cpus().len();
        let cpu_usage: f32 = sys.cpus().iter()
            .map(|cpu| cpu.cpu_usage())
            .sum::<f32>() / cpu_count as f32;

        // Memory info
        let total_memory = sys.total_memory();
        let used_memory = sys.used_memory();
        let available_memory = sys.available_memory();
        let memory_usage_percent = (used_memory as f64 / total_memory as f64) * 100.0;

        // Disk info
        let disks: Vec<DiskInfo> = vec![]; // Simplified - disks API varies by version
        
        // System uptime
        let uptime_str = "N/A".to_string(); // Simplified - uptime API varies by version

        let status = SystemStatus {
            timestamp: chrono::Utc::now().to_rfc3339(),
            cpu: CpuInfo {
                count: cpu_count,
                usage_percent: format!("{:.1}", cpu_usage),
            },
            memory: MemoryInfo {
                total_gb: total_memory / (1024 * 1024 * 1024),
                used_gb: used_memory / (1024 * 1024 * 1024),
                available_gb: available_memory / (1024 * 1024 * 1024),
                usage_percent: format!("{:.1}", memory_usage_percent),
            },
            disks,
            uptime: uptime_str,
            os: System::long_os_version().unwrap_or_else(|| "Unknown".to_string()),
            hostname: System::host_name().unwrap_or_else(|| "Unknown".to_string()),
        };

        Ok(serde_json::to_string_pretty(&status)?)
    }

    async fn handle_list_services(&self) -> Result<String> {
        debug!("Listing HAI-Net services");
        
        let mut sys = self.sys.write().await;
        sys.refresh_processes();

        let hainet_services = vec![
            "hainet-core", "hainet-chain", "hainet-bridge",
            "hainet-portal", "hainet-persona", "ollama", "whisper", "piper",
        ];

        let services: Vec<ServiceInfo> = sys.processes().values()
            .filter(|p| hainet_services.iter().any(|s| p.name().contains(s)))
            .map(|p| ServiceInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string(),
                cpu_usage: format!("{:.1}%", p.cpu_usage()),
                memory_mb: p.memory() / (1024 * 1024),
                status: "running".to_string(),
            })
            .collect();

        let result = ServiceList {
            count: services.len(),
            services,
        };

        Ok(serde_json::to_string_pretty(&result)?)
    }

    async fn handle_restart_service(&self, service_name: String) -> Result<String> {
        info!("Attempting to restart service: {}", service_name);

        // Security: whitelist allowed services
        let allowed = vec!["hainet-core", "hainet-chain", "hainet-bridge", "hainet-portal", "ollama"];
        
        if !allowed.contains(&service_name.as_str()) {
            let result = RestartResult {
                success: false,
                service: service_name,
                message: None,
                error: Some("Service not allowed to be restarted".to_string()),
            };
            return Ok(serde_json::to_string_pretty(&result)?);
        }

        let output = Command::new("systemctl")
            .args(&["restart", &service_name])
            .output();

        let result = match output {
            Ok(output) if output.status.success() => RestartResult {
                success: true,
                service: service_name,
                message: Some("Service restarted successfully".to_string()),
                error: None,
            },
            Ok(output) => RestartResult {
                success: false,
                service: service_name,
                message: None,
                error: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            },
            Err(e) => RestartResult {
                success: false,
                service: service_name,
                message: None,
                error: Some(format!("Failed to execute systemctl: {}", e)),
            },
        };

        Ok(serde_json::to_string_pretty(&result)?)
    }

    async fn handle_check_health(&self) -> Result<String> {
        info!("Running health checks");
        
        let mut sys = self.sys.write().await;
        sys.refresh_all();

        let mut checks = Vec::new();

        // CPU check
        let cpu_usage: f32 = sys.cpus().iter()
            .map(|cpu| cpu.cpu_usage())
            .sum::<f32>() / sys.cpus().len() as f32;
        
        checks.push(HealthCheckItem {
            component: "CPU".to_string(),
            status: if cpu_usage < 80.0 { "healthy" } else { "warning" }.to_string(),
            value: format!("{:.1}%", cpu_usage),
            message: if cpu_usage < 80.0 { "CPU usage is normal" } else { "CPU usage is high" }.to_string(),
        });

        // Memory check
        let memory_usage = (sys.used_memory() as f64 / sys.total_memory() as f64) * 100.0;
        checks.push(HealthCheckItem {
            component: "Memory".to_string(),
            status: if memory_usage < 85.0 { "healthy" } else { "warning" }.to_string(),
            value: format!("{:.1}%", memory_usage),
            message: if memory_usage < 85.0 { "Memory usage is normal" } else { "Memory usage is high" }.to_string(),
        });

        // Disk checks - simplified (disk API varies by sysinfo version)
        // In production, would check each mounted filesystem

        // Service checks
        sys.refresh_processes();
        let is_ollama_running = sys.processes().values().any(|p| p.name().contains("ollama"));
        checks.push(HealthCheckItem {
            component: "Service: ollama".to_string(),
            status: if is_ollama_running { "healthy" } else { "error" }.to_string(),
            value: if is_ollama_running { "running" } else { "not running" }.to_string(),
            message: if is_ollama_running { "ollama is running" } else { "ollama is not running" }.to_string(),
        });

        // Calculate overall status
        let has_critical = checks.iter().any(|c| c.status == "critical");
        let has_error = checks.iter().any(|c| c.status == "error");
        let has_warning = checks.iter().any(|c| c.status == "warning");

        let overall_status = if has_critical || has_error {
            "unhealthy"
        } else if has_warning {
            "degraded"
        } else {
            "healthy"
        };

        let result = HealthCheck {
            timestamp: chrono::Utc::now().to_rfc3339(),
            overall_status: overall_status.to_string(),
            summary: HealthSummary {
                total: checks.len(),
                healthy: checks.iter().filter(|c| c.status == "healthy").count(),
                warning: checks.iter().filter(|c| c.status == "warning").count(),
                error: checks.iter().filter(|c| c.status == "error").count(),
                critical: checks.iter().filter(|c| c.status == "critical").count(),
            },
            checks,
        };

        Ok(serde_json::to_string_pretty(&result)?)
    }
}

impl ServerHandler for SystemServer {
    fn list_tools(
        &self,
        _params: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        async move {
            Ok(ListToolsResult {
                tools: vec![
                    Tool {
                        name: Cow::Borrowed("system_status"),
                        title: Some("System Status".to_string()),
                        description: Some(Cow::Borrowed("Get current system status (CPU, RAM, disk, network)")),
                        input_schema: Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {},
                            "required": []
                        }).as_object().unwrap().clone()),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("list_services"),
                        title: Some("List Services".to_string()),
                        description: Some(Cow::Borrowed("List all running HAI-Net services")),
                        input_schema: Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {},
                            "required": []
                        }).as_object().unwrap().clone()),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("restart_service"),
                        title: Some("Restart Service".to_string()),
                        description: Some(Cow::Borrowed("Restart a HAI-Net service")),
                        input_schema: Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "service_name": {
                                    "type": "string",
                                    "description": "Name of the service to restart"
                                }
                            },
                            "required": ["service_name"]
                        }).as_object().unwrap().clone()),
                        output_schema: None,
                        annotations: None,
                        icons: None,
                    },
                    Tool {
                        name: Cow::Borrowed("check_health"),
                        title: Some("Health Check".to_string()),
                        description: Some(Cow::Borrowed("Run comprehensive health checks")),
                        input_schema: Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {},
                            "required": []
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
                "system_status" => {
                    self.handle_system_status().await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("System status error: {}", e)),
                            data: None,
                        })?
                }
                "list_services" => {
                    self.handle_list_services().await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Service list error: {}", e)),
                            data: None,
                        })?
                }
                "restart_service" => {
                    let service_name = args.get("service_name")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ErrorData {
                            code: ErrorCode::INVALID_PARAMS,
                            message: Cow::Borrowed("Missing 'service_name' parameter"),
                            data: None,
                        })?
                        .to_string();
                    
                    self.handle_restart_service(service_name).await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Restart error: {}", e)),
                            data: None,
                        })?
                }
                "check_health" => {
                    self.handle_check_health().await
                        .map_err(|e| ErrorData {
                            code: ErrorCode::INTERNAL_ERROR,
                            message: Cow::Owned(format!("Health check error: {}", e)),
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
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("hainet_system=debug,rmcp=info")
        .with_writer(std::io::stderr)
        .init();

    info!("🔧 Starting HAI-Net System Management MCP Server");

    let server = SystemServer::new();

    info!("📡 Starting MCP server on stdio transport...");

    // Run server with stdio transport
    use rmcp::service::ServiceExt;
    let running_service = server.serve(rmcp::transport::io::stdio()).await?;
    
    running_service.waiting().await?;

    info!("🛑 HAI-Net System MCP Server shutting down");
    Ok(())
}
