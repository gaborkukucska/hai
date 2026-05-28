//! # Guardian Agent
//!
//! Constitutional compliance and oversight agent with state-based workflows.
//! 
//! The Guardian operates independently to monitor all agent communications
//! and ensure compliance with HAI-Net's constitutional principles.
//!
//! ## States
//! - **Startup**: Initialize Guardian systems and load rules
//! - **Monitoring**: Active oversight of agent communications (default)
//! - **Analyzing**: Deep analysis of potential violations
//! - **Intervening**: Active intervention (blocking/pausing)
//! - **Auditing**: Periodic compliance audits (scheduled)
//! - **Learning**: Update detection rules (scheduled)
//! - **Reporting**: Generate compliance reports (scheduled/triggered)
//!
//! ## Scheduled Workflows
//! - Real-time monitoring (continuous)
//! - Periodic audits (every 6 hours)
//! - Pattern learning (weekly)
//! - Compliance reporting (daily + on-demand)

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::{debug, info, error, warn};

use crate::agents::state::AgentStateMachine;
use crate::agents::llm_config::AgentLLMConfig;
use crate::agents::metrics::MetricsCollector;
use crate::config::HaiNetConfig;
use crate::guardian::{GuardianSystem, GuardianAction};
use crate::messaging::types::{AgentId, AgentType, Message};
use crate::messaging::guardian::{GuardianInterceptor, InterceptResult};
use crate::prompts::AgentState;

/// Guardian-specific states
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardianState {
    /// Initial state - loading rules and configuration
    Startup,
    /// Default active state - continuous message monitoring
    Monitoring,
    /// Deep analysis of potential violations
    Analyzing,
    /// Active intervention (blocking/pausing messages)
    Intervening,
    /// Periodic compliance audit (scheduled task)
    Auditing,
    /// Update detection rules based on patterns (scheduled task)
    Learning,
    /// Generate compliance reports (scheduled/triggered)
    Reporting,
    /// Error state
    Error,
}

impl From<GuardianState> for AgentState {
    fn from(guardian_state: GuardianState) -> Self {
        match guardian_state {
            GuardianState::Startup => AgentState::Startup,
            GuardianState::Monitoring => AgentState::Idle, // Monitoring is the "ready" state
            GuardianState::Analyzing => AgentState::Planning,
            GuardianState::Intervening => AgentState::Working,
            GuardianState::Auditing => AgentState::Planning,
            GuardianState::Learning => AgentState::Planning,
            GuardianState::Reporting => AgentState::Working,
            GuardianState::Error => AgentState::Error,
        }
    }
}

/// Guardian configuration
#[derive(Debug, Clone)]
pub struct GuardianConfig {
    /// LLM configuration from hainet.toml
    pub llm_config: AgentLLMConfig,
    
    /// Monitoring thresholds
    pub pii_threshold: f32,
    pub bias_threshold: f32,
    pub harm_threshold: f32,
    
    /// Scheduling intervals
    pub audit_interval: Duration,
    pub learning_interval: Duration,
    pub reporting_interval: Duration,
    
    /// Which constitutional articles to enforce
    pub enabled_articles: Vec<Article>,
}

impl Default for GuardianConfig {
    fn default() -> Self {
        Self {
            llm_config: AgentLLMConfig::for_agent_type(AgentType::Guardian),
            pii_threshold: 0.7,
            bias_threshold: 0.7,
            harm_threshold: 0.7,
            audit_interval: Duration::from_secs(6 * 60 * 60), // 6 hours
            learning_interval: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
            reporting_interval: Duration::from_secs(24 * 60 * 60), // 24 hours
            enabled_articles: vec![
                Article::Article1Privacy,
                Article::Article2HumanAgency,
                Article::Article3Decentralization,
                Article::Article4Community,
                Article::Article7Transparency,
                Article::Article9Quality,
            ],
        }
    }
}

impl GuardianConfig {
    /// Create GuardianConfig from HaiNetConfig
    pub fn from_hainet_config(hainet_config: &HaiNetConfig) -> Self {
        let llm_config = hainet_config.get_agent_llm_config(AgentType::Guardian);
        
        Self {
            llm_config,
            ..Self::default()
        }
    }
}

/// Constitutional Articles
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Article {
    Article1Privacy,
    Article2HumanAgency,
    Article3Decentralization,
    Article4Community,
    Article5ResourceSharing,
    Article7Transparency,
    Article9Quality,
}

