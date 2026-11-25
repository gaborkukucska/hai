//! START OF FILE hainet-noslop/src/hub/mod.rs
//!
//! # The NoSlop Home HUB node
//!
//! Runs inside `hainet-core` and makes the Hub a correct NoSlop 0.8.x node for the owner's phone:
//!
//! * **Ingress** ([`HubNode::serve`]): mesh listeners for the personal onion and the creator onion
//!   (loopback only; Tor forwards to them). Every frame is size-bounded, parsed, and must carry a
//!   valid envelope signature (`noslop-env-v1`) that is not a replay, before anything else happens.
//! * **Creator duties** (`creator.rs`): while creator hosting is on, the Hub answers the creator
//!   identity's followers itself: follower handshakes (auto-accepted like the phone does), `SYNC_REQUEST`
//!   / `INVENTORY_SYNC_REQUEST` (the creator's own public posts and the engagement on them), media
//!   requests for the creator's public media, and the creator's presence and discoverable announcements.
//! * **Inbox**: everything else that is addressed to the owner is stored durably for the phone, which
//!   pulls it with `hub_inbox_pull` and removes it with `hub_inbox_ack`.
//! * **Delivery** ([`HubNode::deliver`]): packets the phone hands over (`hub_send`, with the exact
//!   destination the phone chose) and packets the Hub writes itself go out over Tor. A packet sent as
//!   one of the owner's identities is signed again at every attempt (fresh `env_ts`), the same as
//!   `MeshTransport.prepareForWire`; anyone else's packet is passed on unchanged.
//!
//! The owner's control API is in `api.rs`; every command there requires an owner signature.

pub mod admin;
pub mod api;
pub mod content;
mod creator;
pub mod dms;
pub mod store;
pub mod tor;

use crate::{envelope::{self, ReplayGuard, Verdict}, json::JValue, keys::Identity, packet::{Packet, MAX_FRAME_BYTES}};
use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use store::{InboxLimits, Store};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, RwLock, Semaphore};
use tracing::{debug, info, warn};

pub type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;
/// Sends one line to an onion (production: [`tor::send_line`]; tests: a recorder).
pub type SendFn = Arc<dyn Fn(String, String) -> BoxFut<bool> + Send + Sync>;
/// Called for a `MESSAGE` the owner's phone addressed to the Hub's Admin AI (`admin_<personal key>`).
pub type AdminHook = Arc<dyn Fn(Packet) -> BoxFut<()> + Send + Sync>;
pub type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Hub settings. Every value can be changed with the environment variable named next to it.
#[derive(Debug, Clone)]
pub struct HubConfig {
    /// `~/.hainet/identity` (`HAINET_IDENTITY_DIR`). The creator identity lives in `creator/` inside it.
    pub identity_dir: PathBuf,
    /// Media and backups (`HAINET_NOSLOP_DIR`, default `<data_dir>/noslop`).
    pub noslop_dir: PathBuf,
    /// Loopback address Tor forwards the personal onion's port 9999 to (`HAINET_MESH_LISTEN`).
    pub personal_listen: SocketAddr,
    /// Loopback address Tor forwards the creator onion's port 9999 to (`HAINET_CREATOR_MESH_LISTEN`).
    pub creator_listen: SocketAddr,
    /// Tor SOCKS5 proxy (`HAINET_TOR_SOCKS`).
    pub socks: String,
    /// Tor control interface (`HAINET_TOR_CONTROL`, `unix:/run/tor/control` or `host:port`).
    pub tor_control: tor::ControlTarget,
    /// Optional control-port password (`HAINET_TOR_CONTROL_PASSWORD`).
    pub tor_control_password: Option<String>,
    /// Concurrent inbound mesh connections (`HAINET_MESH_MAX_CONNECTIONS`).
    pub max_connections: usize,
    /// Concurrent outbound Tor sends (`HAINET_MESH_MAX_SENDS`).
    pub max_sends: usize,
    pub inbox: InboxLimits,
    /// Bytes of creator media the Hub keeps (`HAINET_NOSLOP_MEDIA_QUOTA`, default 20 GB).
    pub media_quota: i64,
    /// Largest single backup accepted (`HAINET_NOSLOP_BACKUP_MAX`, default 2 GB).
    pub backup_max: u64,
    /// Backups kept (`HAINET_NOSLOP_BACKUP_KEEP`, default 5).
    pub backup_keep: usize,
}

