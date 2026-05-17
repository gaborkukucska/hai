//! # START OF FILE hainet-persona/src/agents/cycle_engine.rs
//! Cycle Engine — Ported from TrippleEffect's AgentCycleHandler (2,978 lines)
//!
//! This module implements the core autonomous agent execution loop.
//! Each agent runs in cycles: Assemble Prompt → Call LLM → Parse Response → Execute Tools → Repeat.
//!
//! Key TE patterns preserved:
//! - Turn counting with MAX_CYCLE_TURNS hard limit
//! - Watchdog for agents stuck in same state (10+ cycles)
//! - Cross-cycle duplicate tool call detection
//! - Context summarization for small context models
//! - Framework intervention messages for stuck agents
//! - Constitutional Guardian review for outputs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use tracing::{info, warn};

use crate::prompts::AgentState;

/// Maximum turns within a single cycle before forced termination
/// (From TE: settings.MAX_CYCLE_TURNS — prevents infinite loops)
const MAX_CYCLE_TURNS: u32 = 50;

/// Cycles without state transition before watchdog intervenes
/// (From TE: 10 cycles triggers warning, every 5 after that injects intervention)
const WATCHDOG_THRESHOLD: u32 = 10;
const WATCHDOG_INJECT_INTERVAL: u32 = 5;

/// Maximum consecutive identical tool calls before forcing state transition
/// (From TE: 4 identical calls = force transition)
const MAX_DUPLICATE_TOOL_CALLS: u32 = 4;

/// Hard character limit on LLM output to prevent KV cache exhaustion
/// (From TE: 32K chars)
const MAX_OUTPUT_CHARS: usize = 32_768;

/// Agent operational status (distinct from workflow state)
/// Maps 1:1 to TE's AGENT_STATUS_* constants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Processing,
    Planning,
    AwaitingTool,
    ExecutingTool,
    AwaitingGuardianReview,
    AwaitingUserReview,
    Error,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Processing => write!(f, "processing"),
            Self::Planning => write!(f, "planning"),
            Self::AwaitingTool => write!(f, "awaiting_tool_result"),
            Self::ExecutingTool => write!(f, "executing_tool"),
            Self::AwaitingGuardianReview => write!(f, "awaiting_cg_review"),
            Self::AwaitingUserReview => write!(f, "awaiting_user_review_cg"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Context for a single cycle execution
/// (From TE: CycleContext dataclass)
#[derive(Debug)]
pub struct CycleContext {
    pub agent_id: String,
    pub agent_type: AgentType,
    pub current_state: AgentState,
    pub current_status: AgentStatus,

    /// Turn counter within this cycle
    pub turn_count: u32,

    /// Provider/model tracking for failover
    pub current_provider: String,
    pub current_model: String,

    /// Per-turn flags (reset each iteration)
    pub action_taken: bool,
    pub state_change_requested: bool,
    pub tool_executed_successfully: bool,
    pub needs_reactivation: bool,
    pub trigger_failover: bool,

    /// Error tracking
    pub last_error: Option<String>,

    /// Cycle timing
    pub started_at: Instant,
}

/// Agent type for role-specific behavior
/// (Mirrors TE's AGENT_TYPE_* constants)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentType {
    Admin,
    PM,
    Worker,
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admin => write!(f, "admin"),
            Self::PM => write!(f, "pm"),
            Self::Worker => write!(f, "worker"),
        }
    }
}

/// Tool call signature for deduplication
/// (From TE: _detect_cross_cycle_duplicate_tool_call)
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ToolCallSignature {
    pub tool_name: String,
    pub args_hash: String,
}

/// Tool execution statistics
/// (From TE: _tool_execution_stats)
#[derive(Debug, Default, Clone)]
pub struct ToolExecutionStats {
    pub total_calls: u64,
    pub successful_calls: u64,
    pub failed_calls: u64,
}

impl ToolExecutionStats {
    pub fn success_rate(&self) -> f64 {
        if self.total_calls == 0 {
            return 0.0;
        }
        (self.successful_calls as f64 / self.total_calls as f64) * 100.0
    }

