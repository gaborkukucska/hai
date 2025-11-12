//! # START OF FILE hainet-persona/src/agents/worker_discovery.rs
//! Worker Discovery-Based Execution Module
//! 
//! Implements just-in-time tool loading for focused, efficient LLM prompts.
//! Instead of overwhelming LLMs with all tool information upfront, this module:
//! 1. Shows LLM only tool names initially
//! 2. LLM identifies which tools it needs
//! 3. Loads detailed metadata only for those tools
//! 4. Generates execution plan with focused context

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Tool identification request from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSelectionRequest {
    /// Tools the LLM wants to use (format: "server::tool_name")
    pub needed_tools: Vec<String>,
    /// Brief reasoning for tool selection
    pub reasoning: String,
}

/// Execution step generated with discovery-based approach
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryExecutionStep {
    pub step_number: usize,
    pub tool: String,  // Format: "server::tool_name"
    pub params: Value,
    pub description: String,
    pub depends_on: Vec<usize>,
}

/// Execution plan from discovery-based planning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryExecutionPlan {
    pub steps: Vec<DiscoveryExecutionStep>,
}

/// Step execution feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepFeedback {
    pub status: StepStatus,
    pub reasoning: String,
    pub next_action: String,
    #[serde(default)]
    pub updated_params: HashMap<String, Value>,
}

/// Step execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    Success,
    Retry,
    Failed,
}

/// Discovery-based execution context
pub struct DiscoveryContext {
    /// Task being executed
    pub task_description: String,
    /// Worker's role
    pub worker_role: String,
    /// Worker's capabilities
    pub worker_capabilities: Vec<String>,
    /// Session task list (formatted for prompts)
    pub session_tasks: String,
    /// Results from previous steps
    pub previous_results: Vec<String>,
}

impl DiscoveryContext {
    /// Create new discovery context
    pub fn new(
        task_description: String,
        worker_role: String,
        worker_capabilities: Vec<String>,
        session_tasks: String,
    ) -> Self {
        Self {
            task_description,
            worker_role,
            worker_capabilities,
            session_tasks,
            previous_results: Vec::new(),
        }
    }
    
    /// Add result from executed step
    pub fn add_result(&mut self, result: String) {
        self.previous_results.push(result);
    }
    
