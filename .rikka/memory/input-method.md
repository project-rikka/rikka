---
title: Fcitx5 input-method integration
description: rikka-ime runs and configures fcitx5, hosts kimpanel data, and shares caret and keyboard-source types.
about: crates/daemons/rikka-ime
tags: [ime, fcitx5]
---

- fcitx5 owns composition, engines, protocol grabs, the active input method and the switch shortcut. Rikka stores none of these.
- Rikka owns the candidate UI and the seat keymap sources.
- `Panel` hosts `org.kde.impanel` (preedit, candidates, spot) and exposes `Composing` to the shell. `Fcitx` edits engines and groups via `org.fcitx.Fcitx.Controller1` for Settings.
- `Panel::composing()` and `status_area()` return `Arc`s replaced only when that part changed, so callers compare by pointer. The status area keeps every kimpanel property, the `/Fcitx/im` input-method list and the last `ExecMenu`. `trigger(key)` emits `TriggerProperty`; an `ExecMenu` listing only `/Fcitx/im/*` entries is the method list, any other answers the last asked key. `fold()` drops it.
- `Composer::ensure_running` starts the profile's fcitx5 and replaces stale or foreign instances. fcitx5 autostart is masked. The full shell settles again when profile links change.
- The compositor merges `rikka.ime.sources` layouts into one keymap of at most four xkb groups.
- Wayland IM has no caret forwarding, so the shared `Caret` types back the compositor's `rikka.caret`. Its `aim` method answers a `PanelAnchor`: the focused caret, else the pointer, with the output name.
- Candidate placement tries the compositor caret, then the kimpanel spot, then a safe default. The panel ignores the pointer.
- X11 clients use fcitx5's XIM (`XMODIFIERS=@im=fcitx`). The crate links no engine and owns no window.
- Signals arrive through `rikka_bus::Signals::each` on the panel's own connection (the `Watch` lives in `Panel`); `Fcitx::changes()` is a stream that owns its connection and watch.
