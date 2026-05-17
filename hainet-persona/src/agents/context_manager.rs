//! # START OF FILE hainet-persona/src/agents/context_manager.rs
//! Context Manager — Ported from TrippleEffect's ContextSummarizer
//!
//! Manages agent message history to stay within LLM context windows.
//! Essential for small local models (8K-32K context).
//!
//! From TE: Bounded workspace trees, auto-summarization,
//! 5-message history limit for chat context, context anchors for Admin AI.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Estimated chars per token (rough approximation)
const CHARS_PER_TOKEN: usize = 4;

/// Default context window safety margin (keep 20% free for output)
const CONTEXT_MARGIN: f64 = 0.80;

/// Maximum messages to keep in raw history before summarization kicks in
const MAX_RAW_MESSAGES: usize = 30;

/// Number of recent messages to always preserve (never summarize)
const PRESERVE_RECENT: usize = 5;

/// Maximum context anchors to preserve during summarization
const MAX_CONTEXT_ANCHORS: usize = 3;

/// A message in agent history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// Context window manager
pub struct ContextManager {
    /// Maximum tokens the model supports
    max_context_tokens: usize,
    /// History of summarizations performed
    summarization_count: u32,
}

impl ContextManager {
    pub fn new(max_context_tokens: usize) -> Self {
        Self {
            max_context_tokens,
            summarization_count: 0,
        }
    }

    /// Estimate token count from message history
    /// (From TE: estimate_token_count in ContextSummarizer)
    pub fn estimate_tokens(&self, messages: &[AgentMessage]) -> usize {
        messages
            .iter()
            .map(|m| {
                let content_tokens = m.content.len() / CHARS_PER_TOKEN;
                let tool_tokens = m
                    .tool_calls
                    .as_ref()
                    .map(|tc| {
                        tc.iter()
                            .map(|t| t.to_string().len() / CHARS_PER_TOKEN)
                            .sum::<usize>()
                    })
                    .unwrap_or(0);
                content_tokens + tool_tokens + 4 // 4 tokens overhead per message
            })
            .sum()
    }

    /// Check if context summarization is needed
    /// (From TE: should_summarize_context)
    pub fn needs_summarization(&self, messages: &[AgentMessage]) -> bool {
        let estimated = self.estimate_tokens(messages);
        let threshold = (self.max_context_tokens as f64 * CONTEXT_MARGIN) as usize;

        if estimated > threshold {
            info!(
                estimated_tokens = estimated,
                threshold,
                max_context = self.max_context_tokens,
                "Context summarization needed"
            );
            return true;
        }

        // Also trigger if too many raw messages (even if tokens are OK)
        if messages.len() > MAX_RAW_MESSAGES {
            info!(
                message_count = messages.len(),
                max = MAX_RAW_MESSAGES,
                "Message count exceeds limit — summarization needed"
            );
            return true;
        }

        false
    }