impl HubConfig {
    pub fn from_env(identity_dir: PathBuf, data_dir: PathBuf) -> Self {
        let default_control = if std::path::Path::new("/run/tor/control").exists() { "unix:/run/tor/control".to_string() } else { "127.0.0.1:9051".to_string() };
        HubConfig {
            identity_dir: std::env::var("HAINET_IDENTITY_DIR").map(PathBuf::from).unwrap_or(identity_dir),
            noslop_dir: std::env::var("HAINET_NOSLOP_DIR").map(PathBuf::from).unwrap_or_else(|_| data_dir.join("noslop")),
            personal_listen: env_or("HAINET_MESH_LISTEN", "127.0.0.1:9999".parse().unwrap()),
            creator_listen: env_or("HAINET_CREATOR_MESH_LISTEN", "127.0.0.1:9998".parse().unwrap()),
            socks: std::env::var("HAINET_TOR_SOCKS").unwrap_or_else(|_| "127.0.0.1:9050".to_string()),
            tor_control: tor::ControlTarget::parse(&std::env::var("HAINET_TOR_CONTROL").unwrap_or(default_control)),
            tor_control_password: std::env::var("HAINET_TOR_CONTROL_PASSWORD").ok(),
            max_connections: env_or("HAINET_MESH_MAX_CONNECTIONS", 64),
            max_sends: env_or("HAINET_MESH_MAX_SENDS", 16),
            inbox: InboxLimits {
                max_items: env_or("HAINET_INBOX_MAX_ITEMS", 20_000),
                max_bytes: env_or("HAINET_INBOX_MAX_BYTES", 512 * 1024 * 1024),
                max_per_sender: env_or("HAINET_INBOX_MAX_PER_SENDER", 2_000),
            },
            media_quota: env_or("HAINET_NOSLOP_MEDIA_QUOTA", 20i64 * 1024 * 1024 * 1024),
            backup_max: env_or("HAINET_NOSLOP_BACKUP_MAX", 2u64 * 1024 * 1024 * 1024),
            backup_keep: env_or("HAINET_NOSLOP_BACKUP_KEEP", 5),
        }
    }

    pub fn media_dir(&self) -> PathBuf { self.noslop_dir.join("media") }
    pub fn backup_dir(&self) -> PathBuf { self.noslop_dir.join("backups") }
    pub fn creator_dir(&self) -> PathBuf { self.identity_dir.join("creator") }
}

/// The creator identity's public profile, as the phone resolves it (`IdentityRouter.creator()`)
/// and its Studio switches.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CreatorProfile {
    /// `IdentityKeys.displayName` of the creator identity ("Name.tag"), used as `from_username`.
    pub display_name: String,
    /// The resolved profile handle (`RoutedIdentity.handle`), used in `ANNOUNCE_DISCOVERABLE`.
    pub handle: String,
    pub bio: Option<String>,
    pub avatar_b64: Option<String>,
    pub fund_me_link: Option<String>,
    /// `is_creator_enabled`: creator mode (followers are auto-accepted).
    pub is_creator: bool,
    /// `is_discoverable_enabled`: the creator is announced with `ANNOUNCE_DISCOVERABLE`.
    pub is_discoverable: bool,
}

