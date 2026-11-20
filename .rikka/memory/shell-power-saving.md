---
title: Shell power saving (idle screen off, sleep, dimming, power saver)
description: shell-power-saving hosts rikka.power: idle dim, display sleep, suspend, low-battery saver, taskbar percentage.
about: system/rikka-shell/crates/shell-power-saving
tags: [shell, power]
---

- `PowerSaving` declares the `rikka.power` domain (`rikka_settings::session::power`) and is installed before the taskbar, which follows its `battery-percentage` key into `bar::status::Percentage`.
- `Policy` owns three `Shell::idle()` timers that respect idle inhibitors: dim (30 s before screen off, never under 15 s; restores the brightness on resume), screen off (`rikka_session::display_power::sleep`; the compositor wakes on input) and suspend (logind `Suspend` when allowed). Minutes come from the plugged or battery key by `on_battery`; 0 disarms.
- `Policy` is held in the shell `Host` (`Shell::hold`); the service, the settings followers and the idle callbacks reach it through its `Slot`, so it has no shared cells.
- A `rikka-power-saving` thread forwards battery and power-profile changes to the local `rikka.power-saving/review` service, so the policy runs on the main thread.
- Low battery (on battery and UPower `Warning::Low` or worse) switches to power saver when `saver-on-low-battery` is on, remembering the replaced profile and restoring it on recovery; a profile the person picks meanwhile is kept.
- Lock-on-idle stays in the lock feature (`rikka.lock/idle`); Settings' Power page edits this domain.
