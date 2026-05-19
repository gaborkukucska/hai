// START OF FILE hainet-social/src/feed.rs
//! Social Feed and Post Management
//! 
//! Ports core social post structures and feed generation algorithms from gChat.

use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Visibility level of a post
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PostVisibility {
    Public,
    FriendsOnly,
    Group(String), // Group ID
}

/// Represents a media attachment in a post
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAttachment {
    pub file_id: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub blurhash: Option<String>,
}

/// Represents a social post
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: String,
    pub author_id: String,
    pub content: String,
    pub visibility: PostVisibility,
    pub created_at: u64,
    pub media: Vec<MediaAttachment>,
    pub reply_to: Option<String>, // ID of the parent post if this is a reply
}

impl Post {
    pub fn new(
        author_id: String,
        content: String,
        visibility: PostVisibility,
        media: Vec<MediaAttachment>,
        reply_to: Option<String>
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            author_id,
            content,
            visibility,
            created_at: now,
            media,
            reply_to,
        }
    }
}

/// Defines storage requirements for social feeds
pub trait FeedStorage: Send + Sync {
    /// Save a new post
    fn save_post(&self, post: &Post) -> crate::SocialResult<()>;
    
    /// Retrieve a post by ID
    fn get_post(&self, id: &str) -> crate::SocialResult<Option<Post>>;
    
    /// Get public feed (chronological)
    fn get_public_feed(&self, limit: usize, offset: usize) -> crate::SocialResult<Vec<Post>>;
    
    /// Get personalized feed (friends + public)
    fn get_personalized_feed(&self, user_id: &str, limit: usize, offset: usize) -> crate::SocialResult<Vec<Post>>;
}

/// In-memory storage for testing and quick start
#[derive(Default)]
pub struct InMemoryFeedStorage {
    posts: std::sync::RwLock<HashMap<String, Post>>,
}

impl InMemoryFeedStorage {
    pub fn new() -> Self {
        Self {
            posts: std::sync::RwLock::new(HashMap::new()),
        }
    }
}

impl FeedStorage for InMemoryFeedStorage {
    fn save_post(&self, post: &Post) -> crate::SocialResult<()> {
        let mut posts = self.posts.write().unwrap();
        posts.insert(post.id.clone(), post.clone());
        Ok(())
    }

    fn get_post(&self, id: &str) -> crate::SocialResult<Option<Post>> {
        let posts = self.posts.read().unwrap();
        Ok(posts.get(id).cloned())
    }

    fn get_public_feed(&self, limit: usize, offset: usize) -> crate::SocialResult<Vec<Post>> {
        let posts = self.posts.read().unwrap();
        let mut public_posts: Vec<_> = posts.values()
            .filter(|p| p.visibility == PostVisibility::Public)
            .cloned()
            .collect();
            
        // Sort newest first
        public_posts.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        
        let result = public_posts.into_iter()
            .skip(offset)
            .take(limit)
            .collect();
            
        Ok(result)
    }

    fn get_personalized_feed(&self, _user_id: &str, limit: usize, offset: usize) -> crate::SocialResult<Vec<Post>> {
        // Simplified: just return public feed for now
        // A real implementation would filter by friends graph
        self.get_public_feed(limit, offset)
    }
}
