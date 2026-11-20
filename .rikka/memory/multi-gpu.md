---
title: Multi-GPU displays (render GPU and display tiers)
description: How the KMS backend drives displays on every GPU: one renderer, per-card devices, the Tier ladder, naming, vkms tests.
about: system/rikka-compositor/src/backend/kms
tags: [compositor, kms]
---

- `Kms` opens a `Card` (`card.rs`: `Device`, `Connectors`, lazily built copier `Renderer`) for every card `Gpu::all` lists, keyed by device number; each card's `KmsEvent`s come through its own mailbox, kept across hot-unplug so a returning card reuses it. Screens are found by (card, CRTC).
- The render GPU is the first card (boot VGA first) whose `Renderer` builds and renders into dmabufs. Its screens are `Tier::Native`; only they do direct scan-out and dmabuf feedback advice, and `syncobj_device` is its fd.
- Screens on other cards walk `Tier::fallbacks` (`link/mod.rs`) until `link::enable` succeeds: `Shared` (renderer draws into the card's linear buffers), `Exported` (card scans renderer-exported linear buffers), `Copied` (the card's own renderer copies renderer-exported linear buffers into its native buffers, `link/copied.rs`), `Blitted` (renderer draws offscreen and GPU-copies damaged rows into system-memory `SharedRows` the card scans), `Cpu` (readback into the card's mapped dumb buffers). `Blitted` and `Cpu` share `link/staged.rs`: `Staged::draw` draws through `target.copied_into(sink, missed)`, so the copy of this frame's damage plus the areas the buffer missed (its age) rides in the same frame's submit, and the screen presents from `gpu_reached` once that frame finished.
- HDR works on every tier through 10-bit formats (`link::Depth::Ten` when the KMS config signals PQ): Shared allocates linear XR30-family buffers, Exported/Copied share renderer-exported 10-bit linear buffers (the copier passes the PQ values through unchanged), Blitted draws into an `Rgb10a2` offscreen target whose rows scan out as XB30/AB30, and Cpu reads that target back into 10-bit dumb buffers with `rikka_compositor_pixels::write_deep_rows`. The screen's blend space goes to `destination` and to the staged target.
- A refused commit or a failing copy demotes the screen to `Tier::fallback()`; `floors` remembers each output's lowest working tier until it disconnects.
- NVIDIA's Vulkan driver offers no linear render or export modifiers, so with an NVIDIA render GPU secondary displays land on `Blitted` (or `Cpu`).
- Connector names on cards other than the render GPU get the card prefix (`card2-HDMI-A-1`) only when the bare name exists on the render GPU or a lower-numbered card; a name is chosen once, when the output first connects, and kept across rescans.
- Only `Copied` screens get the card's copier; it is dropped once no `Copied` screen remains on that card.
- Leases are offered per card (`backend/leases.rs` `LeaseCard`, `wayland/leases.rs`); a vanished card's lease device is removed with `DrmLeases::remove_device`.
- `Backend::rest` rests the renderer and every copier.
- Machine tests need vkms (`sudo modprobe vkms`) and run with `LIBSEAT_BACKEND=noop`: rikka-compositor-kms `tests/secondary.rs` lights vkms with each `Buffers` kind, and rikka-compositor `tests/secondary_gpu.rs` runs `Launch::kms()` and checks a vkms-flipped presentation. Without master on the render GPU its own displays simply fail to light.
- In the dev shell Mesa's GBM may fail to load (glibc mismatch with `/run/opengl-driver`); the kms crate then falls back to dumb buffers.
