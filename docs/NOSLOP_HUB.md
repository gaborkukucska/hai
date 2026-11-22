<!-- # START OF FILE docs/NOSLOP_HUB.md -->
# NoSlop Home HUB (`hainet-noslop`)

How HAI-Net acts as the Home HUB for the legacy NoSlop Android app (0.8.x), what it does for a
**creator** (creator mode on, optionally discoverable), and how the phone controls it. This replaces
the relay described in `NOSLOP_INTEGRATION_PLAN.md` Phases 1–3 (kept as history).

Code: `hainet-noslop/` (protocol + `hub` feature), wired in by `hainet-core/src/main.rs` and
`hainet-core/src/api_router.rs`. Android side: `NoSlop/app/.../data/HubClient.kt`,
`NoSlopRepository` (`ensureCreatorHostedOnHub`, `pullHubInbox`, `syncPeersWithHub`), `TorService`
(`NOSLOP_HUB_OWNER_API_V1`, `NOSLOP_HUB_CREATOR_HOSTING_V1`).

## 1. What the Hub is now

| Before (≤ 2026-07-26) | Now |
|---|---|
| Mesh listener on `0.0.0.0:9999` took any JSON line, no signature check | Loopback listeners (personal `127.0.0.1:9999`, creator `127.0.0.1:9998`); every frame ≤ 1 MB, must carry a valid NoSlop envelope signature (`noslop-env-v1`), not a replay, within rate limits |
| Packets kept in an in-memory `Vec`, cleared when read | Durable SQLite inbox (`nh_inbox`), removed only on the phone's ACK |
| Sent everything unsigned; receivers on NoSlop ≥ 0.7 dropped it | Packets from the owner's identities are signed at every send attempt; others pass on byte for byte |
| Broadcast every pushed packet to every trusted contact | The phone names the destination of each packet (`hub_send`); its own routing rules apply |
| `/api/invoke` open for `sync_*`, `get_dms`, `get_mesh_peers`; one global login flag | Every command needs an owner signature (phone) or a per-browser session cookie (portal) |
| Knew only the personal identity; the creator went offline when a Hub was linked | Holds the creator identity too and publishes its onion; answers followers while the phone is away |
| `/api/backup/push` did not exist | Owner-signed, streamed backup push / pull / list |
| Admin AI decrypted DM v1 only (phones send v2 since 0.7) | DM v2 (v1 fallback), immediate `DM_ACK` v3, v2 reply |

## 2. Protocol fidelity

`hainet-noslop` reproduces the app's formats exactly, proven against vectors produced by the app's own
libraries (Gson 2.10, Bouncy Castle 1.78.1) — `hainet-noslop/tests/vectors/`, generator
`tests/vector-generator/GenVectors.java`:

* `encodeForSigning` length-prefixes with Kotlin `String.length` (UTF-16 code units).
* The envelope digest is SHA-256 of **Gson's** rendering of the payload (null members removed, wire
  member order, numbers as written, U+2028/U+2029 escaped) — `json.rs` has its own order-preserving
  value type for this.
* Keys: X.509 / PKCS#8 (Bouncy Castle's 83-byte X25519 `OneAsymmetricKey` included), onion derivation,
  Tor `hs_ed25519_secret_key`.
* DM v2 (directional HKDF-SHA256 + AAD), `DM_ACK` v3, handshake v2, `ANNOUNCE_PEER` v2,
  `ANNOUNCE_DISCOVERABLE`, signed media requests, post/comment/reaction signatures.

## 3. Creator hosting (creator mode on)

The phone hands over, through the owner API: the creator keys (`creator_identity_set`, stored in
`~/.hainet/identity/creator/`, files 0600), the creator profile and switches (`creator_profile_set`),
the creator's **public** posts with their comments and reactions (`creator_content_sync`, each item's
signature verified before it is stored), and the media of those posts (`creator_media_put`, resumable,
checked against the signed SHA-256 before it is served). Then `creator_hosting_set {enabled: true}`.

The Hub publishes the creator onion with `ADD_ONION` on Tor's control interface **without**
`Flags=Detach`: the service exists only while `hainet-core` holds the control connection. The phone
withdraws its own creator onion only after `hub_status` reports `onion_published: true`, and takes it
back when creator mode is turned off, when the Hub reports it lost the onion, or after three rounds
(≈3 min) without reaching the Hub. One owner at a time; a stopped Hub cannot leave a stale claim.

