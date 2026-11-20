---
title: Convex geometry in rikka-physics-geometry
description: rikka-physics-geometry's convex half: shapes, GJK/EPA/TOI, hull blobs, SAT manifolds, batch kernels, collide entry.
about: crates/physics/rikka-physics-geometry/src/convex
tags: [physics, geometry]
---

`src/convex` owns Sphere, tapered Capsule, Cuboid, Cylinder (axis local Y), ConvexHull and ShapeGeometry for the math Plane; `src/triangulated` owns meshes, height fields and compounds.
All shape data lives in the body frame; `collide(a, b, transform_b_to_a, speculative, &mut PairCache)` returns a manifold in frame A with the normal from A to B and midpoint contacts.
Non-convex pairs return an empty manifold from `collide`; the triangulated half dispatches those itself.
`collide` derives linear slop as `speculative / 4`; Cuboid/Hull pairs keep the caller's order, other pairs are ranked and flipped.
Distance queries are generic over the `Support` trait; reuse a `SimplexCache` only while both local shapes stay the same.
`SimplexCache` (40 B) keeps per simplex vertex a support id, or for `CONTINUOUS` supports an oct-encoded local search direction ("heading") that is re-supported on restore; discrete ids are restored through `Support::vertex`, and an id it cannot resolve means a cold start.
`closest_features` runs GJK and falls back to EPA when cores overlap; `Convex::project_point` builds on it.
Hull and cuboid–hull pairs share one SAT/clipping pipeline over the borrowed `Polyhedron` view; cuboids build a stack `BoxPolyhedron` for it.
Sharp cuboid–cuboid (`manifold/cuboids.rs`) works in A's box frame: face axes scalar, the 9 edge axes in three 4-lane groups, its own SAT cache replay, and quad clipping driven by `hull/box_features.rs` tables that keep the BOX_EDGES feature ids; rounded cuboid pairs use the generic path.
Cuboid–capsule (`manifold/cuboid_capsule.rs`) rejects by the box slabs first, runs GJK on an analytic `AlignedBox` and a two-point `Segment`, and shares the deep-penetration SAT with hulls through the `CapsuleFeatures` trait.
ConvexHull is an offset-referenced `Blob<HullHeader>`: SoA points/normals padded to 8, per-face distances from the centroid, 8 SoA edge-pair rows (both face normals, their cosine, the smaller face distance) and `box_axes` for box-shaped hulls; `from_blob` recomputes and compares every derived section.
Half-edges are twin pairs `(2k, 2k+1)`; the edge SAT, edge rows and cache rely on that.
SAT (`manifold/sat`): both face seeds first set a shared floor; faces are pruned per face by `n·δ − distance − reach(other)` (exact box reach via `box_axes`); edge arcs are pruned in SIMD per pair; the larger candidate set goes into SIMD lanes.
Face clipping uses a 16-point stack buffer and retries with 64 on overflow; `clip_polygon` returns `None` when no vertex lies outside so the pass is skipped; `reduce` works on clip vertices directly.
Hot scalar code uses `extrema::{larger, smaller}` instead of `f32::max/min` (NaN handling costs extra instructions).
Cylinders and tapered capsules go through the general path: exact GJK/EPA normal, then supporting faces clipped when their plane lies within `FLAT` of the normal.
Rounded cuboids and hulls expose the radius through `Support::radius`; rounded polyhedron pairs run GJK on the cores first and fall back to SAT only when cores touch.
`collide_batch_{spheres,capsule_sphere,capsules,cuboid_sphere}` take `*Lanes<W>` SoA inputs and a `TransformLanes`; lanes flagged `fallback` (tapered, parallel capsules, degenerate or deep) must use scalar `collide`. They, `BatchManifold::manifold` and every helper they reach are `#[inline(always)]`, and Wide code keeps SIMD math out of closures, so AVX2 monomorphs inline under `Simd::run`.
Tests live in `tests/convex` and `tests/triangulated`, over the public API, with a deterministic xorshift `Random`, a brute-force `ray_triangle` and hull `face_rings` in `tests/support`.
