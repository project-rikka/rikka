---
description: Find Vulkan correctness and performance problems against AMD, NVIDIA and zeux guidance, agree on fixes, and implement them.
argument-hint: "<path, crate, or area>"
---

Improve the Vulkan usage within: $ARGUMENTS

If the scope is missing or ambiguous, ask before proceeding. Read the scoped code, its dependencies, callers, shaders, tests, and project conventions before proposing changes.

## Method

1. Map the scope: instance/device and queue setup, allocation, resource creation, descriptors, pipelines, command recording and submission, synchronization, rendering passes, presentation and shaders (GLSL/SPIR-V sources included). Follow calls into dependencies until the Vulkan call is reached. Trace the frame flow: queues, passes, submits and sync points.
2. Split each path into init, load/streaming and per-frame. Per-frame and per-draw costs matter most; init costs matter only for latency or stutter.
3. Check every item below that applies. Validate each finding against the code and the spec; never raise a guideline as a problem without evidence at `file_path:line_number`. A performance finding that needs profiling to confirm is a hypothesis: profile it first, or propose the profiling as the first step of its fix.
4. For a large scope, split disjoint areas across `Agent`s with this checklist. Have them return evidence-backed findings without editing, and merge them.

## Agree before editing

Before editing, use `AskUserQuestion` to agree on which findings to fix and how. Group consequential choices and recommend one fix per finding.

- Present each consequential finding concisely: severity, the path and how often it runs, evidence, the fix with the before → after of the affected API or recording code, the expected effect, and the rule and vendor it rests on.
- Severity: Critical (UB, hang, corruption, spec violation), High (per-frame stall, drain, wasted bandwidth or CPU), Medium, Low.
- Advice marked AMD, NV, DRIVE or tiler is vendor- or architecture-specific. Where vendors disagree, present the conflict as its own choice and recommend what fits the likely targets instead of picking one side silently.
- Breaking changes to the renderer's API, resource ownership and frame structure are allowed when they remove a problem at its root.
- Offer small, local fixes together as one choice.
- Resolve design questions that arise during implementation before implementing them.
## Correctness

- Every resource is written only after its last GPU use has completed (fence or timeline value), including destruction, pool resets, descriptor updates without `UPDATE_AFTER_BIND`, and mapped writes.
- Every hazard (RAW, WAR, WAW, layout transition) has a barrier with correct stage and access masks. Validation does not catch most missing barriers.
- Image layouts match their real use. `oldLayout = UNDEFINED` only when the contents are disposable.
- `EXCLUSIVE` resources crossing queue families get release and acquire ownership transfers.
- Non-coherent host memory is flushed after writes and invalidated before reads, aligned to `nonCoherentAtomSize`.
- Offsets respect `minUniformBufferOffsetAlignment`, `minStorageBufferOffsetAlignment`, `bufferImageGranularity`, `optimalBufferCopy*Alignment` and each resource's `vkGet*MemoryRequirements`.
- Swapchain: acquire and present semaphores are not reused while pending, `OUT_OF_DATE` and `SUBOPTIMAL` trigger recreation, and resize and minimize are handled.
- Externally synchronized objects (queues, pools, descriptor pools) are never used from two threads at once.
- `pNext` chains and borrowed arrays outlive the call. Features and extensions used are queried and enabled.
- GPU work sized by external input (client buffers, counts, sizes) is capped so it cannot overrun a frame or cause `VK_ERROR_DEVICE_LOST` (DRIVE WCET).

## Memory

- Sub-allocate from large blocks (VMA, gpu-allocator). `vkAllocateMemory` is slow and only 4096 allocations are guaranteed (`maxMemoryAllocationCount`). Block size: AMD 256 MB (smaller on GPUs under 1 GB), zeux 16–256 MB, DRIVE at least 8 MB in multiples of 2 MB. Its cost is about 240 µs plus 0.1 µs per KB on DRIVE Orin.
- Don't allocate or free in the frame loop. Recycle blocks after their frames retire.
- Choose memory types from `vkGetPhysicalDeviceMemoryProperties`, never by fixed index. Device-local resources must get a type that really is `DEVICE_LOCAL`.
  - `DEVICE_LOCAL`: render targets, GPU-only buffers, static textures and geometry.
  - `DEVICE_LOCAL|HOST_VISIBLE`: per-frame CPU-written data. It is ReBAR memory, or a 256 MB window without ReBAR that the driver also uses.
  - `HOST_VISIBLE|COHERENT`: staging and small dynamic buffers.
  - `HOST_CACHED`: readback.
  - `LAZILY_ALLOCATED` + `TRANSIENT_ATTACHMENT`: never-stored MSAA and depth on tilers.
