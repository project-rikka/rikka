---
title: GPU interaction boost (rikka-boost)
description: Root rikka-boost service locks NVIDIA clock floors via NVML while the compositor reports input or animation.
about: system/rikka-boost
tags: [compositor, power]
---

- Why: NVIDIA (open module 610/615) idles at P8 (210 MHz core, 405 MHz memory) and takes 200–300 ms or more to clock up, so desktop animations that start from idle (maximize with glass) miss 180 Hz frames.
- `system/rikka-boost` is a root, D-Bus-activated system service on `os.rikka.Boost1` (`/os/rikka/Boost1`), with one method `Hold(u32 milliseconds)`, clamped to `rikka_boost::service::MAX_HOLD` (2 s). `Hold(0)` only activates it.
- The first hold locks every NVML device's memory clock at its fastest supported clock and its graphics clock to [max/2, max]. It resets when the latest hold expires. It also resets at start, and `ExecStopPost=rikka-boost --lower` resets after a stop or crash.
- When NVML or an NVIDIA GPU is missing, the service exits successfully, so activation fails and callers stop asking. NVML loads through `rikka_nvml::load()` (`crates/status/rikka-nvml`: `libnvidia-ml.so.1`, else `/run/opengl-driver/lib`), the one loader rikka-sysmon and the UI benchmarks use too.
- No polkit: raising clocks grants nothing that running GPU work would not. Only root may own the name, and `tests/shipped.rs` checks that.
- Client: `rikka_boost::Booster::start()` (public crate `crates/daemons/rikka-boost`) runs a `rikka-boost` thread that touches once at start, so activation happens before the first interaction. `touch()` never blocks and sends a 1 s `Hold` at most every 250 ms. After a bus error it reconnects on a touch at most every 30 s, so a service installed mid-session is picked up. Whether the GPU is boosted is logged at info in the compositor log.
- The compositor's KMS backend holds one `Booster` (1 s hold). `Backend::interacted` touches it from `Compositor::person_active` (every person-seat key in `filter_key` and every pending input batch), so virtual input from rikka-remote counts like libinput. It is also touched on every frame that `Animations::needs_frame` the output or that portrays window views (`Scenes::portrays_windows`: switcher, overview, desk editor), because those animations are driven by the shell.
- It ships in the `system` contents of the `base` component (`xtask/src/components.rs`, `//system/rikka-boost:rikka-boost`), next to rikka-nixctl; the distribution's NixOS module installs its D-Bus and systemd files.
