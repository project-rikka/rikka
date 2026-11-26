---
title: Music cross-service playlists (Rikka playlists)
description: music-blend's Blend model, JSPF store, song matching, mirror sync and discovery, and how rikka-music drives them.
about: apps/system/music/crates/music-blend
tags: [music, sync]
---

- A Rikka playlist is a `Blend`: uuid v4 `BlendId`, name, ordered `Entry`s (an origin `Track` snapshot plus per-service matches, `Found(id)` or `Missing { checked }`), and one `Mirror` per (service, account id) with the last pushed contents as the sync base.
- The local copy is JSPF (`urn:uuid:` identifier, Rikka data under the `urn:rikka:music:blend` extension, per-track `fidelity`) in the user-visible `Places.playlists` folder, `$XDG_MUSIC_DIR/Playlists/<name>.jspf` (fallback `$XDG_STATE_HOME/rikka/music/blends`); the file name follows the blend's name (clashes numbered, renames move the file) and identity comes from inside the file. `buried.json` (deleted ids, `Store::delete`/`is_deleted`) stays in `Places.state` = `$XDG_STATE_HOME/rikka/music/blends`. A file naming an unknown service is skipped, never rewritten.
- The playlists folder is watched (`rikka-music-playlists`, 300 ms debounce): a `Ledger` shared with the writer records what the store itself last wrote (path + digest, the writer holds it across each write), so only outside changes become `Store::outside()` events — `Outside::Edited` (edited, added or restored file; restoring undeletes) and `Outside::Gone` (file deleted while every playlist file in the folder was readable). The app applies them through `Music::blend_outside` (Gone = delete the blend and its mirrors).
- Services with `Kind::Local` take part by matching only: no mirror is made, pulled or pushed, and `discover` never lists them.
- Every linked account holds a mirror playlist named `rikka: <name>` (`marker`) whose description carries `rikka:<uuid>`; the description is the identity, the prefix only flags candidates whose listing lacks a description (YouTube Music).
- `sync(blend, links)` pulls each mirror's changes against its base (removals only when the page is whole), matches undecided entries per service through `matching::find` (ISRC, then normalized title/artist with a duration window; misses retried after 7 days), pushes desired contents (append or `arrange_playlist`), rereads them, and marks matches a service refused as `Missing`. `Blend::absorb` applies the `Synced` onto the current state; a blend whose every mirror vanished while settled is gone.
- `discover(Store::snapshot(), links, order)` lists playlists, deletes mirrors of deleted blends, adopts mirrors of known blends from newly linked accounts, and rebuilds unknown blends by merging mirrors in `service-order` priority. Per-account failures come back as `AccountError { service, account, error }` (also in `Synced`).
- A mirror is gone when its page answers `Missing` or `Catalog::keeps_playlist` says no.
- `arrange_playlist` adds the new contents before removing the old slots, so a cut-off edit leaves extra songs rather than lost ones; sync appends only to wholly read pages.
- `Blends` publishes every entry's known matches through a shared `Known` handle; `Player` plays a known match directly and otherwise searches the fallback services concurrently.
- `Store::open(Places { playlists, state }, services)` updates memory at once and writes files on its `rikka-music-blends` thread; `failures()` streams write errors (the app shows them as library notes) and dropping the `Store` joins the writer, so read-after-write in tests needs a drop.
- In the app, `Blends` owns the `Store`; `src/music/blending.rs` debounces syncs per blend, never runs discovery and syncs together, and discovers again after accounts link.
