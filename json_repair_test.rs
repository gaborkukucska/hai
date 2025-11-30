fn repair_json(json_str: &str) -> Option<String> {
    let mut repaired = json_str.trim().to_string();
    
    // Remove trailing commas before closing braces/brackets (common LLM error)
    repaired = repaired.replace(",}", "}").replace(",]", "]");
    
    // Remove any trailing comma at the end
    while repaired.trim_end().ends_with(',') {
        repaired = repaired.trim_end().trim_end_matches(',').trim_end().to_string();
    }
    
    // Count opening and closing braces/brackets
    let open_braces = repaired.matches('{').count();
    let close_braces = repaired.matches('}').count();
    let open_brackets = repaired.matches('[').count();
    let close_brackets = repaired.matches(']').count();
    
    // Add missing closing brackets first (arrays need to close before objects)
    if open_brackets > close_brackets {
        let missing = open_brackets - close_brackets;
        repaired.push_str(&"]".repeat(missing));
    }
    
    // Add missing closing braces
    if open_braces > close_braces {
        let missing = open_braces - close_braces;
        repaired.push_str(&"}".repeat(missing));
    }
    
    Some(repaired)
}

fn main() {
    let truncated = r#"{
  "steps": [
    {
      "step_number": 1,
      "tool": "worker::get_tool_info",
      "params": {
        "tool_name": "hainet-files::file_read"
      },
      "description": "Retrieve information"#;

    let repaired = repair_json(truncated).unwrap();
    println!("Original:\n{}", truncated);
    println!("Repaired:\n{}", repaired);
    
    // Check validity
    match serde_json::from_str::<serde_json::Value>(&repaired) {
        Ok(_) => println!("Valid JSON!"),
        Err(e) => println!("Invalid JSON: {}", e),
    }
}
