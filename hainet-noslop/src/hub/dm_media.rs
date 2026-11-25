//! START OF FILE hainet-noslop/src/hub/dm_media.rs
//!
//! NOSLOP_HUB_DM_MEDIA_V1 — attachments of the owner's one-to-one DMs, for the Web UI chat.
//!
//! A DM carries only the attachment's description (`MediaMetadata`: id, type, mime type, size, SHA-256); the
//! file stays with whoever sent it, and the recipient's phone fetches it with `MEDIA_REQUEST` / `MEDIA_CHUNK`.
//! The Web UI could therefore only say "open this conversation on your phone". Now:
//!
//! * **Incoming** (a contact sent it): when the Hub opens such a DM ([`super::dms::observe`]) it records the
//!   attachment (`nh_dm_media`, state `fetch`) and fetches it itself, exactly as the phone's downloader does:
//!   `MEDIA_REQUEST`s with a byte range of at most 256 KiB (`MediaManager.MAX_CHUNK_BYTES`), signed
//!   (`noslop-media-req-v1`) and sent as the owner's identity the DM was addressed to, to the contact's onion.
//!   The contact's phone allows it (`isAuthorizedByOwnerIndex`: the requester is the DM's other party) and
//!   answers to that identity's onion, which is this Hub's. A `MEDIA_CHUNK` that belongs to a fetch in progress
//!   (same media id, from the holder, a range we asked for) is consumed here and does not go to the phone's
//!   inbox; any other chunk goes on to the phone as before. The file must match the declared size and, when
//!   the sender gave one, its SHA-256. A contact that is offline is asked again later (back-off up to an
//!   hour, for three days).
//! * **Outgoing** (the owner sent it): the file is on the owner's phone, which uploads it (`dm_media_put`,
//!   owner-signed, the same resumable chunks as `creator_media_put`) when it mirrors its DMs to the Hub.
//!
//! Files live in `<noslop_dir>/dm_media`, within `HAINET_NOSLOP_DM_MEDIA_QUOTA` (default 5 GB); a single
//! attachment over `HAINET_NOSLOP_DM_MEDIA_MAX` (default 100 MB) is not kept (`too_large`). The Web UI reads a
//! complete file through `GET /api/noslop/dm_media/<id>` (session cookie), and its state with
//! `hub_dm_media_info`. A deleted DM or a cleared conversation takes its attachments with it.

use super::{store::DmMediaRow, tor, HubNode, Retry};
use crate::{canon, json::JValue, keys, packet::Packet};
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Largest range asked for at once (`MediaManager.MAX_CHUNK_BYTES`: a phone refuses anything longer).
pub const CHUNK_BYTES: i64 = 256 * 1024;
/// Ranges in flight at once.
const WINDOW: usize = 4;
/// How long a fetch waits for any chunk before it asks again for every range in flight.
const CHUNK_TIMEOUT: Duration = Duration::from_secs(90);
/// Rounds without a single chunk before an attempt is given up (the holder is offline).
const STALLS_PER_ATTEMPT: u32 = 3;
/// Back-off between attempts (ms); the last value repeats.
const BACKOFF_MS: [i64; 6] = [60_000, 5 * 60_000, 15 * 60_000, 30 * 60_000, 60 * 60_000, 60 * 60_000];
/// An attachment is fetched for at most this long after its DM (ms).
const GIVE_UP_AFTER_MS: i64 = 3 * 24 * 60 * 60_000;
/// Attachments fetched at the same time.
const PARALLEL_FETCHES: i64 = 2;
/// Largest chunk the owner's phone uploads at once (`dm_media_put`).
pub const PUT_MAX: usize = 1024 * 1024;

fn env_or(key: &str, default: i64) -> i64 { std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default) }
/// Largest attachment kept (`HAINET_NOSLOP_DM_MEDIA_MAX`).
pub fn max_bytes() -> i64 { env_or("HAINET_NOSLOP_DM_MEDIA_MAX", 100 * 1024 * 1024) }
/// All attachments together (`HAINET_NOSLOP_DM_MEDIA_QUOTA`).
pub fn quota() -> i64 { env_or("HAINET_NOSLOP_DM_MEDIA_QUOTA", 5i64 * 1024 * 1024 * 1024) }

pub fn dir(node: &HubNode) -> PathBuf { node.cfg.noslop_dir.join("dm_media") }