    pub fn record_success(&mut self) {
        self.total_calls += 1;
        self.successful_calls += 1;
    }

    pub fn record_failure(&mut self) {
        self.total_calls += 1;
        self.failed_calls += 1;
    }
}

/// Watchdog state for detecting stuck agents
/// (From TE: _cycles_without_transition / _last_state_for_watchdog)
#[derive(Debug)]
pub struct WatchdogState {
    pub cycles_without_transition: u32,
    pub last_observed_state: AgentState,
}

impl WatchdogState {
    pub fn new(initial_state: AgentState) -> Self {
        Self {
            cycles_without_transition: 0,
            last_observed_state: initial_state,
        }
    }

    /// Update watchdog — returns true if intervention is needed
    pub fn check(&mut self, current_state: AgentState) -> bool {
        if current_state == self.last_observed_state {
            self.cycles_without_transition += 1;
        } else {
            self.cycles_without_transition = 0;
            self.last_observed_state = current_state;
        }

        if self.cycles_without_transition >= WATCHDOG_THRESHOLD {
            if self.cycles_without_transition % WATCHDOG_INJECT_INTERVAL == 0 {
                warn!(
                    cycles = self.cycles_without_transition,
                    state = ?current_state,
                    "Watchdog: Agent stuck in same state — intervention needed"
                );
                return true;
            }
        }
        false
    }

    /// Generate intervention message
    /// (From TE: _generate_empty_response_guidance + watchdog intervention)
    pub fn intervention_message(&self, agent_type: AgentType) -> String {
        let base = format!(
            "[Framework Watchdog Intervention]: You have been in the '{:?}' state for {} cycles \
             without transitioning.",
            self.last_observed_state, self.cycles_without_transition
        );

        let guidance = match agent_type {
            AgentType::Admin => {
                "If you are stuck, transition to 'conversation' state by calling the \
                 'request_state' tool. If you have completed your work, transition to 'standby'."
            }
            AgentType::PM => {
                "If you have completed team building, transition to 'pm_manage' state. \
                 If stuck, use 'request_state' tool to move to 'pm_standby'."
            }
            AgentType::Worker => {
                "If you have completed your task, transition to 'worker_report' state. \
                 If stuck, use 'request_state' tool with state='worker_wait'."
            }
        };

        format!("{} {}", base, guidance)
    }
}

/// Duplicate tool call tracker
/// (From TE: _detect_cross_cycle_duplicate_tool_call + 4-consecutive-call limit)
#[derive(Debug, Default)]
pub struct DuplicateToolTracker {
    /// Recent tool call signatures with their count
    consecutive_calls: HashMap<ToolCallSignature, u32>,
    /// Last tool call for consecutive detection
    last_call: Option<ToolCallSignature>,
    last_call_count: u32,
}

impl DuplicateToolTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a tool call. Returns true if it's a duplicate that should be blocked.
    /// (From TE: 4+ consecutive identical calls = force state transition)
    pub fn record_call(&mut self, sig: ToolCallSignature) -> bool {
        if Some(&sig) == self.last_call.as_ref() {
            self.last_call_count += 1;
            if self.last_call_count >= MAX_DUPLICATE_TOOL_CALLS {
                warn!(
                    tool = sig.tool_name,
                    count = self.last_call_count,
                    "Blocked duplicate tool call — forcing state transition"
                );
                return true;
            }
        } else {
            self.last_call = Some(sig);
            self.last_call_count = 1;
        }
        false
    }

    /// Reset tracker (e.g., on state transition)
    pub fn reset(&mut self) {
        self.last_call = None;
        self.last_call_count = 0;
        self.consecutive_calls.clear();
    }
}

/// Cycle outcome from processing a single LLM turn
/// (From TE: CycleOutcomeDeterminer)
#[derive(Debug, Clone)]
pub enum CycleOutcome {
    /// Agent produced text output, no tool calls
    TextOutput(String),
    /// Agent made tool calls that need execution
    ToolCalls(Vec<ToolCallRequest>),
    /// Agent requested a state transition
    StateChangeRequest { new_state: String, task_id: Option<String> },
    /// Agent output was empty (needs intervention)
    EmptyResponse,
    /// Autoregressive loop detected — stream was cut
    LoopDetected,
    /// Output exceeded character limit
    OutputTruncated(String),
    /// Error during LLM call
    LLMError(String),
}

