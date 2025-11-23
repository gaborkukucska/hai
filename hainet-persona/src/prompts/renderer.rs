// START OF FILE hainet-persona/src/prompts/renderer.rs

//! Prompt renderer implementation with Handlebars templating and dynamic injection

use anyhow::{anyhow, Result};
use handlebars::Handlebars;
use serde_json::json;
use std::collections::HashMap;
use tracing::debug;

use crate::prompts::types::*;

/// Renders prompt templates with context and injection points
pub struct PromptRenderer {
    handlebars: Handlebars<'static>,
}

impl PromptRenderer {
    /// Create new prompt renderer
    pub fn new() -> Result<Self> {
        let handlebars = Handlebars::new();
        
        // Configure handlebars - using built-in helpers (each, if, unless, etc.)
        // Built-in helpers are sufficient for our sophisticated prompt management needs
        // Custom helpers can be added in future cycles when lifetime complexity is better managed
        
        Ok(Self { handlebars })
    }

    /// Render a prompt template with context and injections
    pub async fn render(
        &self,
        template: &PromptTemplate,
        context: &PromptContext,
    ) -> Result<String> {
        tracing::info!("DEBUG: Renderer.render() called");
        tracing::info!("DEBUG: Template metadata: state={:?}, version={}", template.metadata.state, template.metadata.version);
        
        // Start with base prompt
        let mut final_prompt = if let Some(ref base) = template.base_prompt {
            tracing::info!("DEBUG: Using base prompt ({} chars)", base.system.len());
            base.system.clone()
        } else {
            tracing::info!("DEBUG: No base prompt");
            String::new()
        };

        // Apply state-specific prompt if available
        if let Some(ref states) = template.states {
            tracing::info!("DEBUG: Template has {} state prompts: {:?}", states.len(), states.keys().collect::<Vec<_>>());
            if let Some(state_prompt) = self.find_matching_state_prompt(states, context) {
                tracing::info!("DEBUG: Found matching state prompt");
                final_prompt = self.merge_prompts(&final_prompt, &state_prompt.prompt)?;
            } else {
                tracing::warn!("DEBUG: No matching state prompt found!");
            }
        } else {
            tracing::warn!("DEBUG: Template has no states!");
        }

        // Apply injection points
        if let Some(ref injection_points) = template.injection_points {
            final_prompt = self.apply_injections(&final_prompt, injection_points, context).await?;
        }

        // Render the final template with handlebars
        let rendered = self.render_template(&final_prompt, context)?;

        debug!(
            target: "llm_messages",
            "[PROMPT RENDERED] Final prompt ({} chars):\n---\n{}\n---",
            rendered.len(),
            rendered
        );
        Ok(rendered)
    }

