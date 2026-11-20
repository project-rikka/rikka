---
title: Discord client (discord-client)
description: discord-client's Discord/Link/Client/Call API over the sans-IO discord-wire/translate/state/rate-limit crates.
about: apps/system/discord/crates/discord-client
tags: [discord]
---

- Public API: `Discord` (`open(cache)`, `resume(folder)`, `sign_in(folder, Method, Prompt)`) → `Link { account, client, updates }`; `Client` (cheap `Clone`, inherent async methods, `limits`, `mint`, `focus`, `view_roster`, `join_voice` → `Call { channel, line: Line, events }`); sign-in vocabulary `Method`, `Prompt`, `SignInStep`, `QrStep`, `WebSignIn`; `writing::{mention, emoji, message_link}` for outgoing markup.
- Every future runs on one process-wide lazy two-worker `rikka-discord` Tokio runtime (`runtime::handle`, `runtime::run`), so callers may await from any executor.
- `resume` answers `Ok(None)` when Discord refuses the saved token; the token is kept in the keyring under `discord_core::account_secret(folder)` (schema `os.rikka.Discord`), and `Accounts` removes it with the account folder.
- The IO-free layers are app-private sans-IO crates, one way: `discord-wire` (flat serde payload types with tolerant `flex` fields, `Preloaded` settings protobuf, gateway `Frame`/`Hello`/`ReadySession` and `Closing::from_code`, `commands::*` building outgoing op payloads as `Value`, `snowflake::{value, id, millis, newer}`) → `discord-translate` (wire → discord-core: `message`/`Reader`, `channel`/`Looks`, `profile`, `avatar` and the CDN rules; markup through `discord-markdown`) → `discord-state` → `discord-client`. `discord-rate-limit` (`Route` templates, `Limiter`, `Headers::read`, `Throttle::read`, `Scope`) stands alone. Each has table tests on captured-shape payloads in its `tests/`.
- `discord-markdown` (`parse`/`parse_line` → `Rich`) follows simple-markdown's precedence; Unicode emoji are `rikka_text_unicode::emoji_sequence` keycap/modifier/flag/tag/ZWJ sequences, `basic_emoji` single characters, plus lone regional indicators (Discord's `:regional_indicator_x:`).
- Internal state keys stay wire strings; `snowflake::id` converts at the boundary.
- `Session` is the live account: state lock, gateway, voice, feed; `Mint` (`mint.rs`) makes strictly rising nonces so two sends in one millisecond never collide.
- `discord_state::State` is a pure machine with no clock: `State::new(Moment)`, `dispatch(event, json, Moment)`, `tick(Moment)`, `adopt(.., Moment)`, `own_presence(Moment)` return or read `Outcome`s of updates, gateway commands and voice/stream `Signal`s; the client passes `Moment::now()`. READY is held until READY_SUPPLEMENTAL or `SUPPLEMENT_WAIT`.
- Rules live in small private modules under `discord-state/src/state/`: `permissions`, `reads`, `notify`, `roster` (GUILD_MEMBER_LIST_UPDATE, list id via murmur3, op 37 ranges), `looks` (mutes, timeouts).
- `gateway/` owns one connection task (zlib-stream, op 40 heartbeats, resume, backoff). `rest/` adds identity headers, the `discord-rate-limit` limiter, status mapping and uploads.
- `voice.rs` turns own voice-state/server signals into `discord_rtc::Transport` sessions; `Line` forwards Opus, speaking and self mute/deaf; one call per account.
- `stream.rs` owns Go Live: `Client::go_live(channel)` (op 18 + op 22, must sit in that call) → `Broadcast { key, line: BroadcastLine, events }`, `Client::watch(StreamKey)` (op 20) → `Watch { key, line: WatchLine, events }`; both wait for STREAM_CREATE + STREAM_SERVER_UPDATE (`StreamSignal` in the state `Outcome`), open a stream `Transport`, reopen on server failover, forward viewers/paused into the events, and end on STREAM_DELETE with a reason (`stream_full`, `unauthorized` → `Denied`). Leaving voice ends every stream with op 19.
- `upload_preview(key, jpeg)` (POST data URI) serves stream thumbnails; `Abilities::STREAM` is permission bit 9.
- `Client::profile(user, space)` asks `/users/{id}/profile` (popout, mutual servers and friends for others) and resolves server overrides (bio, pronouns, banner, accent, theme colours) into one `Profile` with CDN `Badge`s. Notes are `note`/`set_note` (404 reads as empty); `request_friend` sends a request by user id.
- Presence `Activity` carries details, state, `started` and `art` (`cdn::activity_art`: spotify:, mp:, youtube:, twitch: prefixes or numeric app assets).
