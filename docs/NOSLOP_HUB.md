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
| `hub_dms_clear` | `peer`, `before` (ms; default now) | `deleted`, `before` (the conversation cleared on the phone, §9) |
| `hub_dm_events` | `after`, `limit` (≤ 500) | `events: [{seq, dm_id, peer, sender, preview, at}]`, `latest` (incoming DMs that reached the Hub, §9) |
| `hub_dm_media_info` | `media_ids` (≤ 200) | `items: [{media_id, state, type, mime_type, size, received, incoming, error, next_at}]` (DM attachments, §10) |
| `dm_media_put` | `media_id`, `sha256`, `size`, `offset`, `data` (≤ 1 MB, Base64); first call also `peer`, `dm_id`, `dm_ts`, `as_key`, `incoming`, `type`, `mime_type` | `status` (`ok`, `resume`, `busy`), `received`, `complete` (§10) |
| `creator_onion_republish` | none | `republished`: the creator onion published again with a fresh descriptor (§3) |
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
| `HAINET_NOSLOP_DM_MEDIA_QUOTA` / `HAINET_NOSLOP_DM_MEDIA_MAX` | 5 GB / 100 MB (DM attachments, §10) |
| `HAINET_NOSLOP_BACKUP_MAX` / `HAINET_NOSLOP_BACKUP_KEEP` | 2 GB / 5 |
| `HAINET_SESSION_TTL_HOURS` | 168 |
| `HAINET_HTTP_MAX_HEADER_BYTES` / `HAINET_HTTP_MAX_BODY_BYTES` | 32 KB / 16 MB |

## 7. Tests

* `cargo test -p hainet-noslop --features hub`: 7 cross-language vector tests (Gson / Bouncy Castle),
  30 unit and behaviour tests (follower accept, declined / blocked, sync and inventory, media upload
  and chunk serving, re-signing and relays, owner signatures, Admin AI, presence, inbox ACK, backup
  sidecar and pruning, DMs opened for the Web UI and the ones that must not be, schema v1 → v3, Web UI DMs sent from
  the right identity and the ones refused, signed deletes from either side and forged ones,
  cleared conversations), plus 5 for DM attachments (fetched from the sender with signed ranges and kept out of
  the phone's inbox, chunks from anyone else passed on, a wrong SHA-256 refused, the owner's resumable upload,
  deletes and cleared chats removing files, picker GIFs and oversized files not fetched, onion republish).
