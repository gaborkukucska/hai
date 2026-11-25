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

## Verification
* `cargo test -p hainet-noslop --features hub`: 34 tests pass (7 vector, 27 behaviour; 7 new).
* Real `hainet-core` binary + independent Python client: 35/35 end-to-end checks (7 new).
* Portal: `vite build` passes; `tsc --noEmit` reports the same 35 errors as before the change, none new.
* Not verified here: a browser on the device, a live Tor Hub.
<!-- # END OF FILE helperfiles/SESSION_61_NOSLOP_HUB_DM_VIEW.md -->
