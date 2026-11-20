---
title: Animate body fitting and motion style (animate-fit, intensity, jiggle)
description: animate-fit removes limb-body intrusions from baked motion; animate-motion adds intensity shaping and bust jiggle.
about: apps/general/animate/crates/animate-fit
tags: [animation, collision]
---

- `animate-fit` (GPU-free, below animate-motion) builds a `Physique` from `Body` + `rikka_physics_tailor::Body::from_model(model, analysis)` (welded Body-role skin mesh, no colliders), so only skin and tight clothing collide, never cloth pieces or hair.
- Obstacles are radial `Shell`s (slices × 32 sectors of max radius around the bone axis) for Hips/Spine/Chest/UpperChest/Neck/Head/upper and lower legs, each limited to its bone segment; breasts fold into the chest shells. Convex tailor colliders were too loose (they fill the underbust and waist).
- Probes are decimated skin vertices of upper arms (outer 60 %), forearms, palms (no fingers) and lower legs, rigid in their human bone's rest frame, grouped per bone into culling `Cluster`s; pairs touching at rest are spared. `Region::avoids` is the pair table; thighs never probe (pressing thighs is natural flesh) and shin contact allows `FLESH_GIVE` because two max-radius envelopes overlap before the meshes do.
- `Physique::fit(body, first, unfitted, wanted)` is a space-time optimisation: each frame is scanned once, then overlapping windows (64-frame blocks ± 16, aligned to absolute frames so partial rebakes match full bakes) each solve legs (two knee swivels, feet kept) then each arm (muscle-space upper-arm bend/side/twist + elbow) jointly over all window frames by Gauss–Newton on a banded system (`band.rs`): penetration rows (known intrusions use max of path-exit plane and radial depth), a soft wrist anchor (strong when pinned), keep, velocity and acceleration terms. Windows are smoothstep-blended and tiny corrections shrink to exactly zero.
- Never solve frames independently or re-solve after smoothing: per-frame solves make jitter and odd elbows. `Physique::REACH` is the context (frames) a partial rebake needs on both sides; `Physique::reaching(worlds)` gives forearm/palm points the bust jiggle avoids.
- `animate_motion::Automation` holds `fitting`, `jiggle` (0..1) and `Intensity` (size, snap, torso, head, arms, legs, bounce, 0..2); loading refuses values outside their ranges.
- `MotionBake::new(body, physique, motion, eye)`: sample → `Intensity::shape` (slow/fast box-Gaussian bands around a 0.8 s average; muscles, root sway, bounce, root turn) → evaluator → fit (raw pre-fit poses kept per frame as rebake context) → `Jiggles` (VRM-like tip springs on `Body::lobes()`, checkpointed per chunk) → locals. `MotionBake::write` multiplies lobe rotations onto the node's current local.
- `Body::lobes()` come from rikka-humanoid flesh swings (PMX dynamic breast bones); models without breast physics get no jiggle.
- App: `Figure.physique` is rebuilt from each cloth analysis (`animate/cloth.rs` `refit`, rebakes only when it changed); `Performer { body, physique }` is what `TakePlayback::bind/rebind/rebaked` take. The Motion tab's "Motion style" section (`inspector/style.rs`, `tuning.rs` `Tuning` table) shows fit, size, snap and bust bounce, plus per-region gains in Detail mode.
- Horizontal travel scaling is deliberately absent: it needs stride re-planning or planted feet slide.
