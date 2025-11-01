//! # START OF FILE hainet-portal/src-tauri/src/metrics_handler.rs
//! Metrics Handler for HAI-Net Portal
//! 
//! Exposes agent performance metrics to the frontend via Tauri commands.

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use std::sync::Arc;
use anyhow::Result;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::RwLock;
use hainet_persona::agents::{AgentType, metrics::MetricsCollector};
use crate::metrics_storage::{MetricsStorage, TimeRange, TrendInterval, TrendDataPoint, MetricsSnapshot};

/// Frontend-compatible agent metrics structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetricsResponse {
    pub agent_type: String,
    pub total_operations: u64,
    pub success_rate: f32,
    pub avg_response_time_ms: f32,
    pub avg_tokens_used: f32,
    pub json_parse_success_rate: f32,
    pub validation_pass_rate: f32,
    pub syntax_error_rate: f32,
    pub first_operation_unix: u64,
    pub last_operation_unix: u64,
}

/// High-level metrics summary for all agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummaryResponse {
    pub total_tasks: u64,
    pub overall_success_rate: f32,
    pub total_tokens: u64,
    pub total_cost_usd: f32,
    pub agents: Vec<AgentMetricsResponse>,
    pub timestamp_unix: u64,
}

/// Get metrics for all agent types
#[tauri::command]
pub async fn get_agent_metrics(
    metrics_collector: State<'_, Arc<RwLock<MetricsCollector>>>,
) -> Result<Vec<AgentMetricsResponse>, String> {
    tracing::info!("Fetching agent metrics from database...");
    
    let collector = metrics_collector.read().await;
    let mut all_metrics = Vec::new();
    
    // Fetch metrics for each agent type
    for agent_type in [AgentType::Admin, AgentType::PM, AgentType::Worker, AgentType::Guardian] {
        // Check if this agent type has any operations
        let count = collector.count_operations(agent_type).await
            .map_err(|e| format!("Failed to count operations: {}", e))?;
        
        if count > 0 {
            let metrics = collector.get_aggregate(agent_type).await
                .map_err(|e| format!("Failed to get aggregate metrics: {}", e))?;
            
            // Convert to frontend-compatible format
            all_metrics.push(AgentMetricsResponse {
                agent_type: agent_type.to_string(),
                total_operations: metrics.total_operations,
                success_rate: metrics.success_rate,
                avg_response_time_ms: metrics.avg_response_time_ms,
                avg_tokens_used: metrics.avg_tokens_used,
                json_parse_success_rate: metrics.json_parse_success_rate,
                validation_pass_rate: metrics.validation_pass_rate,
                syntax_error_rate: metrics.syntax_error_rate,
                first_operation_unix: metrics.first_operation
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or(Duration::ZERO)
                    .as_secs(),
                last_operation_unix: metrics.last_operation
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or(Duration::ZERO)
                    .as_secs(),
            });
        }
    }
    
    tracing::debug!("Retrieved metrics for {} agent types", all_metrics.len());
    Ok(all_metrics)
}

/// Get metrics for specific agent type
#[tauri::command]
pub async fn get_agent_metrics_by_type(
    agent_type_str: String,
    metrics_collector: State<'_, Arc<RwLock<MetricsCollector>>>,
) -> Result<AgentMetricsResponse, String> {
    tracing::info!("Fetching metrics for agent type: {}", agent_type_str);
    
    // Parse agent type string
    let agent_type = match agent_type_str.as_str() {
        "Admin" => AgentType::Admin,
        "PM" => AgentType::PM,
        "Worker" => AgentType::Worker,
        "Guardian" => AgentType::Guardian,
        _ => return Err(format!("Invalid agent type: {}", agent_type_str)),
    };
    
    let collector = metrics_collector.read().await;
    
    // Check if this agent type has any operations
    let count = collector.count_operations(agent_type).await
        .map_err(|e| format!("Failed to count operations: {}", e))?;
    
    if count == 0 {
        return Err(format!("No metrics found for agent type: {}", agent_type_str));
    }
    
    let metrics = collector.get_aggregate(agent_type).await
        .map_err(|e| format!("Failed to get aggregate metrics: {}", e))?;
    
    Ok(AgentMetricsResponse {
        agent_type: agent_type.to_string(),
        total_operations: metrics.total_operations,
        success_rate: metrics.success_rate,
        avg_response_time_ms: metrics.avg_response_time_ms,
        avg_tokens_used: metrics.avg_tokens_used,
        json_parse_success_rate: metrics.json_parse_success_rate,
        validation_pass_rate: metrics.validation_pass_rate,
        syntax_error_rate: metrics.syntax_error_rate,
        first_operation_unix: metrics.first_operation
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs(),
        last_operation_unix: metrics.last_operation
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs(),
    })
}

