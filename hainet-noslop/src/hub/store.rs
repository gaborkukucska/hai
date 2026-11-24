//! START OF FILE hainet-noslop/src/hub/store.rs
//!
//! Durable state of the NoSlop Home HUB, in the Hub's existing SQLite database (`social.db`), in
//! tables prefixed `nh_` with an explicit schema version (no silently ignored `ALTER TABLE`s).
//!
//! * `nh_inbox`: verified packets (and Hub notices) waiting for the owner's phone. Items leave only
//!   when the phone acknowledges them (`hub_inbox_ack`), so a crash or a lost reply loses nothing.
//! * `nh_contacts`: the owner's contacts as the phone pushes them, plus followers the Hub accepted
//!   for the creator identity while the phone was away (`source = 'hub'`).
//! * `nh_creator_posts` / `nh_comments` / `nh_reactions`: the creator's own public posts, as signed
//!   `PostPayload` JSON, and the engagement on them, served to followers exactly as received.
//! * `nh_media`: public creator media held for followers (bytes on disk, see `HubPaths::media_dir`).
//! * `nh_meta`: key/value settings (creator profile, hosting switch, schema version).

use sqlx::{sqlite::SqlitePool, Row};

pub const SCHEMA_VERSION: i64 = 2;

#[derive(Clone)]
pub struct Store {
    pub pool: SqlitePool,
}

/// How many DM events are kept (the Web UI only needs the recent ones).
pub const DM_EVENTS_KEEP: i64 = 1000;

/// An incoming DM that reached the Hub (NOSLOP_HUB_DM_VIEW_V1).
#[derive(Debug, Clone, PartialEq)]
pub struct DmEvent {
    pub seq: i64,
    pub dm_id: String,
    pub peer: String,
    pub sender: String,
    pub preview: String,
    pub at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Contact {
    pub public_key: String,
    /// `"personal"` or `"creator"`: which of the owner's identities this contact knows.
    pub identity: String,
    /// `NONE`, `INCOMING_PENDING`, `OUTGOING_PENDING`, `ACCEPTED` or `BLOCKED` (`PeerRelationship`).
    pub relationship: String,
    pub is_trusted: bool,
    pub is_temporary: bool,
    pub handle: String,
    pub onion_address: String,
    pub enc_public_key: String,
    /// `"phone"` (pushed by the owner's phone) or `"hub"` (accepted by the Hub, not yet seen by the phone).
    pub source: String,
    /// The `request_nonce` of a request from this peer that the owner declined (`declined_request_<key>`),
    /// so a re-sent probe of that request is answered with `CONNECTION_REJECTED`, not auto-accepted.
    pub declined_nonce: String,
}

#[derive(Debug, Clone)]
pub struct InboxItem {
    pub seq: i64,
    pub kind: String,
    pub packet: String,
}

#[derive(Debug, Clone)]
pub struct StoredPost {
    pub id: String,
    pub author_id: String,
    pub timestamp: i64,
    pub content: String,
    pub media_id: Option<String>,
    pub payload: String,
}

#[derive(Debug, Clone)]
pub struct MediaRow {
    pub media_id: String,
    pub mtype: String,
    pub mime_type: String,
    pub size: i64,
    pub sha256: String,
    pub received: i64,
    pub complete: bool,
}

impl Store {
    pub async fn open(pool: SqlitePool) -> Result<Self, sqlx::Error> {
        let s = Store { pool };
        s.migrate().await?;
        Ok(s)
    }

