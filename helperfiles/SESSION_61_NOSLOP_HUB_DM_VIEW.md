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

## Verification
* `cargo test -p hainet-noslop --features hub`: 31 tests pass (7 vector, 24 behaviour; 4 new).
* Real `hainet-core` binary + independent Python client: 32/32 end-to-end checks (4 new).
* Portal: `vite build` passes; `tsc --noEmit` reports the same 35 errors as before the change, none new.
* Not verified here: a browser on the device, a live Tor Hub.
<!-- # END OF FILE helperfiles/SESSION_61_NOSLOP_HUB_DM_VIEW.md -->
