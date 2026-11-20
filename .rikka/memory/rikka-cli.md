---
title: The rikka command line
description: rikka-cli installs as `rikka`: clap noun-verb commands over seam and settings, --json output, install-time completions.
about: system/rikka-cli
tags: [cli, session]
---

- Package `rikka-cli` (binary `rikka-cli`) installs as `bin/rikka`. It uses only `rikka-seam`, `rikka-session` and `rikka-settings` and has no privileged path.
- The clap grammar lives in `crates/cli-command` (`Cli` plus one `*Command` enum per group) with no Rikka crate dependency. `crates/cli-completions` writes completion scripts from it.
- `//system/rikka-cli/crates/cli-completions:{bash,fish,zsh}` run the small `cli-completions` program at build time (which is why the grammar stays out of the Rikka-heavy crate); the base component ships the three scripts.
- Groups: `settings` (domain and key as separate arguments), `window`, `workspace`, `wallpaper`, `desktop`, `remote` (`status`, `disconnect`, `publish` over `rikka.remote`; status text gives tailnet guidance ending in `rikka remote publish`) and `seam` (`capabilities`, `call <cap> <method> [json]`, `watch [cap]`), plus `completions <shell>`.
- A command that needs one capability connects only to the socket serving it (`CapabilitySocket`, over `SocketClient::for_capability`), and the `CapabilitySocket` must outlive its `SeamClient`. Only `seam capabilities` and `seam watch` with no capability connect to every socket (`Session`). Typed commands reuse the rikka-session clients.
- Each command's output is a `Report` (Serialize plus a text form). The global `--json` switches to JSON, pretty on a terminal, and `watch` prints NDJSON.
- `wallpaper set` checks the id against the shell's library, then writes `rikka.appearance/wallpaper`; the shell follows the setting.
- Exit codes: 0 on success and on a closed pipe, 1 on a runtime error, 2 on a usage error (clap).
- Integration tests run the binary against real settings and `Recorder` services on sockets under a temporary runtime dir (`tests/support`).