    async fn migrate(&self) -> Result<(), sqlx::Error> {
        sqlx::query("CREATE TABLE IF NOT EXISTS nh_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)").execute(&self.pool).await?;
        let current: i64 = self.meta_get("schema_version").await?.and_then(|v| v.parse().ok()).unwrap_or(0);
        if current < 1 {
            let mut tx = self.pool.begin().await?;
            for stmt in [
                "CREATE TABLE IF NOT EXISTS nh_inbox (seq INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, ptype TEXT NOT NULL, sender TEXT NOT NULL, packet TEXT NOT NULL, bytes INTEGER NOT NULL, received_at INTEGER NOT NULL)",
                "CREATE INDEX IF NOT EXISTS nh_inbox_sender ON nh_inbox(sender)",
                "CREATE TABLE IF NOT EXISTS nh_contacts (public_key TEXT PRIMARY KEY, identity TEXT NOT NULL, relationship TEXT NOT NULL, is_trusted INTEGER NOT NULL, is_temporary INTEGER NOT NULL, handle TEXT NOT NULL, onion_address TEXT NOT NULL, enc_public_key TEXT NOT NULL, source TEXT NOT NULL, declined_nonce TEXT NOT NULL DEFAULT '', updated_at INTEGER NOT NULL)",
                "CREATE TABLE IF NOT EXISTS nh_creator_posts (id TEXT PRIMARY KEY, author_id TEXT NOT NULL, timestamp INTEGER NOT NULL, content TEXT NOT NULL, media_id TEXT, payload TEXT NOT NULL, updated_at INTEGER NOT NULL)",
                "CREATE TABLE IF NOT EXISTS nh_comments (id TEXT PRIMARY KEY, post_id TEXT NOT NULL, author_id TEXT NOT NULL, timestamp INTEGER NOT NULL, data TEXT NOT NULL)",
                "CREATE INDEX IF NOT EXISTS nh_comments_post ON nh_comments(post_id)",
                "CREATE TABLE IF NOT EXISTS nh_reactions (id TEXT PRIMARY KEY, post_id TEXT NOT NULL, author_id TEXT NOT NULL, timestamp INTEGER NOT NULL, data TEXT NOT NULL)",
                "CREATE INDEX IF NOT EXISTS nh_reactions_post ON nh_reactions(post_id)",
                "CREATE TABLE IF NOT EXISTS nh_media (media_id TEXT PRIMARY KEY, mtype TEXT NOT NULL, mime_type TEXT NOT NULL, size INTEGER NOT NULL, sha256 TEXT NOT NULL, received INTEGER NOT NULL, complete INTEGER NOT NULL, updated_at INTEGER NOT NULL)",
            ] {
                sqlx::query(stmt).execute(&mut *tx).await?;
            }
            sqlx::query("INSERT OR REPLACE INTO nh_meta (key, value) VALUES ('schema_version', '1')").execute(&mut *tx).await?;
            tx.commit().await?;
        }
        if current < 2 {
            // NOSLOP_HUB_DM_VIEW_V1: DMs from contacts that reached the Hub, for the Web UI's notifications.
            let mut tx = self.pool.begin().await?;
            sqlx::query("CREATE TABLE IF NOT EXISTS nh_dm_events (seq INTEGER PRIMARY KEY AUTOINCREMENT, dm_id TEXT NOT NULL UNIQUE, peer TEXT NOT NULL, sender TEXT NOT NULL, preview TEXT NOT NULL, at INTEGER NOT NULL)").execute(&mut *tx).await?;
            sqlx::query("INSERT OR REPLACE INTO nh_meta (key, value) VALUES ('schema_version', ?)").bind(SCHEMA_VERSION.to_string()).execute(&mut *tx).await?;
            tx.commit().await?;
        }
        Ok(())
    }

    // --- meta ---------------------------------------------------------------------------------

    pub async fn meta_get(&self, key: &str) -> Result<Option<String>, sqlx::Error> {
        Ok(sqlx::query("SELECT value FROM nh_meta WHERE key = ?").bind(key).fetch_optional(&self.pool).await?.map(|r| r.get::<String, _>(0)))
    }
    pub async fn meta_set(&self, key: &str, value: &str) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT OR REPLACE INTO nh_meta (key, value) VALUES (?, ?)").bind(key).bind(value).execute(&self.pool).await?;
        Ok(())
    }
    pub async fn meta_delete(&self, key: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM nh_meta WHERE key = ?").bind(key).execute(&self.pool).await?;
        Ok(())
    }