/// Constitutional compliance checker
pub struct ConstitutionalChecker {
    /// Rules for each article
    articles: Vec<Article>,
}

impl ConstitutionalChecker {
    pub fn new(articles: Vec<Article>) -> Self {
        Self { articles }
    }
    
    /// Check if action complies with specific article
    pub async fn check_article_compliance(
        &self,
        article: &Article,
        context: &ComplianceContext,
    ) -> bool {
        match article {
            Article::Article1Privacy => self.check_privacy(context).await,
            Article::Article2HumanAgency => self.check_human_agency(context).await,
            Article::Article3Decentralization => self.check_decentralization(context).await,
            Article::Article4Community => self.check_community_focus(context).await,
            Article::Article5ResourceSharing => self.check_resource_sharing(context).await,
            Article::Article7Transparency => self.check_transparency(context).await,
            Article::Article9Quality => self.check_quality(context).await,
        }
    }
    
    async fn check_privacy(&self, context: &ComplianceContext) -> bool {
        // Article I: Privacy First
        // - No external API calls without user consent
        // - All PII properly encrypted/anonymized
        // - No data shared outside local hub mesh
        
        if context.involves_external_api && !context.has_user_consent {
            return false;
        }
        
        if context.contains_pii && !context.is_encrypted {
            return false;
        }
        
        true
    }
    
    async fn check_human_agency(&self, _context: &ComplianceContext) -> bool {
        // Article II: Human Agency
        // - Critical decisions require user approval
        // - User can override any agent action
        // - Clear explanations for all actions
        
        // For now, always compliant (full checks require user interaction hooks)
        true
    }
    
    async fn check_decentralization(&self, context: &ComplianceContext) -> bool {
        // Article III: Decentralization
        // - No single points of control
        // - Distributed decision making
        
        !context.is_centralized_action
    }
    
    async fn check_community_focus(&self, _context: &ComplianceContext) -> bool {
        // Article IV: Community Focus
        // - Collaborative actions
        // - Strengthening connections
        
        // For now, always compliant
        true
    }
    
    async fn check_resource_sharing(&self, _context: &ComplianceContext) -> bool {
        // Article V: Resource Sharing
        // - Voluntary exchange
        // - Strict privacy
        
        // For now, always compliant
        true
    }
    
    async fn check_transparency(&self, context: &ComplianceContext) -> bool {
        // Article VII: Transparency
        // - All decisions logged
        // - Constitutional compliance visible
        // - Source code accessible
        
        context.is_logged
    }
    
    async fn check_quality(&self, context: &ComplianceContext) -> bool {
        // Article IX: Quality
        // - Validation before completion
        // - Testing requirements met
        // - Error handling present
        
        context.is_validated
    }
}

/// Context for compliance checking
#[derive(Debug, Clone)]
pub struct ComplianceContext {
    pub involves_external_api: bool,
    pub has_user_consent: bool,
    pub contains_pii: bool,
    pub is_encrypted: bool,
    pub is_centralized_action: bool,
    pub is_logged: bool,
    pub is_validated: bool,
}

impl Default for ComplianceContext {
    fn default() -> Self {
        Self {
            involves_external_api: false,
            has_user_consent: false,
            contains_pii: false,
            is_encrypted: true,
            is_centralized_action: false,
            is_logged: true,
            is_validated: true,
        }
    }
}

/// Violation trigger for analysis
#[derive(Debug, Clone)]
pub enum ViolationTrigger {
    MessageIntercepted(Message),
    ScheduledAuditFinding,
    UserReport,
}

/// Violation report from analysis
#[derive(Debug, Clone)]
pub struct ViolationReport {
    pub trigger: ViolationTrigger,
    pub violations: Vec<String>,
    pub severity: ViolationSeverity,
    pub recommended_action: GuardianAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViolationSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Report trigger
#[derive(Debug, Clone)]
pub enum ReportTrigger {
    Scheduled,
    OnDemand,
    HighViolationRate,
}

/// Audit report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub timestamp: std::time::SystemTime,
    pub total_messages: u64,
    pub violations_found: u64,
    pub actions_taken: Vec<String>,
}

/// Learning report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningReport {
    pub timestamp: std::time::SystemTime,
    pub patterns_updated: u32,
    pub false_positives_corrected: u32,
    pub new_rules_added: u32,
}

