---
title: Weather client and the shell's activities and moments
description: rikka-weather (Open-Meteo, geocoding, cache); shell-activity derives Activities and events; shell Feeds hold them.
about: crates/status/rikka-weather
tags: [weather, shell]
---

- `crates/status/rikka-weather` is public. `system/rikka-shell/crates/shell-activity` is shell-private and offers `ActivityMonitor::start(Sources)`, `Activities`, `Activity`, `ActivityEvent`.
- The shell's `shell_feeds::Feeds::follow` (declaring `rikka.weather`) builds `feeds/services.rs::Services` over the shell's existing `Daemons`, so PipeWire and D-Bus are not opened twice; it lazily starts MPRIS `Players`, `Machine`, `Weather` and the `ActivityMonitor`. The app id is GeoClue's DesktopId.
- `ActivityMonitor` runs one `rikka-activity` thread that subscribes first, then takes baselines, so the first look at any source is never a moment.
- `pulse.rs` is the pure diff engine: volume/output/mic mute, brightness, charger, UPower warning levels, charged, peripheral low, Bluetooth connect, track/playback of the active player, power profile, sensor start/stop, online/offline, VPN.
- `activity.rs` orders ongoing state: sensors (screen > camera > microphone), the active player, charging or low battery. The taskbar island adds its own (recording, timers) and re-ranks; desk widgets read `Services` too.
- Weather: Open-Meteo forecast (current, 48 h, 10 days, `timeformat=unixtime`, m/s wind) plus air quality; coordinates rounded to 0.01°. Reports carry `ATTRIBUTION`, which UI must show.
- `Spot::Here` uses `rikka-location` (GeoClue, city accuracy, 5 km re-fetch threshold) and Nominatim reverse geocoding, which is cached per rounded spot and language; the last "here" place is cached to show at once.
- The disk cache `$XDG_CACHE_HOME/rikka/weather` is a `rikka_net::DiskCache` shared across processes (entries unused for 30 days are pruned): a forecast younger than 15 min (air 60 min) is reused instead of fetched. The forecaster sleeps on a `rikka_worker::WallAlarm` (via tokio `AsyncFd`) until the next refresh, so it never polls and a refresh missed in suspend runs on resume; failures back off 30 s → 15 min.
- `Weather::builder(app).spot(..).online(stream)`: `Services` feeds NetworkManager connectivity (Full = online, Unknown ignored); offline holds refreshes and regaining it retries a failing one at once.
- `weather::search(name, language)` asks Open-Meteo geocoding, then Nominatim search when that is empty (CJK short names). Call it on submit, never per keystroke (Nominatim policy).
- Quantities are canonical (°C, m/s, mm, hPa, m) newtypes with `.in_unit(unit)`; `Units::for_locale()` follows `LC_MEASUREMENT`, `Units::from_settings(..)`/`Spot::from_setting(..)` map the `rikka.weather` settings catalog (`rikka_settings::session::weather`), which the shell feature that shows weather must declare.
