---
title: Writing built-in Slang shaders that match their GLSL originals
description: Slang lowering pitfalls (sign, mod, constant folding, push-constant loads, coherent pointers, names) for exact ports.
tags: [shaders]
---

- HLSL-style `sign` returns int in Slang (FSign → int → float), which turns −0 into +0; float code calls an `OpExtInst glsl450 FSign` `spirv_asm` helper (`float_sign`).
- GLSL `mod` is `OpFMod`; Slang `fmod`/`%` is `OpFRem` (truncated). Use the `floored_mod` `spirv_asm` helper.
- glslang folds constant expressions in double and rounds once; Slang folds in float. Inexact literal quotients are written `float(1.0l / 2.2l)`.
- `pc.v.x` makes Slang load the whole push-constant vector and extract (CSE'd across blocks), while glslang loads the scalar where it is used. NVIDIA fuses differently around the two, so a component read inside a branch can shift results by an ulp. Single push-constant components are therefore always read as `pc.v[0]` (scalar access chain); swizzles and whole vectors stay as written, as glslang loads those as vectors too.
- Vector `lerp`, `step`, `smoothstep`, `atan2`, `distance`, `unpackUnorm4x8ToFloat`, `Load`, `Sample`, `SampleLevel` and `Interlocked*` (also on `T*` elements and `groupshared`) lower to the same SPIR-V as their GLSL counterparts.
- `T*` pointers cannot be `coherent`; cross-invocation data goes through `loadCoherent<4, scope>`/`storeCoherent`, which need the package built with `Shaders::vulkan_memory_model()` (image-edit's kernels are; its `dither.slang` uses workgroup scope since diffusion runs as one workgroup) plus `DeviceMemoryBarrierWithGroupSync`.
- Under the Vulkan memory model plain `groupshared` accesses are private, so data read by other invocations after a barrier is stored and loaded with workgroup-coherent helpers (`shared_load`/`shared_store`: physics `physics.memory`, image-edit `lib/workgroup`); atomics need no helper.
- Inside a struct, a static member named like a free function shadows it (calls resolve to the member and fail with "type cannot refer to non-static member"), so interface requirements get their own names (`INonWritable.load_from`).
- Storage images without a format: `[format("unknown")] WTexture2D<float4>` (`Store`); sampled bindless arrays: `Texture2D<float4> x[]` or `Sampler2D x[]` with `NonUniformResourceIndex`.
- Vertex inputs use `SV_VulkanVertexID`/`SV_VulkanInstanceID` (gl_VertexIndex/gl_InstanceIndex); flat varyings are `nointerpolation` with explicit `[[vk::location(n)]]`.
- `linear` cannot name an entry point. Struct inheritance warns (unstable), so push blocks compose a header struct (`Head head;`) instead.
- Shared code over a push block is a method on a generic push struct (`push.spread(id, axis)` on `ConstantBuffer<RikkaBlurPush<Parameters>>`): Slang then loads every push field at entry instead of where used, with the same arithmetic. A `ConstantBuffer<T>` function parameter fails with a type mismatch and `__constref` crashes slangc. Interface default methods need `override` in the conforming type.
- Moving or renaming constants and helpers across modules leaves the SPIR-V unchanged; compare a change by snapshotting the package's `.bazel/bin/<package>/src/spv/*/*.spv` before and after (or compiling both trees with slangc and the flags of `rikka-shader-build`) (`spirv-opt --skip-validation --strip-debug`, then `cmp`).
- A port is checked by diffing per-shader opcode/ext-inst histograms and float constants against the GLSL SPIR-V (`spirv-dis` from `nixpkgs#spirv-tools`), then by pixel readbacks.
