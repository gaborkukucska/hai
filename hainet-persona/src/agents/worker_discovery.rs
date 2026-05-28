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
    #[serde(default)]  // Allow missing field, defaults to empty Vec
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

// =============================================================================
// TrippleEffect-Style Tool Alias System
// =============================================================================
// Ported from TrippleEffect's executor.py tool_name_mapping and base.py 
// ToolParameter.aliases. Provides two layers of aliasing:
//
// 1. Tool-name aliases: Maps hallucinated/shorthand tool names to the correct
//    MCP tool identifier. Small LLMs frequently produce names like "read_file"
//    instead of "hainet-files::file_read".
//
// 2. Parameter-level semantic aliases: Maps synonymous parameter names so the
//    agent's intent is always caught regardless of the specific word used.
//    e.g., "filepath" → "filename", "text" → "content"
// =============================================================================

use std::sync::LazyLock;

/// Tool name alias entry: maps a hallucinated name to (correct_tool, extra_params)
struct ToolNameAlias {
    correct_tool: &'static str,
    /// Extra parameters to inject when the alias is resolved
    extra_params: &'static [(&'static str, &'static str)],
}

/// Static tool-name alias map (mirrors TE's executor.py tool_name_mapping)
static TOOL_NAME_ALIASES: LazyLock<HashMap<&'static str, ToolNameAlias>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // File system aliases (TE: read_file → file_system + action=read)
    m.insert("read_file", ToolNameAlias { correct_tool: "hainet-files::file_read", extra_params: &[] });
    m.insert("write_file", ToolNameAlias { correct_tool: "hainet-files::file_write", extra_params: &[] });
    m.insert("file_read", ToolNameAlias { correct_tool: "hainet-files::file_read", extra_params: &[] });
    m.insert("file_write", ToolNameAlias { correct_tool: "hainet-files::file_write", extra_params: &[] });
    m.insert("list_files", ToolNameAlias { correct_tool: "hainet-files::file_list", extra_params: &[] });
    m.insert("file_list", ToolNameAlias { correct_tool: "hainet-files::file_list", extra_params: &[] });
    m.insert("list_directory", ToolNameAlias { correct_tool: "hainet-files::file_list", extra_params: &[] });
    m.insert("create_file", ToolNameAlias { correct_tool: "hainet-files::file_write", extra_params: &[] });
    m.insert("save_file", ToolNameAlias { correct_tool: "hainet-files::file_write", extra_params: &[] });
    m.insert("delete_file", ToolNameAlias { correct_tool: "hainet-files::file_delete", extra_params: &[] });
    m.insert("mkdir", ToolNameAlias { correct_tool: "hainet-files::file_mkdir", extra_params: &[] });
    m.insert("create_directory", ToolNameAlias { correct_tool: "hainet-files::file_mkdir", extra_params: &[] });

    // Code editor aliases  
    m.insert("edit_file", ToolNameAlias { correct_tool: "hainet-files::file_write", extra_params: &[] });
    m.insert("code_editor", ToolNameAlias { correct_tool: "hainet-files::file_write", extra_params: &[] });

    // Command/terminal aliases (TE: execute_command → command_executor)
    m.insert("execute_command", ToolNameAlias { correct_tool: "hainet-system::run_command", extra_params: &[] });
    m.insert("run_command", ToolNameAlias { correct_tool: "hainet-system::run_command", extra_params: &[] });
    m.insert("terminal", ToolNameAlias { correct_tool: "hainet-system::run_command", extra_params: &[] });
    m.insert("shell", ToolNameAlias { correct_tool: "hainet-system::run_command", extra_params: &[] });
    m.insert("bash", ToolNameAlias { correct_tool: "hainet-system::run_command", extra_params: &[] });

    // Search aliases
    m.insert("search", ToolNameAlias { correct_tool: "hainet-system::web_search", extra_params: &[] });
    m.insert("web_search", ToolNameAlias { correct_tool: "hainet-system::web_search", extra_params: &[] });

    // Git aliases
    m.insert("git_status", ToolNameAlias { correct_tool: "hainet-dev::git_status", extra_params: &[] });
    m.insert("git_commit", ToolNameAlias { correct_tool: "hainet-dev::git_commit", extra_params: &[] });
    m.insert("git_diff", ToolNameAlias { correct_tool: "hainet-dev::git_diff", extra_params: &[] });

    // Framework aliases (TE migration)
    m.insert("request_state", ToolNameAlias { correct_tool: "framework::request_state", extra_params: &[] });

    m
});

