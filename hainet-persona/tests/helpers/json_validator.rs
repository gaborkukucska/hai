//! # START OF FILE hainet-persona/tests/helpers/json_validator.rs
//! JSON Schema Validation and Repair for HAI-Net Test Suite
//! 
//! Provides robust JSON parsing with multiple fallback strategies:
//! 1. Direct parse (fast path)
//! 2. Markdown extraction + parse
//! 3. JSON repair (braces, brackets) + parse
//! 4. Regex-based extraction + parse
//! 5. Structured validation with error categorization

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};

// ============================================================================
// JSON Schema Definitions
// ============================================================================

/// Schema for Admin AI project plan response
#[derive(Debug, Clone)]
pub struct ProjectPlanSchema {
    pub requires_title: bool,
    pub requires_overview: bool,
    pub requires_tasks: bool,
    pub min_tasks: usize,
}

impl Default for ProjectPlanSchema {
    fn default() -> Self {
        Self {
            requires_title: true,
            requires_overview: true,
            requires_tasks: true,
            min_tasks: 1,
        }
    }
}

impl ProjectPlanSchema {
    /// Validate a JSON value against project plan schema
    pub fn validate(&self, value: &Value) -> Result<()> {
        let obj = value.as_object()
            .ok_or_else(|| anyhow!("Expected JSON object for project plan"))?;
        
        if self.requires_title {
            obj.get("plan_title")
                .ok_or_else(|| anyhow!("Missing required field: plan_title"))?;
        }
        
        if self.requires_overview {
            obj.get("plan_overview")
                .ok_or_else(|| anyhow!("Missing required field: plan_overview"))?;
        }
        
        if self.requires_tasks {
            let tasks = obj.get("plan_task_list")
                .ok_or_else(|| anyhow!("Missing required field: plan_task_list"))?
                .as_array()
                .ok_or_else(|| anyhow!("plan_task_list must be an array"))?;
            
            if tasks.len() < self.min_tasks {
                return Err(anyhow!("plan_task_list must have at least {} tasks", self.min_tasks));
            }
        }
        
        Ok(())
    }
}

/// Schema for PM agent task decomposition response
#[derive(Debug, Clone)]
pub struct TaskDecompositionSchema {
    pub requires_tasks: bool,
    pub min_tasks: usize,
}

impl Default for TaskDecompositionSchema {
    fn default() -> Self {
        Self {
            requires_tasks: true,
            min_tasks: 1,
        }
    }
}

impl TaskDecompositionSchema {
    /// Validate a JSON value against task decomposition schema
    pub fn validate(&self, value: &Value) -> Result<()> {
        let obj = value.as_object()
            .ok_or_else(|| anyhow!("Expected JSON object for task decomposition"))?;
        
        if self.requires_tasks {
            let tasks = obj.get("tasks")
                .ok_or_else(|| anyhow!("Missing required field: tasks"))?
                .as_array()
                .ok_or_else(|| anyhow!("tasks must be an array"))?;
            
            if tasks.len() < self.min_tasks {
                return Err(anyhow!("tasks must have at least {} items", self.min_tasks));
            }
        }
        
        Ok(())
    }
}

// ============================================================================
// JSON Parsing Strategies
// ============================================================================

/// Result of JSON parsing attempt
#[derive(Debug)]
pub struct ParseResult {
    pub value: Option<Value>,
    pub strategy_used: ParsingStrategy,
    pub error: Option<String>,
}

/// Strategy used for parsing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsingStrategy {
    DirectParse,
    MarkdownExtraction,
    JsonRepair,
    RegexExtraction,
    Failed,
}

impl std::fmt::Display for ParsingStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParsingStrategy::DirectParse => write!(f, "Direct Parse"),
            ParsingStrategy::MarkdownExtraction => write!(f, "Markdown Extraction"),
            ParsingStrategy::JsonRepair => write!(f, "JSON Repair"),
            ParsingStrategy::RegexExtraction => write!(f, "Regex Extraction"),
            ParsingStrategy::Failed => write!(f, "All Strategies Failed"),
        }
    }
}

// ============================================================================
// JSON Validator with Multi-Strategy Parsing
// ============================================================================

pub struct JSONValidator;

