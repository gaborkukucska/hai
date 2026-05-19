// START OF FILE hainet-social/src/groups.rs
//! Group Management
//! 
//! Ports gChat's group features: invites, state synchronization, 
//! and admin controls for managing private and public groups.

use serde::{Serialize, Deserialize};

/// Role of a user within a group
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GroupRole {
    Owner,
    Admin,
    Member,
    Pending, // Invited but not accepted
}

/// Represents a member within a group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMember {
    pub user_id: String,
    pub role: GroupRole,
    pub joined_at: u64,
}

/// Represents a social group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_private: bool,
    pub created_at: u64,
    pub members: Vec<GroupMember>,
}

impl Group {
    pub fn new(name: String, description: String, is_private: bool, owner_id: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
            
        let owner = GroupMember {
            user_id: owner_id,
            role: GroupRole::Owner,
            joined_at: now,
        };
            
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description,
            is_private,
            created_at: now,
            members: vec![owner],
        }
    }

    /// Check if a user is an admin or owner
    pub fn is_admin(&self, user_id: &str) -> bool {
        self.members.iter().any(|m| {
            m.user_id == user_id && (m.role == GroupRole::Owner || m.role == GroupRole::Admin)
        })
    }
}

/// Represents a group invitation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupInvite {
    pub group_id: String,
    pub inviter_id: String,
    pub invitee_id: String,
    pub created_at: u64,
}

/// Trait defining storage and management operations for groups
pub trait GroupManager: Send + Sync {
    fn create_group(&self, group: &Group) -> crate::SocialResult<()>;
    fn get_group(&self, id: &str) -> crate::SocialResult<Option<Group>>;
    fn update_group(&self, group: &Group) -> crate::SocialResult<()>;
    
    fn invite_user(&self, invite: &GroupInvite) -> crate::SocialResult<()>;
    fn accept_invite(&self, group_id: &str, user_id: &str) -> crate::SocialResult<()>;
    fn remove_member(&self, group_id: &str, target_user_id: &str, requester_id: &str) -> crate::SocialResult<()>;
}
