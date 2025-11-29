//! # START OF FILE hainet-persona/src/agents/admin_discovery.rs
//! Admin Agent Discovery-Based Execution Module
//! 
//! Implements just-in-time tool loading for Admin agent.
//! Focuses on system orchestration and intent analysis.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Intent classification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserIntent {
    Simple,
    Complex,
}

/// Orchestration action
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrchestrationAction {
    #[serde(rename = "use_tools")]
    UseTools,
    #[serde(rename = "delegate_to_pm")]
    DelegateToPM,
    #[serde(rename = "respond_directly")]
    RespondDirectly,
}

/// Admin tool selection request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminToolSelectionRequest {
    pub intent: UserIntent,
    pub action: OrchestrationAction,
    pub needed_tools: Vec<String>,
    pub delegate_to: Option<String>,
    pub reasoning: String,
}

/// Admin orchestration step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationStep {
    pub step_number: usize,
    pub action: StepAction,
    pub target: String,
    pub params: Value,
    pub description: String,
    pub depends_on: Vec<usize>,
}

/// Step action type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepAction {
    #[serde(rename = "use_tool")]
    UseTool,
    #[serde(rename = "delegate_to_pm")]
    DelegateToPM,
    #[serde(rename = "send_message")]
    SendMessage,
}

/// Admin orchestration plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationPlan {
    pub steps: Vec<OrchestrationStep>,
}

/// Operation feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationFeedback {
    pub status: OperationStatus,
    pub reasoning: String,
    pub next_action: String,
    #[serde(default)]
    pub user_message: Option<String>,
    pub should_notify_user: bool,
}

/// Operation status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationStatus {
    Success,
    #[serde(rename = "needs_adjustment")]
    NeedsAdjustment,
    Failed,
}

/// Admin discovery context
pub struct AdminDiscoveryContext {
    pub user_request: String,
    pub current_state: String,
    pub session_tasks: String,
    pub available_pms: Vec<String>,
    pub pm_status: HashMap<String, String>,
}

impl AdminDiscoveryContext {
    pub fn new(
        user_request: String,
        current_state: String,
        session_tasks: String,
        available_pms: Vec<String>,
    ) -> Self {
        Self {
            user_request,
            current_state,
            session_tasks,
            available_pms,
            pm_status: HashMap::new(),
        }
    }
    
    pub fn update_pm_status(&mut self, pm_name: String, status: String) {
        self.pm_status.insert(pm_name, status);
    }
}

/// Parse admin tool selection from LLM response
pub fn parse_admin_tool_selection(llm_response: &str) -> Result<AdminToolSelectionRequest> {
    // Use robust JSON extraction from worker_discovery
    if let Some(json_str) = super::worker_discovery::extract_json_robust(llm_response) {
        if let Ok(selection) = serde_json::from_str::<AdminToolSelectionRequest>(&json_str) {
            return Ok(selection);
        }
    }
    
    Err(anyhow::anyhow!(
        "Failed to parse admin tool selection: {}",
        &llm_response[..llm_response.len().min(200)]
    ))
}


/// Parse orchestration plan from LLM response
pub fn parse_orchestration_plan(llm_response: &str) -> Result<OrchestrationPlan> {
    // Use robust JSON extraction from worker_discovery
    if let Some(json_str) = super::worker_discovery::extract_json_robust(llm_response) {
        if let Ok(plan) = serde_json::from_str::<OrchestrationPlan>(&json_str) {
            return Ok(plan);
        }
    }
    
    Err(anyhow::anyhow!(
        "Failed to parse orchestration plan: {}",
        &llm_response[..llm_response.len().min(200)]
    ))
}

/// Parse operation feedback from LLM response
pub fn parse_operation_feedback(llm_response: &str) -> Result<OperationFeedback> {
    // Use robust JSON extraction from worker_discovery
    if let Some(json_str) = super::worker_discovery::extract_json_robust(llm_response) {
        if let Ok(feedback) = serde_json::from_str::<OperationFeedback>(&json_str) {
            return Ok(feedback);
        }
    }
    
    Err(anyhow::anyhow!(
        "Failed to parse operation feedback: {}",
        &llm_response[..llm_response.len().min(200)]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_admin_tool_selection() {
        let json = r#"{"intent": "simple", "action": "use_tools", "needed_tools": ["hainet-files::file_read"], "delegate_to": null, "reasoning": "Direct file read"}"#;
        let selection = parse_admin_tool_selection(json).unwrap();
        assert_eq!(selection.needed_tools.len(), 1);
    }
    
    #[test]
    fn test_parse_orchestration_plan() {
        let json = r#"{"steps": [{"step_number": 1, "action": "use_tool", "target": "hainet-files::file_write", "params": {"path": "test.txt"}, "description": "Write test file", "depends_on": []}]}"#;
        let plan = parse_orchestration_plan(json).unwrap();
        assert_eq!(plan.steps.len(), 1);
    }
}