impl JSONValidator {
    /// Parse JSON with multiple fallback strategies
    pub fn parse_with_fallbacks(text: &str) -> ParseResult {
        // Strategy 1: Direct parse (fast path)
        if let Ok(value) = serde_json::from_str::<Value>(text) {
            return ParseResult {
                value: Some(value),
                strategy_used: ParsingStrategy::DirectParse,
                error: None,
            };
        }
        
        // Strategy 2: Markdown extraction
        if let Ok(value) = Self::extract_from_markdown(text) {
            return ParseResult {
                value: Some(value),
                strategy_used: ParsingStrategy::MarkdownExtraction,
                error: None,
            };
        }
        
        // Strategy 3: JSON repair (fix braces, brackets)
        if let Ok(value) = Self::repair_and_parse(text) {
            return ParseResult {
                value: Some(value),
                strategy_used: ParsingStrategy::JsonRepair,
                error: None,
            };
        }
        
        // Strategy 4: Regex extraction
        if let Ok(value) = Self::regex_extract(text) {
            return ParseResult {
                value: Some(value),
                strategy_used: ParsingStrategy::RegexExtraction,
                error: None,
            };
        }
        
        // All strategies failed
        ParseResult {
            value: None,
            strategy_used: ParsingStrategy::Failed,
            error: Some(format!("All parsing strategies failed for text: {}", 
                              &text[..text.len().min(100)])),
        }
    }
    
    /// Extract JSON from markdown code blocks
    fn extract_from_markdown(text: &str) -> Result<Value> {
        // Look for ```json ... ``` or ``` ... ```
        let json_start_markers = ["```json\n", "```\n", "```"];
        let json_end_marker = "```";
        
        for start_marker in json_start_markers.iter() {
            if let Some(start_idx) = text.find(start_marker) {
                let json_start = start_idx + start_marker.len();
                
                if let Some(end_idx) = text[json_start..].find(json_end_marker) {
                    let json_text = &text[json_start..json_start + end_idx];
                    let cleaned = json_text.trim();
                    
                    if let Ok(value) = serde_json::from_str::<Value>(cleaned) {
                        return Ok(value);
                    }
                }
            }
        }
        
        Err(anyhow!("No valid JSON found in markdown blocks"))
    }
    
    /// Repair common JSON issues and parse
    fn repair_and_parse(text: &str) -> Result<Value> {
        let mut repaired = text.trim().to_string();
        
        // Remove whitespace, newlines, carriage returns
        repaired = repaired.replace('\n', "").replace('\r', "").replace("  ", " ");
        
        // Count braces and brackets
        let open_braces = repaired.matches('{').count();
        let close_braces = repaired.matches('}').count();
        let open_brackets = repaired.matches('[').count();
        let close_brackets = repaired.matches(']').count();
        
        // Add missing closing braces
        if open_braces > close_braces {
            for _ in 0..(open_braces - close_braces) {
                repaired.push('}');
            }
        }
        
        // Add missing closing brackets
        if open_brackets > close_brackets {
            for _ in 0..(open_brackets - close_brackets) {
                repaired.push(']');
            }
        }
        
        // Try parsing repaired JSON
        serde_json::from_str::<Value>(&repaired)
            .context("Failed to parse after repair")
    }
    
    /// Extract JSON using regex patterns
    fn regex_extract(text: &str) -> Result<Value> {
        // Look for JSON object pattern: { ... }
        let re = regex::Regex::new(r"\{[^{}]*(?:\{[^{}]*\}[^{}]*)*\}")
            .context("Failed to create regex")?;
        
        if let Some(captures) = re.captures(text) {
            if let Some(json_match) = captures.get(0) {
                let json_text = json_match.as_str();
                
                if let Ok(value) = serde_json::from_str::<Value>(json_text) {
                    return Ok(value);
                }
            }
        }
        
        Err(anyhow!("No valid JSON found via regex extraction"))
    }
    