    // --- inbox --------------------------------------------------------------------------------

    /// Adds an item unless the inbox is over its limits (total items/bytes, or per-sender items).
    /// Returns false when refused.
    pub async fn inbox_push(&self, kind: &str, ptype: &str, sender: &str, packet: &str, now: i64, limits: &InboxLimits) -> Result<bool, sqlx::Error> {
        let row = sqlx::query("SELECT COUNT(*), COALESCE(SUM(bytes), 0) FROM nh_inbox").fetch_one(&self.pool).await?;
        let (count, bytes): (i64, i64) = (row.get(0), row.get(1));
        if count >= limits.max_items || bytes + packet.len() as i64 > limits.max_bytes { return Ok(false); }
        let per: i64 = sqlx::query("SELECT COUNT(*) FROM nh_inbox WHERE sender = ?").bind(sender).fetch_one(&self.pool).await?.get(0);
        if per >= limits.max_per_sender { return Ok(false); }
        sqlx::query("INSERT INTO nh_inbox (kind, ptype, sender, packet, bytes, received_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(kind).bind(ptype).bind(sender).bind(packet).bind(packet.len() as i64).bind(now)
            .execute(&self.pool).await?;
        Ok(true)
    }

    pub async fn inbox_pull(&self, after_seq: i64, limit: i64, max_bytes: i64) -> Result<(Vec<InboxItem>, bool), sqlx::Error> {
        let rows = sqlx::query("SELECT seq, kind, packet FROM nh_inbox WHERE seq > ? ORDER BY seq ASC LIMIT ?")
            .bind(after_seq).bind(limit + 1).fetch_all(&self.pool).await?;
        let mut items = Vec::new();
        let mut total = 0i64;
        let mut more = rows.len() as i64 > limit;
        for r in rows.into_iter().take(limit as usize) {
            let packet: String = r.get(2);
            if !items.is_empty() && total + packet.len() as i64 > max_bytes { more = true; break; }
            total += packet.len() as i64;
            items.push(InboxItem { seq: r.get(0), kind: r.get(1), packet });
        }
        Ok((items, more))
    }

    pub async fn inbox_ack(&self, up_to_seq: i64) -> Result<u64, sqlx::Error> {
        Ok(sqlx::query("DELETE FROM nh_inbox WHERE seq <= ?").bind(up_to_seq).execute(&self.pool).await?.rows_affected())
    }

    // --- DM events (NOSLOP_HUB_DM_VIEW_V1) ----------------------------------------------------------

    /// Records an incoming DM once (by message id); keeps the newest [`DM_EVENTS_KEEP`]. Returns false for a
    /// DM already recorded (a re-delivery).
    pub async fn dm_event_add(&self, dm_id: &str, peer: &str, sender: &str, preview: &str, now: i64) -> Result<bool, sqlx::Error> {
        let added = sqlx::query("INSERT OR IGNORE INTO nh_dm_events (dm_id, peer, sender, preview, at) VALUES (?, ?, ?, ?, ?)")
            .bind(dm_id).bind(peer).bind(sender).bind(preview).bind(now)
            .execute(&self.pool).await?.rows_affected() > 0;
        if added {
            sqlx::query("DELETE FROM nh_dm_events WHERE seq <= (SELECT MAX(seq) FROM nh_dm_events) - ?").bind(DM_EVENTS_KEEP).execute(&self.pool).await?;
        }
        Ok(added)
    }

    /// Events after `after_seq`, oldest first, and the newest sequence number (0 when there are none).
    pub async fn dm_events_after(&self, after_seq: i64, limit: i64) -> Result<(Vec<DmEvent>, i64), sqlx::Error> {
        let rows = sqlx::query("SELECT seq, dm_id, peer, sender, preview, at FROM nh_dm_events WHERE seq > ? ORDER BY seq ASC LIMIT ?")
            .bind(after_seq).bind(limit).fetch_all(&self.pool).await?;
        let latest: Option<i64> = sqlx::query("SELECT MAX(seq) FROM nh_dm_events").fetch_one(&self.pool).await?.get(0);
        Ok((rows.into_iter().map(|r| DmEvent {
            seq: r.get(0), dm_id: r.get(1), peer: r.get(2), sender: r.get(3), preview: r.get(4), at: r.get(5),
        }).collect(), latest.unwrap_or(0)))
    }

    pub async fn inbox_stats(&self) -> Result<(i64, i64), sqlx::Error> {
        let row = sqlx::query("SELECT COUNT(*), COALESCE(SUM(bytes), 0) FROM nh_inbox").fetch_one(&self.pool).await?;
        Ok((row.get(0), row.get(1)))
    }

    // --- contacts -----------------------------------------------------------------------------

    pub async fn contact_upsert(&self, c: &Contact, now: i64) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT OR REPLACE INTO nh_contacts (public_key, identity, relationship, is_trusted, is_temporary, handle, onion_address, enc_public_key, source, declined_nonce, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&c.public_key).bind(&c.identity).bind(&c.relationship).bind(c.is_trusted).bind(c.is_temporary)
            .bind(&c.handle).bind(&c.onion_address).bind(&c.enc_public_key).bind(&c.source).bind(&c.declined_nonce).bind(now)
            .execute(&self.pool).await?;
        Ok(())
    }

