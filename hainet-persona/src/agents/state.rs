//! # Agent State Machine
//! 
//! Manages agent lifecycle states and transitions according to HAI-Net's state model:
//! Startup → Idle → Planning → Working → (Idle | Error)

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, Duration};
use crate::prompts::AgentState;

/// State transition record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    /// Previous state
    pub from: AgentState,
    
    /// New state
    pub to: AgentState,
    
    /// When the transition occurred
    pub timestamp: SystemTime,
    
    /// Reason for transition
    pub reason: String,
}

/// Agent state machine
pub struct AgentStateMachine {
    /// Current state
    current_state: AgentState,
    
    /// State history (last 10 transitions)
    history: Vec<StateTransition>,
    
    /// When current state was entered
    state_entered_at: SystemTime,
    
    /// Maximum time to stay in a state (prevents stuck agents)
    max_state_duration: Duration,
}

impl AgentStateMachine {
    /// Create new state machine starting in Startup state
    pub fn new() -> Self {
        Self {
            current_state: AgentState::Startup,
            history: Vec::new(),
            state_entered_at: SystemTime::now(),
            max_state_duration: Duration::from_secs(300), // 5 minutes default
        }
    }
    
    /// Create with custom max state duration
    pub fn with_max_duration(duration: Duration) -> Self {
        Self {
            current_state: AgentState::Startup,
            history: Vec::new(),
            state_entered_at: SystemTime::now(),
            max_state_duration: duration,
        }
    }
    
    /// Get current state
    pub fn current_state(&self) -> &AgentState {
        &self.current_state
    }
    
    /// Transition to new state
    pub fn transition(&mut self, new_state: AgentState, reason: String) -> Result<()> {
        // Validate transition is allowed
        if !self.is_valid_transition(&new_state) {
            return Err(anyhow!(
                "Invalid state transition from {:?} to {:?}",
                self.current_state,
                new_state
            ));
        }
        
        // Record transition
        let transition = StateTransition {
            from: self.current_state.clone(),
            to: new_state.clone(),
            timestamp: SystemTime::now(),
            reason,
        };
        
        self.history.push(transition);
        
        // Keep only last 10 transitions
        if self.history.len() > 10 {
            self.history.remove(0);
        }
        
        // Update state
        self.current_state = new_state;
        self.state_entered_at = SystemTime::now();
        
        Ok(())
    }
    
    /// Check if transition is valid according to state machine rules
    fn is_valid_transition(&self, new_state: &AgentState) -> bool {
        use AgentState::*;
        
        match (&self.current_state, new_state) {
            // From Startup
            (Startup, Idle) => true,
            (Startup, Conversation) => true, // Admin AI
            (Startup, Error) => true,
            
            // From Idle (Worker agents)
            (Idle, Planning) => true,
            (Idle, Working) => false, // Must plan before working
            (Idle, Error) => true,
            
            // From Conversation (Admin AI)
            (Conversation, Planning) => true, // Complex intent detected
            (Conversation, Monitoring) => true, // Project started
            (Conversation, Error) => true,
            
            // From Planning
            (Planning, Working) => true, // Worker starts task
            (Planning, Managing) => true, // PM starts managing
            (Planning, Idle) => true, // Can cancel planning
            (Planning, Conversation) => true, // Admin returns to conversation
            (Planning, Error) => true,
            
            // From Monitoring (Admin AI)
            (Monitoring, Conversation) => true, // All projects complete
            (Monitoring, Planning) => true, // New project requested
            (Monitoring, Error) => true,
            
            // From Managing (PM agents)
            (Managing, Planning) => true, // Replanning needed
            (Managing, Idle) => true, // Project complete
            (Managing, Error) => true,
            
            // From Working (Worker agents)
            (Working, Reporting) => true, // Task done, report to PM
            (Working, Idle) => true, // Task complete (direct)
            (Working, Planning) => false, // Must return to Idle first
            (Working, Error) => true,
            
            // From Reporting (Worker agents)
            (Reporting, Idle) => true, // PM validated, ready for next task
            (Reporting, Working) => true, // PM rejected, redo task
            (Reporting, Error) => true,
            
            // From Error
            (Error, Idle) => true, // Recovery
            (Error, Conversation) => true, // Admin recovery
            (Error, Startup) => true, // Restart
            
            // Same state (no-op)
            (a, b) if a == b => true,
            
            // All other transitions invalid
            _ => false,
        }
    }
    
    /// Check if agent has been in current state too long
    pub fn is_stuck(&self) -> bool {
        match self.state_entered_at.elapsed() {
            Ok(elapsed) => elapsed > self.max_state_duration,
            Err(_) => false, // Clock issues, assume not stuck
        }
    }
    
    /// Force transition to Error state (for emergencies)
    pub fn force_error(&mut self, reason: String) {
        let transition = StateTransition {
            from: self.current_state.clone(),
            to: AgentState::Error,
            timestamp: SystemTime::now(),
            reason,
        };
        
        self.history.push(transition);
        self.current_state = AgentState::Error;
        self.state_entered_at = SystemTime::now();
    }
    
    /// Get time spent in current state
    pub fn time_in_state(&self) -> Duration {
        self.state_entered_at.elapsed().unwrap_or(Duration::from_secs(0))
    }
    