/// Parameter-level semantic alias definitions (mirrors TE's ToolParameter.aliases)
/// Maps (canonical_param_name → list of aliases)
static PARAM_ALIASES: LazyLock<HashMap<&'static str, Vec<&'static str>>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // File path aliases (TE: filename.aliases = ["filepath", "file", "file_path", ...])
    m.insert("filename", vec!["filepath", "file", "file_path", "file_name", "target_file", "name", "path"]);
    m.insert("content", vec!["text", "data", "body", "code", "string", "content_str"]);
    m.insert("directory", vec!["dir", "folder", "target_dir", "path"]);
    m.insert("find_text", vec!["search", "find", "search_string", "search_text", "search_term"]);
    m.insert("replace_text", vec!["replace", "replacement", "replacement_text", "replace_string", "replace_term"]);
    m.insert("target_agent_id", vec!["target", "agent", "recipient", "to"]);
    m.insert("message_content", vec!["content", "message", "text"]);
    m.insert("query", vec!["q", "keyword", "search_term", "search"]);
    m.insert("command", vec!["cmd", "shell_command", "exec"]);
    m.insert("commit_message", vec!["message", "msg"]);
    m.insert("url", vec!["link", "href", "endpoint"]);
    m.insert("destination_path", vec!["dest", "destination", "target_path", "output"]);
    m.insert("chunks", vec!["replacements", "replace_chunks", "edits", "replacements_json", "modifications"]);
    m.insert("insert_line", vec!["line_number", "line", "at_line"]);
    m.insert("replace_start_line", vec!["start_line_number", "from_line", "start_line"]);
    m.insert("action", vec!["operation", "op", "command"]);
    m.insert("limit", vec!["count", "max_results", "per_page", "max"]);

    m
});

/// Resolve a potentially hallucinated tool name to its correct MCP identifier.
/// Returns (resolved_tool_name, extra_params_to_inject, was_aliased).
///
/// This mirrors TrippleEffect's executor.py `tool_name_mapping` behavior where
/// agents using shorthand names like "read_file" are transparently redirected
/// to the correct tool without breaking the workflow.
pub fn resolve_tool_alias(tool_name: &str) -> (String, Vec<(String, String)>, bool) {
    // 1. Try exact match in alias map
    if let Some(alias) = TOOL_NAME_ALIASES.get(tool_name) {
        let extras: Vec<(String, String)> = alias.extra_params
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        tracing::info!(
            "Tool alias resolved: '{}' → '{}' (injecting {} extra params)",
            tool_name, alias.correct_tool, extras.len()
        );
        return (alias.correct_tool.to_string(), extras, true);
    }

    // 2. Try case-insensitive match
    let lower = tool_name.to_lowercase();
    for (alias_key, alias_val) in TOOL_NAME_ALIASES.iter() {
        if alias_key.to_lowercase() == lower {
            let extras: Vec<(String, String)> = alias_val.extra_params
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            tracing::info!(
                "Tool alias resolved (case-insensitive): '{}' → '{}'",
                tool_name, alias_val.correct_tool
            );
            return (alias_val.correct_tool.to_string(), extras, true);
        }
    }

    // 3. Try fuzzy match: strip server prefix and check
    // e.g., "hainet-files::read_file" → check "read_file" in aliases
    if let Some(tool_part) = tool_name.split("::").last() {
        if let Some(alias) = TOOL_NAME_ALIASES.get(tool_part) {
            let extras: Vec<(String, String)> = alias.extra_params
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            tracing::info!(
                "Tool alias resolved (after stripping prefix): '{}' → '{}'",
                tool_name, alias.correct_tool
            );
            return (alias.correct_tool.to_string(), extras, true);
        }
    }

    // No alias found — return as-is
    (tool_name.to_string(), vec![], false)
}

