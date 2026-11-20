---
title: Triangulated geometry in rikka-physics-geometry
description: rikka-physics-geometry's triangulated half: BVH4, mesh/height-field views, compounds and the mesh contact pipeline.
about: crates/physics/rikka-physics-geometry/src/triangulated
tags: [physics, geometry]
---

`src/triangulated` owns the BVH4 (`bvh/`), `TriangleMesh`/`MeshShape`, `HeightField`, `Compound` and triangle contacts (`contact/`), ported from box3d mesh.c, height_field.c, compound.c, triangle_manifold.c and mesh_contact.c.
`Blob::section` validates bounds on every call, so hot paths resolve sections once through `MeshShape::view()` (`MeshView`: slices, quantized tree, scale) and `HeightField::view()` (`HeightFieldView`).
The crate-private `TriangleSource` trait is implemented on those views, never on the shapes; `triangles()` returns a fetch closure and `overlapping()` yields indices only.
`collide_triangulated(a, b, transform_b_to_a, speculative, cache, scratch, out)` handles convex–mesh/height-field/compound and compound–compound pairs.
`TriangulatedContact` keeps sorted `FeatureKey` entries (key plus triangle `SatCache`) and a parallel `PairCache` vec only for paired contacts (capsules, compounds); keys refresh only when the convex leaves the inflated query box.
`contact/pipeline.rs::Work` transforms fetched triangles with a `Mat3`, runs the sphere/capsule/hull kernel, filters ghost edges with `KeySet` hash sets, clusters by normal and material, and culls each cluster to four points.
`HullTable` is a per-call `F32x4` SoA copy of the hull's points, planes and edge pairs, each part filled lazily on first use by the SAT support, face and edge queries.
`Bvh4` stores a padded SoA copy of item bounds per position (`bvh/items.rs`), refreshed by `build_into`, `refit`/`refit_on` and `rebuild_subtree`; `query`, `self_pairs`, `tree_pairs` and pair tasks report only items whose stored bounds overlap, tested four at a time.
`Bvh4` keeps no arrays sized by item id: `ItemMap` (open addressing, sized to the items built) maps item → position and `leaves` maps position → leaf location, so every rebuild costs O(items built).
Every child node index is greater than its parent's (`rebuild_subtree` reuses only free indices above the subtree root), so `refit` and full `mark_moved` passes walk nodes in reverse index order without a stack.
`refit_on(bounds, executor)` refreshes item bounds in parallel position chunks and then refits nodes serially; it gives the same tree as `refit`.
`bvh/pairs.rs` is the pair traversal: compact `u32` work items, leaf×leaf emitted during expansion, tasks split breadth-first for `self_pair_tasks`/`moved_pair_tasks` and ordered subtree tasks first.
Moved-subset pairs: after refit/rebuild call `mark_moved(items)`, then `moved_pair_tasks` yields only pairs with at least one marked item (`bvh/moved.rs`: position bits, then slot masks by per-item parent chains or one bottom-up pass when many moved); `build_into` and `rebuild_subtree` clear marks, `clear_moved` resets them.
Serial and parallel builds share the 2048-item deferral so both give identical layouts.
The builder always splits a cluster above the leaf size, even when non-finite bounds give it a NaN area; otherwise NaN proxies make a broadphase rebuild allocate nodes forever.
`TriangleMesh` decodes its quantized tree once into float child bounds (`bvh/decoded.rs`, `DecodedTree`); mesh views query and sweep that tree, and `MeshView::for_each_overlapping` tests four triangles per SAT call with `triangles_overlap_box`.
Rotate manifold normals with a `Mat3`, not `Quat`: glam's quaternion path reads the adjacent `u32` fields into a spare lane and hits denormal assists.