/// Get high-level metrics summary
#[tauri::command]
pub async fn get_metrics_summary(
    metrics_collector: State<'_, Arc<RwLock<MetricsCollector>>>,
) -> Result<MetricsSummaryResponse, String> {
    tracing::info!("Fetching metrics summary from database...");
    
    let collector = metrics_collector.read().await;
    let mut all_metrics = Vec::new();
    
    // Fetch metrics for each agent type
    for agent_type in [AgentType::Admin, AgentType::PM, AgentType::Worker, AgentType::Guardian] {
        let count = collector.count_operations(agent_type).await
            .map_err(|e| format!("Failed to count operations: {}", e))?;
        
        if count > 0 {
            let metrics = collector.get_aggregate(agent_type).await
                .map_err(|e| format!("Failed to get aggregate metrics: {}", e))?;
            
            all_metrics.push(AgentMetricsResponse {
                agent_type: agent_type.to_string(),
                total_operations: metrics.total_operations,
                success_rate: metrics.success_rate,
                avg_response_time_ms: metrics.avg_response_time_ms,
                avg_tokens_used: metrics.avg_tokens_used,
                json_parse_success_rate: metrics.json_parse_success_rate,
                validation_pass_rate: metrics.validation_pass_rate,
                syntax_error_rate: metrics.syntax_error_rate,
                first_operation_unix: metrics.first_operation
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or(Duration::ZERO)
                    .as_secs(),
                last_operation_unix: metrics.last_operation
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or(Duration::ZERO)
                    .as_secs(),
            });
        }
    }
    
    // Calculate aggregates
    let total_tasks: u64 = all_metrics.iter().map(|a| a.total_operations).sum();
    let total_tokens: u64 = all_metrics.iter()
        .map(|a| (a.avg_tokens_used * a.total_operations as f32) as u64)
        .sum();
    
    // Calculate weighted average success rate
    let weighted_success: f32 = all_metrics.iter()
        .map(|a| a.success_rate * a.total_operations as f32)
        .sum();
    let overall_success_rate = if total_tasks > 0 {
        weighted_success / total_tasks as f32
    } else {
        0.0
    };
    
    // Cost estimation (OpenAI pricing: ~$0.002 per 1K tokens)
    let total_cost_usd = (total_tokens as f32 / 1000.0) * 0.002;
    
    tracing::debug!(
        "Summary: {} tasks, {:.2}% success rate, {} tokens",
        total_tasks,
        overall_success_rate * 100.0,
        total_tokens
    );
    
    Ok(MetricsSummaryResponse {
        total_tasks,
        overall_success_rate,
        total_tokens,
        total_cost_usd,
        agents: all_metrics,
        timestamp_unix: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    })
}

/// Export full metrics report as JSON string with optional time range
#[tauri::command]
pub async fn export_metrics_json(
    _time_range: Option<TimeRange>,
    metrics_collector: State<'_, Arc<RwLock<MetricsCollector>>>,
) -> Result<String, String> {
    tracing::info!("Exporting metrics as JSON from database...");
    
    let collector = metrics_collector.read().await;
    
    // Use MetricsCollector's built-in export functionality
    // TODO: Integrate time_range filtering when MetricsCollector supports it
    collector.export_json().await
        .map_err(|e| format!("Failed to export metrics: {}", e))
}

/// Export metrics as CSV format
#[tauri::command]
pub async fn export_metrics_csv(
    time_range: Option<TimeRange>,
    metrics_storage: State<'_, Arc<RwLock<MetricsStorage>>>,
) -> Result<String, String> {
    tracing::info!("Exporting metrics as CSV...");
    
    let storage = metrics_storage.read().await;
    let time_range = time_range.unwrap_or_default();
    
    // Get historical metrics
    let snapshots = storage.get_historical_metrics(None, time_range).await?;
    
    // Build CSV
    let mut csv = String::from("Timestamp,Agent Type,Success Rate,Avg Latency (ms),Total Operations,Successful,Failed,Tokens,Cost (USD)\n");
    
    let snapshot_count = snapshots.len();
    
    for snapshot in snapshots {
        csv.push_str(&format!(
            "{},{},{:.4},{:.2},{},{},{},{},{:.6}\n",
            snapshot.timestamp,
            snapshot.agent_type,
            snapshot.success_rate,
            snapshot.avg_latency_ms,
            snapshot.total_operations,
            snapshot.successful_operations,
            snapshot.failed_operations,
            snapshot.tokens_used,
            snapshot.estimated_cost_usd
        ));
    }
    
    tracing::debug!("Exported {} CSV rows", snapshot_count);
    Ok(csv)
}

