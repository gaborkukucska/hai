//! # START OF FILE hainet-persona/src/agents/worker_discovery.rs
//! Worker Discovery-Based Execution Module
//! 
//! Implements just-in-time tool loading for focused, efficient LLM prompts.
//! Instead of overwhelming LLMs with all tool information upfront, this module:
//! 1. Shows LLM only tool names initially
//! 2. LLM identifies which tools it needs
//! 3. Loads detailed metadata only for those tools
//! 4. Generates execution plan with focused context

use anyhow::Result;
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
    pub step_number: Option<usize>,
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
    // Try direct JSON parse first
    if let Ok(selection) = serde_json::from_str::<ToolSelectionRequest>(llm_response) {
        return Ok(selection);
    }
    
    // Try robust extraction
    if let Some(json_str) = extract_json(llm_response) {
        match serde_json::from_str::<ToolSelectionRequest>(&json_str) {
            Ok(selection) => return Ok(selection),
            Err(e) => {
                tracing::warn!("Failed to parse extracted JSON for tool selection: {}. Extracted: {}", e, json_str);
            }
        }
    } else {
        tracing::warn!("Failed to extract JSON from tool selection response");
    }
    
    Err(anyhow::anyhow!(
        "Failed to parse tool selection from LLM response: {}",
        &llm_response[..llm_response.len().min(200)]
    ))
}

/// Parse execution plan from LLM response
pub fn parse_execution_plan(llm_response: &str) -> Result<DiscoveryExecutionPlan> {
    // Helper to parse JSON string into plan
    fn parse_json_str(json_str: &str) -> Result<DiscoveryExecutionPlan> {
        let mut steps = if let Ok(plan) = serde_json::from_str::<DiscoveryExecutionPlan>(json_str) {
            plan.steps
        } else if let Ok(steps) = serde_json::from_str::<Vec<DiscoveryExecutionStep>>(json_str) {
            steps
        } else {
            return Err(anyhow::anyhow!("JSON structure matches neither DiscoveryExecutionPlan nor Vec<DiscoveryExecutionStep>"));
        };

        // Populate missing step numbers
        for (i, step) in steps.iter_mut().enumerate() {
            if step.step_number.is_none() {
                step.step_number = Some(i + 1);
            }
        }

        Ok(DiscoveryExecutionPlan { steps })
    }

    // Try direct JSON parse
    if let Ok(plan) = parse_json_str(llm_response) {
        return Ok(plan);
    }
    
    // Try robust extraction
    if let Some(json_str) = extract_json(llm_response) {
        match parse_json_str(&json_str) {
            Ok(plan) => return Ok(plan),
            Err(e) => {
                tracing::warn!("Failed to parse extracted JSON for execution plan: {}. Extracted: {}", e, json_str);
            }
        }
    } else {
        tracing::warn!("Failed to extract JSON from execution plan response");
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
    
    // Try robust extraction
    if let Some(json_str) = extract_json(llm_response) {
        match serde_json::from_str::<StepFeedback>(&json_str) {
            Ok(feedback) => return Ok(feedback),
            Err(e) => {
                tracing::warn!("Failed to parse extracted JSON for step feedback: {}. Extracted: {}", e, json_str);
            }
        }
    } else {
        tracing::warn!("Failed to extract JSON from step feedback response");
    }
    
    Err(anyhow::anyhow!(
        "Failed to parse step feedback from LLM response: {}",
        &llm_response[..llm_response.len().min(200)]
    ))
}

/// Extract JSON from text (robust)
pub fn extract_json(text: &str) -> Option<String> {
    // 1. Try Markdown code blocks first
    if let Some(json) = extract_json_from_markdown(text) {
        return Some(json);
    }

    // 2. Try finding the first JSON object or array
    extract_json_structure(text)
}

/// Extract JSON from markdown code block
fn extract_json_from_markdown(text: &str) -> Option<String> {
    // Handle various code block markers
    let markers = ["```json", "```JSON", "```"];
    
    for marker in markers.iter() {
        if let Some(start_idx) = text.find(marker) {
            let content_start = start_idx + marker.len();
            if let Some(end_idx) = text[content_start..].find("```") {
                let content = text[content_start..content_start + end_idx].trim();
                if content.is_empty() {
                    return None;
                }
                return Some(content.to_string());
            }
        }
    }
    None
}

/// Extract the first valid JSON structure (object or array) by counting braces
fn extract_json_structure(text: &str) -> Option<String> {
    let mut start_index = None;
    let mut brace_count = 0;
    let mut in_string = false;
    let mut escape = false;
    let mut is_array = false;

    // Use char_indices to handle multi-byte characters correctly
    for (i, c) in text.char_indices() {
        if start_index.is_none() {
            if c == '{' {
                start_index = Some(i);
                brace_count = 1;
                is_array = false;
            } else if c == '[' {
                start_index = Some(i);
                brace_count = 1;
                is_array = true;
            }
            continue;
        }

        // We are inside a potential JSON structure
        if in_string {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' => in_string = true,
            '{' if !is_array => brace_count += 1,
            '}' if !is_array => {
                brace_count -= 1;
                if brace_count == 0 {
                    // i is the byte index of the closing brace
                    // We need to include the closing brace, so we slice up to i + c.len_utf8()
                    let end = i + c.len_utf8();
                    return Some(text[start_index.unwrap()..end].to_string());
                }
            }
            '[' if is_array => brace_count += 1,
            ']' if is_array => {
                brace_count -= 1;
                if brace_count == 0 {
                    let end = i + c.len_utf8();
                    return Some(text[start_index.unwrap()..end].to_string());
                }
            }
            _ => {}
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
