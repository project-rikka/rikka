---
title: VRM codec
description: rikka-vrm reads VRM 0.x and 1.0 into one shape and writes both (VrmExporter 1.0, VrmLegacyExporter 0.x).
about: crates/model/rikka-vrm
tags: [model, vrm]
---

- Sits over the glTF codec. Sniffing ties go to the extension, so `.glb` stays glTF.
- One `Vrm` extension holds `meta, humanoid, first_person, look_at, expressions, spring_bone, constraints`, all addressed by model ids. `read/resolve.rs` warns rather than fails on missing targets.
- Companion extensions carry what `Vrm` cannot: `VrmLegacy` (0.x only: exact `LegacyLicense`, `LegacyHumanoid` description, per-bone `BoneLimit`s, `first_person_bone`, `LookAtCurves`, `LegacyExpression` group names and unconvertible material values) and `ExtendedColliders` (`VRMC_springBone_extended_collider` shapes keyed by index into `spring_bone.colliders`, whose entries hold the spec fallback; `ExtendedShape::fallback` makes it).
- 0.x is lifted to 1.0 per UniVRM's migration: thumbs shift one joint, presets become expressions (an `unknown` preset tries the group name), weights /100, mesh binds become node binds, spring chains walk first children, licence maps through `LegacyLicense::apply_to`, emission stays linear, screen outline ×0.005, UV rotation ×2π.
- 0.x models are declared `legacy::COORDINATES` (`right: NegX, forward: Z`), so normal conversion turns them to +Z. Vectors inside the extension get Z negated first. Colours go from gamma to linear.
- MToon becomes `Shading::Toon` on the ordinary material.
- 1.0 export: `rikka_gltf::compose(.., Container::Binary, Toon::Kept)` plus `VRMC_vrm` (used, not required), `VRMC_springBone` (+ extended collider extensions), `VRMC_node_constraint`, `VRMC_materials_mtoon`, written as GLB; 0.x-only data is warned about.
- 0.x export (`write/legacy/`, registry name `VrmLegacyExporter::NAME` = "VRM 0.x", same `.vrm` extension, register after `VrmExporter`): drops animations, cameras and lights, turns the copy to `legacy::COORDINATES`, then bakes every node to translation only (`normalize.rs`: inverse binds absorb skinned frames, unskinned meshes get the world linear part, shared meshes are cloned per frame, mirrors flip winding) and rebases look-at and collider offsets. Capsules become sphere chains spaced at most one radius; extended colliders go out as their fallbacks; constraints, overrides and matcap binds are dropped with warnings. Materials are renamed unique (0.x binds by name). `exporterVersion` is "rikka-model" (no digits, so UniVRM applies no old-version migrations).
- glTF mesh indices for 0.x mesh annotations and binds are recomputed with rikka-gltf's rule (a mesh is written when a submesh has indices).
- Runtime `Avatar` (reached by `instance.rig_as::<Avatar>()`): `Expressive::set_expression`, `set_look_at_target`, `set_gaze(Gaze { yaw, pitch })` (a direction that overrides the target, radians), per-instance `Tints`. Evaluation runs constraints, then gaze, then expressions, and never writes back into the pose.
- VRM Animation: `VrmAnimationImporter` (`.vrma`, registry name "VRM Animation") reads `VRMC_vrm_animation` 1.0 over `rikka_gltf::parts` into a `VrmAnimation` extension (declared humanoid without eyes, preset/custom expression nodes, lookAt node and offset); `expression_weights(pose)` reads translation.x clamped, `gaze(pose)` the lookAt rotation as extrinsic-ZXY yaw/pitch. A missing extension is `Damaged`, a non-1.0 spec `UnsupportedVersion`.
- Overrides follow the spec (block, blend, binary suppression).
- Spring-bone data is parsed but never simulated; the mesh cloth may read it as a hint. `Avatar` walks constraints in `BeforePhysics` and gaze/expressions in `AfterPhysics`.
- Sample `assets/avatar.vrm` (VRM Public License 1.0, redistributable, unnormalized). Tests: `avatars.rs`, `legacy.rs` (0.x reading), `legacy_export.rs` (0.x writing, baking, round trips), `extended.rs`, and `schema.rs` (the 0.x JSON schema ported as a small validator).
