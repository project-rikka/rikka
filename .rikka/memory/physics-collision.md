---
title: Collision pipeline (rikka-physics-collision)
description: Broadphase over three BVH4 trees, pair set, kind-bucketed narrowphase, recycling and per-worker manifold pools.
about: crates/physics/rikka-physics-collision
tags: [physics, collision]
---

The crate is body-agnostic: callers pass `Collider`s indexed by proxy id and `BodyFrame`s indexed by `Collider::owner`.
`Broadphase` keeps static, kinematic and dynamic `Bvh4` trees plus pending lists; BVH queries and pair traversals test each item's bounds as stored at the last build or refit, so visitors recheck only for pending or removed proxies.
Moved proxies refit their tree (subtree rebuild when few of many moved in a tree over 512 members, otherwise a full refit, parallel `refit_on` only above 4096 members); degraded SAH or many pending trigger a rebuild.
The moved list is rebuilt in ascending proxy order each update, so the order of `touch`/`move_proxy` calls never matters.
Pairs are searched only around moved (enlarged) proxies in one barrier-free stage: dynamic–dynamic pair tasks (`moved_pair_tasks` after `mark_moved`, or `self_pair_tasks` filtered by `is_moved` when over half the dynamic tree moved), one block per moved static or kinematic proxy (querying the dynamic tree for unmoved proxies only), then blocks of moved dynamic proxies querying the static and kinematic trees; pending (unbuilt) dynamic proxies also query the dynamic tree and in-tree ones scan the unmoved pending ones.
Moved-pair visitors recheck only `contains_in_tree`, while `ProxyTree::query` still rechecks fat bounds for queries between steps; `tests/broadphase.rs` checks every overlapping pair against brute force for 1 and 4 workers.
Existing pairs live in an open-addressing hash `PairSet`; candidates pass a small `Lookahead` ring that prefetches the hash slot, then check the set first, proxy compatibility second and the custom filter last.
`update_pairs` returns new pair keys (`pair_key`, lower collider in the high half) radix-sorted, so contact ids follow sort order.
`BodyFrame` carries the per-step `motion` (v·h), `turn` (w·h) and `sweep` (rotation reach + slop), all capped at the speculative distance, plus `reach`, the body's per-axis local extent; `swept(aabb)` is the proxy bounds, and a pair's speculative distance is the base distance or the relative approach, whichever is larger.
Recycling bounds rotation by box3d's per-axis arc (`rotation_arc(turn, reach)`), so long bodies spinning about their long axis still recycle.
Manifold points farther than their own predicted closing distance are dropped (`trim_unreachable`), so co-moving bodies do not become touching.
Contacts are split into a hot `ContactHeader` (ids, flags, kind, point count, recycle anchors, extra-manifold span) and a cold `ContactGeometry` (manifold, pair cache, triangulated cache); callers hold both arrays indexed by contact id.
`Narrowphase::collide` runs one "narrowphase contacts" stage that marks disjoint contacts, recycles, and collides cuboid–cuboid and general pairs inline (prefetching the whole cold record); sphere/capsule/sphere-cuboid kinds and triangulated pairs are bucketed into one flat "narrowphase batches" stage, the former through rikka-physics-geometry's `collide_batch_*` Wide kernels.
`Collider::responds()` is false for sensors and `contact_response(false)` shapes; such proxies never form pairs but stay in queries and sensors.
Triangulated pairs keep extra manifolds in double-buffered per-worker `ManifoldPools`; `ContactHeader::extra` is valid only while its stamp equals the current pool stamp. `ManifoldPools::extra`/`extra_mut` read or write one by span and index.
State changes come back as ascending contact ids with `STARTED_TOUCHING`, `STOPPED_TOUCHING` or `DISJOINT` flags for the caller to clear.
rikka-physics-collision implements rikka-physics-core `Codec` for `Material`, `Filter`, `QueryFilter` and `Mover`; parallel slice views use rikka-physics-core `Shared`.
`QueryWorld` offers ray, SIMD ray-packet (`cast_rays_closest`, W rays per BVH4 node), shape cast, overlap, closest-point and mover collide/cast queries; its optional `custom` closure filters colliders.
`Sensors` keeps sorted visitor lists per sensor collider, recomputes them in parallel after each step and diffs them into begin/end events; removal events carry to the next update.
`SweptShape::new(shape, sweep, until)` carries the swept bounds up to the current best fraction; `shape_time_of_impact` sweeps a convex shape against any shape (mesh triangles use the box3d behind-face early out and centroid fallback).
`mover` holds the capsule `Mover`, per-shape collision planes, `solve_planes` and `clip_vector`.
Broadphase rebuilds use `Bvh4Builder::build_into` so steady-state rebuilds reuse storage (only parallel rebuilds above 4096 members allocate).
