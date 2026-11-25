---
title: Minecraft launcher (rikka-launcher)
description: apps/general/launcher: Modrinth-style launcher pinned to Rikka's Minecraft/Fabric version; crates, auth, install, Java, content.
about: apps/general/launcher
tags: [launcher, minecraft]
---

- `apps/general/launcher` builds `rikka-launcher` (Linux and Windows, component `launcher`). The only playable version is `src/target.rs`: Minecraft and Fabric Loader from `@minecraft//:versions.bzl` (the Rikka Client lock) via rustc_env; Rikka Client from Modrinth is not installed yet and would plug in there.
- Private crates, one way: `launcher-meta` (pure Mojang/Fabric/Java-runtime JSON: rules, `Version::inherit`, `ClientVersion::artifacts`, `expand` with unknown placeholders an error, checked `Sha1`/`RelativePath`), `launcher-modrinth` (API v2 client), `launcher-nbt` (NBT with MUTF-8), `launcher-instance` (instances on disk, content scan/hash cache, worlds, servers.dat, screenshots, logs), `launcher-install` (downloads, `Verify::{Presence, Contents}`, Java), `launcher-content` (identify/plan/apply/updates/migrate against Modrinth), `launcher-auth` (sign-in, accounts, skins), `launcher-game` (command line, process, log levels).
- Data: `<data>/rikka/launcher/game` (Mojang layout: versions, libraries, assets, runtimes, natives) and `instances/<id>/` (game dir plus `instance.json`, whose `content` map caches hashes and Modrinth ids keyed by enabled path; disabled files end in `.disabled`; data packs live in `saves/<world>/datapacks`). Config: `accounts.json`, `preferences.json`.
- `instance.json` is written only through `Instances::update(id, |config| …)` (locked re-read, change, save; `Instance::save` is crate-private), so long jobs merge just the fields they own (`content`, `game`, play time, options). App jobs use `services::{scan, update, blocking}` to keep disk work off async tasks. `GameTarget` (in launcher-meta) is both the install target and the recorded instance game.
- Sign-in uses the official launcher client id with the live.com device-code flow (rikka-oauth `DeviceRequest::field("response_type", "device_code")`), then a P-256 signed Xbox device token, SISU authorize, XSTS and `/launcher/login`, as Modrinth does. Device key and per-account tokens live in the keyring under `rikka_secrets::schema::MINECRAFT`.
- Java: Linux uses the shipped tree `lib/rikka-minecraft` (`java/` JDK + `lib/` native libraries, `RIKKA_MINECRAFT_PATH`, `nix/dist/minecraft-runtime.nix`, a `SHARED` flake tree) and sets `LD_LIBRARY_PATH`; Windows downloads Mojang's runtime component named by the version JSON. A custom java must have a `release` file of the required major.
- An instance whose recorded game differs from the target is migrated before launch: content updated through `version_files/update`, items without a version disabled and flagged `incompatible`. Updates never move a release file to a pre-release.
- UI state is one `Launcher` with plain fields; live game output goes through the `Logs` child state (untracked buffers + `Revision`, viewed with `component_with`) so log lines never re-view the window. Progress is throttled to 100 ms.
- Games run in their own process group (Windows: CREATE_NO_WINDOW) and outlive the launcher; Stop sends SIGTERM / TerminateProcess.
- Rikka Client still loads `librikka.so.0`, so the mod itself does not run on Windows yet.
