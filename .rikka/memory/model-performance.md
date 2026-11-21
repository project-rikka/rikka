---
title: Model library performance rules and threading
description: Threading model, caches and complexity rules for the crates/model mesh libraries.
about: crates/model
tags: [model, performance]
---

- Results never depend on thread count: work is split into contiguous index ranges and every output element is computed by the same sequential arithmetic it had serially; float sums keep their per-element order (stencils keep duplicate entries, no regrouping).
- Data parallelism goes through the public `rikka-parallel` crate (see foundation-runtime note); each crate keeps a `const PARALLEL: Parallel = Parallel::new("rikka-<crate>")` in `lib.rs` and sets a grain (≥ 4–16 k elements per worker) per call so small inputs stay on the caller's thread. Its one thread-local guard makes any nested call inside a worker, from any crate, run serially.
- Polymesh `gather`, normals (`face_normals`, `point_normals`, smooth `shade`) and MikkTSpace welding are parallel; `ensure_edges` resolves missing edges by bucketing keys per low vertex (no hash map) and keeps first-seen edge order; `merge_points` stores welded faces in one flat buffer.
- Subdivision (`evaluators/subdivision/`): `refine` builds CSR stencils (`Stencil`, chunked by worker) and child topology in parallel; adjacency (`Around`) is an atomic counting sort with per-group sorting (identical to index order). The limit surface refines only the 1-ring of the cage points (`Extent::Around`) and folds the last extra refinement into `limit_after_refine`, so it never builds the full deeper cage. Ordering (`order.rs`) matches edges/faces through per-vertex buckets instead of hash maps.
- `StackCache` + `evaluate_cached` keep per-modifier state across evaluations. Subdivision caches its `Chain` (level stencils, limit, final point order) and the last output; when the input differs only in positions/shape-key positions (bitwise compare of everything else) it re-applies the chain and clones the cached output.
- Inherent costs that remain: `Evaluated` results own their `Polymesh` (one clone per evaluation), per-point `Vec<Influence>` vertex groups allocate on every clone/gather, `MeshAssembler` (rikka-model-core) hashing in `to_mesh`, `kdtree::balance`/`find_duplicates` (Weld) are serial, and `MAX_CORNERS` (2^24) reduces 1M-quad level 2 to level 1.