/// Compliance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub timestamp: std::time::SystemTime,
    pub period_start: std::time::SystemTime,
    pub period_end: std::time::SystemTime,
    pub total_messages: u64,
    pub violations_blocked: u64,
    pub violations_paused: u64,
    pub compliance_rate: f32,
    pub top_violations: Vec<String>,
}

/// Scheduler for Guardian tasks
pub struct GuardianScheduler {
    audit_task: Option<JoinHandle<()>>,
    learning_task: Option<JoinHandle<()>>,
    reporting_task: Option<JoinHandle<()>>,
}

impl GuardianScheduler {
    pub fn new() -> Self {
        Self {
            audit_task: None,
            learning_task: None,
            reporting_task: None,
        }
    }
    
    pub async fn stop(&mut self) {
        if let Some(task) = self.audit_task.take() {
            task.abort();
        }
        if let Some(task) = self.learning_task.take() {
            task.abort();
        }
        if let Some(task) = self.reporting_task.take() {
            task.abort();
        }
    }
}

/// Guardian Agent - Constitutional compliance and oversight
pub struct GuardianAgent {
    /// Agent identifier
    agent_id: AgentId,
    
    /// State machine (using Guardian-specific states)
    state: Arc<RwLock<GuardianState>>,
    state_machine: AgentStateMachine,
    
    /// Configuration
    config: GuardianConfig,
    
    /// Detection systems (from existing guardian module)
    guardian_system: Arc<GuardianSystem>,
    
    /// Message interceptor
    interceptor: Arc<GuardianInterceptor>,
    
    /// Constitutional checker
    constitutional_checker: ConstitutionalChecker,
    
    /// Scheduler for periodic tasks
    scheduler: Arc<RwLock<GuardianScheduler>>,
    
    /// Metrics collector
    metrics: Arc<MetricsCollector>,
}

use crate::ai_providers::AIProviderManager;
impl GuardianAgent {
    /// Create new Guardian agent
    pub fn new(config: GuardianConfig, metrics: Arc<MetricsCollector>, ai_provider_manager: Arc<AIProviderManager>) -> Self {
        let agent_id = AgentId::new_guardian("guardian-1".to_string());
        
        // Create guardian system with Ollama client
        let guardian_system = Arc::new(GuardianSystem::new(
            ai_provider_manager,
            Some("llama3.2".to_string()),
        ));
        
        // Create message interceptor
        let interceptor = Arc::new(GuardianInterceptor::new());
        
        // Create constitutional checker
        let constitutional_checker = ConstitutionalChecker::new(config.enabled_articles.clone());
        
        Self {
            agent_id,
            state: Arc::new(RwLock::new(GuardianState::Startup)),
            state_machine: AgentStateMachine::new(),
            config,
            guardian_system,
            interceptor,
            constitutional_checker,
            scheduler: Arc::new(RwLock::new(GuardianScheduler::new())),
            metrics,
        }
    }
    
    /// Create Guardian agent from HAI-Net config
    pub fn from_config(hainet_config: &HaiNetConfig, metrics: Arc<MetricsCollector>, ai_provider_manager: Arc<AIProviderManager>) -> Self {
        let llm_config = hainet_config.get_agent_llm_config(AgentType::Guardian);
        
        let config = GuardianConfig {
            llm_config,
            ..GuardianConfig::default()
        };
        
        Self::new(config, metrics, ai_provider_manager)
    }
    
    /// Get agent ID
    pub fn id(&self) -> &AgentId {
        &self.agent_id
    }
    
    /// Get current state
    pub async fn current_state(&self) -> GuardianState {
        self.state.read().await.clone()
    }
    
    /// Transition to new state
    async fn transition_to(&mut self, new_state: GuardianState, reason: String) -> Result<()> {
        let agent_state: AgentState = new_state.clone().into();
        self.state_machine.transition(agent_state, reason.clone())?;
        
        let mut state = self.state.write().await;
        *state = new_state.clone();
        
        info!("Guardian state transition: {:?} ({})", new_state, reason);
        Ok(())
    }
    