    /// Get formatted previous results
    pub fn formatted_previous_results(&self) -> String {
        if self.previous_results.is_empty() {
            "No previous results yet".to_string()
        } else {
            self.previous_results
                .iter()
                .enumerate()
                .map(|(idx, result)| format!("Step {}: {}", idx + 1, result))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

/// Helper to format tool list for minimal planning prompt
pub fn format_tool_list(tool_names: &[String]) -> String {
    tool_names
        .iter()
        .map(|name| format!("- {}", name))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Helper to format tool metadata for execution prompt
pub fn format_tool_metadata(metadata_map: &HashMap<String, String>) -> String {
    metadata_map
        .iter()
        .map(|(tool, metadata)| format!("{}:\n{}\n", tool, metadata))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse tool selection from LLM response
pub fn parse_tool_selection(llm_response: &str) -> Result<ToolSelectionRequest> {
    // Try direct JSON parse
    if let Ok(selection) = serde_json::from_str::<ToolSelectionRequest>(llm_response) {
        return Ok(selection);
    }
    
    // Try extracting from markdown code block
    if let Some(json_str) = extract_json_from_markdown(llm_response) {
        if let Ok(selection) = serde_json::from_str::<ToolSelectionRequest>(&json_str) {
            return Ok(selection);
        }
    }
    
    // Try extracting braces
    if let Some(json_str) = extract_json_from_braces(llm_response) {
        if let Ok(selection) = serde_json::from_str::<ToolSelectionRequest>(&json_str) {
            return Ok(selection);
        }
    }
    
    Err(anyhow::anyhow!(
        "Failed to parse tool selection from LLM response: {}",
        &llm_response[..llm_response.len().min(200)]
    ))
}

/// Parse execution plan from LLM response
pub fn parse_execution_plan(llm_response: &str) -> Result<DiscoveryExecutionPlan> {
    // Try direct JSON parse
    if let Ok(plan) = serde_json::from_str::<DiscoveryExecutionPlan>(llm_response) {
        return Ok(plan);
    }
    
    // Try extracting from markdown
    if let Some(json_str) = extract_json_from_markdown(llm_response) {
        if let Ok(plan) = serde_json::from_str::<DiscoveryExecutionPlan>(&json_str) {
            return Ok(plan);
        }
    }
    
    // Try extracting braces
    if let Some(json_str) = extract_json_from_braces(llm_response) {
        if let Ok(plan) = serde_json::from_str::<DiscoveryExecutionPlan>(&json_str) {
            return Ok(plan);
        }
    }
    
    Err(anyhow::anyhow!(
        "Failed to parse execution plan from LLM response: {}",
        &llm_response[..llm_response.len().min(200)]
    ))
}

/// Parse step feedback from LLM response
pub fn parse_step_feedback(llm_response: &str) -> Result<StepFeedback> {
    // Try direct JSON parse
    if let Ok(feedback) = serde_json::from_str::<StepFeedback>(llm_response) {
        return Ok(feedback);
    }
    
    // Try extracting from markdown
    if let Some(json_str) = extract_json_from_markdown(llm_response) {
        if let Ok(feedback) = serde_json::from_str::<StepFeedback>(&json_str) {
            return Ok(feedback);
        }
    }
    
    // Try extracting braces
    if let Some(json_str) = extract_json_from_braces(llm_response) {
        if let Ok(feedback) = serde_json::from_str::<StepFeedback>(&json_str) {
            return Ok(feedback);
        }
    }
    
    Err(anyhow::anyhow!(
        "Failed to parse step feedback from LLM response: {}",
        &llm_response[..llm_response.len().min(200)]
    ))
}

/// Extract JSON from markdown code block
fn extract_json_from_markdown(text: &str) -> Option<String> {
    let markers = ["```json\n", "```\n", "```"];
    
    for marker in markers.iter() {
        if let Some(start_idx) = text.find(marker) {
            let json_start = start_idx + marker.len();
            
            if let Some(end_idx) = text[json_start..].find("```") {
                let json_text = &text[json_start..json_start + end_idx];
                return Some(json_text.trim().to_string());
            }
        }
    }
    
    None
}

/// Extract JSON from braces
fn extract_json_from_braces(text: &str) -> Option<String> {
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return Some(text[start..=end].to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_tool_list() {
        let tools = vec![
            "hainet-files::file_read".to_string(),
            "hainet-files::file_write".to_string(),
        ];
        
        let formatted = format_tool_list(&tools);
        assert!(formatted.contains("hainet-files::file_read"));
        assert!(formatted.contains("hainet-files::file_write"));
    }
    
    #[test]
    fn test_parse_tool_selection() {
        let json_response = r#"{"needed_tools": ["hainet-files::file_read"], "reasoning": "Need to read file"}"#;
        
        let selection = parse_tool_selection(json_response).unwrap();
        assert_eq!(selection.needed_tools.len(), 1);
        assert_eq!(selection.needed_tools[0], "hainet-files::file_read");
    }
    
    #[test]
    fn test_parse_tool_selection_with_markdown() {
        let markdown_response = r#"
Here's what I need:
```json
{"needed_tools": ["hainet-files::file_write"], "reasoning": "Need to write file"}
```
"#;
        
        let selection = parse_tool_selection(markdown_response).unwrap();
        assert_eq!(selection.needed_tools.len(), 1);
        assert_eq!(selection.needed_tools[0], "hainet-files::file_write");
    }
    
    #[test]
    fn test_discovery_context() {
        let mut context = DiscoveryContext::new(
            "Test task".to_string(),
            "FileWorker".to_string(),
            vec!["files".to_string()],
            "- [in_progress] Test task".to_string(),
        );
        
        assert_eq!(context.previous_results.len(), 0);
        
        context.add_result("Step 1 complete".to_string());
        assert_eq!(context.previous_results.len(), 1);
        
        let formatted = context.formatted_previous_results();
        assert!(formatted.contains("Step 1:"));
    }
}
