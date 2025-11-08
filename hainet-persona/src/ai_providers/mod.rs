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

pub use discovery::{ProviderDiscovery, DiscoveredProvider};
pub use catalog::{ModelCatalog, CatalogedModel, ModelCapability, CatalogStats};
pub use ranking::{ModelRanker, ModelScore, RankingCriteria};
pub use selection::{ModelSelector, SelectionContext, SelectedModel};
pub use providers::{ProviderClient, ProviderType, ModelInfo};

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Central AI provider management system
pub struct AIProviderManager {
    discovery: ProviderDiscovery,
    catalog: Arc<RwLock<ModelCatalog>>,
    ranker: ModelRanker,
    selector: ModelSelector,
}

impl AIProviderManager {
    /// Create new provider manager and perform initial discovery
    pub async fn new() -> Result<Self> {
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
        };

        // Perform initial discovery
        manager.discover_providers().await?;

        Ok(manager)
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
            let models = self.discovery.fetch_models(&provider).await?;
            for model in models {
                catalog.add_model(model);
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
        let ranked_models = self.ranker.rank_models(&catalog, &context).await?;
        
        let selected = self.selector.select_best(&ranked_models, &context).await?;
        
        info!(
            "Selected model {} for agent {} (score: {:.2})",
            selected.model_id,
            context.agent_type,
            selected.score
        );
        
        Ok(selected)
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