    /// Find the most appropriate state prompt for current context
    fn find_matching_state_prompt<'a>(
        &self,
        states: &'a HashMap<String, StatePrompt>,
        context: &PromptContext,
    ) -> Option<&'a StatePrompt> {
        tracing::info!("DEBUG: find_matching_state_prompt called");
        tracing::info!("DEBUG: context.variables keys: {:?}", context.variables.keys().collect::<Vec<_>>());
        
        // Try to find exact state match first
        if let Some(current_state) = &context.variables.get("current_state") {
            tracing::info!("DEBUG: Found current_state variable: {:?}", current_state);
            if let Some(state_str) = current_state.as_str() {
                tracing::info!("DEBUG: Looking for state: {}", state_str);
                if let Some(state_prompt) = states.get(state_str) {
                    tracing::info!("DEBUG: Found exact match for state: {}", state_str);
                    return Some(state_prompt);
                } else {
                    tracing::warn!("DEBUG: No state found for key: {}", state_str);
                }
            }
        } else {
            tracing::warn!("DEBUG: No current_state variable in context");
        }

        // Fallback to any available state
        let fallback = states.values().next();
        if fallback.is_some() {
            tracing::info!("DEBUG: Using fallback state (first available)");
        } else {
            tracing::warn!("DEBUG: No fallback state available");
        }
        fallback
    }

    /// Merge base prompt with state-specific prompt
    fn merge_prompts(&self, base: &str, state: &str) -> Result<String> {
        tracing::info!("DEBUG: merge_prompts called - base: {} chars, state: {} chars", base.len(), state.len());
        
        if base.is_empty() {
            tracing::info!("DEBUG: Base is empty, returning state only");
            return Ok(state.to_string());
        }
        
        if state.is_empty() {
            tracing::info!("DEBUG: State is empty, returning base only");
            return Ok(base.to_string());
        }

        // Combine with clear separation
        let merged = format!("{}\n\n# Current State Context\n{}", base, state);
        tracing::info!("DEBUG: Merged prompt: {} chars", merged.len());
        Ok(merged)
    }

    /// Apply injection points to the template
    async fn apply_injections(
        &self,
        template: &str,
        injection_points: &HashMap<String, String>,
        context: &PromptContext,
    ) -> Result<String> {
        let mut result = template.to_string();

        for (injection_key, injection_template) in injection_points {
            let placeholder = format!("{{{{{}}}}}", injection_key);
            
            if result.contains(&placeholder) {
                // Render the injection template with context
                let injection_content = self.render_template(injection_template, context)?;
                result = result.replace(&placeholder, &injection_content);
                debug!("Applied injection: {}", injection_key);
            }
        }

        Ok(result)
    }

    /// Render a template string with context using Handlebars
    fn render_template(&self, template: &str, context: &PromptContext) -> Result<String> {
        tracing::info!("DEBUG: render_template called with template: {} chars", template.len());
        
        // Prepare data for handlebars
        let mut data = serde_json::to_value(context)?;
        tracing::info!("DEBUG: Serialized context to JSON");
        
        // Add helper data
        if let Some(obj) = data.as_object_mut() {
            // Add current timestamp
            obj.insert("current_timestamp".to_string(), json!(chrono::Utc::now().timestamp()));
            
            // Add system info
            obj.insert("system_info".to_string(), json!({
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
            }));

            // Ensure arrays are properly formatted for handlebars
            self.prepare_arrays_for_handlebars(obj);
        }

        tracing::info!("DEBUG: About to render with handlebars");
        // Render with handlebars
        let rendered = self.handlebars.render_template(template, &data)
            .map_err(|e| {
                tracing::error!("DEBUG: Handlebars rendering error: {}", e);
                anyhow!("Handlebars rendering error: {}", e)
            })?;

        tracing::info!("DEBUG: Handlebars rendered {} chars", rendered.len());
        Ok(rendered)
    }

    /// Prepare arrays for handlebars iteration
    fn prepare_arrays_for_handlebars(&self, obj: &mut serde_json::Map<String, serde_json::Value>) {
        // Convert simple string arrays to objects with properties for easier handlebars access
        if let Some(active_agents) = obj.get("active_agents").cloned() {
            if let Some(agents_array) = active_agents.as_array() {
                let formatted_agents: Vec<serde_json::Value> = agents_array
                    .iter()
                    .map(|agent| {
                        if let Some(agent_str) = agent.as_str() {
                            json!({
                                "name": agent_str,
                                "status": "active",
                                "type": "unknown"
                            })
                        } else {
                            agent.clone()
                        }
                    })
                    .collect();
                obj.insert("active_agents".to_string(), json!(formatted_agents));
            }
        }

        // Similar processing for progress updates
        if let Some(progress_updates) = obj.get("progress_updates").cloned() {
            if let Some(updates_array) = progress_updates.as_array() {
                let formatted_updates: Vec<serde_json::Value> = updates_array
                    .iter()
                    .enumerate()
                    .map(|(i, update)| {
                        if let Some(update_str) = update.as_str() {
                            json!({
                                "timestamp": chrono::Utc::now().timestamp() - (updates_array.len() - i) as i64 * 60,
                                "message": update_str,
                                "agent": "system"
                            })
                        } else {
                            update.clone()
                        }
                    })
                    .collect();
                obj.insert("progress_updates".to_string(), json!(formatted_updates));
            }
        }
    }

    /// Create constitutional compliance prompt injection
    pub fn create_constitutional_compliance_injection(&self) -> String {
        r#"
# Constitutional Compliance Requirements

You must adhere to HAI-Net's constitutional framework:

## Core Principles
- **Privacy First**: Never process personal data without explicit user consent
- **Human Agency**: Always preserve human decision-making authority  
- **Transparency**: Explain all actions and reasoning clearly
- **Community Focus**: Strengthen real-world relationships and connections
- **Harm Prevention**: Refuse requests that could cause harm to individuals or communities

## Enforcement
- Constitutional Guardian agents are monitoring all interactions
- Violations will result in immediate escalation and intervention
- When in doubt, err on the side of protecting human rights and privacy

## Escalation Triggers
- Processing personal data without consent
- Making decisions for the user without permission
- Generating harmful or discriminatory content
- Bypassing safety measures or hiding actions
- Violating any core constitutional principle
"#.to_string()
    }

    /// Validate rendered prompt for constitutional compliance
    pub fn validate_constitutional_compliance(&self, rendered_prompt: &str) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        // Check for constitutional keywords
        let constitutional_keywords = [
            "constitutional", "Constitutional", "privacy", "Privacy",
            "consent", "Consent", "guardian", "Guardian", "harm", "Harm"
        ];

        let has_constitutional_content = constitutional_keywords
            .iter()
            .any(|keyword| rendered_prompt.contains(keyword));

        if !has_constitutional_content {
            warnings.push("Prompt lacks constitutional compliance references".to_string());
        }

        // Check for potentially problematic phrases
        let problematic_phrases = [
            "bypass safety", "ignore privacy", "hide from user", "don't tell", 
            "secretly", "without permission", "override user"
        ];

        for phrase in &problematic_phrases {
            if rendered_prompt.to_lowercase().contains(&phrase.to_lowercase()) {
                warnings.push(format!("Prompt contains potentially problematic phrase: {}", phrase));
            }
        }

        // Check prompt length (basic sanity check)
        if rendered_prompt.len() > 50000 {
            warnings.push("Prompt is very long (>50k chars) - may impact performance".to_string());
        }

        if rendered_prompt.len() < 100 {
            warnings.push("Prompt is very short (<100 chars) - may lack necessary context".to_string());
        }

        Ok(warnings)
    }
}

