---
title: Spatial queries (rikka-spatial)
description: rikka-spatial's Bvh, TriangleBvh, SegmentBvh, PointTree (reference-exact duplicates), robust kernels and View picking.
about: crates/model/rikka-spatial
tags: [model, geometry]
---

- `rikka-spatial` (public, GPU-free, f32 + glam) is the one spatial library for picking, selection, snapping, shrinkwrap, transfer, merge by distance and intersection candidates.
- `Bvh` is a binary binned-SAH tree over caller AABBs (32-byte nodes, children adjacent, leaves ≤ 8, per-primitive boxes stored in leaf order). `Bvh::builder().threads(..)` builds the top levels on scoped `rikka-bvh-build` threads, and the result is the same for any thread count.
- Queries take closures: `ray_cast`/`sphere_cast` (closest hit), `ray_cast_any`, `ray_cast_all`, `nearest(max, bound, test)` (best-first with a caller lower bound), `nearest_to_point`, `for_each_in_{aabb,sphere,frustum}`, `for_each_crossing_plane`, `traverse` (ordered, `ControlFlow`), `for_each_overlap` (two trees plus an `Affine3A`) and `for_each_self_overlap`.
- Closures return a `Candidate` (`distance()`). Ties always go to the lowest primitive index, so results never depend on tree shape.
- `refit(&[Aabb])` keeps the structure. `update` also applies Kopta tree rotations and rebuilds once the SAH cost exceeds 1.5× the cost at build time. `TriangleBvh/SegmentBvh::set_positions` call `update`.
- `refit_primitives(changed)` refits only the given primitives and walks their ancestors (parent links built lazily, dropped by `update`); `TriangleBvh::patch(positions, triangles, refit)` and `SegmentBvh::patch(positions, refit)` move vertices, replace triangle corners and refit just the listed primitives (modeler-pick's per-move refits).
- Primitives with NaN or empty bounds stay in the tree but are never reported.
- `TriangleBvh` uses watertight ray tests (`Culling::{None, Back, Front}`, CCW is front). It also offers closest point with barycentrics, sorted hits, exact sphere and frustum tests, and tri-tri intersections against another mesh or itself (self pairs that share a vertex index are skipped, as in the reference).
- `SegmentBvh` finds the nearest edge to a point, to a ray (exact segment-box lower bound) or in screen space (`nearest_projected`, perspective-correct parameter).
- `PointTree` balances exactly like the reference's `kdtree_balance`. `find_duplicates` equals `calc_duplicates_fast` in index order (Weld modifier, Merge by Distance node). `find_duplicate_clusters(dist, keep)` equals bmesh `find_doubles`/`remove_doubles`, including its centroid target and the quirk that the searching point appears twice in its cluster. `tests/points/fixture.rs` holds the reference output.
- `orient2d`/`orient3d` (Shewchuk sign conventions) are exact for f32 input: a filtered f64 estimate first, then expansion arithmetic. Tri-tri, segment-2D and winding tests decide with them.
- `View` (view-projection, `Viewport`, `ClipSpace`) projects, unprojects, builds pick rays, and builds whole-view or screen-rectangle frusta. Screen space is pixels with y pointing down. `ClipSpace::VULKAN_REVERSED_Z` matches rikka-renderer-3d.
- AABB/plane classification tests box corners with `Plane::signed_distance`, so an `Inside` node implies every contained vertex passes `contains_point`.
- 1M triangles: build takes about 0.45 s on one thread and 0.22 s in parallel (opt); a random cold ray takes about 1.2 µs.