/// A parsed tool call from LLM output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    pub id: String,
    pub name: String,
    pub arguments: HashMap<String, serde_json::Value>,
}

/// Events emitted during cycle execution (for UI/logging)
#[derive(Debug, Clone)]
pub enum CycleEvent {
    CycleStarted { agent_id: String, turn: u32 },
    StatusChanged { agent_id: String, status: AgentStatus },
    LLMResponseChunk { agent_id: String, content: String },
    ToolExecutionStarted { agent_id: String, tool_name: String },
    ToolExecutionCompleted { agent_id: String, tool_name: String, success: bool },
    StateTransition { agent_id: String, from: AgentState, to: AgentState },
    WatchdogIntervention { agent_id: String, cycles: u32 },
    LoopDetected { agent_id: String, pattern_length: usize },
    CycleCompleted { agent_id: String, outcome: String },
    CycleError { agent_id: String, error: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_status_display() {
        assert_eq!(AgentStatus::Idle.to_string(), "idle");
        assert_eq!(AgentStatus::ExecutingTool.to_string(), "executing_tool");
    }

    #[test]
    fn test_tool_execution_stats() {
        let mut stats = ToolExecutionStats::default();
        stats.record_success();
        stats.record_success();
        stats.record_failure();

        assert_eq!(stats.total_calls, 3);
        assert_eq!(stats.successful_calls, 2);
        assert!((stats.success_rate() - 66.66).abs() < 1.0);
    }

    #[test]
    fn test_watchdog_no_transition() {
        let mut wd = WatchdogState::new(AgentState::Working);

        // First 9 cycles — no intervention
        for _ in 0..9 {
            assert!(!wd.check(AgentState::Working));
        }

        // 10th cycle — intervention!
        assert!(wd.check(AgentState::Working));
    }

    #[test]
    fn test_watchdog_resets_on_transition() {
        let mut wd = WatchdogState::new(AgentState::Working);

        for _ in 0..8 {
            wd.check(AgentState::Working);
        }

        // State changes — counter resets
        assert!(!wd.check(AgentState::Reporting));
        assert_eq!(wd.cycles_without_transition, 0);
    }

    #[test]
    fn test_duplicate_tool_tracker() {
        let mut tracker = DuplicateToolTracker::new();
        let sig = ToolCallSignature {
            tool_name: "file_read".to_string(),
            args_hash: "abc123".to_string(),
        };

        // First 3 calls — allowed
        assert!(!tracker.record_call(sig.clone()));
        assert!(!tracker.record_call(sig.clone()));
        assert!(!tracker.record_call(sig.clone()));

        // 4th identical call — blocked!
        assert!(tracker.record_call(sig.clone()));
    }

    #[test]
    fn test_duplicate_tracker_resets_on_different_call() {
        let mut tracker = DuplicateToolTracker::new();
        let sig1 = ToolCallSignature {
            tool_name: "file_read".to_string(),
            args_hash: "abc".to_string(),
        };
        let sig2 = ToolCallSignature {
            tool_name: "file_write".to_string(),
            args_hash: "def".to_string(),
        };

        assert!(!tracker.record_call(sig1.clone()));
        assert!(!tracker.record_call(sig1.clone()));
        assert!(!tracker.record_call(sig1.clone()));

        // Different call resets counter
        assert!(!tracker.record_call(sig2.clone()));
        assert_eq!(tracker.last_call_count, 1);
    }

    #[test]
    fn test_watchdog_intervention_message() {
        let mut wd = WatchdogState::new(AgentState::Working);
        wd.cycles_without_transition = 15;

        let msg = wd.intervention_message(AgentType::Worker);
        assert!(msg.contains("Watchdog Intervention"));
        assert!(msg.contains("worker_report"));
    }
}