/// Get historical metrics snapshots within a time range
#[tauri::command]
pub async fn get_historical_metrics(
    agent_type: Option<String>,
    time_range: Option<TimeRange>,
    metrics_storage: State<'_, Arc<RwLock<MetricsStorage>>>,
) -> Result<Vec<MetricsSnapshot>, String> {
    tracing::info!("Fetching historical metrics...");
    
    let storage = metrics_storage.read().await;
    let time_range = time_range.unwrap_or_default();
    
    storage.get_historical_metrics(agent_type, time_range).await
}

/// Get trend analysis for a specific agent type
#[tauri::command]
pub async fn get_metrics_trend(
    agent_type: String,
    interval: String,
    time_range: Option<TimeRange>,
    metrics_storage: State<'_, Arc<RwLock<MetricsStorage>>>,
) -> Result<Vec<TrendDataPoint>, String> {
    tracing::info!("Computing metrics trend for agent_type={}, interval={}", agent_type, interval);
    
    // Parse interval
    let interval_enum = match interval.as_str() {
        "hourly" => TrendInterval::Hourly,
        "daily" => TrendInterval::Daily,
        "weekly" => TrendInterval::Weekly,
        _ => return Err(format!("Invalid interval: {}. Must be 'hourly', 'daily', or 'weekly'", interval)),
    };
    
    let storage = metrics_storage.read().await;
    let time_range = time_range.unwrap_or_default();
    
    storage.compute_trend(agent_type, interval_enum, time_range).await
}

/// Start background task to record periodic metrics snapshots
pub fn start_metrics_snapshot_task(
    metrics_collector: Arc<RwLock<MetricsCollector>>,
    metrics_storage: Arc<RwLock<MetricsStorage>>,
) {
    tracing::info!("Starting metrics snapshot recording task...");
    
    tauri::async_runtime::spawn(async move {
        loop {
            // Record snapshots every 5 minutes
            tokio::time::sleep(Duration::from_secs(300)).await;
            
            tracing::debug!("Recording periodic metrics snapshot...");
            
            // Get current metrics for all agent types
            let collector = metrics_collector.read().await;
            
            for agent_type in [AgentType::Admin, AgentType::PM, AgentType::Worker, AgentType::Guardian] {
                // Check if agent has operations
                if let Ok(count) = collector.count_operations(agent_type).await {
                    if count > 0 {
                        // Get aggregate metrics
                        if let Ok(metrics) = collector.get_aggregate(agent_type).await {
                            let snapshot = MetricsSnapshot {
                                agent_type: agent_type.to_string(),
                                timestamp: chrono::Utc::now().timestamp(),
                                success_rate: metrics.success_rate as f64,
                                avg_latency_ms: metrics.avg_response_time_ms as f64,
                                total_operations: metrics.total_operations as u32,
                                successful_operations: (metrics.total_operations as f32 * metrics.success_rate) as u32,
                                failed_operations: (metrics.total_operations as f32 * (1.0 - metrics.success_rate)) as u32,
                                tokens_used: (metrics.avg_tokens_used * metrics.total_operations as f32) as u64,
                                estimated_cost_usd: ((metrics.avg_tokens_used * metrics.total_operations as f32) as f64 / 1000.0) * 0.002,
                            };
                            
                            // Record to storage
                            let storage = metrics_storage.read().await;
                            if let Err(e) = storage.record_snapshot(snapshot).await {
                                tracing::warn!("Failed to record metrics snapshot for {:?}: {}", agent_type, e);
                            }
                        }
                    }
                }
            }
            
            tracing::debug!("Metrics snapshot recorded successfully");
        }
    });
}