    /// A full push from the phone replaces the phone-sourced rows. Hub-accepted followers stay
    /// until the phone's own list contains them (it then owns them).
    /// Returns the keys of phone contacts that are no longer in the list (deleted on the phone): the
    /// Hub then ignores old requests from them for 7 days, as `GossipService.isPeerRecentlyDeleted` does.
    pub async fn contacts_replace_from_phone(&self, contacts: &[Contact], now: i64) -> Result<Vec<String>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let before: Vec<String> = sqlx::query("SELECT public_key FROM nh_contacts WHERE source = 'phone'").fetch_all(&mut *tx).await?.into_iter().map(|r| r.get(0)).collect();
        sqlx::query("DELETE FROM nh_contacts WHERE source = 'phone'").execute(&mut *tx).await?;
        for c in contacts {
            sqlx::query("INSERT OR REPLACE INTO nh_contacts (public_key, identity, relationship, is_trusted, is_temporary, handle, onion_address, enc_public_key, source, declined_nonce, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'phone', ?, ?)")
                .bind(&c.public_key).bind(&c.identity).bind(&c.relationship).bind(c.is_trusted).bind(c.is_temporary)
                .bind(&c.handle).bind(&c.onion_address).bind(&c.enc_public_key).bind(&c.declined_nonce).bind(now)
                .execute(&mut *tx).await?;
        }
        let now_keys: std::collections::HashSet<&str> = contacts.iter().map(|c| c.public_key.as_str()).collect();
        let removed: Vec<String> = before.into_iter().filter(|k| !now_keys.contains(k.as_str())).collect();
        for k in &removed {
            sqlx::query("INSERT OR REPLACE INTO nh_meta (key, value) VALUES (?, ?)").bind(format!("deleted_{}", k)).bind(now.to_string()).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(removed)
    }

    fn row_to_contact(r: &sqlx::sqlite::SqliteRow) -> Contact {
        Contact {
            public_key: r.get("public_key"), identity: r.get("identity"), relationship: r.get("relationship"),
            is_trusted: r.get::<i64, _>("is_trusted") != 0, is_temporary: r.get::<i64, _>("is_temporary") != 0,
            handle: r.get("handle"), onion_address: r.get("onion_address"), enc_public_key: r.get("enc_public_key"),
            source: r.get("source"), declined_nonce: r.get("declined_nonce"),
        }
    }

