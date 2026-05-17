//! # START OF FILE hainet-collab/src/policy.rs
//! User Policy — Ported from PPLPWR's UserPolicy

use serde::{Deserialize, Serialize};

/// User's compute sharing policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputePolicy {
    /// Autonomy level: how much the agent can decide on its own
    pub autonomy: AutonomyLevel,
    /// Maximum concurrent compute tasks
    pub max_concurrent_tasks: u32,
    /// Maximum GPU temperature before pausing (°C)
    pub max_gpu_temp_c: f32,
    /// Never run compute on battery power
    pub require_ac_power: bool,
    /// Networks that are never allowed
    pub blocked_networks: Vec<String>,
    /// Maximum hours per day to contribute
    pub max_daily_hours: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutonomyLevel {
    /// Always ask user before starting compute
    Ask,
    /// Notify user but proceed automatically
    Notify,
    /// Fully autonomous (silent)
    Silent,
}

impl Default for ComputePolicy {
    fn default() -> Self {
        Self {
            autonomy: AutonomyLevel::Notify,
            max_concurrent_tasks: 1,
            max_gpu_temp_c: 85.0,
            require_ac_power: true,
            blocked_networks: vec![],
            max_daily_hours: 12.0,
        }
    }
}
