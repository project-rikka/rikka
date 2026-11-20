---
title: Krisp from Discord's own module (discord-krisp)
description: discord-krisp downloads Discord's official discord_krisp module and drives the raw Krisp C SDK it exports.
about: apps/system/discord/crates/discord-krisp
tags: [discord, voice]
---

- Linux `discord_krisp.node` exports the raw Krisp SDK (`krispGlobalInit`, `krispCreateNcFloat`, `krispProcessNcFloat`, `krispCreateVadFloat`, `krispProcessVadFloat`, destroy calls). Discord's MD5 self-check sits only in its wrapper (`DoKrispInitialize`), so no code is patched.
- ABI (`abi.rs`, reverse-engineered from `KrispNCSetup2`/`KrispVADSetup2`): NC config `{in_rate, frame_ms, out_rate, *ModelInfo, stats: bool, ringtone*}`, VAD config `{rate, frame_ms, *ModelInfo}`, `ModelInfo {wchar path, blob, size}`; process float takes 480 samples in [-1, 1] plus a suppression level (100).
- `krispGlobalInit(working, log, 0)` needs a writable working folder (it creates `KMS/logs`); the module folder may be the read-only Nix store.
- `Module::from_folder(folder)` picks the full-band NC model (`krisp-nc-o-med-*`, never `-nb-`/`-lite-`) and the newest `krisp-vad-*`. Installed Discord clients are never searched.
- `Module::fetch(cache, progress)` reads `updates.discord.com/distributions/app/manifests/latest` (linux x64), returns `cache/<host>-<module>` when present, else downloads `modules.discord_krisp.full` (brotli tar, `files/…`, SHA-256 checked) and keeps only the `.node` and `.kef` files. `Module::cached(cache)` serves the newest complete release; `prune_others` removes older release folders and hidden partial downloads.
- `Sdk::load` dlopens once per process and refuses a different module folder afterwards (`Error::AlreadyLoaded`); discord-voice opens Krisp sessions on a helper thread because they read model files.
- `nix/packages/discord-krisp.nix` pins one release laid out like a cache (`RIKKA_DISCORD_KRISP/<host>-<module>/`); tests read it with `Module::cached` or symlink its releases into a temporary cache, so the background refresh finds the release and downloads nothing.
