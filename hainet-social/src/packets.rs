//! # START OF FILE hainet-social/src/packets.rs
//! Network Packet Schema — Ported from gChat's packetSchema.ts (325 lines)
//!
//! All 32 original gChat packet types + HAI-Net extensions (ComputeAnnouncement, ComputeStatus).
//! Each packet type maps 1:1 to its gChat Zod schema counterpart.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// SHARED TYPES (from gChat's shared schemas)
// ============================================================================

/// Media metadata attached to posts and messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub id: String,
    #[serde(rename = "type")]
    pub media_type: MediaType,
    pub mime_type: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_savable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    Audio,
    Video,
    File,
    Image,
}

/// Privacy level for posts
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Privacy {
    Public,
    Friends,
    Private,
}

/// Vote direction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VoteType {
    Up,
    Down,
}

/// Reaction action (toggle support)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReactionAction {
    Add,
    Remove,
}

/// Encrypted payload for private messages (NaCl box)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    pub id: String,
    pub nonce: String,      // Base64-encoded nonce
    pub ciphertext: String, // Base64-encoded ciphertext
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
}

/// User profile for identity updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub user_id: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bio: Option<String>,
}

/// Connection request between users
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionRequest {
    pub id: String,
    pub from_user_id: String,
    pub from_username: String,
    pub from_display_name: String,
    pub from_home_node: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_encryption_public_key: Option<String>,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// A comment on a post (recursive replies via nested Vec)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: String,
    pub author_id: String,
    pub author_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_avatar: Option<String>,
    pub content: String,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub votes: Option<HashMap<String, VoteType>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reactions: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replies: Option<Vec<Comment>>,
}

/// A social post
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: String,
    pub author_id: String,
    pub author_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_avatar: Option<String>,
    pub author_public_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_node: Option<String>,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<MediaMetadata>,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub votes: Option<HashMap<String, VoteType>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shares: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments_list: Option<Vec<Comment>>,
    pub privacy: Privacy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_edited: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hashtags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reactions: Option<HashMap<String, Vec<String>>>,
}

/// Group definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub members: Vec<String>,
    pub admins: Vec<String>,
    pub owner_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banned_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_muted: Option<bool>,
}

/// Shared post snapshot for reshares
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedPostSnapshot {
    pub author_name: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<MediaMetadata>,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_node: Option<String>,
}

// ============================================================================
// BASE PACKET HEADER
// ============================================================================

/// Common fields for all network packets (maps to gChat's BasePacket)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PacketHeader {
    /// Unique packet identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// TTL hop counter for daisy-chain gossip (default 6)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hops: Option<u8>,
    /// Sender identifier (onion address or peer ID)
    pub sender_id: String,
    /// Optional target user for directed messages
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_user_id: Option<String>,
    /// Ed25519 signature of the payload
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

// ============================================================================
// NETWORK PACKET — All 32 gChat types + HAI-Net extensions
// ============================================================================

