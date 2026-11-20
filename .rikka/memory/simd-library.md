---
title: SIMD library (rikka-simd)
description: rikka-simd's levels, dispatch!, type-level proof, vector families, math and how consumers write kernels.
about: crates/foundation/rikka-simd
tags: [simd, foundation]
---

- `rikka-simd` (public, x86_64 only) is the only place with `core::arch` intrinsics; physics, text-raster and future SIMD code go through it.
- Levels are the x86-64 microarchitecture tiers `Isa::{Sse2, Sse42, Avx2, Avx512}` (v1–v4). `Level` is a proven-supported `Isa`: `Level::detect()` (cached), `baseline()`, `new(isa)`, `supported()`, `with_max_isa(isa)`.
- Kernels are `#[inline(always)] fn k<S: Simd>(simd: S, ..)` called through `rikka_simd::dispatch!(level, simd => k(simd, ..))`, which enters a per-level `#[target_feature]` function via `Simd::vectorize`. Non-inlined helpers lose the features, so kernels and helpers stay `#[inline(always)]`.
- `dispatch!` marks its closure `#[inline(always)]` so the whole body compiles inside the `#[target_feature]` trampoline at any size; check SSE2-baseline release builds with `nm` (no `core_arch` symbols but `_xgetbv`), not only bench builds (`[profile.bench]` targets x86-64-v3).
- Safety is type-level: the `Sse42`/`Avx2`/`Avx512` token types are unnameable, so `S::F32x8` etc. exist only after detection; only `Sse2` (always present) is nameable, giving baseline types like `<Sse2 as Simd>::F32x4`.
- `Simd` exposes 70 associated types: `F32x1..U64x1` and `M*x1`, every 128/256/512-bit element type, masks, and the native set `F32s..U64s`, `M8s..M64s` (4/8/16 f32 lanes on SSE/AVX2/AVX-512).
- Implementation families: `native::Native<S, E, R>` over `__m128i`/`__m256i`/`__m512i` (512 uses k-masks), `pair::Pair` for wider-than-native widths, `single::Single` for one lane. Level differences are `if S::SSE42/AVX2/AVX512` constant branches inside one implementation.
- Traits: `Vector`, `Mask`, `Float` (+ `FloatElement`), `Int`, `SignedInt`, `Widen`, `Narrow`, `NarrowUnsigned`, `Gather` (bounds-checked), `Compress`, `Swizzle` (u8 `swizzle_dyn`), `FloatMath` (blanket over `Float`), `ScalarFloat`, `prefetch`. `prelude::*` imports them; trait methods on associated types need the trait imported.
- Fixed semantics shared by all levels: x86 `min`/`max`, `to_int_*` out-of-range → MIN, shifts ≥ width → 0 or sign fill, halving `reduce_*`, and `prefix_sum` = log-step per 128-bit block then left-to-right block carries. `mul_add` fuses from `Avx2` up.
- `to_array::<N>()`/`from_array` check `N == LANES` at compile time; annotate the array type where it cannot be inferred.
- `FloatMath` (exp, exp2, ln, log2, log10, pow, sin, cos, sin_cos, tan, asin, acos, atan, atan2) is generic over `Float` with constants in `math/constants.rs`; sin/cos/tan reduce accurately for |x| < 1e5 (f32) and 1.6e6 (f64).
- `FlushDenormals` is a scoped, thread-bound MXCSR guard (FTZ + DAZ, restored on drop) for DSP loops such as rikka-apm.
- Tests compare every type at every level the machine supports against scalar references and ULP bounds.