While hosting, for packets addressed to the creator identity the Hub does what the phone does:

| Packet | Hub behaviour (phone counterpart) |
|---|---|
| `CONNECTION_REQUEST` | v2 signature, nonce, freshness, blocked / declined / recently deleted checks; auto-accept (max 5 per sender per hour) and `USER_HANDSHAKE` as the creator (creator profile, never the personal one), on the requester's own connection when fresh, else delivered with retries. The phone gets a `creator_follower_accepted` notice and records the follower without answering again. Creator mode off, over the limit, or a personal contact: passed to the phone (`HandshakePacketHandler.handleConnectionRequest`) |
| `SYNC_REQUEST` / `INVENTORY_SYNC_REQUEST` | The creator's public posts (since `since` / missing or changed by inventory hash), then the comments and reactions on them, in `SYNC_RESPONSE` batches of 25, retried after 30 s and 90 s (`SyncPacketHandler`). Only to known creator contacts |
| `MEDIA_REQUEST` / `MEDIA_RELAY_REQUEST` | Served from the Hub's copy of the creator's public post media: metadata or 256 KB ranges, 16 MB per requester per 60 s, strangers only with a valid fresh signed request answered at the onion of their own key (`MediaManager.handleMediaRequest`, `replyOnionFor`). Media the Hub doesn't hold goes to the phone |
| `COMMENT` / `EDIT_COMMENT` / `DELETE_COMMENT` / `REACTION` on creator posts | Verified and kept so followers see them in sync; also passed to the phone |
| Everything else (DMs to the creator, `FOLLOW`, `DM_ACK`, …) | Passed to the phone |

Presence: every 60 s the Hub sends the creator's `ANNOUNCE_PEER` to the creator's accepted contacts
and, when discoverable, `ANNOUNCE_DISCOVERABLE` to every trusted contact (hops 4 to creator contacts,
3 in relay form to personal contacts, as `GossipService.outboundFor` sends it). The phone stops sending
both while the Hub hosts the creator; it still sends the creator's name update.

Scope (owner decision, 2026-11-22): a Hub hosting a creator serves **only the creator's own content**,
never other authors' posts (Enhancement Plan rules 1–2).

## 4. The owner API

Signature headers on every request (Android `HubClient`):

```
X-NoSlop-Hub-Key:   <owner personal public key, Base64 X.509>   (must equal ~/.hainet/identity/ed25519_pub.b64)
X-NoSlop-Hub-Ts:    <epoch ms>                                    (±5 minutes)
X-NoSlop-Hub-Nonce: <16–64 chars [A-Za-z0-9_-]>                   (never reused within 10 minutes)
X-NoSlop-Hub-Sig:   Ed25519(encodeForSigning("noslop-hub-api-v1", METHOD, PATH, ts, nonce, sha256hex(body)))
```

Routes: `POST /api/invoke` (`{"cmd", "args"}`), `POST /api/backup/push` (raw body, streamed to disk),
`GET /api/backup/pull` (newest backup, `X-Backup-Sha256`), `GET /api/backup/list`. The portal logs in
as before (QR signed by the owner key, or passphrase) and gets an `HttpOnly; SameSite=Strict` session
cookie for its own browser; `POST /api/auth/logout` ends it.

| cmd | args | reply |
|---|---|---|
| `hub_status` | – | `personal`, `creator` (`public_key`, `onion_address`, `hosting`, `onion_published`, `onion_error`, `is_creator`, `is_discoverable`), `creator_hosting`, `inbox`, `contacts`, `creator_content`, `api_protocol`, `server_time` |
| `hub_send` | `items: [{onion, packet}]` (≤ 500) | `queued`, `handled_by_hub` (Admin AI), `rejected` |
| `hub_inbox_pull` | `after`, `limit` (≤ 500), `max_bytes` | `items: [{seq, kind, packet}]`, `more`. `kind`: `packet` or `creator_follower_accepted`; `packet` is the raw wire frame |
| `hub_inbox_ack` | `up_to` | `deleted` |
| `hub_contacts_push` | `contacts: [{public_key, identity, relationship, is_trusted, is_temporary, handle, onion_address, enc_public_key, declined_nonce}]`, `full` | `contacts`, `removed` |
| `hub_feed_push` | `posts: [PostPayload]` | `posts_stored` (Web UI feed only; nothing is sent) |
| `creator_identity_set` | `public_key`, `private_key`, `enc_public_key`, `enc_private_key`, `display_name`, `replace` | `onion_address` |
| `creator_identity_remove` | – | removes the keys, the creator content and followers |
| `creator_profile_set` | `display_name`, `handle`, `bio`, `avatar_b64`, `fund_me_link`, `is_creator`, `is_discoverable` | – |
| `creator_hosting_set` | `enabled` | `hub_status` |
| `creator_content_sync` | `posts`, `comments`, `reactions`, `keep_ids`, `full` | `posts`, `comments`, `reactions`, `rejected`, `media_needed: [{media_id, sha256, received}]` |
| `creator_media_put` | `media_id`, `type`, `mime_type`, `size`, `sha256`, `offset`, `data` (Base64, ≤ 1 MB) | `status` (`ok` / `resume`), `received`, `complete` |
| `creator_media_status` | `media_ids` | per id `received`, `complete` |
| `sync_push_packets` / `sync_pull_packets` / `sync_push_peers` | older clients | same meaning as before, now signed and verified |

