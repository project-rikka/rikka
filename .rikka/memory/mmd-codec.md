---
title: PMX and VMD codec
description: rikka-mmd reads and writes PMX 2.0/2.1 and VMD, following the reference tool's quirks, and solves IK and append bones.
about: crates/model/rikka-mmd
tags: [model, mmd]
---

- Modules: `pmx` (`read/` and `write/` each split into `wire` bytes and `build` model conversion), `vmd`, `runtime`, plus the shared `text`, `euler`; bytes go through `rikka_model_core::ByteReader`/`ByteWriter`. Behaviour follows the reference tool, quirks included.
- PMX 2.0 and 2.1 both ways. The written text encoding is `Pmx::encoding` (UTF-16 for models without a `Pmx`, since the reference tool refuses UTF-8).
- Version written = `Pmx::required_version()` (2.1 only for QDEF weights, flip/impulse morphs, joint kinds other than spring 6DOF, soft bodies, or material flags 0x20/0x40/0x80); `PmxExporter` picks it, `Pmx21Exporter` (name "PMX 2.1") always writes 2.1. Soft bodies follow the joints only in 2.1 files.
- A 2.0 file is read with 2.0 meanings: joint kinds become spring 6DOF, material flags above 0x10 are cleared, QDEF and morph kinds 9/10 are refused as in the reference tool.
- The reader drops dangling references (bones, tails, append, IK, morph members, display targets, rigid-body bones, joint bodies, impulse bodies) so every read `Pmx` writes back; soft bodies with out-of-range material, anchor or pin indices are refused as damaged.
- The writer validates the `Pmx` it uses (`write/validate.rs`): every index in range, ≤4 additional UV sets each one value per vertex, UV morph channel ≤4, known joint kind; failures are `Error::Invalid`.
- Everything else rides the `Pmx` extension: bone flags, IK, append parents, all morph kinds (`PmxOffsets::Flip`, `Impulse` with `PmxImpulseOffset`), panels, rigid bodies, joints (`kind: u8`, named by `PmxJointKind`), `soft_bodies` (`PmxSoftBody` with shape, flags, aero model, config, cluster, iterations, stiffness, `PmxAnchor`s, pins).
- Rigid-body and joint rotations are quaternions; Euler angles are re-derived on write, so they round-trip only to float precision (identity and other exact rotations are byte-exact). Skinned vertex normals are written as stored.
- `VmdImporter` builds a node per named bone and morph. `Vmd::animation` binds a motion to a PMX model.
- VMD Bézier curves are stored 4× shifted. Two spare bytes carry the physics-off flag. Bone tracks and the camera bake through `bake::adaptive` on the whole-frame grid (exact at keys, at most one sample per frame, more only where a curve bends), one bone track per `rikka-mmd` worker; `VmdImporter` refuses a motion whose frame span or per-frame key bound exceeds `Limits::max_keyframes`.
- `Vmd::read(bytes, &ImportOptions)` reads keys only (no model, no bake) and converts them exactly as `Import` would; it checks the frame span. Records are read in bulk and Shift-JIS names decoded once per distinct 15-byte name.
- A span between two keys follows the curves stored on the later key (the reference tool does so for bones and cameras). `VmdCurve::factor` solves x like the reference tool: 12 halvings from 0.5; x1==y1 and x2==y2 is linear.
- `Vmd::bone_at(name, frame)` evaluates one bone track exactly as the sampled animation does; for repeated queries build `Vmd::bone_tracks()` (`VmdBones`: `track`, `at`, `key(name, frame)`) once. `VmdCurve::from_bytes`/`to_bytes` are the public 4-byte curve encoding.
- Space: left-handed, Y up, 8 cm units (`MMD_UNIT_SCALE`). `in_mmd_space` converts before writing.
- `Rig`/`Puppet` resolves group morphs, applies bone and material morphs, walks append bones layer by layer, then runs the reference tool's own IK (half-angle steps, axis limits, reflect-then-clamp). Axis limits decompose into Euler angles and bound only the middle (gimbal) angle to ±88°, so knees fold past a right angle.
- Rig evaluation is split by `Phase`: bones ordered (PHYSICS_AFTER, layer, index); morphs and bones before physics run in `BeforePhysics`, `PHYSICS_AFTER` bones and their IK in `AfterPhysics`. PMX rigid bodies and soft bodies are not simulated; UV, flip and impulse morphs are kept but not applied.
- `Puppet::set_reaching`/`reach_everywhere` switch IK bones off; `Vmd::switches(model)` binds property frames (visibility, IK by name) as step keys, holding the first before it.
- `VmdBoneFrame::toward` and `VmdCameraFrame::{pose, toward}` interpolate one pair of keys (the samplers below and Animate's tracks use them).
- `Vmd::camera()` evaluates camera keys per axis with Bézier curves, treats keys one frame apart as cuts, and gives a `VmdCameraPose` (eye = target + R·(0,0,−distance), looking −Z). The baked camera node samples it adaptively on whole frames and adds a sample just before each cut.
- `Vmd::merge` lays one motion over another, replacing only keys on the same name and frame (dance plus lip-sync).
- `standard_skeleton()` builds a mesh-less PMX skeleton with the standard and semi-standard bones (センター/グルーブ/腰, 肩P/肩C, 腕捩/手捩, 腰キャンセル, 両目, leg IK) at a standard model's proportions (arms 40° below horizontal), converted like an import; it is the source skeleton for playing VMD on non-PMX figures.
- `euler::point_bounds`/`angle_bounds` keep an inverted (lo>hi, free) joint range inverted through the handedness mirror.
- `pmx/surface.rs` owns the material look: `Toon::flavor` = `Mmd`, lit colour = clamp(diffuse × 154/255 + ambient) as `base_color` with the PMX ambient kept in `Toon::ambient`, specular and power, the toon texture becomes a clamped `shading_ramp`, no toon means flat, a missing shared `toonNN.bmp` falls back to a table of shadow colours (shade = lit × colour).
- Every PMX material is blended with depth writes and `render_order` = material index, as the reference tool draws in list order. Additive and multiplying sphere maps become `textures.matcap` with `Toon::matcap_blend`; sub-texture spheres (second UV set) are dropped.
- Material morphs blend the PMX values and repaint through `surface::paint`.