impl CreatorProfile {
    fn to_json(&self) -> JValue {
        JValue::obj()
            .with("display_name", JValue::str(&self.display_name))
            .with("handle", JValue::str(&self.handle))
            .with_opt("bio", self.bio.clone().map(JValue::Str))
            .with_opt("avatar_b64", self.avatar_b64.clone().map(JValue::Str))
            .with_opt("fund_me_link", self.fund_me_link.clone().map(JValue::Str))
            .with("is_creator", JValue::Bool(self.is_creator))
            .with("is_discoverable", JValue::Bool(self.is_discoverable))
    }
    fn from_json(v: &JValue) -> Self {
        let opt = |k: &str| v.get_str(k).filter(|s| !s.trim().is_empty()).map(String::from);
        CreatorProfile {
            display_name: v.get_str("display_name").unwrap_or_default().into(),
            handle: v.get_str("handle").unwrap_or_default().into(),
            bio: opt("bio"),
            avatar_b64: opt("avatar_b64"),
            fund_me_link: opt("fund_me_link"),
            is_creator: v.get_bool("is_creator").unwrap_or(false),
            is_discoverable: v.get_bool("is_discoverable").unwrap_or(false),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listener { Personal, Creator }

/// When a delivery is retried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retry {
    /// One attempt (presence, media chunks: the requester asks again).
    Once,
    /// 3 attempts (0 s, 5 s, 15 s), then once a minute for 12 minutes: a fresh onion descriptor can
    /// take minutes to publish (the Hub's earlier behaviour for directed packets).
    Spool,
    /// 3 attempts, then again after 30 s and 90 s (`SyncPacketHandler.SYNC_RETRY_DELAYS_MS`).
    Sync,
}

#[derive(Default)]
struct Limiter {
    frames: HashMap<String, (i64, u32)>,
    auto_accept: HashMap<String, Vec<i64>>,
    media_out: HashMap<String, i64>,
    media_window_start: i64,
    reply_slots: HashMap<String, i64>,
    stranger_media_sigs: std::collections::VecDeque<String>,
    onion_failures: HashMap<String, (u32, i64)>,
}

/// Inbound frames accepted per sender per minute.
const FRAMES_PER_MINUTE: u32 = 600;
/// Frames read from one connection before it is closed.
const FRAMES_PER_CONNECTION: usize = 256;
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

pub struct HubNode {
    pub cfg: HubConfig,
    pub store: Store,
    personal: RwLock<Option<Identity>>,
    creator: RwLock<Option<Identity>>,
    profile: RwLock<CreatorProfile>,
    hosting: RwLock<bool>,
    replay: Mutex<ReplayGuard>,
    limiter: Mutex<Limiter>,
    send_fn: SendFn,
    send_slots: Arc<Semaphore>,
    onion: Option<tor::CreatorOnion>,
    admin_hook: RwLock<Option<AdminHook>>,
    pub(crate) dm_hook: RwLock<Option<dms::DmHook>>,
    pub(crate) dm_remove_hook: RwLock<Option<dms::DmRemoveHook>>,
    clock: Clock,
}

pub fn system_clock() -> Clock { Arc::new(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)) }

fn read_trim(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Loads an identity stored as `ed25519_pub.b64`, `ed25519_priv.b64`, `x25519_pub.b64`, `x25519_priv.b64`.
pub fn load_identity(dir: &std::path::Path) -> Option<Identity> {
    let (p, s, ep, es) = (read_trim(&dir.join("ed25519_pub.b64"))?, read_trim(&dir.join("ed25519_priv.b64"))?, read_trim(&dir.join("x25519_pub.b64"))?, read_trim(&dir.join("x25519_priv.b64"))?);
    match Identity::from_b64(&p, &s, &ep, &es) {
        Ok(i) => Some(i),
        Err(e) => { warn!("NOSLOP_HUB: identity in {} is unusable: {}", dir.display(), e); None }
    }
}

impl HubNode {
    /// Builds the node. `send_fn` is `None` in production (Tor via `cfg.socks`); `publish_onion`
    /// controls whether the creator onion is published through Tor's control port.
    pub async fn new(cfg: HubConfig, store: Store, send_fn: Option<SendFn>, publish_onion: bool, clock: Clock) -> Arc<Self> {
        let socks = cfg.socks.clone();
        let send_fn = send_fn.unwrap_or_else(|| {
            Arc::new(move |onion: String, line: String| {
                let socks = socks.clone();
                Box::pin(async move { tor::send_line(&socks, &onion, &line, Duration::from_secs(60)).await }) as BoxFut<bool>
            })
        });
        let onion = if publish_onion {
            Some(tor::CreatorOnion::start(cfg.tor_control.clone(), cfg.tor_control_password.clone(), cfg.creator_listen.port()))
        } else { None };
        let profile = store.meta_get("creator_profile").await.ok().flatten()
            .and_then(|s| crate::json::parse(&s).ok()).map(|v| CreatorProfile::from_json(&v)).unwrap_or_default();
        let hosting = store.meta_get("creator_hosting").await.ok().flatten().as_deref() == Some("on");
        let node = Arc::new(HubNode {
            personal: RwLock::new(load_identity(&cfg.identity_dir)),
            creator: RwLock::new(load_identity(&cfg.creator_dir())),
            profile: RwLock::new(profile),
            hosting: RwLock::new(hosting),
            replay: Mutex::new(ReplayGuard::new(20_000)),
            limiter: Mutex::new(Limiter::default()),
            send_slots: Arc::new(Semaphore::new(cfg.max_sends.max(1))),
            send_fn,
            onion,
            admin_hook: RwLock::new(None),
            dm_hook: RwLock::new(None),
            dm_remove_hook: RwLock::new(None),
            clock,
            cfg,
            store,
        });
        node.apply_hosting().await;
        node
    }

    pub fn now(&self) -> i64 { (self.clock)() }
    pub async fn personal(&self) -> Option<Identity> { self.personal.read().await.clone() }
    pub async fn creator(&self) -> Option<Identity> { self.creator.read().await.clone() }
    pub async fn profile(&self) -> CreatorProfile { self.profile.read().await.clone() }
    pub async fn hosting(&self) -> bool { *self.hosting.read().await }
    pub async fn set_admin_hook(&self, hook: AdminHook) { *self.admin_hook.write().await = Some(hook); }
    /// NOSLOP_HUB_DM_VIEW_V1: where the owner's DMs the Hub can open go (the Web UI's `dms` table).
    pub async fn set_dm_hook(&self, hook: dms::DmHook) { *self.dm_hook.write().await = Some(hook); }
    /// NOSLOP_HUB_DM_DELETE_V1: how deletes and cleared conversations reach the Web UI's `dms` table.
    pub async fn set_dm_remove_hook(&self, hook: dms::DmRemoveHook) { *self.dm_remove_hook.write().await = Some(hook); }

    /// Whether the creator duties run on this Hub: hosting switched on and a creator identity present.
    pub async fn creator_active(&self) -> bool { self.hosting().await && self.creator().await.is_some() }

    /// Starts or stops publishing the creator onion to match the hosting switch.
    async fn apply_hosting(&self) {
        let seed = if self.creator_active().await { self.creator().await.map(|c| c.seed()) } else { None };
        if let Some(o) = &self.onion { o.set(seed); }
    }

    pub async fn onion_status(&self) -> tor::OnionStatus {
        match &self.onion { Some(o) => o.status.lock().await.clone(), None => tor::OnionStatus { published: false, service_id: None, error: Some("onion publishing disabled".into()) } }
    }

    pub async fn reload_personal(&self) { *self.personal.write().await = load_identity(&self.cfg.identity_dir); }

    pub(crate) async fn set_creator(&self, id: Option<Identity>) {
        *self.creator.write().await = id;
        self.apply_hosting().await;
    }
    pub(crate) async fn set_profile(&self, p: CreatorProfile) -> Result<(), String> {
        self.store.meta_set("creator_profile", &p.to_json().render()).await.map_err(|e| e.to_string())?;
        *self.profile.write().await = p;
        Ok(())
    }
    pub(crate) async fn set_hosting(&self, on: bool) -> Result<(), String> {
        self.store.meta_set("creator_hosting", if on { "on" } else { "off" }).await.map_err(|e| e.to_string())?;
        *self.hosting.write().await = on;
        self.apply_hosting().await;
        Ok(())
    }

    async fn own_keys(&self) -> (Option<String>, Option<String>) {
        (self.personal().await.map(|i| i.public_key_b64), self.creator().await.map(|i| i.public_key_b64))
    }

    // --- Ingress ------------------------------------------------------------------------------

    /// Accepts mesh connections on `addr` forever.
    pub async fn serve(self: Arc<Self>, addr: SocketAddr, which: Listener) -> std::io::Result<()> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        info!("🕸️  NOSLOP_HUB: {:?} mesh listener on {}", which, addr);
        let slots = Arc::new(Semaphore::new(self.cfg.max_connections.max(1)));
        loop {
            let (stream, peer) = match listener.accept().await { Ok(x) => x, Err(e) => { warn!("NOSLOP_HUB: accept failed: {}", e); continue; } };
            let Ok(permit) = slots.clone().try_acquire_owned() else {
                debug!("NOSLOP_HUB: connection limit reached; dropping {}", peer);
                continue;
            };
            let node = self.clone();
            tokio::spawn(async move {
                node.handle_connection(stream, which).await;
                drop(permit);
            });
        }
    }

    async fn handle_connection(self: Arc<Self>, stream: tokio::net::TcpStream, which: Listener) {
        let (rd, mut wr) = stream.into_split();
        let mut reader = BufReader::new(rd);
        for _ in 0..FRAMES_PER_CONNECTION {
            let mut buf = Vec::new();
            let read = tokio::time::timeout(IDLE_TIMEOUT, read_frame(&mut reader, &mut buf)).await;
            match read {
                Ok(Ok(true)) => {}
                Ok(Ok(false)) => return, // EOF
                Ok(Err(e)) => { debug!("NOSLOP_HUB: dropping connection: {}", e); return; }
                Err(_) => return, // idle
            }
            let Ok(line) = String::from_utf8(buf) else { debug!("NOSLOP_HUB: frame is not UTF-8"); continue };
            let line = line.trim();
            if line.is_empty() { continue; }
            for reply in self.handle_frame(line, which).await {
                if wr.write_all(format!("{}\n", reply).as_bytes()).await.is_err() { return; }
                let _ = wr.flush().await;
            }
        }
    }

    /// Processes one received frame. Returns frames to write back on the same connection (only the
    /// handshake reply path uses this, WIRE_PROTOCOL_REFERENCE §3.1).
    pub async fn handle_frame(self: &Arc<Self>, line: &str, which: Listener) -> Vec<String> {
        let pkt = match Packet::from_wire(line) { Ok(p) => p, Err(e) => { debug!("NOSLOP_HUB: unreadable frame: {}", e); return vec![] } };
        let now = self.now();
        match envelope::verify(&pkt, now) {
            Verdict::Valid => {}
            v => { warn!("NOSLOP_HUB: dropped {} from {}: envelope {:?}", pkt.ptype, tor::short(&pkt.sender_id), v); return vec![]; }
        }
        let (my_pub, my_creator) = self.own_keys().await;
        if Some(&pkt.sender_id) == my_pub.as_ref() || Some(&pkt.sender_id) == my_creator.as_ref() {
            debug!("NOSLOP_HUB: dropped a looped-back {} from one of our own identities", pkt.ptype);
            return vec![];
        }
        if !self.replay.lock().await.admit(&pkt, now) {
            debug!("NOSLOP_HUB: dropped a replayed {} {}", pkt.ptype, pkt.id.as_deref().unwrap_or(""));
            return vec![];
        }
        if !self.frame_allowed(&pkt.sender_id, now).await {
            warn!("NOSLOP_HUB: rate limit: dropped {} from {}", pkt.ptype, tor::short(&pkt.sender_id));
            return vec![];
        }
        // Which of our identities is addressed: the named target when it is ours, else the onion it came to.
        let addressed = match pkt.target_user_id.as_deref() {
            Some(t) if Some(t) == my_creator.as_deref() => Listener::Creator,
            Some(t) if Some(t) == my_pub.as_deref() => Listener::Personal,
            Some(_) if pkt.ptype.starts_with("MEDIA_") => {
                debug!("NOSLOP_HUB: dropped {} addressed to someone else (media is point-to-point)", pkt.ptype);
                return vec![];
            }
            _ => which,
        };
        // NOSLOP_HUB_DM_VIEW_V1: a DM for one of the owner's identities is also opened for the Web UI (the
        // packet itself still goes to the phone unchanged).
        if pkt.ptype == "MESSAGE" {
            let (node, p) = (self.clone(), pkt.clone());
            tokio::spawn(async move { dms::observe(&node, &p, true).await });
        } else if pkt.ptype == "DELETE_MESSAGE" {
            // NOSLOP_HUB_DM_DELETE_V1: a contact deleting their own message (it still goes to the phone).
            let (node, p) = (self.clone(), pkt.clone());
            tokio::spawn(async move { dms::observe_delete(&node, &p, true).await });
        }
        if addressed == Listener::Creator && self.creator_active().await {
            let out = creator::handle(self, &pkt).await;
            if let Some(kind) = out.inbox {
                self.inbox(kind, &pkt, line).await;
            }
            return out.replies;
        }
        self.inbox("packet", &pkt, line).await;
        vec![]
    }

    async fn frame_allowed(&self, sender: &str, now: i64) -> bool {
        let mut l = self.limiter.lock().await;
        if l.frames.len() > 50_000 { l.frames.retain(|_, (start, _)| now - *start < 60_000); }
        let e = l.frames.entry(sender.to_string()).or_insert((now, 0));
        if now - e.0 >= 60_000 { *e = (now, 0); }
        e.1 += 1;
        e.1 <= FRAMES_PER_MINUTE
    }

    /// Stores `line` for the phone. `kind`: `packet` (process normally) or a Hub notice kind.
    pub async fn inbox(&self, kind: &str, pkt: &Packet, line: &str) {
        match self.store.inbox_push(kind, &pkt.ptype, &pkt.sender_id, line, self.now(), &self.cfg.inbox).await {
            Ok(true) => debug!("NOSLOP_HUB: inbox <- {} {} from {}", kind, pkt.ptype, tor::short(&pkt.sender_id)),
            Ok(false) => warn!("NOSLOP_HUB: inbox full; dropped {} from {}", pkt.ptype, tor::short(&pkt.sender_id)),
            Err(e) => warn!("NOSLOP_HUB: inbox write failed: {}", e),
        }
    }

    /// `HandshakePacketHandler.claimReplySlot`: a request timestamp is answered on its connection at
    /// most once per sender, and only when strictly newer than the last one answered.
    pub(crate) async fn claim_reply_slot(&self, sender: &str, ts: i64) -> bool {
        let mut l = self.limiter.lock().await;
        if l.reply_slots.len() > 20_000 { l.reply_slots.clear(); }
        match l.reply_slots.get(sender) {
            Some(last) if *last >= ts => false,
            _ => { l.reply_slots.insert(sender.to_string(), ts); true }
        }
    }

    /// At most 5 automatic follower accepts per sender per hour (`autoAcceptRateLimits`).
    pub(crate) async fn auto_accept_allowed(&self, sender: &str, now: i64) -> bool {
        let mut l = self.limiter.lock().await;
        let v = l.auto_accept.entry(sender.to_string()).or_default();
        v.retain(|t| now - *t <= 3_600_000);
        if v.len() < creator::AUTO_ACCEPTS_PER_HOUR_PUB { v.push(now); true } else { false }
    }

    /// `MediaManager.seenStrangerRequests`: a signed request from a stranger is answered once (LRU of 4096).
    pub(crate) async fn remember_stranger_request(&self, sig: &str) -> bool {
        let mut l = self.limiter.lock().await;
        if l.stranger_media_sigs.iter().any(|s| s == sig) { return false; }
        l.stranger_media_sigs.push_back(sig.to_string());
        while l.stranger_media_sigs.len() > 4096 { l.stranger_media_sigs.pop_front(); }
        true
    }

    /// `MediaManager` outbound limit: 16 MB per requester per 60 s; a request over it waits for the
    /// next window, at most three times, before it is dropped.
    pub(crate) async fn admit_media_bytes(&self, sender: &str, bytes: i64) -> bool {
        for _ in 0..3 {
            let wait = {
                let mut l = self.limiter.lock().await;
                let now = self.now();
                if now - l.media_window_start > creator::MEDIA_WINDOW_MS_PUB { l.media_window_start = now; l.media_out.clear(); }
                let sent = *l.media_out.get(sender).unwrap_or(&0);
                if sent + bytes <= creator::MEDIA_BYTES_PER_WINDOW_PUB {
                    l.media_out.insert(sender.to_string(), sent + bytes);
                    return true;
                }
                (l.media_window_start + creator::MEDIA_WINDOW_MS_PUB - now).max(1)
            };
            tokio::time::sleep(Duration::from_millis(wait as u64)).await;
        }
        false
    }

    /// Deletes a media file once no stored creator post uses it.
    pub(crate) async fn drop_unused_media(&self, media_id: &str) {
        if self.store.media_in_use(media_id).await.unwrap_or(true) { return; }
        let _ = tokio::fs::remove_file(self.cfg.media_dir().join(media_id)).await;
        let _ = tokio::fs::remove_file(self.cfg.media_dir().join(format!("{}.part", media_id))).await;
        let _ = self.store.media_delete(media_id).await;
    }

    // --- Delivery -----------------------------------------------------------------------------

    /// Sends `pkt` to `onion` in the background with the given retry policy. A packet sent as one
    /// of our identities is signed again at every attempt; anyone else's packet must already carry an
    /// envelope and is sent as it is.
    pub fn deliver(self: &Arc<Self>, onion: String, pkt: Packet, retry: Retry) {
        let node = self.clone();
        tokio::spawn(async move { node.deliver_now(&onion, pkt, retry).await; });
    }

    pub async fn deliver_now(self: &Arc<Self>, onion: &str, pkt: Packet, retry: Retry) -> bool {
        if !tor::is_valid_onion(onion) { warn!("NOSLOP_HUB: not sending {}: invalid onion", pkt.ptype); return false; }
        let onion = onion.trim().to_ascii_lowercase();
        let own_onions: Vec<String> = [self.personal().await.map(|i| i.onion), self.creator().await.map(|i| i.onion)].into_iter().flatten().collect();
        if own_onions.contains(&onion) { debug!("NOSLOP_HUB: not sending {} to our own onion", pkt.ptype); return true; }
        let mut schedule: Vec<u64> = match retry { Retry::Once => vec![0], _ => vec![0, 5, 10] };
        match retry { Retry::Spool => schedule.extend(std::iter::repeat(60).take(12)), Retry::Sync => schedule.extend([30, 90]), Retry::Once => {} }
        for (i, wait) in schedule.iter().enumerate() {
            if *wait > 0 { tokio::time::sleep(Duration::from_secs(*wait)).await; }
            let Some(line) = self.wire_for(&pkt).await else { warn!("NOSLOP_HUB: not sending {}: no envelope and not one of our identities", pkt.ptype); return false; };
            let ok = {
                let _permit = self.send_slots.acquire().await;
                (self.send_fn)(onion.clone(), line).await
            };
            self.record_send(&onion, ok).await;
            if ok {
                if i > 0 { info!("NOSLOP_HUB: {} to {} delivered on attempt {}", pkt.ptype, tor::short(&onion), i + 1); }
                return true;
            }
        }
        warn!("NOSLOP_HUB: {} to {} not delivered after {} attempts", pkt.ptype, tor::short(&onion), schedule.len());
        false
    }

    /// The wire form of `pkt` for one attempt.
    async fn wire_for(&self, pkt: &Packet) -> Option<String> {
        let now = self.now();
        for id in [self.personal().await, self.creator().await].into_iter().flatten() {
            if id.public_key_b64 == pkt.sender_id {
                return envelope::sign(pkt, &id, now).ok().map(|p| p.to_wire());
            }
        }
        if envelope::has_envelope(pkt) { Some(pkt.to_wire()) } else { None }
    }

    async fn record_send(&self, onion: &str, ok: bool) {
        let mut l = self.limiter.lock().await;
        if ok { l.onion_failures.remove(onion); } else {
            let now = self.now();
            let e = l.onion_failures.entry(onion.to_string()).or_insert((0, now));
            e.0 += 1;
            e.1 = now;
        }
    }

    /// Presence is not sent to an onion that failed 3 times in a row in the last 10 minutes
    /// (`GossipService.isPeerInCooldown` keeps presence off unreachable peers the same way).
    async fn in_cooldown(&self, onion: &str) -> bool {
        let l = self.limiter.lock().await;
        l.onion_failures.get(onion).map(|(n, at)| *n >= 3 && self.now() - at < 10 * 60_000).unwrap_or(false)
    }

    /// Runs the creator presence heartbeat forever (every 60 s while creator hosting is active).
    pub async fn run_presence(self: Arc<Self>) {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            if self.creator_active().await {
                creator::announce_presence(&self).await;
            }
        }
    }

