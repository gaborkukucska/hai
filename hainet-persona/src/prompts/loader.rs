// START OF FILE hainet-persona/src/prompts/loader.rs

//! Prompt loader implementation with TOML parsing, template inheritance, and hot-reload

use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs as async_fs;
use tracing::{debug, info, warn};

use crate::prompts::types::*;

/// Loads and manages prompt templates from TOML files
pub struct PromptLoader {
    prompts_dir: PathBuf,
    template_cache: HashMap<String, PromptTemplate>,
    file_timestamps: HashMap<PathBuf, std::time::SystemTime>,
}

impl PromptLoader {
    /// Create new prompt loader
    pub fn new(prompts_dir: PathBuf) -> Result<Self> {
        if !prompts_dir.exists() {
            return Err(anyhow!("Prompts directory does not exist: {:?}", prompts_dir));
        }

        Ok(Self {
            prompts_dir,
            template_cache: HashMap::new(),
            file_timestamps: HashMap::new(),
        })
    }

    /// Load a prompt template for a specific agent and state
    pub async fn load_prompt_template(
        &mut self,
        agent_id: &AgentId,
        state: AgentState,
    ) -> Result<PromptTemplate> {
        // Try to load agent-type-state specific template first
        let specific_path = self.get_agent_state_prompt_path(agent_id, state);
        if let Ok(template) = self.load_template_from_path(&specific_path).await {
            debug!("Loaded specific prompt: {:?}", specific_path);
            return Ok(template);
        }

        // Fallback to agent-type generic template
        let agent_path = self.get_agent_prompt_path(agent_id);
        if let Ok(mut template) = self.load_template_from_path(&agent_path).await {
            // Inject state-specific prompt if available
            if let Some(state_prompt) = self.load_state_prompt(state).await? {
                template = self.merge_state_prompt(template, state_prompt, state)?;
            }
            debug!("Loaded agent template with state injection: {:?}", agent_path);
            return Ok(template);
        }

        // Fallback to generic state template
        let state_path = self.get_state_prompt_path(state);
        if let Ok(template) = self.load_template_from_path(&state_path).await {
            debug!("Loaded state template: {:?}", state_path);
            return Ok(template);
        }

        Err(anyhow!(
            "No prompt template found for agent {:?} in state {:?}",
            agent_id,
            state
        ))
    }

    /// Load template from a specific file path
    async fn load_template_from_path(&mut self, path: &Path) -> Result<PromptTemplate> {
        if !path.exists() {
            return Err(anyhow!("Template file does not exist: {:?}", path));
        }

        // Check if we need to reload (file changed)
        let metadata = async_fs::metadata(path).await?;
        let modified = metadata.modified()?;

        let cache_key = path.to_string_lossy().to_string();
        
        if let Some(cached_time) = self.file_timestamps.get(path) {
            if *cached_time >= modified {
                if let Some(cached_template) = self.template_cache.get(&cache_key) {
                    debug!("Using cached template: {:?}", path);
                    return Ok(cached_template.clone());
                }
            }
        }

        // Load and parse the TOML file
        let content = async_fs::read_to_string(path).await?;
        let template: PromptTemplate = toml::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse TOML template {:?}: {}", path, e))?;

        // Validate the template
        self.validate_template(&template, path)?;

        // Update cache
        self.template_cache.insert(cache_key, template.clone());
        self.file_timestamps.insert(path.to_path_buf(), modified);

        info!("Loaded prompt template: {:?}", path);
        Ok(template)
    }

    /// Load state-specific prompt snippet
    async fn load_state_prompt(&mut self, state: AgentState) -> Result<Option<StatePrompt>> {
        let state_path = self.get_state_prompt_path(state);
        
        if !state_path.exists() {
            return Ok(None);
        }

        let content = async_fs::read_to_string(&state_path).await?;
        
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct StateFile {
            admin: Option<StatePrompt>,
            pm_comms: Option<StatePrompt>,
            pm_knowledge: Option<StatePrompt>,
            pm_system: Option<StatePrompt>,
            workers: Option<HashMap<String, StatePrompt>>,
        }

        let state_file: StateFile = toml::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse state file {:?}: {}", state_path, e))?;

        // For now, return the admin prompt as default
        // TODO: Make this more sophisticated based on agent type
        Ok(state_file.admin)
    }

    /// Merge a state prompt into an agent template
    fn merge_state_prompt(
        &self,
        mut template: PromptTemplate,
        state_prompt: StatePrompt,
        state: AgentState,
    ) -> Result<PromptTemplate> {
        let state_name = format!("{:?}", state).to_lowercase();
        
        if template.states.is_none() {
            template.states = Some(HashMap::new());
        }
        
        template.states.as_mut().unwrap().insert(state_name, state_prompt);
        Ok(template)
    }

    /// Get path for agent-type-state specific prompt
    fn get_agent_state_prompt_path(&self, agent_id: &AgentId, state: AgentState) -> PathBuf {
        let agent_type = format!("{:?}", agent_id.agent_type).to_lowercase();
        let state_name = format!("{:?}", state).to_lowercase();
        let filename = format!("{}-{}.toml", agent_type, state_name);
        self.prompts_dir.join("agents").join(filename)
    }

    /// Get path for agent-type generic prompt
    fn get_agent_prompt_path(&self, agent_id: &AgentId) -> PathBuf {
        let agent_type = format!("{:?}", agent_id.agent_type).to_lowercase();
        let filename = format!("{}.toml", agent_type);
        self.prompts_dir.join("agents").join(filename)
    }