    /// Start the Guardian agent with message monitoring
    pub async fn start(&mut self, monitoring_rx: mpsc::Receiver<Message>) -> Result<()> {
        info!("Starting Guardian agent: {}", self.agent_id);
        
        // Transition from Startup to Monitoring
        self.transition_to(
            GuardianState::Monitoring,
            "Guardian initialized and ready".to_string()
        ).await?;
        
        // Start scheduled tasks
        self.start_scheduled_tasks().await?;
        
        // Start message monitoring loop (spawn as background task)
        let agent_handle = self.clone_for_task();
        tokio::spawn(async move {
            agent_handle.run_monitoring_loop(monitoring_rx).await;
        });
        
        info!("Guardian monitoring loop started");
        Ok(())
    }
    
    /// Stop the Guardian agent
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping Guardian agent: {}", self.agent_id);
        
        // Stop all scheduled tasks
        let mut scheduler = self.scheduler.write().await;
        scheduler.stop().await;
        
        Ok(())
    }
    
    /// Start scheduled background tasks
    async fn start_scheduled_tasks(&mut self) -> Result<()> {
        let mut scheduler = self.scheduler.write().await;
        
        // Audit task (every 6 hours)
        let audit_interval = self.config.audit_interval;
        let audit_agent = self.clone_for_task();
        let audit_task = tokio::spawn(async move {
            loop {
                sleep(audit_interval).await;
                if let Err(e) = audit_agent.run_audit_workflow().await {
                    error!("Audit workflow error: {}", e);
                }
            }
        });
        scheduler.audit_task = Some(audit_task);
        
        // Learning task (weekly)
        let learning_interval = self.config.learning_interval;
        let learning_agent = self.clone_for_task();
        let learning_task = tokio::spawn(async move {
            loop {
                sleep(learning_interval).await;
                if let Err(e) = learning_agent.run_learning_workflow().await {
                    error!("Learning workflow error: {}", e);
                }
            }
        });
        scheduler.learning_task = Some(learning_task);
        
        // Reporting task (daily)
        let reporting_interval = self.config.reporting_interval;
        let reporting_agent = self.clone_for_task();
        let reporting_task = tokio::spawn(async move {
            loop {
                sleep(reporting_interval).await;
                if let Err(e) = reporting_agent.run_reporting_workflow(ReportTrigger::Scheduled).await {
                    error!("Reporting workflow error: {}", e);
                }
            }
        });
        scheduler.reporting_task = Some(reporting_task);
        
        info!("Guardian scheduled tasks started");
        Ok(())
    }
    
    /// Clone for background task (Arc-wrapped components)
    fn clone_for_task(&self) -> GuardianAgentHandle {
        GuardianAgentHandle {
            agent_id: self.agent_id.clone(),
            state: Arc::clone(&self.state),
            guardian_system: Arc::clone(&self.guardian_system),
            interceptor: Arc::clone(&self.interceptor),
            metrics: Arc::clone(&self.metrics),
        }
    }
    
    /// Run audit workflow (scheduled task)
    async fn run_audit_workflow(&self) -> Result<AuditReport> {
        info!("Running scheduled audit workflow");
        
        // TODO: Implement actual audit logic
        // This is a placeholder for the audit workflow
        
        let report = AuditReport {
            timestamp: std::time::SystemTime::now(),
            total_messages: 0,
            violations_found: 0,
            actions_taken: vec![],
        };
        
        Ok(report)
    }
    
    /// Run learning workflow (scheduled task)
    async fn run_learning_workflow(&self) -> Result<LearningReport> {
        info!("Running scheduled learning workflow");
        
        // TODO: Implement actual learning logic
        // This is a placeholder for the learning workflow
        
        let report = LearningReport {
            timestamp: std::time::SystemTime::now(),
            patterns_updated: 0,
            false_positives_corrected: 0,
            new_rules_added: 0,
        };
        
        Ok(report)
    }
    
    /// Run reporting workflow (scheduled/triggered task)
    async fn run_reporting_workflow(&self, _trigger: ReportTrigger) -> Result<ComplianceReport> {
        info!("Running reporting workflow");
        
        // Get stats from interceptor
        let stats = self.interceptor.get_stats().await;
        
        let total = stats.messages_intercepted;
        let blocked = stats.messages_blocked;
        let paused = stats.messages_paused;
        
        let compliance_rate = if total > 0 {
            (total - blocked - paused) as f32 / total as f32
        } else {
            1.0
        };
        
        let report = ComplianceReport {
            timestamp: std::time::SystemTime::now(),
            period_start: std::time::SystemTime::now() - self.config.reporting_interval,
            period_end: std::time::SystemTime::now(),
            total_messages: total,
            violations_blocked: blocked,
            violations_paused: paused,
            compliance_rate,
            top_violations: vec![], // TODO: Track violation types
        };
        
        Ok(report)
    }
    
    /// Intercept and analyze a message (real-time monitoring)
    pub async fn intercept_message(&self, msg: &Message) -> Result<InterceptResult> {
        debug!("Guardian intercepting message from {} to {}", msg.from, msg.to);
        
        let start_time = std::time::Instant::now();
        
        // Use the message interceptor
        let result = self.interceptor.intercept(msg).await?;
        
        let elapsed = start_time.elapsed();
        
        // Record metrics using OperationResult
        let operation_result = crate::agents::metrics::OperationResult {
            agent_type: AgentType::Guardian,
            agent_id: self.agent_id.clone(),
            config_hash: crate::agents::metrics::hash_config(&self.config.llm_config),
            operation_type: "message_intercept".to_string(),
            success: matches!(result, InterceptResult::Allow),
            response_time: elapsed,
            tokens_used: None, // Guardian uses rule-based detection primarily
            error_message: None,
            json_parse_success: true,
            had_syntax_errors: false,
            validation_passed: true,
        };
        
        // Record operation (fire and forget, don't block on DB)
        let metrics_clone = Arc::clone(&self.metrics);
        tokio::spawn(async move {
            if let Err(e) = metrics_clone.record_operation(operation_result).await {
                error!("Failed to record Guardian metrics: {}", e);
            }
        });
        
        Ok(result)
    }
}