/// The complete set of network packet types.
///
/// Original gChat types (32):
///   Messaging: MESSAGE
///   Social:    POST, EDIT_POST, DELETE_POST
///   Interact:  VOTE, COMMENT, COMMENT_VOTE, COMMENT_REACTION, REACTION, CHAT_REACTION, CHAT_VOTE
///   Identity:  CONNECTION_REQUEST, IDENTITY_UPDATE, ANNOUNCE_PEER, FOLLOW, UNFOLLOW
///   Groups:    GROUP_INVITE, GROUP_UPDATE, GROUP_DELETE, GROUP_QUERY, GROUP_SYNC
///   Sync:      TYPING, READ_RECEIPT, INVENTORY_SYNC_REQUEST, INVENTORY_SYNC_RESPONSE
///   Media:     MEDIA_RELAY_REQUEST, MEDIA_REQUEST, MEDIA_CHUNK, MEDIA_RECOVERY_FOUND,
///              MEDIA_TRANSFER_ACK, MEDIA_PENDING
///   Lifecycle: USER_EXIT, NODE_SHUTDOWN
///
/// HAI-Net extensions (2):
///   Compute:   COMPUTE_ANNOUNCEMENT, COMPUTE_STATUS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPacket {
    #[serde(flatten)]
    pub header: PacketHeader,
    #[serde(flatten)]
    pub payload: PacketPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PacketPayload {
    // ── 1. MESSAGING ──────────────────────────────────────────────────────
    #[serde(rename = "MESSAGE")]
    Message { payload: EncryptedPayload },

    // ── 2. SOCIAL STREAM ──────────────────────────────────────────────────
    #[serde(rename = "POST")]
    Post { payload: Post },
    #[serde(rename = "EDIT_POST")]
    EditPost { payload: EditPostPayload },
    #[serde(rename = "DELETE_POST")]
    DeletePost { payload: DeletePostPayload },

    // ── 3. INTERACTIONS ───────────────────────────────────────────────────
    #[serde(rename = "VOTE")]
    Vote { payload: VotePayload },
    #[serde(rename = "COMMENT")]
    CommentPacket { payload: CommentPayload },
    #[serde(rename = "COMMENT_VOTE")]
    CommentVote { payload: CommentVotePayload },
    #[serde(rename = "COMMENT_REACTION")]
    CommentReaction { payload: CommentReactionPayload },
    #[serde(rename = "REACTION")]
    Reaction { payload: ReactionPayload },
    #[serde(rename = "CHAT_REACTION")]
    ChatReaction { payload: ChatReactionPayload },
    #[serde(rename = "CHAT_VOTE")]
    ChatVote { payload: ChatVotePayload },

    // ── 4. IDENTITY & CONNECTION ──────────────────────────────────────────
    #[serde(rename = "CONNECTION_REQUEST")]
    ConnectionRequestPacket { payload: ConnectionRequest },
    #[serde(rename = "IDENTITY_UPDATE")]
    IdentityUpdate { payload: UserProfile },
    #[serde(rename = "ANNOUNCE_PEER")]
    AnnouncePeer { payload: AnnouncePeerPayload },
    #[serde(rename = "FOLLOW")]
    Follow { payload: FollowPayload },
    #[serde(rename = "UNFOLLOW")]
    Unfollow { payload: FollowPayload },

    // ── 5. GROUPS ─────────────────────────────────────────────────────────
    #[serde(rename = "GROUP_INVITE")]
    GroupInvite { payload: Group },
    #[serde(rename = "GROUP_UPDATE")]
    GroupUpdate { payload: Group },
    #[serde(rename = "GROUP_DELETE")]
    GroupDelete { payload: GroupDeletePayload },
    #[serde(rename = "GROUP_QUERY")]
    GroupQuery { payload: GroupQueryPayload },
    #[serde(rename = "GROUP_SYNC")]
    GroupSync { payload: GroupSyncPayload },

    // ── 6. SYNC / ACKS ───────────────────────────────────────────────────
    #[serde(rename = "TYPING")]
    Typing { payload: TypingPayload },
    #[serde(rename = "READ_RECEIPT")]
    ReadReceipt { payload: ReadReceiptPayload },
    #[serde(rename = "INVENTORY_SYNC_REQUEST")]
    InventorySyncRequest { payload: InventorySyncRequestPayload },
    #[serde(rename = "INVENTORY_SYNC_RESPONSE")]
    InventorySyncResponse { payload: InventorySyncResponsePayload },

    // ── 7. MEDIA RELAY ────────────────────────────────────────────────────
    #[serde(rename = "MEDIA_RELAY_REQUEST")]
    MediaRelayRequest { payload: MediaRelayRequestPayload },
    #[serde(rename = "MEDIA_REQUEST")]
    MediaRequest { payload: MediaRequestPayload },
    #[serde(rename = "MEDIA_CHUNK")]
    MediaChunk { payload: MediaChunkPayload },
    #[serde(rename = "MEDIA_RECOVERY_FOUND")]
    MediaRecoveryFound { payload: MediaRecoveryFoundPayload },
    #[serde(rename = "MEDIA_TRANSFER_ACK")]
    MediaTransferAck { payload: MediaTransferAckPayload },
    #[serde(rename = "MEDIA_PENDING")]
    MediaPending { payload: MediaPendingPayload },

    // ── 8. LIFECYCLE ──────────────────────────────────────────────────────
    #[serde(rename = "USER_EXIT")]
    UserExit { payload: UserExitPayload },
    #[serde(rename = "NODE_SHUTDOWN")]
    NodeShutdown { payload: NodeShutdownPayload },

    // ── 9. HAI-NET EXTENSIONS ─────────────────────────────────────────────
    /// Compute resource announcement (replaces PPLPWR's Matrix announcements)
    #[serde(rename = "COMPUTE_ANNOUNCEMENT")]
    ComputeAnnouncement { payload: ComputeAnnouncementPayload },
    /// Node compute contribution status
    #[serde(rename = "COMPUTE_STATUS")]
    ComputeStatus { payload: ComputeStatusPayload },
}

