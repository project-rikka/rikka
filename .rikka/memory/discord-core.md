---
title: Discord model and store (discord-core)
description: discord-core is rikka-discord's model: snowflake ids, Update stream, Store, Accounts book, voice events.
about: apps/system/discord/crates/discord-core
tags: [discord, architecture]
---

- No UI crates: plain data plus `Store`; its Rikka dependencies are `rikka-account-book` (the account book it plugs `SavedAccount` into) and `rikka-secrets` (schema). `discord-client` produces `Update`s; the GUI applies them.
- Ids (`SpaceId`, `ChannelId`, `MessageId`, `UserId`, `RoleId`, `EmojiId`, `StickerId`, `TagId`) are `Copy` u64 snowflakes: `new`, `get`, `created_at`, `FromStr`, `From<u64>`; serde writes strings and reads strings or numbers.
- Messages sort by `MessageId`; there is no separate order. `MessageId::minted_at(ms)` makes a nonce-shaped id; `DISCORD_EPOCH_MS` is the one epoch constant.
- A pending send is a `Message` whose id and `nonce` are the `Draft.nonce` the client minted; the echo replaces it by nonce. `Store::pend(channel, draft)`, `fail(channel, nonce)`, `withdraw(channel, nonce)`.
- `Store::apply(Update) -> Vec<Area>` owns state rules (rail from folders, sections per space, timelines capped at 1000 with pending/failed sends, reactions, typing expiry, unread with mutes, voice states, calls). Areas are as narrow as views need.
- Requests vocabulary lives in `requests.rs`: `Draft`, `Page`, `Focus`, `Limits`, `Upload`, `ForumPage`, `PostQuery`, `Progress`.
- `Channel::topic` is parsed `Rich`; forum and media channels carry `Channel::forum: ForumSettings` (tags, default `PostSort`/`PostLayout`, `TagMatch`, required tag, default reaction) and threads carry `Thread::pinned`.
- Voice: `VoiceEvent`/`VoiceSink`/`VoicePacket`/`Privacy` plus Discord's fixed format constants `SAMPLE_RATE`, `VOICE_CHANNELS`, `VOICE_FRAME`, `VOICE_BITRATE`.
- The app keeps a `rikka_account_book::Accounts<SavedAccount>` book (`accounts.json`): `SavedAccount { key, account: Account { id: UserId, name, avatar } }` is its `Record`, one 0700 folder per account; signing in to the same user again replaces it; forgetting the current account makes the first remaining one current. `account_secret(folder)` is the account's `FolderSecret` (schema `os.rikka.Discord`, no kind). `Emoji::custom(id, name, animated)` builds a custom emoji with its CDN picture.
- `Error` is the public failure vocabulary; `Refused` means Discord signed the account out.
