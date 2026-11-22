<!-- # START OF FILE helperfiles/SESSION_60_NOSLOP_HUB_CREATOR_HOSTING.md -->
# Session 60: NoSlop 0.8 Home HUB — Creator Hosting, Verified Mesh, Owner-Signed API

Date: 2026-11-22. Request (`isues.txt`): catch HAI-Net up with NoSlop (0.8.2-alpha) so it acts as the
Home HUB for the legacy Android app, focusing on users with creator mode and discoverability on: hold
and back up their identities and serve their content. Reference: `docs/NOSLOP_HUB.md`.

## Owner decisions (2026-11-22)
1. The Hub takes over the **same** creator onion (creator keys go to the Hub, the phone stops publishing it).
2. The Hub **auto-accepts** follower requests to the creator identity, as the phone does (v2, 5/sender/hour).
3. A Hub hosting a creator serves **only the creator's own content**.

## What was found
* The Hub could not talk to NoSlop ≥ 0.7: no envelope signatures (dropped on arrival), no answers to
  sync / media / handshakes, and the phone's packets were relayed unsigned to every trusted contact.
* Linking a Hub took the creator offline: the phone stopped publishing both onions, the Hub only had the personal identity.
* `/api/invoke` exempted `sync_*`, `get_dms`, `get_mesh_peers` from login; login was one global flag;
  the API is on the personal onion, so any contact could read the DM mirror or drain the inbox.
* `/api/backup/push` (used daily by the app) did not exist. Admin AI decrypted DM v1 only.

## What changed
* **New crate `hainet-noslop`**: Gson-exact JSON, `encodeForSigning` (UTF-16 lengths), envelope sign /
  verify / replay guard, key formats, onion and Tor key derivation, DM v2 / v1, all checked against
  vectors from the app's own Gson 2.10 and Bouncy Castle 1.78.1.
* **`hub` feature**: verified loopback mesh listeners (personal 9999, creator 9998), durable inbox with
  ACK, Tor delivery that re-signs the owner's packets per attempt, creator duties (follower handshakes,
  sync / inventory, media serving, engagement, presence and discovery), creator onion via `ADD_ONION`
  held by the process (single-owner fence), owner API (`hub_*`, `creator_*`), streamed backups.
* **`hainet-core`**: HTTP server reads bytes with header (32 KB) and body (16 MB) limits; owner signature
  or per-browser session cookie on every `/api/invoke`; `/api/backup/push|pull|list`; Admin AI on DM v2 with
  immediate `DM_ACK` v3; static assets served as bytes; `get_mesh_peers` from the Hub's contact list;
  `hub_feed_push` for the Web UI feed; the old relay (`send_packet_over_tor`, in-memory buffer) replaced.
* **`hainet-seed`**: Tor control access for the service user (group, cookie ControlPort).

## Verification
* `cargo test -p hainet-noslop --features hub`: 27 tests pass (7 vector, 20 behaviour).
* `cargo check -p hainet-core` / `-p hainet-seed`: clean (no new warnings).
* Real `hainet-core` binary + independent Python client: 28/28 end-to-end checks pass.
* Not verified here: a live Tor daemon (control port, onion publishing), the Android build, devices.

## Next session
* Device test: creator mode on, Hub linked, phone fully offline; a follower on another phone connects,
  syncs posts and plays media from the Hub.
* Then: discovery-cache and community-mirroring work from the NoSlop Enhancement Plan; iOS owner client.
<!-- # END OF FILE helperfiles/SESSION_60_NOSLOP_HUB_CREATOR_HOSTING.md -->
