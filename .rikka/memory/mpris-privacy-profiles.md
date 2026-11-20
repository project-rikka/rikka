---
title: MPRIS, privacy, power profiles and location crates
description: rikka-mpris (Players client, Publisher server), rikka-privacy (mic/camera/screen users), power-profiles, location.
about: crates/daemons
tags: [daemons, media]
---

- All four are public crates; the shell's `Services` starts them lazily, apart from its `Daemons` bundle.
- `rikka-mpris` keeps client and server in one crate so the metadata codec stays private. `Players` watches `org.mpris.MediaPlayer2.*` (skips `playerctld`), settles signals in 30–200 ms, rereads only the interface that changed, keeps the old position anchor while extrapolation still agrees, extrapolates `position_now()`, and picks `active`: last started playing, else most recent paused, never stopped. `control(id, Control)` / `control_active`.
- `Publisher::builder(program).identity(..).desktop_entry(..).serve()` claims `org.mpris.MediaPlayer2.<program>` (falls back to `.instance<pid>`) and returns a `Commands` stream. `show(Playback)` emits one batched PropertiesChanged and a Seeked when the position jumps > 1 s on the same track. Track ids become FNV object paths.
- rikka-music publishes through `src/mpris.rs`; `Player::announce()` runs after every playback-changing method and each engine `heard` event.
- `rikka-privacy` joins a PipeWire watcher (running `Stream/Input/Audio` that are neither `stream.monitor` nor `stream.capture.sink`; consumers linked to v4l2/libcamera `Video/Source` nodes; consumers of `rikka-screencast` nodes) with an inotify watch on `/dev/video*` that rescans `/proc/*/fd` (excluding PipeWire/WirePlumber and itself). Pids come from the node or its client's `pipewire.sec.pid`.
- PipeWire node info carries properties only when `change_mask` has `PROPS` (otherwise an empty dict); rikka-privacy and rikka-sound read them only then, so a state change never wipes `stream.capture.sink` or app names.
- rikka-cast stamps each cast stream with `rikka_privacy::CAST_APP`, naming the node `rikka_privacy::SCREENCAST`, (the portal's app id) so a nameless consumer is still attributed.
- `rikka-power-profiles` talks to `org.freedesktop.UPower.PowerProfiles`, falling back to `net.hadess.PowerProfiles` (also tuned-ppd); `set` is optimistic only for offered profiles.
- `rikka-location` is a GeoClue2 client; `Location::start(desktop_id, Accuracy)` uses `Monitor::start_with` to pass its config, and stops the client on drop.
- The installer's desktop module enables UPower, power-profiles-daemon and GeoClue2 with `rikka-shell` allowed as a system app.