/// Handle for background tasks (Arc-wrapped components only)
struct GuardianAgentHandle {
    agent_id: AgentId,
    state: Arc<RwLock<GuardianState>>,
    guardian_system: Arc<GuardianSystem>,
    interceptor: Arc<GuardianInterceptor>,
    metrics: Arc<MetricsCollector>,
}

impl GuardianAgentHandle {
    /// Run monitoring loop for real-time message interception
    async fn run_monitoring_loop(&self, mut monitoring_rx: mpsc::Receiver<Message>) {
        info!("Guardian {} monitoring loop active", self.agent_id);
        
        while let Some(message) = monitoring_rx.recv().await {
            // Intercept and analyze message
            if let Err(e) = self.process_monitored_message(message).await {
                error!("Guardian monitoring error: {}", e);
            }
        }
        
        warn!("Guardian {} monitoring loop terminated", self.agent_id);
    }
    
    /// Process a monitored message copy
    async fn process_monitored_message(&self, message: Message) -> Result<()> {
        debug!("Guardian processing monitored message: {:?} -> {:?}", message.from, message.to);
        
        let start_time = std::time::Instant::now();
        
        // Use the interceptor to analyze the message
        let result = self.interceptor.intercept(&message).await?;
        
        let elapsed = start_time.elapsed();
        
        // Record metrics
        let operation_result = crate::agents::metrics::OperationResult {
            agent_type: crate::prompts::AgentType::Guardian,
            agent_id: self.agent_id.clone(),
            config_hash: "guardian_monitor".to_string(),
            operation_type: "message_monitor".to_string(),
            success: matches!(result, crate::messaging::guardian::InterceptResult::Allow),
            response_time: elapsed,
            tokens_used: None,
            error_message: None,
            json_parse_success: true,
            had_syntax_errors: false,
            validation_passed: true,
        };
        
        // Record operation (fire and forget)
        let metrics_clone = Arc::clone(&self.metrics);
        tokio::spawn(async move {
            if let Err(e) = metrics_clone.record_operation(operation_result).await {
                error!("Failed to record Guardian metrics: {}", e);
            }
        });
        
        // Log violations
        match result {
            crate::messaging::guardian::InterceptResult::Block(reason) => {
                error!("Guardian BLOCKED message {:?} -> {:?}: {:?}", 
                    message.from, message.to, reason);
            }
            crate::messaging::guardian::InterceptResult::Pause(reason) => {
                warn!("Guardian PAUSED message {:?} -> {:?}: {:?}", 
                    message.from, message.to, reason);
            }
            crate::messaging::guardian::InterceptResult::Allow => {
                // Normal operation, no logging needed
            }
        }
        
        Ok(())
    }
    