    /// Validate JSON structure before parsing
    pub fn validate_structure(text: &str) -> Result<()> {
        let trimmed = text.trim();
        
        // Check for markdown wrapper
        if trimmed.starts_with("```") {
            return Err(anyhow!("JSON wrapped in markdown code block (will extract)"));
        }
        
        // Check brace/bracket balance
        let open_braces = trimmed.matches('{').count();
        let close_braces = trimmed.matches('}').count();
        let open_brackets = trimmed.matches('[').count();
        let close_brackets = trimmed.matches(']').count();
        
        if open_braces != close_braces {
            return Err(anyhow!("Unbalanced braces: {} open, {} close", 
                              open_braces, close_braces));
        }
        
        if open_brackets != close_brackets {
            return Err(anyhow!("Unbalanced brackets: {} open, {} close", 
                              open_brackets, close_brackets));
        }
        
        Ok(())
    }
    
    /// Parse and validate against schema
    pub fn parse_and_validate<S>(text: &str, schema: &S) -> Result<Value>
    where
        S: SchemaValidator,
    {
        let parse_result = Self::parse_with_fallbacks(text);
        
        let value = parse_result.value
            .ok_or_else(|| anyhow!("Parsing failed: {}", 
                                   parse_result.error.unwrap_or_else(|| "Unknown error".to_string())))?;
        
        schema.validate(&value)?;
        
        Ok(value)
    }
}

// ============================================================================
// Schema Validator Trait
// ============================================================================

pub trait SchemaValidator {
    fn validate(&self, value: &Value) -> Result<()>;
}

impl SchemaValidator for ProjectPlanSchema {
    fn validate(&self, value: &Value) -> Result<()> {
        ProjectPlanSchema::validate(self, value)
    }
}

impl SchemaValidator for TaskDecompositionSchema {
    fn validate(&self, value: &Value) -> Result<()> {
        TaskDecompositionSchema::validate(self, value)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_direct_parse() {
        let json = r#"{"plan_title": "Test", "plan_overview": "Overview", "plan_task_list": ["Task 1"]}"#;
        let result = JSONValidator::parse_with_fallbacks(json);
        
        assert!(result.value.is_some());
        assert_eq!(result.strategy_used, ParsingStrategy::DirectParse);
    }
    
    #[test]
    fn test_markdown_extraction() {
        let json = r#"```json
{"plan_title": "Test", "plan_overview": "Overview", "plan_task_list": ["Task 1"]}
```"#;
        let result = JSONValidator::parse_with_fallbacks(json);
        
        assert!(result.value.is_some());
        assert_eq!(result.strategy_used, ParsingStrategy::MarkdownExtraction);
    }
    
    #[test]
    fn test_json_repair_missing_brace() {
        let json = r#"{"plan_title": "Test", "plan_overview": "Overview", "plan_task_list": ["Task 1"]"#;
        let result = JSONValidator::parse_with_fallbacks(json);
        
        assert!(result.value.is_some());
        assert_eq!(result.strategy_used, ParsingStrategy::JsonRepair);
    }
    
    #[test]
    fn test_json_repair_missing_bracket() {
        let json = r#"{"plan_title": "Test", "plan_overview": "Overview", "plan_task_list": ["Task 1"}"#;
        let result = JSONValidator::parse_with_fallbacks(json);
        
        assert!(result.value.is_some());
        assert_eq!(result.strategy_used, ParsingStrategy::JsonRepair);
    }
    
    #[test]
    fn test_project_plan_schema_validation() {
        let json = json!({
            "plan_title": "Test Project",
            "plan_overview": "This is a test",
            "plan_task_list": ["Task 1", "Task 2"]
        });
        
        let schema = ProjectPlanSchema::default();
        assert!(schema.validate(&json).is_ok());
    }
    
    #[test]
    fn test_project_plan_schema_missing_title() {
        let json = json!({
            "plan_overview": "This is a test",
            "plan_task_list": ["Task 1", "Task 2"]
        });
        
        let schema = ProjectPlanSchema::default();
        assert!(schema.validate(&json).is_err());
    }
    
    #[test]
    fn test_validate_structure() {
        let json = r#"{"test": "value"}"#;
        assert!(JSONValidator::validate_structure(json).is_ok());
        
        let markdown_wrapped = r#"```json
{"test": "value"}
```"#;
        assert!(JSONValidator::validate_structure(markdown_wrapped).is_err());
        
        let unbalanced = r#"{"test": "value""#;
        assert!(JSONValidator::validate_structure(unbalanced).is_err());
    }
}