    /// Starts the listeners and the presence loop.
    pub fn spawn_all(self: &Arc<Self>) {
        let (n1, n2, n3) = (self.clone(), self.clone(), self.clone());
        let (a1, a2) = (self.cfg.personal_listen, self.cfg.creator_listen);
        tokio::spawn(async move { if let Err(e) = n1.serve(a1, Listener::Personal).await { warn!("NOSLOP_HUB: personal mesh listener failed on {}: {}", a1, e); } });
        tokio::spawn(async move { if let Err(e) = n2.serve(a2, Listener::Creator).await { warn!("NOSLOP_HUB: creator mesh listener failed on {}: {}", a2, e); } });
        tokio::spawn(async move { n3.run_presence().await });
    }
}

/// Reads one `\n`-terminated frame into `buf` (without the newline and any `\r`). Ok(false) at a
/// clean EOF. A frame over [`MAX_FRAME_BYTES`] is an error (the connection is dropped).
async fn read_frame<R: tokio::io::AsyncBufRead + Unpin>(r: &mut R, buf: &mut Vec<u8>) -> std::io::Result<bool> {
    loop {
        let available = r.fill_buf().await?;
        if available.is_empty() { return Ok(!buf.is_empty()); }
        if let Some(pos) = available.iter().position(|b| *b == b'\n') {
            buf.extend(available[..pos].iter().filter(|b| **b != b'\r'));
            r.consume(pos + 1);
            if buf.len() > MAX_FRAME_BYTES { return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too large")); }
            return Ok(true);
        }
        let n = available.len();
        buf.extend(available.iter().filter(|b| **b != b'\r'));
        r.consume(n);
        if buf.len() > MAX_FRAME_BYTES { return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too large")); }
    }
}

#[cfg(test)]
mod tests;