    /// Get path for state-specific prompt
    fn get_state_prompt_path(&self, state: AgentState) -> PathBuf {
        let state_name = format!("{:?}", state).to_lowercase();
        let filename = format!("{}.toml", state_name);
        self.prompts_dir.join("states").join(filename)
    }

    /// Validate a prompt template
    fn validate_template(&self, template: &PromptTemplate, path: &Path) -> Result<()> {
        // Check metadata
        if template.metadata.version.is_empty() {
            warn!("Template {:?} missing version", path);
        }

        // Check for required constitutional compliance
        if template.metadata.constitutional_compliance == Some(true) {
            // Ensure constitutional elements are present
            if let Some(ref base_prompt) = template.base_prompt {
                if !base_prompt.system.contains("constitutional") 
                    && !base_prompt.system.contains("Constitutional") {
                    warn!("Template {:?} claims constitutional compliance but has no constitutional references", path);
                }
            }
        }

        // Validate injection points
        if let Some(ref injection_points) = template.injection_points {
            for (key, value) in injection_points {
                if value.trim().is_empty() {
                    warn!("Template {:?} has empty injection point: {}", path, key);
                }
            }
        }

        Ok(())
    }

    /// Reload all cached templates
    pub async fn reload_all(&mut self) -> Result<()> {
        self.template_cache.clear();
        self.file_timestamps.clear();
        info!("Cleared all cached templates for reload");
        Ok(())
    }

    /// Validate all prompt templates in the directory
    pub async fn validate_all(&self) -> Result<ValidationReport> {
        let mut report = ValidationReport {
            total_templates: 0,
            valid_templates: 0,
            errors: Vec::new(),
            warnings: Vec::new(),
        };

        // Scan all TOML files in the prompts directory
        let paths = self.find_all_prompt_files().await?;
        
        for path in paths {
            report.total_templates += 1;
            
            match self.validate_single_file(&path).await {
                Ok(warnings) => {
                    report.valid_templates += 1;
                    report.warnings.extend(warnings);
                }
                Err(e) => {
                    report.errors.push(ValidationError {
                        file_path: path.to_string_lossy().to_string(),
                        error_type: "ParseError".to_string(),
                        message: e.to_string(),
                    });
                }
            }
        }

        Ok(report)
    }

    /// Find all TOML files in the prompts directory
    async fn find_all_prompt_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        
        // Recursively scan the prompts directory
        let mut stack = vec![self.prompts_dir.clone()];
        
        while let Some(dir) = stack.pop() {
            let mut entries = async_fs::read_dir(dir).await?;
            
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                    files.push(path);
                }
            }
        }
        
        Ok(files)
    }

    /// Validate a single prompt file
    async fn validate_single_file(&self, path: &Path) -> Result<Vec<ValidationWarning>> {
        let content = async_fs::read_to_string(path).await?;
        let _template: PromptTemplate = toml::from_str(&content)?;
        
        // For now, just return empty warnings
        // TODO: Add more sophisticated validation
        Ok(Vec::new())
    }

    /// Load system-wide core instructions
    pub async fn load_core_instructions(&mut self) -> Result<PromptTemplate> {
        let path = self.prompts_dir.join("system").join("core_instructions.toml");
        self.load_template_from_path(&path).await
    }

    /// Load system-wide safety guidelines
    pub async fn load_safety_guidelines(&mut self) -> Result<PromptTemplate> {
        let path = self.prompts_dir.join("system").join("safety.toml");
        self.load_template_from_path(&path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    async fn create_test_prompt(dir: &Path, subdir: &str, filename: &str, content: &str) -> Result<()> {
        let full_dir = dir.join(subdir);
        fs::create_dir_all(&full_dir).await?;
        fs::write(full_dir.join(filename), content).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_prompt_loader_basic() {
        let temp_dir = TempDir::new().unwrap();
        let prompts_dir = temp_dir.path().to_path_buf();

        // Create test prompt structure
        create_test_prompt(
            &prompts_dir,
            "system",
            "core_instructions.toml",
            r#"
[metadata]
version = "1.0.0"
description = "Test core instructions"

[core_principles]
test = "test value"
"#,
        ).await.unwrap();

        let mut loader = PromptLoader::new(prompts_dir).unwrap();
        let core = loader.load_core_instructions().await.unwrap();
        
        assert_eq!(core.metadata.version, "1.0.0");
        assert_eq!(core.metadata.description, "Test core instructions");
    }

    #[tokio::test]
    async fn test_validation_report() {
        let temp_dir = TempDir::new().unwrap();
        let prompts_dir = temp_dir.path().to_path_buf();

        // Create valid and invalid prompts
        create_test_prompt(
            &prompts_dir,
            "agents",
            "valid.toml",
            r#"
[metadata]
version = "1.0.0"
description = "Valid prompt"
"#,
        ).await.unwrap();

        create_test_prompt(
            &prompts_dir,
            "agents", 
            "invalid.toml",
            "invalid toml content {{{",
        ).await.unwrap();

        let loader = PromptLoader::new(prompts_dir).unwrap();
        let report = loader.validate_all().await.unwrap();
        
        assert_eq!(report.total_templates, 2);
        assert_eq!(report.valid_templates, 1);
        assert_eq!(report.errors.len(), 1);
    }
}