    /// Trim message history by removing old messages, keeping recent ones
    /// and system messages. Returns the trimmed history.
    ///
    /// This is the "cheap" summarization — just drops old messages.
    /// Full LLM-based summarization would be done by the sidecar.
    ///
    /// (From TE: summarize_agent_context with context anchors)
    pub fn trim_history(&mut self, messages: &[AgentMessage]) -> Vec<AgentMessage> {
        if messages.len() <= PRESERVE_RECENT + 1 {
            return messages.to_vec();
        }

        let mut trimmed = Vec::new();

        // Always keep the system prompt (first message)
        if let Some(first) = messages.first() {
            if first.role == MessageRole::System {
                trimmed.push(first.clone());
            }
        }

        // Find and preserve context anchors (task descriptions, user requests)
        let mut anchors_found = 0;
        for msg in messages.iter().skip(1) {
            if anchors_found >= MAX_CONTEXT_ANCHORS {
                break;
            }
            let is_anchor = msg.role == MessageRole::User
                || (msg.role == MessageRole::System
                    && msg.content.to_lowercase().contains("current task"));
            if is_anchor {
                trimmed.push(msg.clone());
                anchors_found += 1;
            }
        }

        // Add a summary marker
        let skipped = messages.len() - PRESERVE_RECENT - trimmed.len();
        if skipped > 0 {
            trimmed.push(AgentMessage {
                role: MessageRole::System,
                content: format!(
                    "[Context Summary]: {} earlier messages were summarized to stay within \
                     context limits. The most recent messages follow.",
                    skipped
                ),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        // Always keep the last PRESERVE_RECENT messages
        let recent_start = messages.len().saturating_sub(PRESERVE_RECENT);
        for msg in &messages[recent_start..] {
            trimmed.push(msg.clone());
        }

        self.summarization_count += 1;
        info!(
            original_count = messages.len(),
            trimmed_count = trimmed.len(),
            summarizations = self.summarization_count,
            "Context trimmed"
        );

        trimmed
    }

    /// Deduplicate framework messages in PM agent history
    /// (From TE: _deduplicate_pm_framework_messages)
    pub fn deduplicate_framework_messages(messages: &mut Vec<AgentMessage>) {
        let framework_prefixes = [
            "[Framework System Message",
            "[Constitutional Guardian",
            "[Framework Watchdog Intervention]",
        ];

        // Find indices of framework messages (excluding kickoff plans)
        let indices_to_remove: Vec<usize> = messages
            .iter()
            .enumerate()
            .filter_map(|(i, msg)| {
                if matches!(msg.role, MessageRole::System | MessageRole::User) {
                    let content = &msg.content;
                    let is_framework = framework_prefixes
                        .iter()
                        .any(|prefix| content.starts_with(prefix));
                    let is_protected = content.contains("**MASTER KICKOFF PLAN SUMMARY**")
                        || content.contains("**create_agent Tool Usage:**");

                    if is_framework && !is_protected {
                        return Some(i);
                    }
                }
                None
            })
            .collect();

        if !indices_to_remove.is_empty() {
            let original_len = messages.len();
            let remove_set: std::collections::HashSet<usize> =
                indices_to_remove.into_iter().collect();
            let mut idx = 0;
            messages.retain(|_| {
                let keep = !remove_set.contains(&idx);
                idx += 1;
                keep
            });
            debug!(
                removed = original_len - messages.len(),
                "Deduplicated framework messages"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_msg(role: MessageRole, content: &str) -> AgentMessage {
        AgentMessage {
            role,
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    #[test]
    fn test_token_estimation() {
        let cm = ContextManager::new(8000);
        let messages = vec![
            make_msg(MessageRole::System, "You are a helpful assistant."),
            make_msg(MessageRole::User, "Hello!"),
        ];
        let tokens = cm.estimate_tokens(&messages);
        assert!(tokens > 0);
        assert!(tokens < 100); // Should be small for short messages
    }

    #[test]
    fn test_needs_summarization_large_context() {
        let cm = ContextManager::new(100); // Tiny context window
        let messages: Vec<_> = (0..50)
            .map(|i| make_msg(MessageRole::User, &format!("Message number {}", i)))
            .collect();
        assert!(cm.needs_summarization(&messages));
    }

    #[test]
    fn test_trim_keeps_system_and_recent() {
        let mut cm = ContextManager::new(8000);
        let mut messages = vec![make_msg(MessageRole::System, "System prompt")];
        for i in 0..20 {
            messages.push(make_msg(MessageRole::User, &format!("Msg {}", i)));
        }

        let trimmed = cm.trim_history(&messages);

        // Should have: system + context anchors + summary + last 5
        assert!(trimmed.len() < messages.len());
        assert_eq!(trimmed[0].role, MessageRole::System);
        assert_eq!(trimmed[0].content, "System prompt");

        // Last message should be preserved
        let last_trimmed = trimmed.last().unwrap();
        assert!(last_trimmed.content.contains("19"));
    }

    #[test]
    fn test_deduplicate_framework_messages() {
        let mut messages = vec![
            make_msg(MessageRole::System, "System prompt"),
            make_msg(MessageRole::System, "[Framework System Message] Old intervention"),
            make_msg(MessageRole::User, "User request"),
            make_msg(MessageRole::System, "[Framework System Message] Another old one"),
            make_msg(MessageRole::Assistant, "Response"),
        ];

        ContextManager::deduplicate_framework_messages(&mut messages);
        assert_eq!(messages.len(), 3); // System + User + Assistant
    }

    #[test]
    fn test_protected_messages_not_removed() {
        let mut messages = vec![
            make_msg(
                MessageRole::System,
                "[Framework System Message] **MASTER KICKOFF PLAN SUMMARY** important plan",
            ),
            make_msg(MessageRole::System, "[Framework System Message] Old removable"),
        ];

        ContextManager::deduplicate_framework_messages(&mut messages);
        assert_eq!(messages.len(), 1); // Only the protected one remains
    }
}