    /// Get recent transition history
    pub fn get_history(&self) -> &[StateTransition] {
        &self.history
    }
    
    /// Check if agent is ready to accept tasks
    pub fn is_ready(&self) -> bool {
        matches!(self.current_state, AgentState::Idle)
    }
    
    /// Check if agent is currently working
    pub fn is_working(&self) -> bool {
        matches!(self.current_state, AgentState::Working | AgentState::Planning)
    }
    
    /// Check if agent is in error state
    pub fn is_error(&self) -> bool {
        matches!(self.current_state, AgentState::Error)
    }
}

impl Default for AgentStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    
    #[test]
    fn test_state_machine_creation() {
        let sm = AgentStateMachine::new();
        assert_eq!(sm.current_state(), &AgentState::Startup);
        assert!(sm.history.is_empty());
    }
    
    #[test]
    fn test_valid_transition_startup_to_idle() {
        let mut sm = AgentStateMachine::new();
        let result = sm.transition(AgentState::Idle, "Initialization complete".to_string());
        
        assert!(result.is_ok());
        assert_eq!(sm.current_state(), &AgentState::Idle);
        assert_eq!(sm.history.len(), 1);
    }
    
    #[test]
    fn test_invalid_transition_idle_to_working() {
        let mut sm = AgentStateMachine::new();
        sm.transition(AgentState::Idle, "Init".to_string()).unwrap();
        
        let result = sm.transition(AgentState::Working, "Skip planning".to_string());
        assert!(result.is_err());
        assert_eq!(sm.current_state(), &AgentState::Idle);
    }
    
    #[test]
    fn test_valid_workflow() {
        let mut sm = AgentStateMachine::new();
        
        // Startup → Idle
        sm.transition(AgentState::Idle, "Init".to_string()).unwrap();
        assert_eq!(sm.current_state(), &AgentState::Idle);
        
        // Idle → Planning
        sm.transition(AgentState::Planning, "User request".to_string()).unwrap();
        assert_eq!(sm.current_state(), &AgentState::Planning);
        
        // Planning → Working
        sm.transition(AgentState::Working, "Plan approved".to_string()).unwrap();
        assert_eq!(sm.current_state(), &AgentState::Working);
        
        // Working → Idle
        sm.transition(AgentState::Idle, "Task complete".to_string()).unwrap();
        assert_eq!(sm.current_state(), &AgentState::Idle);
        
        assert_eq!(sm.history.len(), 4);
    }
    
    #[test]
    fn test_error_transition() {
        let mut sm = AgentStateMachine::new();
        sm.transition(AgentState::Idle, "Init".to_string()).unwrap();
        
        sm.transition(AgentState::Error, "Something failed".to_string()).unwrap();
        assert_eq!(sm.current_state(), &AgentState::Error);
        assert!(sm.is_error());
    }
    
    #[test]
    fn test_force_error() {
        let mut sm = AgentStateMachine::new();
        sm.transition(AgentState::Idle, "Init".to_string()).unwrap();
        
        sm.force_error("Emergency stop".to_string());
        assert_eq!(sm.current_state(), &AgentState::Error);
    }
    
    #[test]
    fn test_history_limit() {
        let mut sm = AgentStateMachine::new();
        
        // Create more than 10 transitions
        for i in 0..12 {
            if i % 2 == 0 {
                sm.transition(AgentState::Idle, format!("Trans {}", i)).ok();
            } else {
                sm.transition(AgentState::Planning, format!("Trans {}", i)).ok();
            }
        }
        
        // History should be capped at 10
        assert!(sm.history.len() <= 10);
    }
    
    #[test]
    fn test_is_stuck() {
        let mut sm = AgentStateMachine::with_max_duration(Duration::from_millis(50));
        
        assert!(!sm.is_stuck());
        
        sleep(Duration::from_millis(100));
        assert!(sm.is_stuck());
    }
    
    #[test]
    fn test_time_in_state() {
        let sm = AgentStateMachine::new();
        sleep(Duration::from_millis(50));
        
        let elapsed = sm.time_in_state();
        assert!(elapsed >= Duration::from_millis(50));
    }
    
    #[test]
    fn test_state_checks() {
        let mut sm = AgentStateMachine::new();
        
        assert!(!sm.is_ready());
        assert!(!sm.is_working());
        assert!(!sm.is_error());
        
        sm.transition(AgentState::Idle, "Init".to_string()).unwrap();
        assert!(sm.is_ready());
        assert!(!sm.is_working());
        
        sm.transition(AgentState::Planning, "Task".to_string()).unwrap();
        assert!(!sm.is_ready());
        assert!(sm.is_working());
        
        sm.transition(AgentState::Error, "Fail".to_string()).unwrap();
        assert!(sm.is_error());
    }
    
    #[test]
    fn test_same_state_transition() {
        let mut sm = AgentStateMachine::new();
        
        // Same state transition should be allowed (no-op)
        let result = sm.transition(AgentState::Startup, "Refresh".to_string());
        assert!(result.is_ok());
        assert_eq!(sm.current_state(), &AgentState::Startup);
    }
}
