---
title: Shared session protocols
description: rikka-session's typed compositor/shell contracts: windows, workspaces, shortcuts, wallpaper, agent, surfaces, profile.
about: crates/session/rikka-session
tags: [session, protocol]
---

- Each module owns its capability constants, payloads, event recognition and typed client. One `PROTOCOL: Protocol` pairs capability and version. `call/call_with/notify/notify_with` include the version, `decode_event(event, name)` recognizes events; each module's `changed(event)` decodes its `changed` event.
- `following::{changes, states}` follow a capability and re-read after `Lagged`; a `states` stream first yields the current state (fetched with the refresh request after subscribing), so followers need no separate fetch. `toplevels::states` and `workspaces::states` are what the shell uses.
- A `states` stream (e.g. `wallpaper::listings`, `displayed`) yields nothing on subscribe; a follower also makes the matching call (`list`, `showing`) once for the current value.
- `toplevels`: state (on screen, minimized), focus, close, minimize, move. `workspaces`: per-output state, show, `COUNT = 10`.
- `gestures` (Full): the compositor's `swiped` events for 3-finger swipes (`Began { axis, output }`, `Moved`, `Ended`); `swipes(seam)` streams them.
- `switcher`: the compositor's Alt+Tab state (`changed` with `Option<Switch>`: serial, output, MRU `Candidate`s, selected) and the shell's `select`/`choose` (by window id) and `dismiss`/`arrange` notifications for a serial; `Rows::above/below` is the up/down rule over the shell's card centres.
- `activation` mints xdg-activation tokens. `desktop` covers shell pid, start, stop and restart (xtask reloads).
- `wallpaper`: library ops by opaque id. `Preview` is a video or a `Thumbnail` (made, making, or never).
- `agent`: the seat name, `placement()` (pointer position and display), ring on/off, and `wait_idle(quiet, timeout)` → `Idle::{Quiescent, TimedOut}`: the compositor answers once the agent's output has presented no frame and has none requested or in flight for `quiet`, counted from the request (callers send it after their injected input reached the compositor).
- `lock` (1.2): `state`, `wanted` (`is_lock_request`) and the latest-value `changed` event carrying `LockState` (`changed(event)`).
- `remote` (served by rikka-remote on `remote.sock`): `RemoteStatus { serving: Serving { port, addresses: Vec<Address { address, reach: Reach::{Local, Tailnet, Network} }>, fingerprint }, viewer (with `Encoding { codec: Codec, … }`, `Codec` displays as H.264/H.265), problem }`, `RemoteClient::{status, disconnect}`, `statuses(seam)`.
- `shortcuts` is the one catalogue: a `Shortcut` has a name, default chord, summary and `Group`. Adding one covers the compositor binding, shell matching and the Settings row.
- Shortcuts travel as kebab-case names. `is_published()` marks shell-performed ones.
- `surfaces` holds the layer namespaces the compositor treats specially: `OVERVIEW`/`DESK` hide windows, `SCRIM` (`is_scrim`) always blurs with the fixed scrim blur, faded in and out, in any layer, `INPUT_PANEL` (`is_input_panel`, the shell's IME candidate panel) is stacked `LayerOrder::Topmost` in its level.
- `Profile::{Full, Simple}` / `RIKKA_SESSION_PROFILE` is chosen by compositor backend selection. Consumers never infer it.
- `settings_app` is no seam protocol but the Settings app's command-line contract: `PROGRAM`, `Section`, `Spot`, `Link::{argv, from_arguments}`; the shell builds every Settings launch from it.
