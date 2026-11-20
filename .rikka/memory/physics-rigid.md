---
title: Rigid-body core (rikka-physics-rigid)
description: RigidWorld layout, per-step stage list, island tasks vs coloured path and the joint framework.
about: crates/physics/rikka-physics-rigid
tags: [physics, solver]
---

`RigidWorld::builder()` builds the world; `create_body(Body::dynamic()...)`, `create_joint(Spherical::between(a, b)...)`, `step(dt)`.
Cold records live in `Table<T>` by id; hot data lives in `SolverSets` (0 static, 1 disabled, 2 awake, 3+ one per sleeping island) with swap-remove and `local` back-references.
Constraint lanes address bodies by awake index + 1 (0 = static identity); `refresh_solver_indices` must run whenever an awake body moves.
Every touching contact and every joint is linked into its dynamic body's island and coloured in `ConstraintGraph` (24 colours, last is overflow).
Per step: reorder (every 32) → kinematic targets → broadphase pairs → narrowphase + state changes → `build_plan` → stages → `after_solve` (proxies, events, sleep, one split).
Stages: prepare coloured batches, island tasks, then per substep integrate velocities, warm/solve colours, integrate positions, relax colours (last fused with restitution and impulse store), then finalize.
Islands of at most 256 constraints, every island of a scene with at most 2048 awake constraints (one typical figure model), and islands up to 4096 that are at most 1/16 of the awake work are island tasks: one worker prepares once and solves all substeps; batches come from depth-ordered greedy colouring (BFS depth from kinematic/static anchors over the island's own constraints) cached per island until `island.dirty` and rebuilt in a parallel `schedule islands` stage, and consecutive small islands are packed into one task (about 32 × lanes constraints plus bodies) whose lanes span islands by an order-preserving greedy merge (each island batch goes into the first later merged batch of its class with free lanes); `FilterJoint`s never enter batches.
Coloured batches come from the graph colour lists: a parallel `plan gather` stage (`solver/gather.rs`) classifies each listed constraint into a code, then a serial pass buckets them; the plan is cached while `graph.version` and the coloured island set are unchanged.
Moving kinematic bodies wake touching sleeping islands only on the step they start moving (`moving_bits` edge); afterwards `sleep_islands` skips candidates whose contacts or joints touch a moving kinematic body.
`reorder_if_due` also sorts `awake_contacts` by id so the narrowphase contacts stage streams contact headers in address order.
Warm start stays a separate pass before solve; fusing it per batch broke stacking and chain tests.
The per-body speculative margin grows with velocity but is capped at `Tolerance::speculative_distance()`; faster bodies rely on continuous collision.
`ShapeDef::contact_response(false)` keeps a shape in queries and sensors without contacts or CCD hits.
Kernels run inside `Simd::run` and must be `#[inline(always)]`; batch storage is raw `WideBuffer` memory viewed as `ContactBlock<W>` or `JointBlock<W, M>`.
Closures never inherit the AVX2 target feature, so wide math inside `map`/`from_fn`/`then` closures in a kernel compiles to out-of-line calls per SIMD op; keep closures to scalar per-lane work.
Adding a joint type is one file implementing `JointModel` plus one `joint_kinds!` line in `joint/kinds.rs`.
D6, Revolute, Prismatic, Wheel, Motor and Parallel are thin builders over one `Axes` description and share one `JointKind::Axes` (`joint/axes_model.rs`); frame Z is the twist/hinge/slide axis, X/Y are pyramid swings, wheel suspension and steering use Y.
`Axes` layout bits (blocks, rows, ranges, drives, pose drives) split batches, so runtime setters reached through `RigidWorld::joint_mut::<T>()` may only change targets, velocities and caps, never the layout.
Axes prepare fixes swing-twist axes, inertia-premultiplied rows (`AxisW`), the three angular row couplings and the 2×2 hinge swing inverse once per step; solves recompute angles and lever-arm masses.
Every lever-arm constraint (linear rows, the Axes point lock and pose drive, Spherical, Weld) builds its effective mass each solve from the current lever arms and the step-start world inverse inertia used by `apply`; never pair a prepare-time 3×3 mass with solve-time lever arms.
Island tasks see kinematic bodies through `States::with_kinematic_steps`, which repeats the per-substep integration so they match the coloured path bit for bit; kinematic bodies are never speed-capped.
Linear axis rows (`LinearRows`) rebuild their Jacobian and its effective mass from the current levers every solve; a prepare-time mass with rotated levers overshoots and blows up spinning PMX bodies (`tests/joints/spinning.rs`). `linear_masses` only sets linear drive softness.
Angular rows are Gauss–Seidel through scalar couplings: each row's delta updates the later rows' `cdot` and all three apply to the bodies once.
Every present Axes limit side stays active; a solve skips a batch's angle limits only when every lane is provably far (no limit impulse, and a quaternion bound on twist and swing plus relative spin × 2h stays inside the smallest limit around zero).
Each axis may carry a spring/velocity drive row and a separate capped motor row (joint friction); drive clamps run only when some lane has a finite max effort.
`Axes` stores drives as a plain `DriveTable` (f32 arrays + flag bytes) and caches the locked-axes rotation; prepare reads params, impulses, frames and masses by 4-wide transposes (`joint/columns.rs`, `repr(C)` `BodyMotion`/`Sym3`), never per-lane `from_fn` gathers.
`JointLanes` carries per-lane `LaneKinds` (dynamic/kinematic bits) from prepare, so joint gathers and scatters skip per-lane flag checks; `JointRecord::reaction()` is computed lazily from stored impulses.
Distance and FilterJoint have their own models; `Ragdoll::humanoid()` builds a 12-bone D6 humanoid with box3d's limits and filter joints.
Joint parameters are publicly readable: `RigidWorld::joint::<T>`, `joint_axes` (the shared `Axes` of every Axes joint), `JointDef::{kind, params::<T>, axes}`, getters on `Spherical`/`Weld`/`Distance`, and `Spring`/`Drive` settings; `joint_revision` counts `joint_mut` borrows.
Task/colour decisions never depend on worker count, so `state_hash` is identical for any thread count on one SIMD path.
Joint batches reach their solver through `solver/joint_kernels.rs`: each `JointModel` runs in its own `simd.run(JointOp::<M>)`, so the stage kernel does not inline every joint kind (this keeps the rlib compile at seconds, not minutes).
Finalize writes enlarged fat bounds itself and splits fast bodies into regular and bullet lists (sleepy bodies are never fast); `solve_continuous` sweeps them in two parallel stages (regular vs static, then bullets vs everything non-bullet) without sorting, since sweep order does not change results; sensors update last in `after_solve`.
Fast and sleep tests use box3d's per-axis rotation arcs over `BodyPose::reach` (per-axis local extent), not the scalar radius, so long bodies turning about their long axis are neither fast nor restless.
Queries take `impl Into<ShapeQuery>` (a `QueryFilter` plus an optional `&dyn Fn(ShapeId) -> bool`); `Character` is a capsule controller over `collide_mover`/`cast_mover` with ledge-aware slope limits, step-up and platform velocity.
`Body`, `ShapeDef` and `JointDef` implement `rikka_file_format::Codec` (rigid `Error`, format failures as `Error::Format`) in `codec/`, plus headerless `to_bytes`/`from_bytes`; lists and blobs there use u32 counts, `Shape` goes through free functions (orphan rule), `Material`/`Filter` codecs live in rikka-physics-collision, and joint params need per-type impls inside `joint/`.
Stage timings use rikka-physics-core `Stopwatch` laps, so a step with profiling off never reads the clock.
Spherical and Axes joints share `swing_twist.rs`'s wide `twist_angle` (sign-normalised so q and -q agree), `swing_angles` and `cone_angle` over `Wide::atan2`.
New pairs become contacts in a parallel `create contacts` stage; header and geometry arrays grow by a parallel move (`reserve_on`) and ids link serially in pair order.
Per-worker output vectors reserve the merged total after every collect, so shifting work between workers never allocates.
`tests/allocations.rs` counts heap allocations over steady `step` calls with a moving kinematic body and must stay at zero; a wrapping `Executor` marks the world's worker threads so the global allocator counts only them and the test thread.
The world steps only through `step(dt)`: there is no external solver or cloth coupling.
