---
title: Animate liquid emitters (animate-fluid)
description: animate-fluid's GPU-free emitter model: Pour span, keyed settings, blob landing, ballistic aim, liquids, file kind.
about: apps/general/animate/crates/animate-fluid
tags: [animate, fluid]
---

- Private, GPU-free; depends only on `rikka-model-core`, `rikka-mmd`, `rikka-physics-liquid` value types, serde and thiserror; never runs the solver or renderer. The app maps its plain values onto them.
- `Emitter` is one timeline subject: `Shape` (Stream, Shower, Drip, Blob{size}, Surface{figure, materials; empty = whole figure}), `Aim` (Ballistic, Direction), launch and target `Spot`s, resting `Levels`, a `Formula` and keyed tracks.
- `EmitterSetting` (macro table: id, `EmitterSection`, `ValueKind`) names each track: rate, volume, launch, target, speed, arc, width, spread; values are `EmitterValue`, resting levels `EmitterLevels`. `EmitterSetting::applies(shape, aim)` decides the rows; keys of settings that do not apply are kept but hidden from `keyed()` and `last_frame()`; the keys themselves are `tracks()`/`tracks_mut()`.
- `Emitter::pour` (`Pour`: start frame in `Pour::STARTS` + length in `Pour::LENGTHS` (1 frame to 10 min), default `Pour::LENGTH` = 2 s) is the only on/off: flowing shapes pour in [start, end), a Blob lands once at `start` (`lands_between(after, through)`, start a run with `f32::NEG_INFINITY`) and ignores the length. `stops()` (end, or start for a Blob) counts in `last_frame()`; `litres()` integrates the rate over the span (closed form for an unkeyed rate, Simpson per key segment otherwise; a Blob's volume). There are no switch keys.
- `Pour::new` and its edits clamp into range (`moved_to` keeps the length, `starting_at` keeps the end, `ending_at`/`lasting`/`lasting_seconds` set the length); `Pour::checked` refuses out-of-range file values. `can_start_at`/`can_end_at` gate the playhead buttons.
- `EmitterState::set(setting, value)` overrides one sampled value (the app's unregistered values).
- `Throw`, `Liquid` and `Preset` live in rikka-physics-liquid and are re-exported here. `EmitterState::throw` aims `Throw::ballistic` and falls back to `Throw::direct` at the speed; Direction aims from launch toward the target.
- `Property` is the table over `Liquid`'s fields (UI kinds and ranges), `Formula` a preset plus overrides (custom when any).
- `FluidSettings` is the scene value: particle size (diameter; `radius()` for the solver), capacity, `Wetness` (`drying_rate()` per second, 0 when off), `Gravity` (Earth 9.81 down by default).
- `Emitter::changed_from(&before)` names the first frame an edit affects (the key before the first differing key; the earlier differing pour edge; 0 for structure, resting values or liquid; `None` for a rename) so the app invalidates only from there.
- `EmitterFile` is the `.rmotion` content of kind `fluid`; the kind tag and envelope are in animate-document (`Motion::Fluid` holds the `Emitter`, id 0 until the app assigns one). Loading validates ids, kinds and ranges.
