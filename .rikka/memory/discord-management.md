---
title: Discord server management (menus, admin views, moderation, MFA)
description: rikka-discord's context menus, server/channel settings views, moderation, MFA retry and their client/core APIs.
about: apps/system/discord
tags: [discord]
---

- Core vocabulary lives in `discord-core/src/manage/` (`SpaceSettings`/`SpaceEdit`, `RoleEdit`, `MemberEdit`, `ChannelSettings`/`ChannelEdit`/`Overwrite`/`Grant`, AutoMod, onboarding, welcome, invites, webhooks, audit, `MfaChallenge`, `SpacePrivacy`) plus `model/permissions.rs` (`Permissions(u64)`, Discord bit names) and `model/notify.rs` (`SpaceNotify`, `ChannelNotify`, `MuteSpan`, `NotifyChange`).
- `discord-state` computes `Space::{permissions, notify, features, premium_tier}` and `Channel::{permissions, notify}`; `refresh(guild)` re-emits spaces too, so role/member/settings changes update menus.
- `discord-client/src/manage/` holds one `impl Client` file per area (space, roles, members/bans/prune/audit, expressions, hooks/invites/integrations, community, channels, notify, preloaded, mfa); `Client::work` wraps `session.work`.
- `Request::reason` sends `X-Audit-Log-Reason`; `Request::form` sends multipart (stickers). A 401 with code 60003 becomes `Error::Mfa(MfaChallenge)`; `Client::finish_mfa` stores a 4.5-minute elevation token that every later request sends as `X-Discord-MFA-Authorization`.
- Settings-proto edits keep unknown fields: `Preloaded::privacy` is raw tag-8 bytes edited through `discord-wire/src/proto.rs` `Fields`; folders round-trip `GuildFolders` incl. `guild_positions`. Patches are absorbed back through `Session::absorb(event, json)`.
- App: `menus/` (`Menus`, `Subject::{Space, Folder, Channel}`) opened by right-clicking rail tiles, the sidebar header (also a click) and channel/category rows via `Chat::open_context_menu`.
- `manage/` (`Manage` state) owns modal dialogs (`Dialog` enum), confirm prompts with boxed actions, the MFA dialog and failure alerts; `manage::overlays(chat, stage)` renders them in the desk and in both admin views.
- `Chat::act(key, task, done)` / `attempt(..., settled)` run a re-runnable client task; on `Error::Mfa` they queue a retry and open one MFA dialog, then rerun every queued task after verification (`settled(None)` on cancel or failure).
- `space_admin/` (`SpaceAdmin`, full-window like Settings) has one file per section; drafts are `Edited<T>` and the shared `manage::unsaved_bar` saves/resets and blocks leaving while dirty (unless the server is gone).
- Loads go through `Chat::fetch(task, store)`, which drops results for a space no longer open; store closures also check the request is still current (selected role, audit filter, search text). `Chat::save_edited(slot, task)` saves an `Edited` draft and moves only the sent value into `original`, keeping edits made meanwhile.
- `channel_admin/` (`ChannelAdmin`) edits overview and permission overwrites (tri-state per permission from `permission_catalog`), invites and webhooks; saving diffs overwrites into PUT/DELETE calls.
- Boost status, subscriptions, insights and the App Directory open in the browser (`Chat::open_page`).