    pub async fn contact_get(&self, public_key: &str) -> Result<Option<Contact>, sqlx::Error> {
        Ok(sqlx::query("SELECT * FROM nh_contacts WHERE public_key = ?").bind(public_key).fetch_optional(&self.pool).await?.as_ref().map(Self::row_to_contact))
    }

    pub async fn contacts_for_identity(&self, identity: &str) -> Result<Vec<Contact>, sqlx::Error> {
        Ok(sqlx::query("SELECT * FROM nh_contacts WHERE identity = ?").bind(identity).fetch_all(&self.pool).await?.iter().map(Self::row_to_contact).collect())
    }

    pub async fn contacts_all(&self) -> Result<Vec<Contact>, sqlx::Error> {
        Ok(sqlx::query("SELECT * FROM nh_contacts").fetch_all(&self.pool).await?.iter().map(Self::row_to_contact).collect())
    }

    // --- creator content ----------------------------------------------------------------------

    pub async fn post_upsert(&self, p: &StoredPost, now: i64) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT OR REPLACE INTO nh_creator_posts (id, author_id, timestamp, content, media_id, payload, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&p.id).bind(&p.author_id).bind(p.timestamp).bind(&p.content).bind(&p.media_id).bind(&p.payload).bind(now)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn post_get(&self, id: &str) -> Result<Option<StoredPost>, sqlx::Error> {
        Ok(sqlx::query("SELECT id, author_id, timestamp, content, media_id, payload FROM nh_creator_posts WHERE id = ?").bind(id).fetch_optional(&self.pool).await?.map(|r| StoredPost {
            id: r.get(0), author_id: r.get(1), timestamp: r.get(2), content: r.get(3), media_id: r.get(4), payload: r.get(5),
        }))
    }