- Randomly accessed dynamic data (textures, large storage buffers) goes to device-local memory through staging, not read over PCIe.
- Write-combined memory (`HOST_VISIBLE` without `HOST_CACHED`) takes only sequential writes or `memcpy`. Never read from it; `p[i] += v` is a read.
- Map persistently once and never map per frame.
- Use dedicated allocations when `requiresDedicatedAllocation` or `prefersDedicatedAllocation` is set, and for large render targets (NV: especially pre-Turing). Not for everything.
- Honour `bufferImageGranularity`: over-align images, pad between types, or keep buffers and images in separate blocks.
- Use `VK_EXT_memory_budget` and stay under about 80% of the budget, but don't query it every frame. Linux has no OS paging for over-commit (NV). `VK_EXT_memory_priority` and `pageable_device_local_memory` protect render targets. On device-local OOM, fall back to host-visible memory and place render targets first.
- Alias transient resources only with correct barriers, and fully initialize them (clear, `DONT_CARE` or copy) after each alias switch. NV compresses depth and targets over 512×512 but not small or BC textures, so don't alias across that line.
- Avoid sparse resources on hot data (AMD). Prefer sub-allocation plus transfer-queue defragmentation.

## Resources and formats

- Use `TILING_OPTIMAL` and staging buffers with `vkCmdCopyBufferToImage`, never `TILING_LINEAR` images. Prefer buffer↔image copies to image↔image copies.
- Set minimal usage flags. Extra flags disable compression and add barrier flushes. `STORAGE` on render targets breaks colour compression on older AMD.
- Avoid `MUTABLE_FORMAT` and typeless use on render targets, depth and storage images.
- Write all channels of a colour target, since partial writes defeat compression (AMD). Mipmapped render-target arrays are not compressed (AMD).
- Depth formats: AMD says D24 costs as much as D32, so prefer D32. NV prefers `D24_UNORM_S8_UINT` or `D32_SFLOAT`, and calls `D32_SFLOAT_S8_UINT` not optimal. DRIVE prefers D24S8 or D16 unless precision is needed.
- Use reversed depth (near = 1, `GREATER`) with the full range. Keep the compare direction fixed within a pass, because flipping it disables Z-cull (NV).
- Copy depth and stencil together, since depth-only copies take a slow path (NV).
- Prefer small formats: `B10G11R11_UFLOAT` over `R16G16B16A16_SFLOAT`, no RGB32, BC compression for textures, mipmaps when minified, and 16-bit indices where the counts fit.
- Use at most 4× MSAA. `VkSharingMode` is free on NV but hurts render and depth targets on AMD.
- Vertex data: AMD puts positions in their own stream for depth-only passes. DRIVE prefers interleaved static attributes with dynamic ones split out. Avoid primitive restart (older AMD).

## Descriptors and constants

- Group sets by change frequency: 0 global, 1 material, 2 per draw. Rebind only the sets that changed. Keep few sets and tightly packed bindings without gaps.
- Use the narrowest stage flags, never `ALL`. Keep set layouts small so user data doesn't spill (AMD: under 13 DWORDs). Costs per item: set 1, dynamic buffer 2 (4 with robust access), push constants 1 per 4 bytes.
- Per-draw data goes in push constants (NV: fastest) or dynamic UBO/SSBO offsets, never in new descriptor sets. Push space before spilling can be about 12 bytes on some hardware (zeux).
- Pools: allocate per frame and thread, `vkResetDescriptorPool` whole pools, and avoid `FREE_DESCRIPTOR_SET_BIT`. Size them by measured averages or size classes, batch `vkAllocateDescriptorSets`, and skip reallocating unchanged sets.
- Avoid descriptor copies. AMD prefers plain `vkUpdateDescriptorSets` to templates; zeux finds templates faster on some drivers.
- Bindless (descriptor indexing, update-after-bind) cuts CPU binding and enables GPU-driven draws. NV sets limits of 1M unique descriptors and 2K samplers. With `buffer_reference`, declare `buffer_reference_align`.
- Use UBOs for small, uniformly accessed data and SSBOs for divergent or large data (NV: bones, lookup tables). Prefer combined image samplers (NV) and immutable or reused samplers.

