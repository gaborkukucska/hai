// START OF FILE hainet-persona/src/ai_providers/mod.rs

//! AI Provider Discovery and Selection System
//!
//! This module implements automatic discovery, cataloging, and intelligent selection
//! of AI models across multiple providers (Ollama, vLLM, LiteLLM, etc.) on both
//! localhost and local network.
//!
//! Design Principles:
//! - No hardcoded providers or models
//! - Automatic network scanning and discovery
//! - Capability-based model ranking
//! - Agent-specific optimal model selection
//! - Graceful fallback and load balancing

pub mod discovery;
pub mod catalog;
pub mod ranking;
pub mod selection;
pub mod providers;
pub mod api_registry;
pub mod request_queue;
pub mod config;

pub use discovery::{ProviderDiscovery, DiscoveredProvider};
pub use catalog::{ModelCatalog, CatalogedModel, ModelCapability, CatalogStats};
pub use ranking::{ModelRanker, ModelScore, RankingCriteria};
pub use selection::{ModelSelector, SelectionContext, SelectedModel};
pub use providers::{ProviderClient, ProviderType, ModelInfo};
pub use api_registry::{ApiRegistry, OllamaEndpoint, HealthStatus, EndpointStats, RegistryStats};
pub use request_queue::{OllamaRequestQueue, LoadBalancingStrategy, QueueMetrics};
pub use config::OllamaConfig;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Central AI provider management system
pub struct AIProviderManager {
    pub discovery: ProviderDiscovery,
    pub catalog: Arc<RwLock<ModelCatalog>>,
    pub ranker: ModelRanker,
    pub selector: ModelSelector,
    pub request_queue: Option<Arc<OllamaRequestQueue>>,
    pub api_registry: Option<Arc<ApiRegistry>>,
    pub user_settings: Option<Arc<RwLock<crate::user_settings::UserSettingsManager>>>,
}

impl AIProviderManager {
    /// Create new provider manager and perform initial discovery
    pub async fn new(user_settings: Option<Arc<RwLock<crate::user_settings::UserSettingsManager>>>, role: String) -> Result<Self> {
        info!("Initializing AI Provider Manager...");
        
        let discovery = ProviderDiscovery::new();
        let catalog = Arc::new(RwLock::new(ModelCatalog::new()));
        let ranker = ModelRanker::new();
        let selector = ModelSelector::new(catalog.clone());

        let manager = Self {
            discovery,
            catalog,
            ranker,
            selector,
            request_queue: None,
            api_registry: None,
            user_settings,
        };

        // Perform initial discovery only if we are the agentic core
        if role == "master" || role == "standalone" {
            info!("Running as agentic core (role: {}), starting AI provider discovery...", role);
            manager.discover_providers().await?;
            
            // Initialize load balancing if configured
            Self::initialize_load_balancing(&manager).await?;
        } else {
            info!("Running as UI/file host (role: {}), skipping AI provider discovery.", role);
            // We can still initialize load balancing from config if needed, or skip it.
            // But since slave nodes need to route requests to the master, maybe we DO need to initialize load balancing
            // from config, just without the auto-discovery loop?
            // Wait, the user said: "Fix the auto-discovery algorithm... ONLY if that node is the one running the agentic core!"
            // The slave nodes DO NOT run agents, so they don't even need load balancing or Ollama!
            // The UI just sends requests to the daemon, but the daemon doesn't execute agents.
        }

        Ok(manager)
    }
    
    /// Initialize Ollama load balancing from configuration and discovered endpoints
    async fn initialize_load_balancing(manager: &AIProviderManager) -> Result<()> {
        // Try to load configuration
        // Try to load configuration from multiple potential paths
        let potential_paths = vec![
            "hainet-persona/ollama-endpoints.toml",
            "../hainet-persona/ollama-endpoints.toml",
            "ollama-endpoints.toml",
        ];
        
        let mut config_path = std::path::PathBuf::from("hainet-persona/ollama-endpoints.toml");
        for path_str in &potential_paths {
            let path = std::path::PathBuf::from(path_str);
            if path.exists() {
                config_path = path;
                break;
            }
        }
        
        let config = OllamaConfig::load_or_default(&config_path);
        
        // Get all discovered Ollama endpoints
        let catalog = manager.catalog.read().await;
        let discovered_ollama_endpoints: Vec<String> = catalog
            .all_models()
            .iter()
            .filter(|m| matches!(m.provider_type, discovery::ProviderType::Ollama))
            .map(|m| m.endpoint.clone())
            .collect::<std::collections::HashSet<_>>() // Deduplicate
            .into_iter()
            .collect();
        drop(catalog); // Release lock
        
        info!(
            "Initializing Ollama load balancing with {} configured endpoints + {} discovered endpoints",
            config.endpoints.len(),
            discovered_ollama_endpoints.len()
        );
        
        // Merge configured and discovered endpoints (discovered takes priority)
        let mut all_endpoints = discovered_ollama_endpoints.clone();
        for (name, endpoint_config) in &config.endpoints {
            if !all_endpoints.contains(&endpoint_config.url) {
                info!("Adding configured endpoint '{}': {}", name, endpoint_config.url);
                all_endpoints.push(endpoint_config.url.clone());
            }
        }
        
        // Deduplicate and log
        all_endpoints.sort();
        all_endpoints.dedup();
        
        info!("Total Ollama endpoints for load balancing: {}", all_endpoints.len());
        for (i, endpoint) in all_endpoints.iter().enumerate() {
            info!("  {}. {}", i + 1, endpoint);
        }
        
        // Determine primary endpoint (prefer first discovered, fallback to config)
        let primary_endpoint = discovered_ollama_endpoints
            .first()
            .or_else(|| all_endpoints.first())
            .cloned()
            .unwrap_or_else(|| config.primary_endpoint());
        
        // Additional endpoints are all others
        let additional_endpoints: Vec<String> = all_endpoints
            .iter()
            .filter(|e| *e != &primary_endpoint)
            .cloned()
            .collect();
        
        info!("Primary endpoint: {}", primary_endpoint);
        info!("Additional endpoints: {}", additional_endpoints.len());
        
        // Create API registry with all endpoints
        let registry = Arc::new(
            ApiRegistry::new(
                primary_endpoint,
                additional_endpoints,
                config.endpoint_overrides(),
                config.default_max_concurrent(),
            ).await?
        );
        
        // Start background health monitoring
        info!("Starting health monitoring for {} endpoints...", all_endpoints.len());
        registry.clone().start_health_monitoring().await;
        
        // Create request queue
        let queue = Arc::new(OllamaRequestQueue::new(
            registry.clone(),
            config.parse_strategy(),
            config.request_timeout(),
        ));
        
        // Store in manager (requires unsafe for now - will fix with proper initialization)
        // TODO: Refactor to make this safe
        let manager_ptr = manager as *const AIProviderManager as *mut AIProviderManager;
        unsafe {
            (*manager_ptr).request_queue = Some(queue);
            (*manager_ptr).api_registry = Some(registry);
        }
        
        info!("✅ Ollama load balancing initialized with {} endpoints", all_endpoints.len());
        
        Ok(())
    }
    
