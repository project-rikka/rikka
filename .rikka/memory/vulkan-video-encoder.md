---
title: Vulkan Video encoder (rikka-vulkan-video)
description: Vendor-neutral H.264/H.265 Vulkan Video encoder with in-place rate control and reference-frame invalidation.
about: crates/media/rikka-vulkan-video
tags: [video, vulkan]
---

- Public crate over a `rikka_gpu::Gpu` device (ash); no FFmpeg and no rikka-renderer. `Encoder::builder(&gpu, Codec, w, h).frame_rate(..).references(n).rate(Rate).build()`, `Encoder::supports(&gpu, codec)`.
- `import(&DmabufSpec) -> Imported` wraps a capture buffer (RGB → NV12 compute conversion through `rikka-dmabuf-convert`, shaders in `crates/media/rikka-video/shaders`); `encode(&Imported, Request) -> Encoded { frame, reference, intra, data (Annex B, parameter sets in front of every intra frame), took }`; `parameter_sets()` are the session's SPS/PPS(/VPS).
- It is the only dmabuf-fed encoder: rikka-remote drives it directly; rikka-video's `VideoPackets`/`VideoEncoder` with `Feed::Dmabuf` wrap it for replay, the recorder and Go Live.
- `Request::intra()` / `Request::predicted(reference_frame)` choose the reference explicitly; `.constant(qp)` forces constant QP (refinements). `holds(frame)` says whether a frame is still in the reference ring; `set_rate(Rate)` retunes through `vkCmdControlVideoCodingKHR` without an IDR.
- `syntax/` writes parameter sets and slice headers itself: H.264 uses sliding window + `gaps_in_frame_num_allowed` + ref-list modification; H.265 uses explicit RPS that drops frames newer than the chosen reference.
- `tests/machine.rs` encodes, recovers from a chosen reference and checks the stream with FFmpeg as the decoding oracle.
