// START OF FILE hainet-social/src/interactions.rs
//! Social Interactions
//! 
//! Implements structures and logic for post interactions including
//! votes (up/down), comments, and emoji reactions.

use serde::{Serialize, Deserialize};

/// Represents an emoji reaction to a post or comment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub author_id: String,
    pub emoji: String, // Unicode emoji or shortcode
    pub target_id: String, // Post ID or Comment ID
    pub created_at: u64,
}

/// Represents a vote direction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VoteType {
    Upvote,
    Downvote,
}

/// Represents a vote on a post or comment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    pub author_id: String,
    pub target_id: String, // Post ID or Comment ID
    pub vote_type: VoteType,
    pub created_at: u64,
}

/// Represents a comment on a post
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub post_id: String,
    pub author_id: String,
    pub content: String,
    pub created_at: u64,
    pub reply_to: Option<String>, // ID of parent comment, if nested
}

impl Comment {
    pub fn new(post_id: String, author_id: String, content: String, reply_to: Option<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            post_id,
            author_id,
            content,
            created_at: now,
            reply_to,
        }
    }
}

/// Trait defining storage operations for interactions
pub trait InteractionStorage: Send + Sync {
    fn add_reaction(&self, reaction: &Reaction) -> crate::SocialResult<()>;
    fn remove_reaction(&self, target_id: &str, author_id: &str, emoji: &str) -> crate::SocialResult<()>;
    fn get_reactions(&self, target_id: &str) -> crate::SocialResult<Vec<Reaction>>;

    fn cast_vote(&self, vote: &Vote) -> crate::SocialResult<()>;
    fn get_votes(&self, target_id: &str) -> crate::SocialResult<(u32, u32)>; // (upvotes, downvotes)

    fn add_comment(&self, comment: &Comment) -> crate::SocialResult<()>;
    fn get_comments(&self, post_id: &str) -> crate::SocialResult<Vec<Comment>>;
}