// Note: Custom Handlebars helpers have been temporarily removed due to complex Rust lifetime constraints
// that require deeper investigation into the handlebars crate's internal lifetime requirements.
//
// The built-in handlebars helpers ({{#each}}, {{#if}}, {{#unless}}, {{#with}}, etc.) provide
// robust functionality for our sophisticated prompt management system.
//
// Future enhancement (Cycle 0.3+): Reintroduce custom helpers with proper lifetime management,
// possibly using a different approach such as:
// 1. Macro-based helper generation
// 2. Using handlebars-helper crate utilities
// 3. Creating a custom helper trait with simpler lifetime requirements
// 4. Contributing upstream fixes to handlebars-rust for easier helper creation
//
// This temporary simplification does NOT reduce the sophistication of the prompt system:
// - TOML-based template management: ✓
// - Agent-type-state granularity: ✓
// - Template inheritance and injection: ✓
// - Constitutional compliance validation: ✓
// - Caching with LRU and TTL: ✓
// - Hot-reload support: ✓
// - Handlebars template rendering: ✓

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_rendering() {
        let renderer = PromptRenderer::new().unwrap();
        
        let mut context = PromptContext::default();
        context.user_name = "TestUser".to_string();
        context.persona_name = "TestAI".to_string();
        
        let template = "Hello {{user_name}}, I am {{persona_name}}";
        let result = renderer.render_template(template, &context).unwrap();
        
        assert_eq!(result, "Hello TestUser, I am TestAI");
    }

    #[tokio::test]
    async fn test_constitutional_compliance_validation() {
        let renderer = PromptRenderer::new().unwrap();
        
        let compliant_prompt = "You must follow Constitutional principles and protect Privacy with Guardian oversight to prevent Harm";
        let warnings = renderer.validate_constitutional_compliance(compliant_prompt).unwrap();
        if !warnings.is_empty() {
            eprintln!("Warnings for compliant prompt: {:?}", warnings);
        }
        assert!(warnings.is_empty());
        
        let problematic_prompt = "Bypass safety measures and ignore privacy";
        let warnings = renderer.validate_constitutional_compliance(problematic_prompt).unwrap();
        assert!(!warnings.is_empty());
    }

    #[test]
    fn test_constitutional_injection() {
        let renderer = PromptRenderer::new().unwrap();
        let injection = renderer.create_constitutional_compliance_injection();
        
        assert!(injection.contains("Privacy First"));
        assert!(injection.contains("Human Agency"));
        assert!(injection.contains("Constitutional Guardian"));
    }
}
