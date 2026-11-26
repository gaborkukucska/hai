//! START OF FILE hainet-noslop/src/hub/outbox.rs
//!
//! NOSLOP_HUB_OUTBOX_V1 — packets the Hub sends on its own (today: DMs written in the Web UI) are kept in
//! `nh_outbox` until they are delivered, like the phone's own outbox.
//!
//! Found 2026-11-26 (device test, the owner's phone off): DMs written in the Web UI never reached Gabby. Her phone
//! could not reach any onion that morning, and the Hub tried each DM for 12 minutes only (`Retry::Spool`, in
//! memory: a restart lost it too). Now a DM is tried at once; if that fails it stays in `nh_outbox` and is tried
//! again after 30 s, 2, 5, 10 and 30 minutes, then hourly, for seven days, and at once whenever the contact is
//! heard from (any verified frame from them, see
//! [`super::HubNode::handle_frame`]). A delivered packet is removed; deleting the DM or clearing the conversation
//! removes it too. The packet is stored without its envelope and signed again at every attempt.

use super::{store::OutboxItem, tor, HubNode, Retry};
use crate::packet::Packet;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

/// Back-off between rounds (ms) after the first one failed; the last value repeats.
const BACKOFF_MS: [i64; 6] = [30_000, 2 * 60_000, 5 * 60_000, 10 * 60_000, 30 * 60_000, 60 * 60_000];
/// A packet still not delivered after this long is dropped (ms).
pub const GIVE_UP_AFTER_MS: i64 = 7 * 24 * 60 * 60_000;
/// Packets tried per round of [`run`].
const PER_ROUND: i64 = 20;

/// Sends `pkt` to `onion` and keeps it until it is delivered. `key`: what removes it again (a DM's message id).
/// Returns at once; the work goes on in the background.
pub async fn send(node: &Arc<HubNode>, key: &str, peer: &str, onion: &str, pkt: Packet) -> Result<(), String> {
    let item = OutboxItem {
        id: key.to_string(), peer: peer.to_string(), onion: onion.to_string(), ptype: pkt.ptype.clone(),
        packet: pkt.to_wire(), created_at: node.now(), attempts: 0,
        // Not due for the worker while the first attempt runs.
        next_at: node.now() + BACKOFF_MS[0],
    };
    node.store.outbox_add(&item).await.map_err(|e| e.to_string())?;
    let node = node.clone();
    tokio::spawn(async move { attempt(&node, item).await; });
    Ok(())
}

/// One attempt; on failure the next round is scheduled.
async fn attempt(node: &Arc<HubNode>, item: OutboxItem) {
    let pkt = match Packet::from_wire(&item.packet) {
        Ok(p) => p,
        Err(e) => { warn!("NOSLOP_HUB: outbox {} unreadable, dropped: {}", item.id, e); let _ = node.store.outbox_remove(&item.id).await; return; }
    };
    if node.deliver_now(&item.onion, pkt, Retry::Once).await {
        if item.attempts > 0 { info!("NOSLOP_HUB: {} {} delivered to {} from the outbox (round {})", item.ptype, item.id, tor::short(&item.onion), item.attempts + 1); }
        let _ = node.store.outbox_remove(&item.id).await;
        return;
    }
    // Deleted meanwhile (the owner deleted the DM or cleared the chat): nothing to schedule.
    if !matches!(node.store.outbox_get(&item.id).await, Ok(Some(_))) { return; }
    let now = node.now();
    if now - item.created_at > GIVE_UP_AFTER_MS {
        warn!("NOSLOP_HUB: {} {} to {} not delivered in 7 days; dropped", item.ptype, item.id, tor::short(&item.onion));
        let _ = node.store.outbox_remove(&item.id).await;
        return;
    }
    let attempts = item.attempts + 1;
    let wait = BACKOFF_MS[((attempts - 1) as usize).min(BACKOFF_MS.len() - 1)];
    if let Err(e) = node.store.outbox_reschedule(&item.id, attempts, now + wait).await { warn!("NOSLOP_HUB: outbox {} not rescheduled: {}", item.id, e); }
    info!("NOSLOP_HUB: {} {} to {} waits in the outbox; next try in {} min", item.ptype, item.id, tor::short(&item.onion), wait / 60_000);
}

/// Tries what is due (every 30 s, and at once when [`kick`]ed).
async fn round(node: &Arc<HubNode>) {
    let due = match node.store.outbox_due(node.now(), PER_ROUND).await {
        Ok(d) => d,
        Err(e) => { warn!("NOSLOP_HUB: outbox not read: {}", e); return; }
    };
    for item in due {
        // Claimed for this round, so a second round does not send it twice meanwhile.
        let _ = node.store.outbox_reschedule(&item.id, item.attempts, node.now() + BACKOFF_MS[0]).await;
        let node = node.clone();
        tokio::spawn(async move { attempt(&node, item).await; });
    }
}

pub async fn run(node: Arc<HubNode>) {
    loop {
        round(&node).await;
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

/// The contact was just heard from: what waits for them is sent now.
pub fn wake(node: &Arc<HubNode>, peer: &str) {
    let node = node.clone();
    let peer = peer.to_string();
    tokio::spawn(async move {
        if matches!(node.store.outbox_wake_peer(&peer).await, Ok(n) if n > 0) { round(&node).await; }
    });
}

// END OF FILE hainet-noslop/src/hub/outbox.rs
