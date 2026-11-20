---
title: Animate wind subject (animate-wind)
description: animate-wind's keyed WindMotion (yaw, pitch, speed, turbulence, gust), .rmotion kind wind, the Wind subject and vane.
about: apps/general/animate/crates/animate-wind
tags: [animate, wind]
---

- Private, GPU-free; `WindMotion` holds resting `WindLevels` plus per-`WindSetting` `WindKey` tracks (motion frames, `VmdCurve` of the key led to); `sample(frame)` returns `rikka_physics_tailor::Wind` (re-exported as `animate_wind::Wind`).
- Direction convention: yaw 0 blows toward -Z (from the front, away from the default camera), positive yaw turns counter-clockwise seen from above, positive pitch blows upward. Yaw keys interpolate the short way round.
- `WindFile` is the `.rmotion` body of kind `wind`; `animate_document::Motion::Wind` wraps it. Loading checks ids and `WindSetting::range()`.
- Keys live in `WindMotion::tracks()`/`tracks_mut()` (no forwarding methods); `value_at` falls back to the resting level.
- App: one scene-wide `Subject::Wind` after Camera and Look; `Lane::Wind`/`GroupKind::Wind` (one open group), `Key::Wind`, `SubjectMotion::Wind`, `Edit::Replaced` snapshots for resting values.
- Editing is the generic keyed-settings code of `animate/settings.rs` (`SettingSubject` for `stage::Blowing`, shared with emitters): unkeyed settings change resting levels at once, keyed ones wait in `Drafts::wind` (one frame) until Register.
- `Stage::wind_at(time)` (keys plus unsettled values) is the hook for the cloth integrator; `animate-stage` `wind/` publishes the floor-level vane, which `animate-viewport` `wind/` draws under the view target while Wind is chosen, its tip dragged to set yaw by a top-view screen mapping.
- Inspector: `animate-inspector` `inspector/wind/` card with a `Compass` (top view, front at the bottom) and pitch/speed/turbulence/gust sliders.
