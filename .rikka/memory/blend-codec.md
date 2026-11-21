---
title: .blend file reader (rikka-blend)
description: rikka-blend reads .blend files into a Model, evaluating Blender modifiers through rikka-mesh-modifiers.
about: crates/model/rikka-blend
tags: [model, blend]
---

- Facade: `BlendImporter` (registered in `rikka-model`) and `sniff`. There is no writer; `.blend` is import-only.
- `file/` is the format layer: header (4/8-byte pointers, both endians, 5.0's 17-byte header), SDNA and blocks; `Record` reads fields by name through the file's own SDNA. DATA blocks belong to the ID before them; data pointers resolve per owner, ID pointers globally. Link placeholders resolve into library `Blend`s opened once each by `read/library.rs` through the `read::resources::Resources` trait (implemented by `ImportCtx`).
- `read/` builds the triangle `Model` in the file's own space (`RIGHT_HANDED_Z_UP`, `unit_scale` = `scale_length`): `mesh/source.rs` reads every mesh-storage generation into a `Polymesh`; `material/` walks node trees into PBR through a `Store` trait and `image::load` returns `LoadedImage` (name, bytes); `modifier/<kind>.rs` parses each modifier's SDNA record into a `rikka_mesh_modifiers` description via the `References` trait, `apply` evaluates the stack and the Armature modifier binds a skin instead.
- Lattice modifiers: `read/modifier/lattice.rs` turns the referenced lattice object (world matrices via `read::scene::world_matrix`, fu/du folded into the transform) into the embedded `LatticeGrid`.
- Tests: `tests/scenes.rs` and `tests/versions.rs` compare evaluated shapes with reference numbers; hostile files are in `tests/refused.rs`.
