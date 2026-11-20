---
title: Physics foundation (rikka-physics-math, rikka-physics-core)
description: Wide over rikka-simd, the lane-choosing Simd, row transposes, Shared views, Fnv1a, Stopwatch and Workers.
about: crates/physics/rikka-physics-core
tags: [physics, foundation]
---

- rikka-physics-math `Wide` is a blanket extension of `rikka_simd::Float<Elem = f32> + FloatMath` adding `one`, `masked` and pointer-row transposes; comparisons use the library's `simd_*` and zero is `W::default()`. `MaskW` adds `bits()` (u32). `F32x1`/`F32x4` are `rikka_simd::Sse2` baseline aliases; `MAX_LANES` is 8.
- `Simd` wraps a `rikka_simd::Level`: kernels run with `S::F32x8` when the native f32 width is ≥ 8 (AVX2, AVX-512) and `S::F32x4` otherwise, so `lanes()` is 8 or 4 and `with_lanes` picks the best level for a recording. Block sizes come from the same `Kernel`, never from a hard-coded width.
- `Wide::load_rows`/`store_rows` turn up to 8 row pointers of four floats into four columns and back; only the first `LANES` pointers are touched. `store_rows` writes through `&mut` rows, so every lane needs its own target: padding lanes get one sink per lane, never a shared one. Rigid (`solver/lanes.rs`, `joint/columns.rs`, `joint/batch.rs`) uses nothing else for AoS↔SoA.
- Wide math shared by solvers: library `atan2`, `Vec3W::any_orthogonal` (unnormalised), `QuatW::{select, nlerp, between}` and `-q`; all `#[inline(always)]` and closure-free. Files using concrete `F32x4` import `Vector`/`Float`/`Mask` from rikka-physics-math.
- rikka-physics-core `Shared<'a, T>` is the only unsafe parallel slice view (`get`, `at`, `read`, `write`, `slice`, `prefetch`); `prefetch(&item, bytes)` sits beside it. For a block per item or per chunk use the safe `ExecutorExt::for_each_item`/`for_each_chunk`; keep `Shared` for indirect or multi-stage access.
- `Pool<I, T>` is the one generational slot container (LIFO index reuse, `slots()`/`slots_mut()` by index for parallel views); `reset()` drops everything and restarts generations at zero, which world rebuilds rely on to reproduce ids bit for bit.
- The byte codec is `rikka_file_format::Codec` (see the file-formats note); physics-math implements it for `Transform`/`Aabb`, and `id!` types encode as index then generation through the hidden `__file_format` re-export.
- `Fnv1a` (FNV-1a over u32 words; `u64` is low then high word) produces every state hash and blob content hash.
- `Stopwatch::start(&profile)` + `lap` records nothing and reads no clock while profiling is off.
- `Workers` (`From<usize>`, `From<Threads>`, `From<Arc<dyn Executor>>`) is the single threading choice; `build()` rejects 0 and names pool threads `rikka-physics-{i}` unless `Threads::named` says otherwise.
- Scalar `cos_sin`/`atan2` run rikka-simd's `FloatMath` on the one-lane `F32x1`, so scalar and wide paths share one implementation.