## 5. Deployment

Debian's `tor@default` already provides a control socket (`/run/tor/control`) and a group-readable
cookie. The NoSlop SSH deployer (full deploy, *Update Hub Software*, *Reset identity*) and the seed
installer now add the Hub's service user to Tor's group (`debian-tor`, else `tor`) with a systemd
drop-in (`/etc/systemd/system/hainet-core.service.d/noslop-tor.conf`, `SupplementaryGroups=`), and add
`ControlPort 127.0.0.1:9051` with cookie authentication to `/etc/tor/torrc` when missing. The personal
onion keeps `HiddenServicePort 8080` (owner API, signed) and `9999`; the creator onion has `9999` only.

## 6. Settings (environment variables)

| Variable | Default |
|---|---|
| `HAINET_IDENTITY_DIR` | `~/.hainet/identity` |
| `HAINET_NOSLOP_DIR` | `<data_dir>/noslop` (media, backups) |
| `HAINET_MESH_LISTEN` / `HAINET_CREATOR_MESH_LISTEN` | `127.0.0.1:9999` / `127.0.0.1:9998` |
| `HAINET_TOR_SOCKS` | `127.0.0.1:9050` |
| `HAINET_TOR_CONTROL` | `unix:/run/tor/control` if present, else `127.0.0.1:9051` |
| `HAINET_TOR_CONTROL_PASSWORD` | unset (cookie / null auth) |
| `HAINET_MESH_MAX_CONNECTIONS` / `HAINET_MESH_MAX_SENDS` | 64 / 16 |
| `HAINET_INBOX_MAX_ITEMS` / `_MAX_BYTES` / `_MAX_PER_SENDER` | 20,000 / 512 MB / 2,000 |
| `HAINET_NOSLOP_MEDIA_QUOTA` | 20 GB |
| `HAINET_NOSLOP_BACKUP_MAX` / `HAINET_NOSLOP_BACKUP_KEEP` | 2 GB / 5 |
| `HAINET_SESSION_TTL_HOURS` | 168 |
| `HAINET_HTTP_MAX_HEADER_BYTES` / `HAINET_HTTP_MAX_BODY_BYTES` | 32 KB / 16 MB |

## 7. Tests

* `cargo test -p hainet-noslop --features hub`: 7 cross-language vector tests (Gson / Bouncy Castle),
  20 unit and behaviour tests (follower accept, declined / blocked, sync and inventory, media upload
  and chunk serving, re-signing and relays, owner signatures, Admin AI, presence, inbox ACK, backup
  sidecar and pruning).
* End to end against the real `hainet-core` binary with an independent Python client (28 checks):
  owner auth and replay, creator hand-over, a follower's handshake answered on its own connection,
  dropped unsigned and forged frames, Admin AI round trip, streamed backup push / pull, portal
  sessions, header limits, onion-only dialling.

## 8. Not done yet

* iOS (`mvp/`) has no owner-API client; iOS **followers** are served like any other.
* The Hub does not relay gossip for others while the phone is offline (it never did), and it does
  not decrypt DMs for the personal identity (they wait in the inbox for the phone).
* Non-creator community mirroring and the discovery-cache work of the Enhancement Plan.
* `sync_push_dms` / `get_dms` still mirror plaintext DMs for the Web UI chat (now behind owner auth).
<!-- # END OF FILE docs/NOSLOP_HUB.md -->
