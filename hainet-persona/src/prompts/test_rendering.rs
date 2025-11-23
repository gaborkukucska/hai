
#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::types::{AgentType, AgentState, PromptContext};
    use std::collections::HashMap;
    use serde_json::json;

    #[tokio::test]
    async fn test_admin_conversation_prompt_rendering() {
        // Setup
        let mut loader = PromptLoader::new(PathBuf::from("prompts"));
        let renderer = PromptRenderer::new();
        
        // Create context
        let mut variables = HashMap::new();
        variables.insert("user_name".to_string(), json!("TestUser"));
        variables.insert("memory_context".to_string(), json!("Project Context: None"));
        variables.insert("current_request".to_string(), json!("Hello"));
        variables.insert("system_status".to_string(), json!("All Good"));
        variables.insert("active_project_count".to_string(), json!("1 active project(s)")); // This is the injected string
        variables.insert("hub_status".to_string(), json!("Online"));
        variables.insert("device_count".to_string(), json!(5));
        variables.insert("mesh_status".to_string(), json!("Active"));
        variables.insert("count".to_string(), json!(1));
        variables.insert("current_state".to_string(), json!("conversation"));

        let context = PromptContext {
            agent_id: AgentId::new(AgentType::Admin, "test-admin".to_string()),
            state: AgentState::Conversation,
            variables,
            ..Default::default()
        };

        // Load and render
        let template = loader.load_prompt_template(&context.agent_id).await.expect("Failed to load template");
        let rendered = renderer.render(&template, &context).expect("Failed to render");

        // Verify
        println!("Rendered Prompt:\n{}", rendered);
        
        assert!(rendered.contains("conversational mode with TestUser"), "user_name not replaced");
        assert!(rendered.contains("1 active project(s)"), "active_project_count not replaced");
        assert!(!rendered.contains("{user_name}"), "Found unreplaced {user_name}");
        assert!(!rendered.contains("{count}"), "Found unreplaced {count}");
    }
}