## Pipelines

- Never compile on the render thread. Create pipelines ahead of time or on idle-priority workers. AMD and NV suggest a quick generic pipeline first, then specialized ones; DRIVE warns against large generalized shaders.
- Share one `VkPipelineCache` serialized between runs (`vkGetPipelineCacheData`, `pInitialData`) and pre-warm known states. Concurrent creation from the same shaders serializes.
- Keep lookups cheap: hash small state handles, not whole create-infos, per draw, and avoid lock contention (an immutable per-frame cache plus a locked miss path).
- Sort draws by pipeline and layout, and minimize `vkCmdBindPipeline`.
  - Switching tessellation, geometry or mesh stages on and off is expensive.
  - So is toggling between compute and graphics on a queue.
- Dynamic state: NV uses it to cut pipeline count; AMD says don't enable dynamic states that never change.
- Use identical defaults for don't-care fields. Pipeline derivatives give no speedup (NV). Use specialization constants for variants and workgroup sizes. Free shader modules after creation, and remember pipelines use VRAM.

## Command buffers and submission

- Use one pool per frame-in-flight per recording thread (plus N for secondaries). Reset whole pools and reuse the buffers; never create or destroy pools per frame. Allocate, begin and end on the recording thread.
- Use `ONE_TIME_SUBMIT`, and avoid `SIMULTANEOUS_USE`. Record every frame instead of reusing whole-scene buffers.
- Secondary buffers: AMD says avoid them (at least 10 draws each if used). On tilers they are the only way to split one render pass across threads.
- Sizing (zeux): under 10 `VkSubmitInfo`s per frame, each at least 0.5 ms of GPU work, and under 100 command buffers, each at least 0.1 ms. Put many command buffers in one `VkSubmitInfo`. NV aims for 5–10 submits per frame at 50–80 µs of OS scheduling each. AMD wants at least 10 draws per command buffer.
- Submit early work (shadows, for example) before recording the rest, but never block on a submit or wait idle in the frame loop (`vkQueueWaitIdle`, `vkDeviceWaitIdle`).
- Group draws, dispatches and copies instead of interleaving them. Compute in different command buffers can't overlap (NV).
- Prefer GPU-driven work: `vkCmdDrawIndexedIndirectCount` with a compacted count buffer, `gl_DrawID`, and one large vertex/index buffer with `vertexOffset`/`firstInstance` instead of rebinding per draw.

## Synchronization

- Batch all barriers for one point into one `vkCmdPipelineBarrier2`, since each call can flush caches and drain the GPU.
- Use the narrowest `srcStageMask`/`dstStageMask`, never `ALL_COMMANDS` where a stage suffices. Fragment-only or compute-only barriers keep vertex work flowing, which is critical on tilers.
- Avoid redundant and read→read barriers. Put resources in their final read state once. Avoid `GENERAL`.
- Don't transition an image you won't read again (for example depth to `SHADER_READ_ONLY`), because that can decompress it.
- Prefer image and buffer barriers to global memory barriers (NV), unless one global barrier replaces many.
- Use split barriers (`vkCmdSetEvent2`/`vkCmdWaitEvents2`) with real work between them. Reorder work so there are fewer barriers: all emits, then one barrier, then all simulations.
- Order work within a queue with barriers, never semaphores or fences. Keep semaphores and fences to the minimum, and prefer timeline semaphores.
- Wait on the CPU with fences using wait-all. On DRIVE/Vulkan SC, waiting on any fence, `vkWaitSemaphores` and `vkGetQueryPoolResults(WAIT_BIT)` busy-poll.
- Keep 2–3 frames in flight, with CPU recording overlapping GPU execution.

## Render passes, clears and resolve