/// Resolve parameter-level semantic aliases in a JSON params object.
/// Maps synonymous parameter names to their canonical form so the tool
/// receives the expected parameter names.
///
/// This mirrors TrippleEffect's executor.py dynamic parameter alias resolution
/// where e.g., `filepath` → `filename`, `text` → `content`.
pub fn resolve_param_aliases(params: &mut serde_json::Map<String, Value>) {
    // Build a reverse lookup: alias → canonical name
    // We only remap if the canonical name is NOT already present
    for (canonical, aliases) in PARAM_ALIASES.iter() {
        if params.contains_key(*canonical) {
            continue; // Canonical name already present, no need to alias
        }
        for alias in aliases {
            if let Some(value) = params.remove(*alias) {
                tracing::info!(
                    "Param alias resolved: '{}' → '{}'",
                    alias, canonical
                );
                params.insert(canonical.to_string(), value);
                break; // Only remap the first matching alias
            }
        }
    }
}

/// Resolve tool aliases within a parsed DiscoveryExecutionPlan.
/// Walks through each step and resolves both tool names and parameter aliases.
pub fn resolve_plan_aliases(plan: &mut DiscoveryExecutionPlan, available_tools: &[String]) {
    for step in &mut plan.steps {
        // Resolve tool name
        let (resolved_name, extra_params, was_aliased) = resolve_tool_alias(&step.tool);
        
        if was_aliased {
            // Verify the resolved tool actually exists in the available tools
            let exists = available_tools.iter().any(|t| t == &resolved_name);
            if exists {
                tracing::info!(
                    "Plan step {}: resolved tool '{}' → '{}'",
                    step.step_number.unwrap_or(0), step.tool, resolved_name
                );
                step.tool = resolved_name;
                
                // Inject extra params
                if let Some(params_obj) = step.params.as_object_mut() {
                    for (key, value) in extra_params {
                        if !params_obj.contains_key(&key) {
                            params_obj.insert(key, Value::String(value));
                        }
                    }
                }
            } else {
                tracing::warn!(
                    "Plan step {}: alias '{}' resolved to '{}' but tool not available. Keeping original.",
                    step.step_number.unwrap_or(0), step.tool, resolved_name
                );
            }
        }

        // Resolve parameter aliases
        if let Some(params_obj) = step.params.as_object_mut() {
            resolve_param_aliases(params_obj);
        }
    }
}