    /// Removes a post and the engagement on it (a creator `DELETE_POST`, or a full sync without it).
    pub async fn post_remove(&self, id: &str) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM nh_creator_posts WHERE id = ?").bind(id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM nh_comments WHERE post_id = ?").bind(id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM nh_reactions WHERE post_id = ?").bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn posts_since(&self, author_id: &str, since: i64) -> Result<Vec<StoredPost>, sqlx::Error> {
        Ok(sqlx::query("SELECT id, author_id, timestamp, content, media_id, payload FROM nh_creator_posts WHERE author_id = ? AND timestamp >= ? ORDER BY timestamp ASC")
            .bind(author_id).bind(since).fetch_all(&self.pool).await?
            .into_iter().map(|r| StoredPost { id: r.get(0), author_id: r.get(1), timestamp: r.get(2), content: r.get(3), media_id: r.get(4), payload: r.get(5) }).collect())
    }

    pub async fn post_ids(&self) -> Result<Vec<String>, sqlx::Error> {
        Ok(sqlx::query("SELECT id FROM nh_creator_posts").fetch_all(&self.pool).await?.into_iter().map(|r| r.get(0)).collect())
    }

    pub async fn comment_upsert(&self, id: &str, post_id: &str, author_id: &str, timestamp: i64, data: &str) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT OR REPLACE INTO nh_comments (id, post_id, author_id, timestamp, data) VALUES (?, ?, ?, ?, ?)")
            .bind(id).bind(post_id).bind(author_id).bind(timestamp).bind(data).execute(&self.pool).await?;
        Ok(())
    }
    pub async fn comment_get(&self, id: &str) -> Result<Option<(String, String, i64)>, sqlx::Error> {
        Ok(sqlx::query("SELECT post_id, author_id, timestamp FROM nh_comments WHERE id = ?").bind(id).fetch_optional(&self.pool).await?.map(|r| (r.get(0), r.get(1), r.get(2))))
    }
    pub async fn comment_delete(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM nh_comments WHERE id = ?").bind(id).execute(&self.pool).await?;
        Ok(())
    }
    pub async fn comments_for_posts_since(&self, post_ids: &[String], since: i64) -> Result<Vec<String>, sqlx::Error> {
        let mut out = Vec::new();
        for id in post_ids {
            for r in sqlx::query("SELECT data FROM nh_comments WHERE post_id = ? AND timestamp >= ? ORDER BY timestamp ASC").bind(id).bind(since).fetch_all(&self.pool).await? {
                out.push(r.get(0));
            }
        }
        Ok(out)
    }

    pub async fn reaction_upsert(&self, id: &str, post_id: &str, author_id: &str, timestamp: i64, data: &str) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT OR REPLACE INTO nh_reactions (id, post_id, author_id, timestamp, data) VALUES (?, ?, ?, ?, ?)")
            .bind(id).bind(post_id).bind(author_id).bind(timestamp).bind(data).execute(&self.pool).await?;
        Ok(())
    }
    pub async fn reaction_timestamp(&self, id: &str) -> Result<Option<i64>, sqlx::Error> {
        Ok(sqlx::query("SELECT timestamp FROM nh_reactions WHERE id = ?").bind(id).fetch_optional(&self.pool).await?.map(|r| r.get(0)))
    }
    pub async fn reaction_delete(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM nh_reactions WHERE id = ?").bind(id).execute(&self.pool).await?;
        Ok(())
    }
    pub async fn reactions_for_posts_since(&self, post_ids: &[String], since: i64) -> Result<Vec<String>, sqlx::Error> {
        let mut out = Vec::new();
        for id in post_ids {
            for r in sqlx::query("SELECT data FROM nh_reactions WHERE post_id = ? AND timestamp >= ? ORDER BY timestamp ASC").bind(id).bind(since).fetch_all(&self.pool).await? {
                out.push(r.get(0));
            }
        }
        Ok(out)
    }

    // --- media --------------------------------------------------------------------------------

    pub async fn media_get(&self, media_id: &str) -> Result<Option<MediaRow>, sqlx::Error> {
        Ok(sqlx::query("SELECT media_id, mtype, mime_type, size, sha256, received, complete FROM nh_media WHERE media_id = ?").bind(media_id).fetch_optional(&self.pool).await?.map(|r| MediaRow {
            media_id: r.get(0), mtype: r.get(1), mime_type: r.get(2), size: r.get(3), sha256: r.get(4), received: r.get(5), complete: r.get::<i64, _>(6) != 0,
        }))
    }
    pub async fn media_put(&self, m: &MediaRow, now: i64) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT OR REPLACE INTO nh_media (media_id, mtype, mime_type, size, sha256, received, complete, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&m.media_id).bind(&m.mtype).bind(&m.mime_type).bind(m.size).bind(&m.sha256).bind(m.received).bind(m.complete).bind(now)
            .execute(&self.pool).await?;
        Ok(())
    }
    pub async fn media_delete(&self, media_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM nh_media WHERE media_id = ?").bind(media_id).execute(&self.pool).await?;
        Ok(())
    }
    pub async fn media_total_bytes(&self) -> Result<i64, sqlx::Error> {
        Ok(sqlx::query("SELECT COALESCE(SUM(received), 0) FROM nh_media").fetch_one(&self.pool).await?.get(0))
    }
    pub async fn media_ids(&self) -> Result<Vec<String>, sqlx::Error> {
        Ok(sqlx::query("SELECT media_id FROM nh_media").fetch_all(&self.pool).await?.into_iter().map(|r| r.get(0)).collect())
    }
    /// Whether any stored creator post still uses this media (otherwise it can be deleted).
    pub async fn media_in_use(&self, media_id: &str) -> Result<bool, sqlx::Error> {
        let n: i64 = sqlx::query("SELECT COUNT(*) FROM nh_creator_posts WHERE media_id = ?").bind(media_id).fetch_one(&self.pool).await?.get(0);
        Ok(n > 0)
    }
}

#[derive(Debug, Clone)]
pub struct InboxLimits {
    pub max_items: i64,
    pub max_bytes: i64,
    pub max_per_sender: i64,
}
