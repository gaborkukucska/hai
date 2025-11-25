//! # START OF FILE hainet-persona/tests/worker_json_parsing_test.rs
//! Integration tests for worker JSON parsing robustness
//! Tests the enhanced JSON extraction and repair logic

use hainet_persona::agents::worker_discovery::{
    parse_execution_plan, DiscoveryExecutionPlan, DiscoveryExecutionStep,
};

#[test]
fn test_valid_json_in_markdown() {
    let response = r#"
Here's the execution plan:

```json
{
  "steps": [
    {
      "step_number": 1,
      "tool": "hainet-files::file_write",
      "params": {"path": "/test.txt", "content": "Hello"},
      "description": "Write test file",
      "depends_on": []
    }
  ]
}
```

That should work!
"#;

    let result = parse_execution_plan(response);
    assert!(result.is_ok(), "Should parse valid JSON in markdown: {:?}", result.err());
    
    let plan = result.unwrap();
    assert_eq!(plan.steps.len(), 1);
    assert_eq!(plan.steps[0].tool, "hainet-files::file_write");
}

#[test]
fn test_truncated_json_missing_closing_braces() {
    // Simulates LLM response cut off mid-JSON
    let response = r#"
```json
{
  "steps": [
    {
      "step_number": 1,
      "tool": "hainet-files::file_write",
      "params": {"path": "/test.txt", "content": "Hello"},
      "description": "Write test file",
      "depends_on": []
    },
    {
      "step_number": 2,
      "tool": "hainet-files::file_read",
      "params": {"path": "/test.txt"
"#;

    let result = parse_execution_plan(response);
    // Should successfully repair and extract at least the first step
    assert!(result.is_ok(), "Should handle truncated JSON: {:?}", result.err());
    
    let plan = result.unwrap();
    assert!(plan.steps.len() >= 1, "Should extract at least one valid step");
}

#[test]
fn test_json_with_trailing_comma() {
    let response = r#"
{
  "steps": [
    {
      "step_number": 1,
      "tool": "hainet-files::file_write",
      "params": {"path": "/test.txt", "content": "Hello"},
      "description": "Write test file",
      "depends_on": [],
    }
  ]
}
"#;

    let result = parse_execution_plan(response);
    assert!(result.is_ok(), "Should handle trailing comma: {:?}", result.err());
    
    let plan = result.unwrap();
    assert_eq!(plan.steps.len(), 1);
}

#[test]
fn test_partial_json_multiple_steps() {
    // JSON with 3 steps but last one is incomplete
    let response = r#"
{
  "steps": [
    {
      "step_number": 1,
      "tool": "hainet-files::directory_create",
      "params": {"path": "/project"},
      "description": "Create directory",
      "depends_on": []
    },
    {
      "step_number": 2,
      "tool": "hainet-files::file_write",
      "params": {"path": "/project/main.rs", "content": "fn main() {}"},
      "description": "Create main file",
      "depends_on": [1]
    },
    {
      "step_number": 3,
      "tool": "hainet-files::file_read",
      "params": {"path": "/project/main.rs"
    }
  ]
}
"#;

    let result = parse_execution_plan(response);
    assert!(result.is_ok(), "Should extract valid steps from partial JSON: {:?}", result.err());
    
    let plan = result.unwrap();
    // Should get first 2 complete steps, skip incomplete 3rd step
    assert!(plan.steps.len() >= 2, "Should extract at least 2 valid steps, got {}", plan.steps.len());
}

#[test]
fn test_json_array_format() {
    // Some LLMs return array directly instead of object with "steps" field
    let response = r#"
[
  {
    "step_number": 1,
    "tool": "hainet-files::file_write",
    "params": {"path": "/test.txt", "content": "Hello"},
    "description": "Write test file",
    "depends_on": []
  }
]
"#;

    let result = parse_execution_plan(response);
    assert!(result.is_ok(), "Should parse array format: {:?}", result.err());
    
    let plan = result.unwrap();
    assert_eq!(plan.steps.len(), 1);
}

#[test]
fn test_missing_step_numbers() {
    // LLM sometimes omits step_number field
    let response = r#"
{
  "steps": [
    {
      "tool": "hainet-files::file_write",
      "params": {"path": "/test.txt", "content": "Hello"},
      "description": "Write test file",
      "depends_on": []
    },
    {
      "tool": "hainet-files::file_read",
      "params": {"path": "/test.txt"},
      "description": "Read test file",
      "depends_on": []
    }
  ]
}
"#;

    let result = parse_execution_plan(response);
    assert!(result.is_ok(), "Should handle missing step numbers: {:?}", result.err());
    
    let plan = result.unwrap();
    assert_eq!(plan.steps.len(), 2);
    // Step numbers should be auto-populated
    assert_eq!(plan.steps[0].step_number, Some(1));
    assert_eq!(plan.steps[1].step_number, Some(2));
}

#[test]
fn test_completely_malformed_json() {
    let response = "This is not JSON at all, just plain text response from LLM.";
    
    let result = parse_execution_plan(response);
    assert!(result.is_err(), "Should fail on completely malformed input");
}

#[test]
fn test_empty_steps_array() {
    let response = r#"{"steps": []}"#;
    
    let result = parse_execution_plan(response);
    // Empty steps is technically valid JSON but not a useful plan
    assert!(result.is_ok(), "Should parse empty steps array");
    
    let plan = result.unwrap();
    assert_eq!(plan.steps.len(), 0);
}

#[test]
fn test_nested_json_in_params() {
    // Complex nested JSON in params field
    let response = r#"
{
  "steps": [
    {
      "step_number": 1,
      "tool": "hainet-files::file_write",
      "params": {
        "path": "/config.json",
        "content": "{\"nested\": {\"key\": \"value\", \"array\": [1, 2, 3]}}"
      },
      "description": "Write config with nested JSON",
      "depends_on": []
    }
  ]
}
"#;

    let result = parse_execution_plan(response);
    assert!(result.is_ok(), "Should handle nested JSON in params: {:?}", result.err());
    
    let plan = result.unwrap();
    assert_eq!(plan.steps.len(), 1);
}
