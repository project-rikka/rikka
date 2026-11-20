---
title: Discord application GUI (rikka-discord)
description: apps/system/discord/src: Chat root state, per-account LinkedAccount stores with Area→Revision marks, voice, overlays.
about: apps/system/discord
tags: [application, discord]
---

- `rikka-discord` is a Discord-only client; there is no adapter layer. Layers: `discord-core` ← `discord-wire` ← `discord-translate` (+ `discord-markdown`) ← `discord-state` ← `discord-client` (also over `discord-rtc` ← `discord-rtp` and `discord-rate-limit`) ← `discord-voice` (+ `discord-krisp`) ← `discord-stream` ← the program.
- Paths: cache `$XDG_CACHE_HOME/rikka/discord` (handed to `Discord::open`), state `$XDG_STATE_HOME/rikka/discord` (the `Accounts` book); pictures cache in `cache/discord/pictures`.
- `main.rs` builds one `Arc<Discord>`; `Chat` keeps it untracked and calls `resume(folder)` / `sign_in(folder, Method, Prompt)` from spawned jobs.
- `Chat` (`src/chat/`) holds only tracked, untracked and `#[state]` fields, so handlers on `&mut Chat` never re-view the whole window. Orchestration lives in `chat/*.rs` (`linking`, `signing`, `nav`, `me`, `calls`, `people`, `alerts`, `configure`).
- Several Discord accounts stay linked at once; `current` is the shown one, `pending_account` the one still connecting. Each link streams its `Updates` into `Chat::apply_update(key, update)`; `account::Marks::invalidate` bumps the `Revision` of each changed `Area`.
- `account::LinkedAccount` owns one account's `Store`, its concrete `discord_client::Client` and location (`Location { scope: Scope::Home|Space, channel }`, kept by `account::Navigation`). Reads go through `LinkedAccount::watching(areas)`; writes through `LinkedAccount::change` or `apply`.
- Sign-in (`signing/`): `Method::ALL` (QR code, browser) as a welcome choice, then tabs; `add_account`/`sign_in_again` start QR directly. The browser method's `Portal` wraps a `rikka_web::SignInPortal` (one private `Session` of its own ephemeral `Runtime`, shared with Music's cookie portal): a page-world user script (`discord_client`'s, posting to `window.rikka.messageHandlers.rikkaDiscordToken`) captures the token, new windows load in the same session, engine exit fails the attempt, and dropping the portal closes the session and engine.
- Brand mark and accent come from `brand.rs` (`BrandSymbol::DISCORD`, blurple).
- Voice: `voice::Calls` holds the one `discord_voice::Voice` engine plus seat (account key + channel); a call survives account switches. `Chat` owns the process's `discord_voice::Krisp` (cache `cache/discord/krisp`) and tracks its `KrispState` for the voice settings.
- Screen sharing: `screen::Screens` (`#[state]` on `Chat`) holds the Go Live `Setup` popover, `Sharing`, `Viewing`, the stage flag and the untracked `Broadcaster`/`Watcher`/`Feed`s; handlers live in `chat/screens.rs`. Going live is portal pick (`spawn_blocking`) → `client.go_live` → `Broadcaster::start`; leaving voice ends both.
- Watching starts from a LIVE badge, the member menu or the voice panel's stream chips; if the person is not seated it joins the call and watches once `VoiceUpdate::Connected` arrives. `desk::region` shows `screen::stage_view` instead of the conversation while the stage is open.
- Preferences `rikka.discord` (`src/preferences.rs`): folded categories (`ChannelId` sets) and user volumes (`UserId` maps) per account key, voice settings, push-to-talk key, notifications, appearance, stream resolution/frame rate/whole-system sound.
- The server rail (`rail.rs`) is a named, uniform `lazy_column` of owned `Line`s; an open folder's members are their own rows carrying a band piece, so riders slide them out from under the folder row like ADE's explorer.
- Rail tiles wrap in `widgets::pointing`: hover chrome follows `Pointing::is_over()`, but the name tooltip shows only at `Pointing::Hovered` (the pointer itself moved), so a scroll hides it and rows sliding under a still pointer never raise one.
- User-facing text says "server" for guilds; the model keeps the names `Space`/`SpaceId`.
- Profiles (`src/profile/`): `Profiles` (`#[state]` on `Chat`) holds the anchored popout `(ProfileTarget, Anchor)` and the full profile `(ProfileTarget, Tab)`; `Look::resolve(chat, target, Surface)` resolves the user, member, roles, presence and the fetched `Profile` once per surface, and `Look::mark()` keys every part so the popout and DM panel of one user never share keys.
- The popout (member rows, authors, mentions) is a flush `popover` with banner, ringed avatar, mood bubble, badges, bio, activity, member-since, roles, mutuals and a "Message @handle" box that opens the DM and sends; the DM side panel is `profile::panel`.
- The full profile is a `sheet` centred on `desk::BODY` with a scrim layer (`profile::scrim`) in `desk()`; tabs About / Mutual Servers / Mutual Friends, connections and a note saved on blur, submit or close (`Chat::save_note`).
- `Chat::{open_profile, open_full_profile, close_profiles, message_user, send_from_profile}` live in `chat/people.rs`; `conversation::rich_view` renders a `Rich` (bios) outside messages.