/// Resolve tool aliases within a ToolSelectionRequest.
/// Walks through `needed_tools` and resolves any aliased names.
pub fn resolve_selection_aliases(selection: &mut ToolSelectionRequest, available_tools: &[String]) {
    let mut resolved_tools = Vec::new();
    for tool_name in &selection.needed_tools {
        let (resolved, _, was_aliased) = resolve_tool_alias(tool_name);
        if was_aliased {
            let exists = available_tools.iter().any(|t| t == &resolved);
            if exists {
                tracing::info!("Selection alias: '{}' → '{}'", tool_name, resolved);
                resolved_tools.push(resolved);
            } else {
                // Keep original if resolved name doesn't exist
                resolved_tools.push(tool_name.clone());
            }
        } else {
            resolved_tools.push(tool_name.clone());
        }
    }
    selection.needed_tools = resolved_tools;
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
    
    // Try robust extraction with multiple strategies
    if let Some(json_str) = extract_json_robust(llm_response) {
        tracing::debug!("Extracted JSON (robust): {}", &json_str[..json_str.len().min(200)]);
        
        // Try parsing extracted JSON
        match parse_json_str(&json_str) {
            Ok(plan) => return Ok(plan),
            Err(e) => {
                tracing::warn!("Failed to parse extracted JSON for execution plan: {}. Extracted: {}", e, &json_str[..json_str.len().min(300)]);
                
                // Try repairing common JSON errors
                if let Some(repaired) = repair_json(&json_str) {
                    tracing::debug!("Attempting to parse repaired JSON: {}", &repaired[..repaired.len().min(200)]);
                    match parse_json_str(&repaired) {
                        Ok(plan) => {
                            tracing::info!("Successfully parsed execution plan after JSON repair");
                            return Ok(plan);
                        }
                        Err(repair_err) => {
                            tracing::warn!("Failed to parse repaired JSON: {}", repair_err);
                        }
                    }
                }
                
                // Try partial parse (extract valid steps from incomplete JSON)
                if let Ok(plan) = parse_partial_execution_plan(&json_str) {
                    tracing::info!("Successfully extracted {} steps from partial JSON", plan.steps.len());
                    return Ok(plan);
                }
            }
        }
    } else {
        tracing::warn!("Failed to extract JSON from execution plan response");
    }
    
    // Log full response for debugging (truncated to avoid log spam)
    tracing::debug!("Full LLM response (truncated): {}", &llm_response[..llm_response.len().min(500)]);
    
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
            
            // Try to find closing ```
            if let Some(end_idx) = text[content_start..].find("```") {
                let content = text[content_start..content_start + end_idx].trim();
                if content.is_empty() {
                    return None;
                }
                return Some(content.to_string());
            } else {
                // No closing ``` found - LLM response was likely truncated
                // Extract everything after the opening marker
                let content = text[content_start..].trim();
                if !content.is_empty() {
                    tracing::debug!("Markdown code block missing closing ```, extracting {} chars", content.len());
                    return Some(content.to_string());
                }
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

/// Extract JSON with multiple fallback strategies (robust version)
/// This function is designed to handle messy LLM output with explanatory text,
/// multiple JSON objects, and incorrect/missing markdown markers.
pub fn extract_json_robust(text: &str) -> Option<String> {
    // Strategy 1: Try markdown code blocks first (most reliable when present)
    if let Some(mut json) = extract_json_from_markdown(text) {
        // Always attempt repair on extracted JSON (handles truncation in markdown blocks)
        if let Some(repaired) = repair_json(&json) {
            json = repaired;
        }
        return Some(json);
    }

    // Strategy 2: Extract ALL JSON-like structures and try each one
    // This handles cases where LLM writes explanatory text before/after JSON
    let json_candidates = extract_all_json_structures(text);
    
    if !json_candidates.is_empty() {
        tracing::debug!("Found {} JSON candidate(s) in text", json_candidates.len());
        
        // Try each candidate, preferring the largest valid one
        // (larger JSON is more likely to be the complete execution plan)
        let mut best_candidate: Option<String> = None;
        let mut best_size = 0;
        
        for candidate in &json_candidates {
            let mut repaired = candidate.clone();
            if let Some(fixed) = repair_json(candidate) {
                repaired = fixed;
            }
            
            // Quick validation: try to parse as JSON Value
            if serde_json::from_str::<serde_json::Value>(&repaired).is_ok() {
                if repaired.len() > best_size {
                    best_size = repaired.len();
                    best_candidate = Some(repaired);
                }
            }
        }
        
        if let Some(best) = best_candidate {
            tracing::debug!("Selected best JSON candidate ({} chars)", best.len());
            return Some(best);
        }
        
        // If no candidate parsed successfully, return the first one anyway
        // (let the caller's repair logic handle it)
        if let Some(first) = json_candidates.first() {
            tracing::debug!("No valid JSON found, returning first candidate for repair");
            return repair_json(first).or_else(|| Some(first.clone()));
        }
    }

    // Strategy 3: Handle truncated JSON - find first { to last available }
    // This handles cases where LLM response was cut off mid-JSON
    if let Some(start) = text.find('{') {
        // Find the last closing brace (even if JSON is incomplete)
        if let Some(end) = text.rfind('}') {
            if end > start {
                let extracted = &text[start..=end];
                tracing::debug!("Extracted truncated JSON (start {{ to last }}): {} chars", extracted.len());
                // Attempt repair
                if let Some(repaired) = repair_json(extracted) {
                    return Some(repaired);
                }
                return Some(extracted.to_string());
            }
        }
        
        // If no closing brace found, extract up to end and definitely repair
        let extracted = &text[start..];
        tracing::debug!("Extracted incomplete JSON (no closing brace found): {} chars", extracted.len());
        if let Some(repaired) = repair_json(extracted) {
            return Some(repaired);
        }
        return Some(extracted.to_string());
    }

    None
}

/// Extract ALL JSON structures from text (objects and arrays)
/// Returns a vector of JSON strings, ordered by appearance in text
fn extract_all_json_structures(text: &str) -> Vec<String> {
    let mut results = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    
    while i < chars.len() {
        // Skip non-JSON characters
        if chars[i] != '{' && chars[i] != '[' {
            i += 1;
            continue;
        }
        
        // Found potential JSON start
        let start = i;
        let is_array = chars[i] == '[';
        let mut brace_count = 1;
        let mut in_string = false;
        let mut escape = false;
        i += 1;
        
        // Find matching closing brace/bracket
        while i < chars.len() {
            let c = chars[i];
            
            if in_string {
                if escape {
                    escape = false;
                } else if c == '\\' {
                    escape = true;
                } else if c == '"' {
                    in_string = false;
                }
            } else {
                match c {
                    '"' => in_string = true,
                    '{' if !is_array => brace_count += 1,
                    '}' if !is_array => {
                        brace_count -= 1;
                        if brace_count == 0 {
                            // Found complete JSON object
                            let json_str: String = chars[start..=i].iter().collect();
                            results.push(json_str);
                            break;
                        }
                    }
                    '[' if is_array => brace_count += 1,
                    ']' if is_array => {
                        brace_count -= 1;
                        if brace_count == 0 {
                            // Found complete JSON array
                            let json_str: String = chars[start..=i].iter().collect();
                            results.push(json_str);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            i += 1;
        }
        
        // Move to next character after this JSON structure
        i += 1;
    }
    
    results
}

/// Repair common JSON errors (missing closing braces, trailing commas, etc.)
fn repair_json(json_str: &str) -> Option<String> {
    let mut repaired = json_str.trim().to_string();
    
    // Check for unclosed string
    let mut in_string = false;
    let mut escape = false;
    for c in repaired.chars() {
        if in_string {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
        } else {
            if c == '"' {
                in_string = true;
            }
        }
    }
    
    if in_string {
        tracing::debug!("Repairing JSON: closing unclosed string");
        repaired.push('"');
    }

    // Remove trailing commas before closing braces/brackets (common LLM error)
    repaired = repaired.replace(",}", "}").replace(",]", "]");
    
    // Remove any trailing comma at the end
    while repaired.trim_end().ends_with(',') {
        repaired = repaired.trim_end().trim_end_matches(',').trim_end().to_string();
    }
    
    // Count opening and closing braces/brackets
    let mut open_braces = 0;
    let mut close_braces = 0;
    let mut open_brackets = 0;
    let mut close_brackets = 0;
    let mut in_string = false;
    let mut escape = false;
    
    for c in repaired.chars() {
        if in_string {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
        } else {
            match c {
                '"' => in_string = true,
                '{' => open_braces += 1,
                '}' => close_braces += 1,
                '[' => open_brackets += 1,
                ']' => close_brackets += 1,
                _ => {}
            }
        }
    }
    
    // Add missing closing brackets first (arrays need to close before objects)
    if open_brackets > close_brackets {
        let missing = open_brackets - close_brackets;
        tracing::debug!("Repairing JSON: adding {} missing closing bracket(s)", missing);
        repaired.push_str(&"]".repeat(missing));
    }
    
    // Add missing closing braces
    if open_braces > close_braces {
        let missing = open_braces - close_braces;
        tracing::debug!("Repairing JSON: adding {} missing closing brace(s)", missing);
        repaired.push_str(&"}".repeat(missing));
    }
    
    Some(repaired)
}

/// Parse partial execution plan from incomplete JSON
/// Extracts as many valid steps as possible from truncated JSON
fn parse_partial_execution_plan(json_str: &str) -> Result<DiscoveryExecutionPlan> {
    // First, try to parse as a JSON Value
    let value_result = serde_json::from_str::<Value>(json_str);
    
    if let Ok(value) = value_result {
        // Successfully parsed as Value, extract steps array
        let steps_array = if let Some(obj) = value.as_object() {
            obj.get("steps")
                .and_then(|v| v.as_array())
                .ok_or_else(|| anyhow::anyhow!("No 'steps' array found in object"))?
        } else if let Some(arr) = value.as_array() {
            arr
        } else {
            return Err(anyhow::anyhow!("JSON is neither object with 'steps' nor array"));
        };
        
        // Parse each step individually, skipping invalid ones
        let mut valid_steps = Vec::new();
        for (i, step_value) in steps_array.iter().enumerate() {
            match serde_json::from_value::<DiscoveryExecutionStep>(step_value.clone()) {
                Ok(mut step) => {
                    if step.step_number.is_none() {
                        step.step_number = Some(i + 1);
                    }
                    valid_steps.push(step);
                }
                Err(e) => {
                    tracing::debug!("Skipping invalid step {}: {}", i + 1, e);
                }
            }
        }
        
        if valid_steps.is_empty() {
            return Err(anyhow::anyhow!("No valid steps found in partial JSON"));
        }
        
        tracing::info!("Extracted {} valid steps from partial JSON (out of {} total)", 
                       valid_steps.len(), steps_array.len());
        
        return Ok(DiscoveryExecutionPlan { steps: valid_steps });
    }
    
    // If parsing as Value failed, try to extract valid JSON objects manually
    // This handles cases where JSON is severely truncated mid-structure
    tracing::debug!("Failed to parse as JSON Value, attempting manual extraction");
    
    // Look for complete step objects by finding {...} patterns
    let mut valid_steps = Vec::new();
    let mut current_pos = 0;
    let chars: Vec<char> = json_str.chars().collect();
    
    while current_pos < chars.len() {
        // Find next opening brace
        while current_pos < chars.len() && chars[current_pos] != '{' {
            current_pos += 1;
        }
        
        if current_pos >= chars.len() {
            break;
        }
        
        // Try to find matching closing brace
        let start = current_pos;
        let mut depth = 0;
        let mut in_string = false;
        let mut escape = false;
        
        while current_pos < chars.len() {
            let c = chars[current_pos];
            
            if in_string {
                if escape {
                    escape = false;
                } else if c == '\\' {
                    escape = true;
                } else if c == '"' {
                    in_string = false;
                }
            } else {
                match c {
                    '"' => in_string = true,
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            // Found complete object
                            let obj_str: String = chars[start..=current_pos].iter().collect();
                            match serde_json::from_str::<DiscoveryExecutionStep>(&obj_str) {
                                Ok(mut step) => {
                                    if step.step_number.is_none() {
                                        step.step_number = Some(valid_steps.len() + 1);
                                    }
                                    valid_steps.push(step);
                                    tracing::debug!("Extracted valid step from manual parsing");
                                }
                                Err(e) => {
                                    tracing::debug!("Found complete object but failed to parse as step: {}", e);
                                }
                            }
                            break;
                        }
                    }
                    _ => {}
                }
            }
            
            current_pos += 1;
        }
        
        current_pos += 1;
    }
    
    if valid_steps.is_empty() {
        return Err(anyhow::anyhow!("No valid steps found in severely truncated JSON"));
    }
    
    tracing::info!("Manually extracted {} valid steps from severely truncated JSON", valid_steps.len());
    Ok(DiscoveryExecutionPlan { steps: valid_steps })
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
