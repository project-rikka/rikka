---
title: glTF codec
description: rikka-gltf reads and writes .gltf/.glb, merges primitives, keeps sparse morphs, and exposes parts for VRM.
about: crates/model/rikka-gltf
tags: [model, gltf]
---

- Hand-written: private `document.rs` (serde schema), `glb.rs`, `uri.rs`, `read/`, `write/`; lib.rs is a flat facade. glTF space is canonical, so import converts nothing.
- `GltfImporter` reads both containers. `GltfExporter::text()` and `binary()` are separate registrations over `Container { Text, Binary }`.
- Accessors: every component type, normalized, strided, sparse. A missing buffer view reads as zeros.
- Primitives merge into one vertex stream with one submesh each, sharing identical accessors. A primitive without `NORMAL` is unshared and flat-shaded.
- Dense morph deltas become sparse `MorphTarget`s and are written back as sparse accessors.
- Extensions: `KHR_lights_punctual`, `mesh_quantization`, `texture_transform` (base colour only → `Material::uv_transform`) and `KHR_materials_{anisotropy, clearcoat, dispersion, emissive_strength, ior, iridescence, sheen, specular, transmission, unlit, volume}`. Any other required extension is `Unsupported`; others warn.
- Accessor counts are checked before anything is allocated: storage without a buffer view is charged to the import budget, and merged vertices, indices, morph deltas and keyframes count against `Limits` as they accumulate.
- A light without a range imports as `f32::INFINITY`.
- Derived codecs use `parts` (model, meshes, `TextureMap`, and `SourceDocument` for extension lookups by document, material or node) and `compose`/`finish` (`Composition::use_extension`, `set_extension`, `set_material_extension`, `set_node_extension`). `Toon::Kept` says the caller writes toon shading.
- Writes one buffer: the GLB chunk, a data URI, or `<name>.bin` when not embedding. Images follow the same rule.
- Sample `assets/fox.glb` (CC0) serves `tests/documents.rs` and the render benchmarks; `tests/refused.rs` holds the malformed documents. Writer changes are checked against `cgltf` cloned into `reference/`.