* End to end against the real `hainet-core` binary with an independent Python client (43 checks, 6 for DM
  attachments: upload, session required, served with its type and `nosniff`, a byte range, 404, state):
  owner auth and replay, creator hand-over, a follower's handshake answered on its own connection,
  dropped unsigned and forged frames, a follower's DM decrypted into the Web UI chat with one
  notification event (and still in the phone's inbox), the owner's reply shown without one, a Web UI
  reply sent by the Hub as the creator and flagged for the phone, no null media ids, a contact's signed
  delete and a cleared conversation leaving the Web UI chat, Admin AI
  round trip, streamed backup push / pull, portal sessions, header limits, onion-only dialling.

## 8. Not done yet

* iOS (`mvp/`) has no owner-API client; iOS **followers** are served like any other.
* The Hub does not relay gossip for others while the phone is offline (it never did).
* Non-creator community mirroring and the discovery-cache work of the Enhancement Plan.
* Group messages are not shown in the Web UI chat (§9), and the Web UI sends text only (no attachments).
* The Hub does not serve the owner's own DM attachments to the contact while the phone is away (the contact's
  phone asks the owner's phone, as before).

## 9. DMs in the Web UI (`NOSLOP_HUB_DM_VIEW_V1`, 2026-11-24)

Before, the Web UI chat only had the plaintext copies the phone pushed (`sync_push_dms`), so a DM showed
there only after the phone had pulled it from the Hub's inbox, decrypted and copied it; and the phone's
copy tried only the personal keys, so every DM with a contact who follows the owner as a creator showed
as `...` (fixed on the phone, NoSlop `NOSLOP_HUB_DM_MIRROR_KEYS_V1`).

Now the Hub opens the owner's one-to-one DMs itself (`hainet-noslop/src/hub/dms.rs`), with the keys of
both identities it holds:

* **Incoming** (a contact's `MESSAGE` that reaches either onion): opened with the addressed identity,
  then the other one, as `DmPacketHandler.handleDirectMessage` does (DM v2: directional HKDF key and
  AAD). The packet still goes to the phone's inbox unchanged. Each one is also recorded once (by message
  id) in `nh_dm_events` (schema v2; the newest 1,000 kept) for notifications.
* **Outgoing** (the phone's `MESSAGE` through `hub_send`): opened with the sending identity, shown on the
  owner's side, no notification.
* **Not opened:** group messages (their keys come from a group directory the Hub does not keep), the
  Admin AI conversation (§1), DMs from or to keys that are not in the contact list, anything that fails
  authentication, and legacy v1 DMs (unauthenticated; the phone refuses them from senders that speak v2).
* `hainet-core` writes each opened DM to the Web UI's `dms` table (`INSERT OR REPLACE` by message id, so
  the phone's copies of the same messages never duplicate them).
* **Web UI** (`hainet-portal/src/lib/dmNotifications.tsx`): polls `hub_dm_events` every 5 s; unread
  counts on the Chat menu item, on each conversation and in the tab title, kept per browser
  (localStorage; what was on the Hub before the first visit counts as read); a desktop notification per
  new DM when Settings → "Enable Notifications" is on and the browser allows it (secure origins only:
  `http://localhost`, or the `.onion` in Tor Browser), a short sound when "Enable Sound" is on. Clicking a
  notification opens that conversation.

**Sending from the Web UI** (`NOSLOP_HUB_WEBUI_SEND_V1`, 2026-11-24). `send_dm {peer_id, content}` used to
store the text only; the phone sent it later, always from the personal identity, so a contact who follows
the owner as a creator dropped it ("no X25519 key on record"). Now the Hub sends it itself
(`hub::dms::send_from_web_ui`): DM v2 from the identity the contact knows (`nh_contacts.identity`), to the
contact's onion, signed, with the usual retries; the reply names the `id` and `sender`. It refuses a
non-contact, a blocked or declined contact, one without an X25519 key yet, and a creator follower when the
creator identity is not on the Hub. The `dms` row is flagged `sent_by_hub`, so the phone keeps a copy and
does not send it again (NoSlop ≥ the 2026-11-24 build; an older phone sends it a second time with the same
message id, which the contact drops as a duplicate). `get_dms` / `sync_pull_dms` never return a null
`mediaId` / `mediaType` any more (the phone read null as the text "null": a phantom attachment).

**Deletes and cleared chats** (`NOSLOP_HUB_DM_DELETE_V1`, 2026-11-25). A `DELETE_MESSAGE` for a one-to-one DM
that passes through the Hub (a contact deleting their own message, or the owner deleting theirs through
`hub_send`) also removes that message from the Web UI's chat and its notifications. It is checked as the phone
checks it: the author is the packet's sender and signed `encodeForSigning(message_id, author_id, timestamp)` (or
the older `id|author|ts` text). "Clear Chat" on the phone calls `hub_dms_clear {peer, before}`: the conversation
up to `before` leaves the Web UI and its notifications, and older copies (a contact re-sending, a DM sync answer)
are not shown again. The mark never moves back. NoSlop side: `NOSLOP_DM_DELETE_SIGNATURE_V1` (the phone itself
never applied a DM delete before, see its TECHNICAL_REFERENCE §16.65) and `NOSLOP_DM_CLEAR_STICKS_V1`.

**Attachments in the Web UI** (`NOSLOP_HUB_DM_MEDIA_V1`, 2026-11-25). A GIF from NoSlop's picker is a link
(`mediaId` `noslop-gif://https://…`, empty text), which showed as an empty message. The Web UI now shows a
"GIF · Load from <host>" button and loads the GIF (no referrer) only when clicked, as the app asks before
loading it (the GIF host sees the address of the computer that loads it). Other attachments say to open the
conversation on the phone. The notification for a GIF reads "[GIF]". Superseded for other attachments by §10.

## 10. DM attachments in the Web UI chat (`NOSLOP_HUB_DM_MEDIA_V1`, 2026-11-25)

A DM carries only an attachment's description (`MediaMetadata`: id, type, mime type, size, SHA-256); the file stays
with the sender. The Hub now keeps a copy for the Web UI (`hub/dm_media.rs`, table `nh_dm_media`, store schema v3,
files in `<noslop_dir>/dm_media`):

* **A contact's attachment** is fetched by the Hub from the contact as soon as it opens the DM: byte ranges of at
  most 256 KiB (what a phone serves), 4 in flight, each `MEDIA_REQUEST` signed `noslop-media-req-v1` and sent as
  the owner's identity the DM was addressed to. The contact's phone allows it (the requester is the DM's other
  party) and answers to that identity's onion, which is the Hub's. A `MEDIA_CHUNK` that belongs to such a fetch
  (same id, from the holder, a range asked for) stays on the Hub; every other chunk goes to the phone's inbox as
  before. The file must have the declared size and, when given, SHA-256. A contact that does not answer (three
  90 s rounds) is asked again later (1, 5, 15, 30, 60 min …) for up to three days. Picker GIFs (links) and files
  over `HAINET_NOSLOP_DM_MEDIA_MAX` are not fetched.
* **The owner's own attachments** (and the contact's ones the phone already downloaded) are uploaded by the phone
  (`dm_media_put`, owner-signed, resumable) when it mirrors its DMs, after asking `hub_dm_media_info` what is missing.
* **Web UI**: `GET /api/noslop/dm_media/<id>` (session cookie, or an owner signature) streams a complete file with
  its type, one byte range when asked (audio and video seek), `nosniff`, a sandboxing CSP, and never an HTML or
  script type. The chat shows images and GIFs, plays audio, voice recordings and video, offers other files for
  download, and says when an attachment is still being fetched or waits for the phone.
* A deleted DM (`DELETE_MESSAGE`) or a cleared conversation (`hub_dms_clear`) takes its attachments with it.

**Creator onion republish** (`NOSLOP_HUB_ONION_REPUBLISH_V1`, same day). When the phone could not reach the Hub
for a while it published the creator onion itself; after handing it back, the phone's descriptor stayed the newest
on the directories and contacts kept finding the frozen phone ("Host unreachable") until the Hub restarted. On
hand-back the phone now calls `creator_onion_republish`: the Hub withdraws and re-adds the onion (`DEL_ONION`,
`ADD_ONION`), which uploads a fresh descriptor. NoSlop side: TECHNICAL_REFERENCE §16.66.
<!-- # END OF FILE docs/NOSLOP_HUB.md -->