    async fn run_audit_workflow(&self) -> Result<AuditReport> {
        // Transition to Auditing state
        {
            let mut state = self.state.write().await;
            *state = GuardianState::Auditing;
        }
        
        // Perform audit
        info!("Guardian {} running audit", self.agent_id);
        
        let stats = self.interceptor.get_stats().await;
        
        let report = AuditReport {
            timestamp: std::time::SystemTime::now(),
            total_messages: stats.messages_intercepted,
            violations_found: stats.messages_blocked + stats.messages_paused,
            actions_taken: vec![
                format!("Blocked: {}", stats.messages_blocked),
                format!("Paused: {}", stats.messages_paused),
            ],
        };
        
        // Return to Monitoring
        {
            let mut state = self.state.write().await;
            *state = GuardianState::Monitoring;
        }
        
        Ok(report)
    }
    
    async fn run_learning_workflow(&self) -> Result<LearningReport> {
        // Transition to Learning state
        {
            let mut state = self.state.write().await;
            *state = GuardianState::Learning;
        }
        
        info!("Guardian {} running learning cycle", self.agent_id);
        
        // TODO: Actual learning logic
        let report = LearningReport {
            timestamp: std::time::SystemTime::now(),
            patterns_updated: 0,
            false_positives_corrected: 0,
            new_rules_added: 0,
        };
        
        // Return to Monitoring
        {
            let mut state = self.state.write().await;
            *state = GuardianState::Monitoring;
        }
        
        Ok(report)
    }
    
    async fn run_reporting_workflow(&self, _trigger: ReportTrigger) -> Result<ComplianceReport> {
        // Transition to Reporting state
        {
            let mut state = self.state.write().await;
            *state = GuardianState::Reporting;
        }
        
        info!("Guardian {} generating compliance report", self.agent_id);
        
        let stats = self.interceptor.get_stats().await;
        
        let total = stats.messages_intercepted;
        let blocked = stats.messages_blocked;
        let paused = stats.messages_paused;
        
        let compliance_rate = if total > 0 {
            (total - blocked - paused) as f32 / total as f32
        } else {
            1.0
        };
        
        let report = ComplianceReport {
            timestamp: std::time::SystemTime::now(),
            period_start: std::time::SystemTime::now(),
            period_end: std::time::SystemTime::now(),
            total_messages: total,
            violations_blocked: blocked,
            violations_paused: paused,
            compliance_rate,
            top_violations: vec![],
        };
        
        // Return to Monitoring
        {
            let mut state = self.state.write().await;
            *state = GuardianState::Monitoring;
        }
        
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_guardian_creation() {
        let config = GuardianConfig::default();
        let metrics = Arc::new(MetricsCollector::new(":memory:").await.unwrap());
        let ai_provider_manager = Arc::new(AIProviderManager::new(None, "standalone".to_string()).await.unwrap());
        let guardian = GuardianAgent::new(config, metrics, ai_provider_manager);
        
        assert_eq!(guardian.current_state().await, GuardianState::Startup);
    }
    
    #[tokio::test]
    async fn test_guardian_start() {
        let config = GuardianConfig::default();
        let metrics = Arc::new(MetricsCollector::new(":memory:").await.unwrap());
        let ai_provider_manager = Arc::new(AIProviderManager::new(None, "standalone".to_string()).await.unwrap());
        let mut guardian = GuardianAgent::new(config, metrics, ai_provider_manager);
        
        // Create monitoring channel for Guardian
        use tokio::sync::mpsc;
        let (_tx, rx) = mpsc::channel::<Message>(100);
        
        guardian.start(rx).await.unwrap();
        assert_eq!(guardian.current_state().await, GuardianState::Monitoring);
    }
    
    #[tokio::test]
    async fn test_constitutional_checker() {
        let checker = ConstitutionalChecker::new(vec![Article::Article1Privacy]);
        
        let mut context = ComplianceContext::default();
        context.involves_external_api = true;
        context.has_user_consent = false;
        
        let compliant = checker.check_article_compliance(&Article::Article1Privacy, &context).await;
        assert!(!compliant); // Should fail without user consent
        
        context.has_user_consent = true;
        let compliant = checker.check_article_compliance(&Article::Article1Privacy, &context).await;
        assert!(compliant); // Should pass with user consent
    }
}
