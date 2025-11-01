//! # START OF FILE hainet-portal/src-tauri/src/metrics_handler.rs
//! Metrics Handler for HAI-Net Portal
//! 
//! Exposes agent performance metrics to the frontend via Tauri commands.

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use anyhow::Result;

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
pub async fn get_agent_metrics() -> Result<Vec<AgentMetricsResponse>, String> {
    // TODO: Connect to hainet-persona MetricsCollector
    // For now, return mock data until integration is complete
    
    tracing::info!("Fetching agent metrics...");
    
    let mock_metrics = vec![
        AgentMetricsResponse {
            agent_type: "Admin".to_string(),
            total_operations: 42,
            success_rate: 0.95,
            avg_response_time_ms: 250.5,
            avg_tokens_used: 1024.0,
            json_parse_success_rate: 0.92,
            validation_pass_rate: 0.97,
            syntax_error_rate: 0.03,
            first_operation_unix: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() - 86400, // 1 day ago
            last_operation_unix: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        },
        AgentMetricsResponse {
            agent_type: "PM".to_string(),
            total_operations: 28,
            success_rate: 0.92,
            avg_response_time_ms: 180.3,
            avg_tokens_used: 768.0,
            json_parse_success_rate: 0.89,
            validation_pass_rate: 0.94,
            syntax_error_rate: 0.06,
            first_operation_unix: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() - 72000,
            last_operation_unix: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() - 120,
        },
        AgentMetricsResponse {
            agent_type: "Worker".to_string(),
            total_operations: 156,
            success_rate: 0.98,
            avg_response_time_ms: 120.7,
            avg_tokens_used: 512.0,
            json_parse_success_rate: 0.96,
            validation_pass_rate: 0.99,
            syntax_error_rate: 0.01,
            first_operation_unix: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() - 43200,
            last_operation_unix: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() - 30,
        },
    ];
    
    Ok(mock_metrics)
}

/// Get metrics for specific agent type
#[tauri::command]
pub async fn get_agent_metrics_by_type(agent_type: String) -> Result<AgentMetricsResponse, String> {
    tracing::info!("Fetching metrics for agent type: {}", agent_type);
    
    let all_metrics = get_agent_metrics().await?;
    
    all_metrics
        .into_iter()
        .find(|m| m.agent_type == agent_type)
        .ok_or_else(|| format!("No metrics found for agent type: {}", agent_type))
}

/// Get high-level metrics summary
#[tauri::command]
pub async fn get_metrics_summary() -> Result<MetricsSummaryResponse, String> {
    tracing::info!("Fetching metrics summary...");
    
    let agents = get_agent_metrics().await?;
    
    let total_tasks: u64 = agents.iter().map(|a| a.total_operations).sum();
    let total_tokens: u64 = agents.iter()
        .map(|a| (a.avg_tokens_used * a.total_operations as f32) as u64)
        .sum();
    
    // Calculate weighted average success rate
    let weighted_success: f32 = agents.iter()
        .map(|a| a.success_rate * a.total_operations as f32)
        .sum();
    let overall_success_rate = if total_tasks > 0 {
        weighted_success / total_tasks as f32
    } else {
        0.0
    };
    
    // Rough cost estimation (OpenAI pricing: ~$0.002 per 1K tokens)
    let total_cost_usd = (total_tokens as f32 / 1000.0) * 0.002;
    
    Ok(MetricsSummaryResponse {
        total_tasks,
        overall_success_rate,
        total_tokens,
        total_cost_usd,
        agents,
        timestamp_unix: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    })
}

/// Export full metrics report as JSON string
#[tauri::command]
pub async fn export_metrics_json() -> Result<String, String> {
    tracing::info!("Exporting metrics as JSON...");
    
    // TODO: Call hainet-persona MetricsCollector::export_json()
    // For now, return summary as JSON
    
    let summary = get_metrics_summary().await?;
    
    serde_json::to_string_pretty(&summary)
        .map_err(|e| format!("Failed to serialize metrics: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_get_agent_metrics() {
        let metrics = get_agent_metrics().await.unwrap();
        assert_eq!(metrics.len(), 3); // Admin, PM, Worker
        assert!(metrics.iter().any(|m| m.agent_type == "Admin"));
    }
    
    #[tokio::test]
    async fn test_get_metrics_by_type() {
        let admin_metrics = get_agent_metrics_by_type("Admin".to_string()).await.unwrap();
        assert_eq!(admin_metrics.agent_type, "Admin");
        
        let invalid = get_agent_metrics_by_type("Invalid".to_string()).await;
        assert!(invalid.is_err());
    }
    
    #[tokio::test]
    async fn test_get_metrics_summary() {
        let summary = get_metrics_summary().await.unwrap();
        assert!(summary.total_tasks > 0);
        assert_eq!(summary.agents.len(), 3);
        assert!(summary.overall_success_rate > 0.0 && summary.overall_success_rate <= 1.0);
    }
    
    #[tokio::test]
    async fn test_export_metrics_json() {
        let json = export_metrics_json().await.unwrap();
        assert!(json.contains("total_tasks"));
        assert!(json.contains("agents"));
        
        // Verify valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["total_tasks"].is_number());
    }
}