/// Start background task to broadcast metrics updates via Tauri events
pub fn start_metrics_broadcast(app_handle: AppHandle, metrics_collector: Arc<RwLock<MetricsCollector>>) {
    tracing::info!("Starting metrics broadcast service...");
    
    tauri::async_runtime::spawn(async move {
        loop {
            // Wait 5 seconds between updates
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            // Fetch latest metrics summary
            let collector = metrics_collector.read().await;
            let mut all_metrics = Vec::new();
            
            // Fetch metrics for each agent type
            for agent_type in [AgentType::Admin, AgentType::PM, AgentType::Worker, AgentType::Guardian] {
                if let Ok(count) = collector.count_operations(agent_type).await {
                    if count > 0 {
                        if let Ok(metrics) = collector.get_aggregate(agent_type).await {
                            all_metrics.push(AgentMetricsResponse {
                                agent_type: agent_type.to_string(),
                                total_operations: metrics.total_operations,
                                success_rate: metrics.success_rate,
                                avg_response_time_ms: metrics.avg_response_time_ms,
                                avg_tokens_used: metrics.avg_tokens_used,
                                json_parse_success_rate: metrics.json_parse_success_rate,
                                validation_pass_rate: metrics.validation_pass_rate,
                                syntax_error_rate: metrics.syntax_error_rate,
                                first_operation_unix: metrics.first_operation
                                    .duration_since(SystemTime::UNIX_EPOCH)
                                    .unwrap_or(Duration::ZERO)
                                    .as_secs(),
                                last_operation_unix: metrics.last_operation
                                    .duration_since(SystemTime::UNIX_EPOCH)
                                    .unwrap_or(Duration::ZERO)
                                    .as_secs(),
                            });
                        }
                    }
                }
            }
            
            drop(collector); // Release read lock
            
            // Calculate summary
            let total_tasks: u64 = all_metrics.iter().map(|a| a.total_operations).sum();
            let total_tokens: u64 = all_metrics.iter()
                .map(|a| (a.avg_tokens_used * a.total_operations as f32) as u64)
                .sum();
            let weighted_success: f32 = all_metrics.iter()
                .map(|a| a.success_rate * a.total_operations as f32)
                .sum();
            let overall_success_rate = if total_tasks > 0 {
                weighted_success / total_tasks as f32
            } else {
                0.0
            };
            let total_cost_usd = (total_tokens as f32 / 1000.0) * 0.002;
            
            let summary = MetricsSummaryResponse {
                total_tasks,
                overall_success_rate,
                total_tokens,
                total_cost_usd,
                agents: all_metrics,
                timestamp_unix: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };
            
            // Emit event to all frontend listeners
            if let Err(e) = app_handle.emit("metrics-updated", summary) {
                tracing::warn!("Failed to emit metrics-updated event: {}", e);
            } else {
                tracing::debug!("Metrics update broadcast successful");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use hainet_persona::agents::metrics::{MetricsCollector, OperationResult};
    
    async fn create_test_collector() -> Arc<RwLock<MetricsCollector>> {
        let collector = MetricsCollector::new("sqlite::memory:").await.unwrap();
        Arc::new(RwLock::new(collector))
    }
    
    async fn add_test_data(collector: &MetricsCollector) {
        // Add some test operations
        for i in 0..5 {
            let result = OperationResult {
                agent_type: AgentType::Admin,
                agent_id: crate::agents::AgentId::new(AgentType::Admin, "test".to_string()),
                config_hash: "test_hash".to_string(),
                operation_type: "test_op".to_string(),
                success: true,
                response_time: Duration::from_millis(100),
                tokens_used: Some(50),
                error_message: None,
                json_parse_success: true,
                had_syntax_errors: false,
                validation_passed: true,
            };
            collector.record_operation(result).await.unwrap();
        }
    }
    
    #[tokio::test]
    async fn test_metrics_database_integration() {
        let collector = create_test_collector().await;
        
        // Add test data
        {
            let c = collector.read().await;
            add_test_data(&c).await;
        }
        
        // Verify count
        let count = {
            let c = collector.read().await;
            c.count_operations(AgentType::Admin).await.unwrap()
        };
        assert_eq!(count, 5);
    }
    
    #[tokio::test]
    async fn test_aggregation_calculations() {
        let collector = create_test_collector().await;
        
        // Add test data
        {
            let c = collector.read().await;
            add_test_data(&c).await;
        }
        
        // Get aggregate metrics
        let metrics = {
            let c = collector.read().await;
            c.get_aggregate(AgentType::Admin).await.unwrap()
        };
        
        assert_eq!(metrics.total_operations, 5);
        assert_eq!(metrics.success_rate, 1.0); // All succeeded
    }
}