// ============================================================================
// PAYLOAD STRUCTS (one per packet type)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditPostPayload {
    pub post_id: String,
    pub new_content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletePostPayload {
    pub post_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotePayload {
    pub post_id: String,
    pub user_id: String,
    #[serde(rename = "type")]
    pub vote_type: VoteType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentPayload {
    pub post_id: String,
    pub comment: Comment,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_comment_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentVotePayload {
    pub post_id: String,
    pub comment_id: String,
    pub user_id: String,
    #[serde(rename = "type")]
    pub vote_type: VoteType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentReactionPayload {
    pub post_id: String,
    pub comment_id: String,
    pub user_id: String,
    pub emoji: String,
    pub action: ReactionAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionPayload {
    pub post_id: String,
    pub user_id: String,
    pub emoji: String,
    pub action: ReactionAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatReactionPayload {
    pub message_id: String,
    pub user_id: String,
    pub emoji: String,
    pub action: ReactionAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatVotePayload {
    pub message_id: String,
    pub user_id: String,
    #[serde(rename = "type")]
    pub vote_type: VoteType,
    pub action: ReactionAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnouncePeerPayload {
    pub onion_address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowPayload {
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupDeletePayload {
    pub group_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupQueryPayload {
    pub requester_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupSyncPayload {
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingPayload {
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadReceiptPayload {
    pub message_id: String,
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItem {
    pub id: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventorySyncRequestPayload {
    pub inventory: Vec<InventoryItem>,
    pub since: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventorySyncResponsePayload {
    pub posts: Vec<Post>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaRelayRequestPayload {
    pub media_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<MediaMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaRequestPayload {
    pub media_id: String,
    pub chunk_index: u32,
    pub chunk_size: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaChunkPayload {
    pub media_id: String,
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub data: String, // Base64-encoded chunk data
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaRecoveryFoundPayload {
    pub media_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaTransferAckPayload {
    pub media_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPendingPayload {
    pub media_id: String,
    pub chunk_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserExitPayload {
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeShutdownPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

// ── HAI-NET EXTENSIONS ────────────────────────────────────────────────────

/// Compute resource announcement (replaces PPLPWR's Matrix-based announcements)
/// Travels via gossipsub topic: /hainet/collab/announcements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeAnnouncementPayload {
    pub network: String,         // "petals", "prime_intellect", etc.
    pub run_id: String,
    pub min_vram_gb: f32,
    pub framework: String,       // "pytorch", "jax", etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join_endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Node's compute contribution status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeStatusPayload {
    pub node_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contributing_to: Option<String>,
    pub uptime_hours: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_utilization: Option<f32>,
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_packet_roundtrip() {
        let packet = NetworkPacket {
            header: PacketHeader {
                id: Some("test-1".to_string()),
                hops: Some(6),
                sender_id: "abc123.onion".to_string(),
                target_user_id: None,
                signature: None,
            },
            payload: PacketPayload::Post {
                payload: Post {
                    id: "post-1".to_string(),
                    author_id: "user-1".to_string(),
                    author_name: "Alice".to_string(),
                    author_avatar: None,
                    author_public_key: "pk_base64".to_string(),
                    origin_node: Some("abc123.onion".to_string()),
                    content: "Hello HAI-Net!".to_string(),
                    content_hash: None,
                    image_url: None,
                    media: None,
                    timestamp: 1700000000,
                    votes: None,
                    shares: None,
                    comments_count: None,
                    comments_list: None,
                    privacy: Privacy::Public,
                    is_edited: None,
                    hashtags: Some(vec!["hainet".to_string()]),
                    reactions: None,
                },
            },
        };

        let json = serde_json::to_string(&packet).unwrap();
        let deserialized: NetworkPacket = serde_json::from_str(&json).unwrap();

        match &deserialized.payload {
            PacketPayload::Post { payload } => {
                assert_eq!(payload.content, "Hello HAI-Net!");
                assert_eq!(payload.privacy, Privacy::Public);
            }
            _ => panic!("Expected Post packet"),
        }
    }

    #[test]
    fn test_compute_announcement_roundtrip() {
        let packet = NetworkPacket {
            header: PacketHeader {
                id: Some("compute-1".to_string()),
                hops: Some(4),
                sender_id: "node-42".to_string(),
                target_user_id: None,
                signature: None,
            },
            payload: PacketPayload::ComputeAnnouncement {
                payload: ComputeAnnouncementPayload {
                    network: "petals".to_string(),
                    run_id: "run-abc".to_string(),
                    min_vram_gb: 8.0,
                    framework: "pytorch".to_string(),
                    join_endpoint: Some("http://10.0.0.5:8080".to_string()),
                    description: Some("LLaMA 70B fine-tune".to_string()),
                },
            },
        };

        let json = serde_json::to_string(&packet).unwrap();
        let deserialized: NetworkPacket = serde_json::from_str(&json).unwrap();

        match &deserialized.payload {
            PacketPayload::ComputeAnnouncement { payload } => {
                assert_eq!(payload.network, "petals");
                assert_eq!(payload.min_vram_gb, 8.0);
            }
            _ => panic!("Expected ComputeAnnouncement packet"),
        }
    }

    #[test]
    fn test_encrypted_message_roundtrip() {
        let packet = NetworkPacket {
            header: PacketHeader {
                id: Some("msg-1".to_string()),
                hops: None,
                sender_id: "sender.onion".to_string(),
                target_user_id: Some("recipient.onion".to_string()),
                signature: Some("sig_base64".to_string()),
            },
            payload: PacketPayload::Message {
                payload: EncryptedPayload {
                    id: "msg-1".to_string(),
                    nonce: "nonce_base64".to_string(),
                    ciphertext: "cipher_base64".to_string(),
                    group_id: None,
                },
            },
        };

        let json = serde_json::to_string(&packet).unwrap();
        let deserialized: NetworkPacket = serde_json::from_str(&json).unwrap();

        match &deserialized.payload {
            PacketPayload::Message { payload } => {
                assert_eq!(payload.ciphertext, "cipher_base64");
            }
            _ => panic!("Expected Message packet"),
        }
    }
}
