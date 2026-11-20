---
title: Remote desktop viewer (rikka-remote-viewer)
description: apps/general/remote: native QUIC viewer: hosts list, sign-in, GPU decode into a Feed, key passthrough, escape chords.
about: apps/general/remote
tags: [remote, app]
---

- `apps/general/remote` builds `rikka-remote-viewer`; private crate `viewer-hosts` (`Target::parse`, `Hosts` JSON at `$XDG_CONFIG_HOME/rikka/remote-viewer/hosts.json` with pinned fingerprints).
- UI (`viewer.rs` hosts list, `live.rs` phases Connecting/Password/Streaming/Changed/Ended, `screen.rs` the `Screen` widget). Remembered sign-in tokens live in the keyring under `rikka_secrets::schema::REMOTE`, keyed by the host pin.
- `session/` runs on the `rikka-remote-viewer-link` `rikka_worker::Worker` with a current-thread Tokio runtime and the client endpoint; dropping the `Link` wakes a `Notify` that ends the session, the endpoint is closed and given up to 500 ms (`CLOSING_PATIENCE`) to tell the host, and the thread is joined. Signing out flushes `SignOut` on the control stream (finished and acknowledged, same bound) before closing. `network.rs` (handshake, datagrams → `Assembler`/`Chain`, frames of a stream not yet announced wait for its Stream message, feedback every 25 ms and on loss, input and file streams), `video.rs` (a `Worker` over a command channel, at most 4 queued frames; GPU decode via `rikka_video::GpuLiveDecoder` on the renderer's device from `Feed::wait_for_device` into `Feed::show_video`, software fallback through `Feed::show`), `sound.rs` (a `Worker` whose stop also stops the output: a `rikka_playout::Playout<OpusDecoder>` fed with each packet and its redundant previous frame, paced by `QueueTarget` with an untimed `AudioFeed::wait_until_queued_below`), `clipboard.rs` (a reader `Worker` fed by the clipboard watch through a command channel).
- `computer/` (unix: `uname` nodename, windows: `GetComputerNameExW` physical DNS host name) supplies the Hello `device`; `platform` is `std::env::consts::OS`.
- `Screen` grabs keys (`Behavior::GRABS_KEYS` → shortcut inhibitor) and sends evdev codes; Super, Alt+Tab etc. reach the remote. Escape chords: Ctrl+Alt+Shift+M pointer lock, F fullscreen, S stats, Q leave. Remote cursors become hardware cursors (`rikka_seat::define_cursor`); while locked the widget draws the shape at the server's pointer position.
