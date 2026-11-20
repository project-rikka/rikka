---
title: Physics World facade (rikka-physics-world)
description: rikka_physics_world::World is the rigid-only facade: fixed ticks, interpolation, seek/rebuild history, async steps.
about: crates/physics/rikka-physics-world
tags: [physics, facade]
---

- Rigid-engine users depend only on `rikka-physics-world` (no product uses it yet; `rikka-physics` does not depend on it): lib.rs re-exports rigid, geometry, collision, math and core user types. Cloth lives elsewhere (GPU `rikka-physics-cloth`); the world has no cloth, wind or simulation spaces.
- New public types in rikka-physics-rigid need a line in the matching re-export of `src/lib.rs`.
- `World::builder()` defaults: 120 Hz, 2 substeps, 1 worker (`SerialExecutor`), frame budget 0.1 s → `max_ticks = round(budget·rate)`; `.game()` is 60 Hz × 4.
- Threading is one setting: `.workers(impl Into<Workers>)` takes a count, `Threads::new(n).named(..)` or a shared `Arc<dyn Executor>`; rikka-physics-core `Workers::build` picks serial for 1 and a pool otherwise.
- `advance(frame_dt)` plans ticks in `Clock` (accumulator, f64 tick grid, dropped time when over budget); `advance_to(motion_time)` aligns ticks to the grid without the budget (video export).
- Per tick (`world/tick.rs`): kinematic targets (`Targets`, timed as `bone targets`) → `RigidWorld::step` → write-back (body poses from move events, contact/joint events, tick hash when recording or replaying).
- `Targets` finds a body's target through a slot index by body index, so `move_to` is O(1).
- `pose(id)` lerps the last two tick states (`BodyPoses`, from body move events); `latest_pose` is the newest tick.
- `History` keeps every creation def (Body, ShapeDef, JointDef, Character); each logged call has a `Scope`: Structure (always kept: destroys, gravity, origin, materials), State (kept before the first tick), Input (targets, impulses, advance: never kept). `reset`/`seek` rebuild the rigid world from it, so ids and state reproduce bit-exactly.
- `seek(time, preroll, animate)` resets (rebuild), restarts the clock on the tick grid, snaps targets, then pre-rolls with `advance_to`; `animate(world, t)` supplies targets. Reset, restart and snap are logged as calls, so seeks replay.
- `Recording` starts before the first tick (`start_recording` copies the history so far) and holds the world `Settings`, creation defs (rigid codecs for Body/ShapeDef/JointDef), calls and per-tick hashes. Everything encodes through `rikka_file_format::Codec`; `Recording` itself is the value of `BinaryFormat` `PHYSREC\0` (v1, changed in place), facade types use `Error` as `Codec::Error`, and header problems surface as `Error::Recording`. `Recording::replayer()` builds its own serial world; `Replayer::world()` allows perturbing between frames.
- Characters are World-owned (`create_character(CharacterDef)` → `CharacterId`, kept in a rikka-physics-core `Pool`): the def is a Setup, `move_character` an Input call that runs `Character::update` and `push_bodies` immediately, `set_character_position` a State call; `debug_draw` outlines them as capsules.
- `set_profiling`/`profile()` merge rigid stage timings with facade phases.
- `state_hash` (rikka-physics-core `Fnv1a`) = rigid `state_hash` + character positions and velocities.
- `AsyncWorld` moves the boxed World to a `rikka-physics-step` thread over `sync_channel(1)` and double-buffers `Snapshot`s (time, hash, body poses, events).
- `tests/allocation.rs` counts heap allocations of steady frames (moving kinematic head, alternating `advance` and `advance_to`) with a global allocator that counts only on the test's own thread (the default world steps serially there), so it runs beside the other tests of the one binary.
- `benchmarks/physics` drives `RigidWorld` directly; it does not use the facade.