/// A fetch in progress: chunks from `holder` for this media id go to `tx` as (offset, bytes).
pub struct ActiveFetch {
    holder: String,
    tx: mpsc::UnboundedSender<(i64, Vec<u8>)>,
}

/// The attachment a DM body describes (`{"media": {...}}`), when it is a file the Hub can keep: not a picker GIF
/// (`noslop-gif://…`, a link the Web UI loads itself) and with a valid id and a size.
pub struct Described {
    pub media_id: String,
    pub mtype: String,
    pub mime_type: String,
    pub size: i64,
    pub sha256: String,
    pub access_key: String,
}

pub fn describe(media: &JValue) -> Option<Described> {
    let id = media.get_str("id")?;
    if id.starts_with("noslop-gif://") || !super::creator::is_valid_media_id(id) { return None; }
    let size = media.get_i64("size").filter(|s| *s > 0)?;
    let sha = media.get_str("sha256").unwrap_or("").trim().to_ascii_lowercase();
    let sha = if sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit()) { sha } else { String::new() };
    Some(Described {
        media_id: id.to_string(),
        mtype: media.get_str("type").unwrap_or("file").to_string(),
        mime_type: media.get_str("mime_type").filter(|m| !m.trim().is_empty() && m.len() <= 128).unwrap_or("application/octet-stream").to_string(),
        size,
        sha256: sha,
        access_key: media.get_str("access_key").unwrap_or("").to_string(),
    })
}

/// Records the attachment of an opened DM. `me`: the owner's identity in the conversation; `incoming`: the
/// contact wrote it (the Hub fetches it) or the owner did (the phone uploads it).
pub async fn register(node: &Arc<HubNode>, d: Described, dm_id: &str, dm_ts: i64, peer: &str, me: &str, incoming: bool) {
    let too_large = d.size > max_bytes();
    let row = DmMediaRow {
        media_id: d.media_id, dm_id: dm_id.to_string(), dm_ts, peer: peer.to_string(),
        holder: if incoming { peer.to_string() } else { me.to_string() }, as_key: me.to_string(),
        mtype: d.mtype, mime_type: d.mime_type, size: d.size, sha256: d.sha256, access_key: d.access_key,
        received: 0,
        state: if too_large { "too_large" } else if incoming { "fetch" } else { "upload" }.to_string(),
        attempts: 0, next_at: 0,
        error: if too_large { format!("larger than this Hub keeps ({} MB)", max_bytes() / (1024 * 1024)) } else { String::new() },
    };
    match node.store.dm_media_add(&row, node.now()).await {
        Ok(true) => {
            debug!("NOSLOP_HUB: DM attachment {} recorded ({})", row.media_id, row.state);
            if row.state == "fetch" { kick(node); }
        }
        Ok(false) => {}
        Err(e) => warn!("NOSLOP_HUB: DM attachment {} not recorded: {}", row.media_id, e),
    }
}

/// A `MEDIA_CHUNK` that answers a fetch in progress: handed to it, and true (it does not go to the phone).
pub async fn on_chunk(node: &Arc<HubNode>, pkt: &Packet) -> bool {
    if pkt.ptype != "MEDIA_CHUNK" { return false; }
    let Some(p) = pkt.payload.as_ref() else { return false };
    let Some(media_id) = p.get_str("media_id") else { return false };
    let fetches = node.dm_fetches.lock().await;
    let Some(f) = fetches.get(media_id) else { return false };
    if f.holder != pkt.sender_id { return false; }
    let (Some(offset), Some(data)) = (p.get_i64("byte_offset"), p.get_str("data")) else { return false };
    let Ok(bytes) = keys::b64_decode(data) else { return false };
    f.tx.send((offset, bytes)).is_ok()
}

/// Starts due fetches now (also run every 30 s by [`run`]).
pub fn kick(node: &Arc<HubNode>) {
    let node = node.clone();
    tokio::spawn(async move { start_due(&node).await });
}

