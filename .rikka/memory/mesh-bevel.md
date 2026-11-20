---
title: Bevel operator (rikka-mesh-bevel)
description: rikka-mesh-bevel ports the reference bevel onto EditMesh: entry points, input model, profiles, modules, oracle tests.
about: crates/model/rikka-mesh-bevel
tags: [model, modeling]
---

- Public crate `rikka_mesh_bevel` (deps: rikka-mesh-edit, rikka-polymesh). `bevel(&mut EditMesh, &BevelInput, &BevelOptions) -> Result<Geometry>` is the reference bevel; the returned `Geometry` is the reference's `faces.out/edges.out/verts.out`. `bevel_selected(mesh, options)` is MESH_OT_bevel: selected visible verts+edges in, then the reference's reselection and select-mode flushes.
- `BevelOptions::default()` = operator defaults (width 0 does nothing), `BevelOptions::modifier()` = Bevel modifier defaults (width 0.1, clamp on). `width` is the percent for `WidthType::Percent`. `ProfileType::Custom(CurveProfile)`; `profile_shape` stays the miter profile shape.
- `BevelInput { verts, edges, weights: Option<WeightLayers>, vertex_group }`: only manifold edges bevel; `weights` = use_weights with named float attributes (other types/domains are converted through Polymesh), `vertex_group` scales vertex-only offsets. `BevelInput::from_limit(mesh, affect, Limit::{None, Angle, Weight, VertexGroup}, layers)` reproduces MOD_bevel's selection.
- `CurveProfile` mirrors BKE CurveProfile: points with Free/Auto/Vector/Align handles, presets (Line, SupportLoops, Cornice, CrownMolding, Steps; the dynamic ones take the segment count at reset), `sample(n)` with sample-straight-edges / even-length modes, including glibc mergesort tie order and forward-difference overflow writes.
- Tool helpers (`DragValue`, `ValueDrag`, `segments_from_value`, `pan_segments`, `next_*` cycles) are the pure value mapping of the modal operator.
- Internals: `state::Bevel` owns arenas (`EdgeHalf`/`BoundVert`/`BevVert`, `Grid` = VMesh with (i,j,k) canon indexing) and uses kernel TAG flags exactly like the reference tag flag (copied by `copy_*_data`, so outputs match the reference). Modules follow the reference phases: construct → boundary/offset/adjust/limit/orientation → profile/subdivide/patterns → build/edges/rebuild → marks/uv/harden.
- Float details matter for parity: f32 math with the reference's helpers in `math.rs`, double comparison in `tri_corner_test`, per-face mean-value interpolation via cached `FaceSample` with edge snapping, the reference's crossing test for point-in-face.
- Harden normals encodes per-loop targets in the kernel's fan spaces (`NormalSpaces`) and decodes through `Polymesh::decode_custom_normals`, so smooth fans average like the reference; the result is stored as object-space `custom_normal`.
- Tests: the bevel oracle file under `tests/oracle/` holds the reference inputs (exact f32) and outputs (micro units) parsed at run time; cases compare faces order-independently (positions, UVs, normals, materials, strength, seams, sharp, selection counts), monkey cases compare summaries. The generator script is not committed.
