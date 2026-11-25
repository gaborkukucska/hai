<!-- # START OF FILE helperfiles/SESSION_61_NOSLOP_HUB_DM_VIEW.md -->
# Session 61: The Home HUB Shows the Owner's DMs in the Web UI

Date: 2026-11-24. Device test (Hub on the owner's laptop, owner's phone linked, a second phone "Gabby"
following the owner as a creator): DMs between the phones worked both ways, but the Web UI showed `...` for
every message and no notifications. Reference: `docs/NOSLOP_HUB.md` §9.

## What was found
* The Web UI chat only ever had the plaintext copies the phone pushed (`sync_push_dms`). The Hub received
  every DM (it answers both onions) but stored it, encrypted, for the phone without opening it.
* The phone's copy tried only the personal keys; Gabby's DMs are encrypted to the creator identity, so each
  was copied as `...` (fixed in NoSlop, `NOSLOP_HUB_DM_MIRROR_KEYS_V1`).
* The Web UI's "Enable Notifications" setting existed and was saved, but nothing used it.

## What changed
* `hainet-noslop/src/hub/dms.rs`: incoming DMs to either identity and the phone's outgoing DMs (through
  `hub_send`) are opened with the Hub's keys (DM v2, as `DmPacketHandler` does) and handed to a hook;
  incoming ones are recorded in `nh_dm_events` (store schema v2). The packets still reach the phone unchanged.
* `hainet-core`: the hook writes the Web UI's `dms` table (same ids as the phone's copies).
* API: `hub_dm_events {after, limit}`.
* Portal: `lib/dmNotifications.tsx` (unread counts in the menu, per conversation and in the tab title;
  desktop notifications behind the existing setting; optional sound; a click opens the conversation),
  Settings asks the browser for permission and says when the address does not allow it.
* Removed `hai.patch` (committed by mistake with Session 60).

## Follow-up the same day: sending from the Web UI (`NOSLOP_HUB_WEBUI_SEND_V1`)
Device test: a DM typed in the Web UI to Gabby never arrived (Gabby's log: "Dropping DM from MCowBQYDK2Vw: no
X25519 key on record") and showed on the owner's phone with an attachment that never downloaded ("MEDIA_REQUEST
for null").
* `send_dm` only stored the text; the phone re-sent it from the personal identity, which a creator follower does
  not know. Now `hub::dms::send_from_web_ui` sends it from the Hub, DM v2, as the identity the contact knows, and
  the row is flagged `sent_by_hub` so the phone (NoSlop `NOSLOP_HUB_WEBUI_SEND_V1`) keeps a copy without sending it.
* `get_dms` / `sync_pull_dms` returned `"mediaId": null`; the phone's `optString` made it `"null"`. Now `""`.

## Follow-up 2026-11-25: GIFs (`NOSLOP_HUB_DM_MEDIA_V1`)
GIFs Gabby sent showed as empty messages in the Web UI: a picker GIF is a link in the media id
(`noslop-gif://https://…`) with empty text, and the chat showed only the text. The chat now offers to load the
GIF (on click, no referrer) and names other attachments as viewable on the phone; the notification says "[GIF]".
Device observation the same day: in Hub mode the phone collects DMs from the Hub inbox only while open (it is
frozen in the background, owner decision 2026-11-24), so the sender's app re-sends a DM every minute until the
phone acknowledges it.

## Follow-up 2026-11-25 (evening): deletes and "Clear Chat" (`NOSLOP_HUB_DM_DELETE_V1`)
"Clear Chat" on the owner's phone came back a minute later, and a message Gabby deleted stayed. Two NoSlop bugs
(fixed there, TECHNICAL_REFERENCE §16.65): the receiver checked a delete's signature against an older text than
senders sign, so no DM delete was ever applied; and after a clear the next DM sync asked the contact for everything
(`since=0`). On the Hub: `DELETE_MESSAGE`s passing through now remove the message from the Web UI chat (checked like
the phone does), and `hub_dms_clear` (called by the phone's Clear Chat) clears the Web UI conversation and keeps
older copies out.

## Follow-up 2026-11-25 (night): attachments, and the split creator onion (`NOSLOP_HUB_DM_MEDIA_V1`)
Device test: an image and a GIF Gabby sent showed only "open this conversation on your phone", and nothing reached
the owner's phone for half an hour. Logs: from 19:00 the phone's signed calls to the Hub's onion timed out after
20 s; at 19:03 and 19:27 it took the creator onion back ("Hub unreachable for 3 rounds") while the Hub kept
publishing it; Gabby got "SOCKS: Host unreachable" for the creator onion from 19:14 until the Hub restarted at
19:19 (a fresh descriptor), and again after 19:27. The Hub's journal has no Tor lines, so the Hub's own circuits
could not be checked.
* `hub/dm_media.rs` (new, `nh_dm_media`, schema v3): the Hub fetches a contact's DM attachments itself (signed
  `MEDIA_REQUEST` ranges as the addressed identity; its chunks stay on the Hub) and takes the owner's from the phone
  (`dm_media_put`); `GET /api/noslop/dm_media/<id>` serves them to the Web UI with byte ranges; deletes and cleared
  chats remove them. NOSLOP_HUB.md §10.
* `creator_onion_republish`: the phone calls it when it hands the creator onion back (fresh descriptor).
* Portal: the chat shows images and GIFs, plays audio and video, offers files, and says when one is on its way.
* NoSlop (§16.66): 45 s onion connect, one failed connection per round instead of one per call, creator onion taken
  back only after 10 minutes of silence, DM attachments uploaded to the Hub, a refresh button and a Tor status line
  in the chat.

## Follow-up 2026-11-25 (late): device test passed for attachments; empty Admin AI answers
Image and GIF from Gabby were fetched by the Hub (15 KB, 3.8 MB) and an image the phone held was uploaded; the
creator onion was published again on hand-back. The Admin AI's second answer reached the phone empty: the model
(qwen3.5:9b, a thinking model) used its 512 tokens on reasoning. `hainet-persona` Ollama client: thinking-aware
(`OLLAMA_THINKING_EMPTY_ANSWER_V1`, NOSLOP_HUB.md §10); `hainet-core` never sends an empty reply. The three
outdated `OllamaClient::new` calls in that file's tests were updated (other persona test modules still do not
compile, as before).

## Verification
* `cargo test -p hainet-noslop --features hub`: 42 tests pass (7 vector, 35 behaviour; 15 new this session).
* Real `hainet-core` binary + independent Python client: 43/43 end-to-end checks (15 new this session).
* Portal: `vite build` passes; `tsc --noEmit` reports the same 35 errors as before the change, none new.
* Not verified here: a browser on the device, a live Tor Hub.
<!-- # END OF FILE helperfiles/SESSION_61_NOSLOP_HUB_DM_VIEW.md -->