async fn start_due(node: &Arc<HubNode>) {
    let running = node.dm_fetches.lock().await.len() as i64;
    if running >= PARALLEL_FETCHES { return; }
    let due = match node.store.dm_media_due(node.now(), PARALLEL_FETCHES * 4).await {
        Ok(d) => d,
        Err(e) => { warn!("NOSLOP_HUB: DM attachments not read: {}", e); return; }
    };
    let mut started = running;
    for row in due {
        if started >= PARALLEL_FETCHES { break; }
        {
            let mut fetches = node.dm_fetches.lock().await;
            if fetches.contains_key(&row.media_id) { continue; }
            let (tx, rx) = mpsc::unbounded_channel();
            fetches.insert(row.media_id.clone(), ActiveFetch { holder: row.holder.clone(), tx });
            let node2 = node.clone();
            tokio::spawn(async move {
                let id = row.media_id.clone();
                attempt(&node2, row, rx).await;
                node2.dm_fetches.lock().await.remove(&id);
            });
        }
        started += 1;
    }
}

/// Fetches DM attachments forever (every 30 s, and at once when [`kick`]ed).
pub async fn run(node: Arc<HubNode>) {
    loop {
        start_due(&node).await;
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

async fn identity_for(node: &HubNode, key: &str) -> Option<crate::keys::Identity> {
    [node.personal().await, node.creator().await].into_iter().flatten().find(|i| i.public_key_b64 == key)
}

/// One attempt at fetching `row`; records the outcome.
async fn attempt(node: &Arc<HubNode>, mut row: DmMediaRow, rx: mpsc::UnboundedReceiver<(i64, Vec<u8>)>) {
    let result = fetch(node, &mut row, rx).await;
    let now = node.now();
    match result {
        Ok(()) => {
            row.state = "complete".into();
            row.received = row.size;
            row.error.clear();
            info!("NOSLOP_HUB: DM attachment {} fetched from {} ({} bytes)", row.media_id, tor::short(&row.holder), row.size);
        }
        Err(Failure::Final(e)) => {
            row.state = "failed".into();
            row.error = e;
            warn!("NOSLOP_HUB: DM attachment {} not fetched: {}", row.media_id, row.error);
        }
        Err(Failure::Retry(e)) => {
            row.attempts += 1;
            if now - row.dm_ts > GIVE_UP_AFTER_MS {
                row.state = "failed".into();
                row.error = format!("{} (gave up after {} attempts)", e, row.attempts);
                warn!("NOSLOP_HUB: DM attachment {}: {}", row.media_id, row.error);
            } else {
                let wait = BACKOFF_MS[(row.attempts as usize - 1).min(BACKOFF_MS.len() - 1)];
                row.next_at = now + wait;
                row.error = e;
                info!("NOSLOP_HUB: DM attachment {} not fetched yet ({}); trying again in {} min", row.media_id, row.error, wait / 60_000);
            }
        }
    }
    // The DM may have been deleted meanwhile: only a row that still exists is updated.
    match node.store.dm_media_get(&row.media_id).await {
        Ok(Some(_)) => { if let Err(e) = node.store.dm_media_update(&row, now).await { warn!("NOSLOP_HUB: DM attachment {} state not saved: {}", row.media_id, e); } }
        _ => remove_files(node, &[row.media_id.clone()]).await,
    }
}

enum Failure { Retry(String), Final(String) }

async fn fetch(node: &Arc<HubNode>, row: &mut DmMediaRow, mut rx: mpsc::UnboundedReceiver<(i64, Vec<u8>)>) -> Result<(), Failure> {
    use tokio::io::{AsyncSeekExt, AsyncWriteExt};
    let me = identity_for(node, &row.as_key).await.ok_or_else(|| Failure::Retry("the identity it was sent to is not on this Hub".into()))?;
    let contact = node.store.contact_get(&row.holder).await.map_err(|e| Failure::Retry(e.to_string()))?
        .ok_or_else(|| Failure::Final("the sender is not one of your contacts".into()))?;
    if contact.relationship == "BLOCKED" { return Err(Failure::Final("the sender is blocked".into())); }
    if !tor::is_valid_onion(&contact.onion_address) { return Err(Failure::Retry("no onion address for the sender".into())); }
    let used = node.store.dm_media_total_bytes().await.map_err(|e| Failure::Retry(e.to_string()))? - row.received;
    if used + row.size > quota() { return Err(Failure::Retry("the Hub's DM attachment space is full".into())); }

    let dir = dir(node);
    tokio::fs::create_dir_all(&dir).await.map_err(|e| Failure::Retry(e.to_string()))?;
    let part = dir.join(format!("{}.part", row.media_id));
    let mut file = tokio::fs::OpenOptions::new().create(true).write(true).truncate(true).open(&part).await.map_err(|e| Failure::Retry(e.to_string()))?;
    row.received = 0;

    let size = row.size;
    let expected = |off: i64| CHUNK_BYTES.min(size - off);
    let mut pending: VecDeque<i64> = (0..size).step_by(CHUNK_BYTES as usize).collect();
    let total = pending.len();
    let mut inflight: HashSet<i64> = HashSet::new();
    let mut done: HashSet<i64> = HashSet::new();
    let mut stalls = 0u32;
    let mut last_saved = 0i64;
    while done.len() < total {
        while inflight.len() < WINDOW {
            let Some(off) = pending.pop_front() else { break };
            request(node, &me, &row.holder, &contact.onion_address, row, off, expected(off));
            inflight.insert(off);
        }
        match tokio::time::timeout(CHUNK_TIMEOUT, rx.recv()).await {
            Ok(Some((off, data))) => {
                if !inflight.remove(&off) { continue; } // not asked for, or already written
                if data.len() as i64 != expected(off) {
                    // A short answer (the holder's file differs): ask once more for this range.
                    debug!("NOSLOP_HUB: DM attachment {}@{}: {} bytes instead of {}", row.media_id, off, data.len(), expected(off));
                    if data.is_empty() { return Err(Failure::Final("the sender no longer has this file".into())); }
                    pending.push_back(off);
                    continue;
                }
                file.seek(std::io::SeekFrom::Start(off as u64)).await.map_err(|e| Failure::Retry(e.to_string()))?;
                file.write_all(&data).await.map_err(|e| Failure::Retry(e.to_string()))?;
                done.insert(off);
                row.received += data.len() as i64;
                stalls = 0;
                if row.received - last_saved >= 4 * 1024 * 1024 {
                    last_saved = row.received;
                    let _ = node.store.dm_media_update(row, node.now()).await;
                }
            }
            Ok(None) => return Err(Failure::Retry("fetch stopped".into())),
            Err(_) => {
                stalls += 1;
                if stalls >= STALLS_PER_ATTEMPT {
                    let _ = tokio::fs::remove_file(&part).await;
                    return Err(Failure::Retry("no answer from the sender (offline?)".into()));
                }
                // Ask again for every range still in flight.
                let mut again: Vec<i64> = inflight.drain().collect();
                again.sort_unstable();
                for off in again.into_iter().rev() { pending.push_front(off); }
            }
        }
    }
    file.flush().await.map_err(|e| Failure::Retry(e.to_string()))?;
    drop(file);
    if !row.sha256.is_empty() {
        let bytes = tokio::fs::read(&part).await.map_err(|e| Failure::Retry(e.to_string()))?;
        if crate::envelope::sha256_hex(&bytes) != row.sha256 {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(Failure::Final("the received file does not match its SHA-256".into()));
        }
    }
    tokio::fs::rename(&part, dir.join(&row.media_id)).await.map_err(|e| Failure::Retry(e.to_string()))?;
    Ok(())
}

/// One `MEDIA_REQUEST` for `len` bytes at `off`, as `MediaManager` sends it (byte range, signed, timestamped).
fn request(node: &Arc<HubNode>, me: &crate::keys::Identity, holder: &str, onion: &str, row: &DmMediaRow, off: i64, len: i64) {
    let ts = node.now();
    let chunk_index = off / (128 * 1024); // MediaManager.MIN_CHUNK_SIZE
    let payload = JValue::obj()
        .with("media_id", JValue::str(&row.media_id))
        .with("chunk_index", JValue::int(chunk_index))
        .with("chunk_size", JValue::int(len))
        .with("byte_offset", JValue::int(off))
        .with("byte_length", JValue::int(len))
        .with_opt("access_key", Some(row.access_key.as_str()).filter(|k| !k.is_empty()).map(JValue::str))
        .with("origin_onion", JValue::str(&me.onion))
        .with("timestamp", JValue::int(ts));
    let mut pkt = Packet::new("MEDIA_REQUEST", &me.public_key_b64, Some(holder), payload, 1);
    pkt.signature = Some(me.sign(&canon::media_request(&me.public_key_b64, Some(holder), &row.media_id, chunk_index, len, Some(off), Some(len), Some(ts))));
    node.deliver(onion.to_string(), pkt, Retry::Once);
}

/// Deletes the files of these attachments (rows already gone).
pub async fn remove_files(node: &HubNode, ids: &[String]) {
    let dir = dir(node);
    for id in ids {
        if !super::creator::is_valid_media_id(id) { continue; }
        let _ = tokio::fs::remove_file(dir.join(id)).await;
        let _ = tokio::fs::remove_file(dir.join(format!("{}.part", id))).await;
    }
}

/// NOSLOP_HUB_DM_DELETE_V1 + MEDIA: a deleted DM's attachments go too.
pub async fn forget_dm(node: &HubNode, dm_id: &str) {
    match node.store.dm_media_remove_dm(dm_id).await {
        Ok(ids) => remove_files(node, &ids).await,
        Err(e) => warn!("NOSLOP_HUB: attachments of DM {} not removed: {}", dm_id, e),
    }
}

/// A cleared conversation's attachments go too.
pub async fn forget_conversation(node: &HubNode, peer: &str, before: i64) {
    match node.store.dm_media_remove_peer(peer, before).await {
        Ok(ids) => remove_files(node, &ids).await,
        Err(e) => warn!("NOSLOP_HUB: attachments of a cleared chat not removed: {}", e),
    }
}

/// A complete attachment's file, mime type and size (for `GET /api/noslop/dm_media/<id>`).
pub async fn file_for(node: &HubNode, media_id: &str) -> Option<(PathBuf, String, i64)> {
    if !super::creator::is_valid_media_id(media_id) { return None; }
    let row = node.store.dm_media_get(media_id).await.ok().flatten()?;
    if row.state != "complete" { return None; }
    let path = dir(node).join(media_id);
    let len = tokio::fs::metadata(&path).await.ok()?.len() as i64;
    Some((path, row.mime_type, len))
}

/// `hub_dm_media_info`: the state of each named attachment (unknown ids are left out).
pub async fn info(node: &HubNode, ids: &[String]) -> Result<Vec<JValue>, String> {
    let mut out = Vec::new();
    for id in ids.iter().take(200) {
        let Some(r) = node.store.dm_media_get(id).await.map_err(|e| e.to_string())? else { continue };
        out.push(JValue::obj()
            .with("media_id", JValue::str(&r.media_id))
            .with("state", JValue::str(&r.state))
            .with("type", JValue::str(&r.mtype))
            .with("mime_type", JValue::str(&r.mime_type))
            .with("size", JValue::int(r.size))
            .with("received", JValue::int(r.received))
            .with("incoming", JValue::Bool(r.holder == r.peer))
            .with("error", JValue::str(&r.error))
            .with("next_at", JValue::int(r.next_at)));
    }
    Ok(out)
}

/// `dm_media_put` (owner only): one chunk of an attachment the owner's phone holds, resumable like
/// `creator_media_put`. The first call for an id the Hub does not know yet records it from the arguments
/// (`peer`, `dm_id`, `dm_ts`, `as_key`, `incoming`, `type`, `mime_type`); an attachment the Hub already has is
/// acknowledged as complete, and one the Hub is fetching right now gets `busy`.
pub async fn put(node: &Arc<HubNode>, args: &JValue) -> Result<JValue, String> {
    use tokio::io::{AsyncSeekExt, AsyncWriteExt};
    let s = |k: &str| args.get_str(k).unwrap_or("").to_string();
    let media_id = s("media_id");
    if !super::creator::is_valid_media_id(&media_id) { return Err("invalid media id".into()); }
    let sha = s("sha256").to_ascii_lowercase();
    if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) { return Err("sha256 must be 64 hex characters".into()); }
    let size = args.get_i64("size").filter(|v| *v > 0).ok_or("missing size")?;
    let offset = args.get_i64("offset").filter(|v| *v >= 0).ok_or("missing offset")?;
    let data = keys::b64_decode(&s("data")).map_err(|_| "data is not Base64")?;
    if data.len() > PUT_MAX { return Err(format!("chunk larger than {} bytes", PUT_MAX)); }
    if size > max_bytes() { return Err("larger than this Hub keeps".into()); }
    let now = node.now();
    let mut row = match node.store.dm_media_get(&media_id).await.map_err(|e| e.to_string())? {
        Some(r) => r,
        None => {
            let peer = s("peer");
            if peer.is_empty() { return Err("missing peer".into()); }
            let me = s("as_key");
            let r = DmMediaRow {
                media_id: media_id.clone(), dm_id: s("dm_id"), dm_ts: args.get_i64("dm_ts").unwrap_or(now), peer: peer.clone(),
                // `incoming`: the phone downloaded a contact's attachment (the contact holds it); else it is the owner's own.
                holder: if args.get_bool("incoming") == Some(true) { peer.clone() } else if me.is_empty() { "owner".into() } else { me.clone() },
                as_key: me,
                mtype: args.get_str("type").unwrap_or("file").to_string(),
                mime_type: args.get_str("mime_type").filter(|m| !m.is_empty() && m.len() <= 128).unwrap_or("application/octet-stream").to_string(),
                size, sha256: sha.clone(), access_key: String::new(), received: 0, state: "upload".into(), attempts: 0, next_at: 0, error: String::new(),
            };
            node.store.dm_media_add(&r, now).await.map_err(|e| e.to_string())?;
            r
        }
    };
    if row.state == "complete" { return Ok(JValue::obj().with("status", JValue::str("ok")).with("received", JValue::int(row.size)).with("complete", JValue::Bool(true))); }
    if node.dm_fetches.lock().await.contains_key(&media_id) {
        return Ok(JValue::obj().with("status", JValue::str("busy")).with("received", JValue::int(0)).with("complete", JValue::Bool(false)));
    }
    if row.sha256 != sha || row.size != size { row.sha256 = sha.clone(); row.size = size; row.received = 0; }
    if offset != row.received { return Ok(JValue::obj().with("status", JValue::str("resume")).with("received", JValue::int(row.received)).with("complete", JValue::Bool(false))); }
    if offset + data.len() as i64 > size { return Err("chunk runs past the declared size".into()); }
    let used = node.store.dm_media_total_bytes().await.map_err(|e| e.to_string())? - row.received;
    if used + size > quota() { return Err("the Hub's DM attachment space is full".into()); }
    let dir = dir(node);
    tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
    let part = dir.join(format!("{}.part", media_id));
    let mut f = tokio::fs::OpenOptions::new().create(true).write(true).truncate(offset == 0).open(&part).await.map_err(|e| e.to_string())?;
    if offset > 0 {
        let len = f.metadata().await.map_err(|e| e.to_string())?.len() as i64;
        if len < offset {
            row.received = 0;
            node.store.dm_media_update(&row, now).await.map_err(|e| e.to_string())?;
            return Ok(JValue::obj().with("status", JValue::str("resume")).with("received", JValue::int(0)).with("complete", JValue::Bool(false)));
        }
        f.set_len(offset as u64).await.map_err(|e| e.to_string())?;
        f.seek(std::io::SeekFrom::Start(offset as u64)).await.map_err(|e| e.to_string())?;
    }
    f.write_all(&data).await.map_err(|e| e.to_string())?;
    f.flush().await.map_err(|e| e.to_string())?;
    drop(f);
    row.received = offset + data.len() as i64;
    let mut complete = false;
    if row.received == size {
        let bytes = tokio::fs::read(&part).await.map_err(|e| e.to_string())?;
        if crate::envelope::sha256_hex(&bytes) != sha {
            let _ = tokio::fs::remove_file(&part).await;
            row.received = 0;
            node.store.dm_media_update(&row, now).await.map_err(|e| e.to_string())?;
            return Err("the uploaded file does not match its SHA-256; upload it again".into());
        }
        tokio::fs::rename(&part, dir.join(&media_id)).await.map_err(|e| e.to_string())?;
        row.state = "complete".into();
        row.error.clear();
        complete = true;
        info!("NOSLOP_HUB: DM attachment {} uploaded by the owner's phone ({} bytes)", media_id, size);
    }
    node.store.dm_media_update(&row, now).await.map_err(|e| e.to_string())?;
    Ok(JValue::obj().with("status", JValue::str("ok")).with("received", JValue::int(row.received)).with("complete", JValue::Bool(complete)))
}

/// Ids of the map, for tests.
#[cfg(test)]
pub async fn active(node: &HubNode) -> Vec<String> { node.dm_fetches.lock().await.keys().cloned().collect::<Vec<_>>() }

// END OF FILE hainet-noslop/src/hub/dm_media.rs
