---
title: Removal and merge operators (rikka-mesh-reduce)
description: rikka-mesh-reduce's exact delete, dissolve, merge, weld, auto-merge, tris-to-quads, unsubdivide and decimate operators.
about: crates/model/rikka-mesh-reduce
tags: [model, modeling]
---

- Public crate `rikka_mesh_reduce` (flat facade) over `rikka-mesh-edit`; every edit-mode operator is `fn name(&mut EditMesh, &NameOptions) -> Result<…>` acting on the selection (hidden excluded) and leaving the reference's selection; `*_geometry` variants take explicit element lists for modifiers and other crates.
- Operators: `delete`/`delete_loose`/`delete_edge_loop`, `dissolve_verts|edges|faces|mode`, `dissolve_limited` (+ `decimate_planar`), `dissolve_degenerate`, `merge` (center, cursor, first/last from history, collapse, uvs), `collapse_edges`, `remove_doubles` (+ `find_doubles`), `weld` (the reference `weld_verts` target map), `auto_merge`/`auto_merge_split` (after transform: moved set + threshold; `intersect_edges` exposes the split pairs), `join_triangles`, `unsubdivide` (+ `decimate_unsubdivide`), `decimate` (+ `decimate_collapse` taking `Collapse` for the modifier), `remove_duplicate_faces`, `collapse_short_edges`.
- Exactness rests on the reference's element order: the kernel's LIFO slot reuse matches the reference mempools, so kills before allocations must follow the reference's order; face swap is emulated by killing a face and copying the replacement into the freed slot.
- All mesh helpers come from the kernel (`vert_loops`, `join_faces(_keep_shared)`, `dissolve_vert`, `triangulate_face(Beauty)`, `Heap`, `LocalSet`, `weld_verts`, `MathLayer`); `WeldOptions` is the kernel type re-exported. Local: dissolve-specific `collapse_vert_merging`/`split_corners`, layer arithmetic in `layers.rs`.
- Custom normals stay per-corner vectors; `use_sharp_edge_from_normals` sharpens edges through `Polymesh::encode_custom_normals`. UV-select clearing removes the `.uv_select_*` attributes.
- `auto_merge_split` sorts BVH pair candidates (k-DOP 14 test replicated) instead of the reference's thread-dependent order, so its element order can differ from the reference on large inputs.
- Tests compare against the reference dumps in `tests/oracle/` (f32 literals; fixtures from primitives are captured once per run because primitive element order varies between the reference runs, and positions must be the exact f32 the reference used or decimate ties break differently).