- Prefer dynamic rendering (1.3) and use the weakest load and store ops: `DONT_CARE` for disposable contents, `CLEAR` over a separate clear, and `storeOp DONT_CARE` for depth and MSAA that are never read.
- Resolve MSAA with resolve attachments at the end of the pass, not `vkCmdResolveImage`. For custom resolves use an input-attachment subpass with `BY_REGION`.
- Fast clears need whole-image clears. AMD fast-clears only colours 0/1 (RGBA 0000, 0001, 1110, 1111) and depth 0 or 1 with stencil 0. NV takes any colour but only a few unique ones per frame (about 25–30 on Ampere, 10–15 on Turing).
- The NV depth clear value must suit the compare op: [0.5, 1] for `LESS`, [0, 0.5) for `GREATER`. Reversed depth cleared to 0 satisfies both vendors.
- Inside a pass, use `CLEAR` or `vkCmdClearAttachments` (AMD). Outside a pass, use `vkCmdClearColorImage` rather than clear shaders, unless the clear overlaps neighbouring compute. Batch clears, and don't clear inside secondaries (AMD).

## Presentation, queues and async work

- Present modes: `FIFO` is tear-free and never drops frames, `FIFO_RELAXED` suits occasional lateness (AMD's VSync pick where supported), `MAILBOX` lowers latency but drops frames, and `IMMEDIATE` is for VSync off. Check support before choosing.
- Size the swapchain at 1–2 images above the queued frames. The CPU keeps working through present.
- Use the transfer queue for PCIe uploads and defragmentation. Copy on graphics or compute when both sides are on the GPU and the result is needed now.
- Async compute should pair unlike loads: bandwidth-heavy with ALU-heavy, like z-prepass or shadows beside post-processing or BVH builds.
  - Skip tasks under 0.2 ms, don't create unused queues, and avoid export-bound overlap.
  - Use 64-thread groups when async. Small dispatches pipeline better on the same queue.
  - Present from the compute queue on AMD.

## Shaders

- Avoid `atan`/`acos`, which take 100+ cycles. Transcendentals run at quarter rate, and `tan` costs three. Prefer approximations or lookup tables.
- Use `textureGather` for single-channel quads (about 4× less latency, fewer VGPRs). Point filtering at native resolution is up to 2× faster (NV). Limit dependent texture reads.
- Workgroups should be multiples of 64 (AMD: wave32 RDNA, wave64 GCN). NV suggests 2–8 warps, such as 8×8 or 16×16, set through specialization constants.
  - Write coalesced 256-byte blocks per wave (8×8 tiles) and swizzle thread group IDs for cache locality.
- Prefer subgroup operations to shared memory, and coalesce atomics per subgroup. Shared memory has 32 banks of 4 bytes, so use SoA or padding.
- Full-screen passes can run as compute (about 2% gain, no helper lanes).
- `discard`, depth writes and storage writes disable early-Z. After a z-prepass, draw with `EQUAL` instead of `discard`. Prefer depth-bounds or stencil tests to manual tests. Draw opaque geometry front to back.
- Keep register pressure low. FP16 is double rate on AMD and Turing but equal to FP32 on Ampere. Minimize bytes written by the fragment shader.
- Move invariant work up the pipeline (fragment → vertex → CPU), avoid divergent branches and needless `normalize`, and use built-ins.
- Use compressed vertex formats and cache-optimized index order. Cull on the CPU or GPU before the vertex shader; in the vertex shader a NaN position culls cheaply (AMD). Don't cull in geometry shaders.
- Mesh shaders: 64 vertices and 126 primitives per meshlet, task payload under 108 bytes (236 at most), and meshlets built offline.

## Debugging and measurement

- Validation should come up clean. Enable Best Practices with `VK_LAYER_ENABLES=VK_VALIDATION_FEATURE_ENABLE_BEST_PRACTICES_EXT;VALIDATION_CHECK_ENABLE_VENDOR_SPECIFIC_AMD;VALIDATION_CHECK_ENABLE_VENDOR_SPECIFIC_NVIDIA`.
- Name objects and label passes with `VK_EXT_debug_utils`, and keep pass labels in release builds. Put markers and timestamps per pass, not per draw.
- Never measure with validation on or while CPU-bound. Lock clocks and profile with RGP/RMV, Nsight or RenderDoc.

## Implement and verify

- Implement the agreed fixes completely, preserving rendered output unless a change is approved. Update affected callers, shaders and tests, remove obsolete code, and avoid unrelated refactors.
- Run the affected paths with validation, including synchronization validation, and keep them clean.
- Settle each performance fix with its agreed profile, counter or timestamp before and after the change. Drop a fix whose gain stays within noise unless it also simplifies the code.
- Run project-required formatting, lint, and tests, including affected consumers and the tests that need a real GPU.

Finish with a brief summary of each fix and its measured effect, the verification, the findings left unfixed, and any remaining blockers.
