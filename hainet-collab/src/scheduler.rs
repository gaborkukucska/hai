//! # START OF FILE hainet-collab/src/scheduler.rs
//! Compute Scheduler — Ported from PPLPWR's Scheduler.ts

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;
use crate::CollabResult;

/// A compute network adapter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub name: String,
    pub weight: f64,           // Proportion of compute time (0.0-1.0)
    pub min_vram_gb: f32,
    pub enabled: bool,
}

/// Scheduler that distributes compute time across networks by weight
pub struct ComputeScheduler {
    networks: HashMap<String, NetworkConfig>,
    active_network: Option<String>,
}

impl ComputeScheduler {
    pub fn new() -> Self {
        Self { networks: HashMap::new(), active_network: None }
    }

    pub fn add_network(&mut self, config: NetworkConfig) {
        info!(name = config.name, weight = config.weight, "Adding compute network");
        self.networks.insert(config.name.clone(), config);
    }

    /// Select the next network to run based on weights
    pub fn select_next(&mut self) -> CollabResult<Option<&NetworkConfig>> {
        let enabled: Vec<&NetworkConfig> = self.networks.values()
            .filter(|n| n.enabled)
            .collect();

        if enabled.is_empty() {
            return Ok(None);
        }

        // Weighted random selection
        let total_weight: f64 = enabled.iter().map(|n| n.weight).sum();
        let mut rng_val: f64 = rand::random::<f64>() * total_weight;

        for network in &enabled {
            rng_val -= network.weight;
            if rng_val <= 0.0 {
                self.active_network = Some(network.name.clone());
                return Ok(Some(network));
            }
        }

        Ok(enabled.last().copied())
    }

    pub fn active_network(&self) -> Option<&str> {
        self.active_network.as_deref()
    }
}

impl Default for ComputeScheduler {
    fn default() -> Self { Self::new() }
}
