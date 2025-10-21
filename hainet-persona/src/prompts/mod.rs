// START OF FILE hainet-persona/src/prompts/mod.rs

//! HAI-Net Prompt Management System
//! 
//! This module provides sophisticated prompt management for the multi-agent AI system,
//! supporting granular agent-type-state templates, prompt injection, and constitutional
//! compliance integration.

pub mod loader;
pub mod renderer; 
pub mod cache;
pub mod types;

pub use loader::PromptLoader;
pub use renderer::PromptRenderer;
pub use cache::PromptCache;
pub use types::*;

use anyhow::Result;
use std::path::PathBuf;

/// Main prompt management system
pub struct PromptManager {
    loader: PromptLoader,
    renderer: PromptRenderer,
    cache: PromptCache,
}

impl PromptManager {
    /// Create new prompt manager with specified prompts directory
    pub fn new(prompts_dir: PathBuf) -> Result<Self> {
        let loader = PromptLoader::new(prompts_dir)?;
        let renderer = PromptRenderer::new()?;
        let cache = PromptCache::new();
        
        Ok(Self {
            loader,
            renderer,
            cache,
        })
    }

    /// Load and render a prompt for a specific agent in a specific state
    pub async fn get_prompt(
        &mut self,
        agent_id: &AgentId,
        state: AgentState,
        context: &PromptContext,
    ) -> Result<String> {
        // Check cache first
        let cache_key = PromptCacheKey::new(agent_id, state, context);
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached);
        }

        // Load prompt template
        let template = self.loader.load_prompt_template(agent_id, state).await?;
        
        // Render with context and injections
        let rendered = self.renderer.render(&template, context).await?;
        
        // Cache the result
        self.cache.insert(cache_key, rendered.clone());
        
        Ok(rendered)
    }

    /// Hot reload all prompts (useful for development)
    pub async fn reload_all(&mut self) -> Result<()> {
        self.cache.clear();
        self.loader.reload_all().await?;
        tracing::info!("Hot reloaded all prompts");
        Ok(())
    }

    /// Validate all prompt templates
    pub async fn validate_all(&self) -> Result<ValidationReport> {
        self.loader.validate_all().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_prompt_manager_basic() {
        let temp_dir = TempDir::new().unwrap();
        let prompts_dir = temp_dir.path().to_path_buf();
        
        let mut manager = PromptManager::new(prompts_dir).unwrap();
        
        // This would require test prompt files to be created
        // For now, just test that the manager can be created
        assert!(manager.reload_all().await.is_ok());
    }
}