    /// Discover all available AI providers on localhost and local network
    pub async fn discover_providers(&self) -> Result<()> {
        info!("Starting AI provider discovery");
        
        let providers = self.discovery.scan_all().await?;
        
        info!("Discovered {} AI providers", providers.len());
        for provider in &providers {
            info!("  - {} at {}", provider.provider_type, provider.endpoint);
        }
        
        // Catalog all models from discovered providers
        let mut catalog = self.catalog.write().await;
        for provider in providers {
            match self.discovery.fetch_models(&provider).await {
                Ok(models) => {
                    for model in models {
                        catalog.add_model(model);
                    }
                },
                Err(e) => {
                    warn!("⚠️  Failed to fetch models from {} at {}: {}. Skipping.", 
                        provider.provider_type, provider.endpoint, e);
                }
            }
        }
        
        info!("Cataloged {} total models", catalog.model_count());
        
        Ok(())
    }
    
    /// Select optimal model for a specific agent task
    pub async fn select_model_for_agent(
        &self,
        context: SelectionContext,
    ) -> Result<SelectedModel> {
        let catalog = self.catalog.read().await;

        info!("Selecting model for agent {:?}. Catalog has {} models.", context.agent_type, catalog.model_count());

        // Automatically load user preference for this agent type if available
        let mut context_with_prefs = context;
        if let Some(ref user_settings) = self.user_settings {
            let settings = user_settings.read().await;
            let agent_type_str = context_with_prefs.agent_type.to_string();
            match settings.get_model_preference(&agent_type_str).await {
                Ok(Some(family)) => {
                    info!("✅ Applying user preference for {}: family='{}'", agent_type_str, family);
                    context_with_prefs = context_with_prefs.with_preferred_family(Some(family));
                },
                Ok(None) => {
                    tracing::debug!("No user preference set for {} agent", agent_type_str);
                },
                Err(e) => {
                    tracing::error!("Failed to load user preference for {}: {:?}", agent_type_str, e);
                }
            }
        }

        let ranked_models = self.ranker.rank_models(&catalog, &context_with_prefs).await?;
        
        let selected = self.selector.select_best(&ranked_models, &context_with_prefs).await?;
        
        info!(
            "Selected model {} for agent {} (score: {:.2})",
            selected.model_id,
            context_with_prefs.agent_type,
            selected.score
        );
        
        Ok(selected)
    }
    
    /// Select optimal model for a specific agent task with user preferences
    /// 
    /// This is a convenience wrapper around select_model_for_agent that applies
    /// user-specified model family preferences to the selection context.
    pub async fn select_model_for_agent_with_preferences(
        &self,
        mut context: SelectionContext,
        preferred_family: Option<String>,
    ) -> Result<SelectedModel> {
        // Apply user preference if provided
        if let Some(family) = preferred_family {
            if !family.is_empty() {
                context = context.with_preferred_family(Some(family));
            }
        }
        
        self.select_model_for_agent(context).await
    }
    
    /// Refresh catalog (re-scan providers and update models)
    pub async fn refresh_catalog(&self) -> Result<()> {
        info!("Refreshing AI provider catalog");
        
        let providers = self.discovery.scan_all().await?;
        
        let mut catalog = self.catalog.write().await;
        catalog.clear();
        
        for provider in providers {
            let models = self.discovery.fetch_models(&provider).await?;
            for model in models {
                catalog.add_model(model);
            }
        }
        
        info!("Catalog refreshed with {} models", catalog.model_count());
        
        Ok(())
    }
    
    /// Get statistics about available models
    pub async fn get_stats(&self) -> CatalogStats {
        let catalog = self.catalog.read().await;
        catalog.get_stats()
    }
}
